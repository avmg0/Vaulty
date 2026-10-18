use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, db, history, ledger, recurring::{self, Frequency}, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn schedules_keep_their_day_through_short_months() {
    let start = d("2026-01-31");
    let dates: Vec<String> = (0..5).map(|n| recurring::occurrence(start, Frequency::Monthly, 1, n).unwrap().to_string()).collect();
    assert_eq!(dates, ["2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30", "2026-05-31"]);
    assert_eq!(recurring::occurrence(d("2028-02-29"), Frequency::Yearly, 1, 1).unwrap(), d("2029-02-28"));
    assert_eq!(recurring::occurrence(d("2026-10-01"), Frequency::Weekly, 2, 3).unwrap(), d("2026-11-12"));
    assert_eq!(recurring::occurrence(d("2026-01-15"), Frequency::Monthly, 3, 2).unwrap(), d("2026-07-15"), "quarterly");
    assert_eq!(recurring::next_on_or_after(d("2026-01-05"), Frequency::Monthly, 1, None, d("2026-10-06")), Some(d("2026-11-05")));
    assert_eq!(recurring::next_on_or_after(d("2026-01-05"), Frequency::Monthly, 1, Some(d("2026-10-31")), d("2026-10-06")), None, "ended");
}

#[test]
fn pay_skip_auto_pay_and_upcoming() {
    let c = db::open_in_memory().unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "100000" })).unwrap();
    let cash = accounts::create(&c, &json!({ "name": "Cash", "type": "cash", "currency": "INR" })).unwrap();
    let today = d("2026-10-19");
    let rent = recurring::create(&c, &json!({ "name": "Rent", "type": "expense", "amount": "22000", "account_id": bank["id"], "category_id": "cat_home",
        "frequency": "monthly", "start_date": "2026-10-05", "tags": "home" }), today).unwrap();
    assert_eq!((rent["next_due_date"].as_str(), rent["status"].as_str(), rent["amount"].as_f64()), (Some("2026-10-05"), Some("overdue"), Some(22000.0)));

    let id = rent["id"].as_str().unwrap();
    let t = history::record(&c, "recurring.pay", |c| recurring::mark_paid(c, id, None, None, today)).unwrap();
    assert_eq!((t["date"].as_str(), t["recurring_id"].as_str(), t["category_id"].as_str(), t["tags"].as_str()), (Some("2026-10-05"), Some(id), Some("cat_home"), Some("home")));
    let rent = recurring::get(&c, id, today).unwrap();
    assert_eq!((rent["next_due_date"].as_str(), rent["last_paid_date"].as_str(), rent["status"].as_str()), (Some("2026-11-05"), Some("2026-10-05"), Some("upcoming")));
    // Undoing the payment removes the transaction and puts the due date back.
    history::undo_last(&c).unwrap();
    assert_eq!(recurring::get(&c, id, today).unwrap()["next_due_date"], "2026-10-05");
    assert!(recurring::history(&c, id).unwrap().is_empty());
    // A different amount this time.
    let t = recurring::mark_paid(&c, id, Some("2026-10-06"), Some(&json!("21500")), today).unwrap();
    assert_eq!((t["amount"].as_f64(), t["date"].as_str()), (Some(21500.0), Some("2026-10-06")));
    recurring::skip(&c, id, today).unwrap();
    assert_eq!(recurring::get(&c, id, today).unwrap()["next_due_date"], "2026-12-05");

    // Weekly auto-pay transfer, three weeks behind: caught up, and nothing more until next week.
    let pocket = recurring::create(&c, &json!({ "name": "Pocket money", "type": "transfer", "amount": "500", "account_id": bank["id"],
        "to_account_id": cash["id"], "frequency": "weekly", "start_date": "2026-09-28", "auto_pay": true }), today).unwrap();
    let made = recurring::run_auto_pay(&c, today).unwrap();
    assert_eq!(made.iter().map(|t| t["date"].as_str().unwrap().to_string()).collect::<Vec<_>>(), ["2026-09-28", "2026-10-05", "2026-10-12", "2026-10-19"]);
    assert!(recurring::run_auto_pay(&c, today).unwrap().is_empty());
    assert_eq!(accounts::get_all(&c).unwrap().into_iter().find(|a| a["id"] == cash["id"]).unwrap()["current_balance"].as_f64(), Some(2000.0));

    // What's coming in the next month.
    let next = recurring::upcoming(&c, today, d("2026-11-19")).unwrap();
    let names: Vec<_> = next.iter().map(|x| format!("{} {}", x["date"].as_str().unwrap(), x["name"].as_str().unwrap())).collect();
    assert_eq!(names, ["2026-10-26 Pocket money", "2026-11-02 Pocket money", "2026-11-09 Pocket money", "2026-11-16 Pocket money"]);
    let _ = pocket;

    // Validation, pausing, deleting keeps past transactions.
    assert!(recurring::create(&c, &json!({ "name": "x", "amount": "10", "account_id": bank["id"], "frequency": "fortnightly", "start_date": "2026-10-01" }), today).is_err());
    assert!(recurring::create(&c, &json!({ "name": "x", "amount": "0", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-01" }), today).is_err());
    assert_eq!(recurring::set_active(&c, id, false, today).unwrap()["status"], "paused");
    let before = transactions::get_all(&c, None).unwrap().len();
    recurring::delete(&c, id).unwrap();
    assert_eq!(transactions::get_all(&c, None).unwrap().len(), before);
    assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
}
