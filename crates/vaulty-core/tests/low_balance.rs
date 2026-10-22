use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, db, recurring, reminders, reports, settings, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn levels_are_checked_and_kept() {
    let c = db::open_in_memory().unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Savings", "type": "savings", "currency": "INR", "initial_balance": "30000", "low_balance": "10000" })).unwrap();
    assert_eq!(bank["low_balance"], 10000.0);
    let id = bank["id"].as_str().unwrap();
    // Other edits keep it; an empty value clears it.
    assert_eq!(accounts::update(&c, id, &json!({ "name": "HDFC Savings" })).unwrap()["low_balance"], 10000.0);
    assert!(accounts::update(&c, id, &json!({ "low_balance": "-5" })).is_err(), "not below zero");
    assert_eq!(accounts::update(&c, id, &json!({ "low_balance": "" })).unwrap()["low_balance"], json!(null));
    assert!(accounts::create(&c, &json!({ "name": "Card", "type": "credit", "currency": "INR", "low_balance": "100" })).is_err(), "not for cards");
}

#[test]
fn below_now_and_heading_below_are_reminded() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = d("2026-10-23");
    let salary = accounts::create(&c, &json!({ "name": "Salary account", "type": "checking", "currency": "INR", "initial_balance": "40000", "low_balance": "15000" })).unwrap();
    let wallet = accounts::create(&c, &json!({ "name": "Cash", "type": "cash", "currency": "INR", "initial_balance": "800", "low_balance": "1000" })).unwrap();
    let spare = accounts::create(&c, &json!({ "name": "Spare", "type": "savings", "currency": "INR", "initial_balance": "5000" })).unwrap();
    // Rent on the 1st takes the salary account to 12,000. An account with no level is never
    // reported, even when a bill would empty it.
    recurring::create(&c, &json!({ "name": "House rent", "type": "expense", "amount": "28000", "account_id": salary["id"], "frequency": "monthly", "start_date": "2026-11-01" }), today).unwrap();
    recurring::create(&c, &json!({ "name": "School fees", "type": "expense", "amount": "9000", "account_id": spare["id"], "frequency": "yearly", "start_date": "2026-11-20" }), today).unwrap();

    let low = reports::low_balances(&c, today).unwrap();
    assert_eq!(low.len(), 2, "{low:?}");
    assert_eq!((low[0].name.as_str(), low[0].now, low[0].balance, low[0].level), ("Cash", true, 800.0, 1000.0));
    assert_eq!((low[1].name.as_str(), low[1].now, low[1].date.as_str(), low[1].because.as_deref(), low[1].lowest), ("Salary account", false, "2026-11-01", Some("House rent"), 12000.0));

    let first = reminders::take_due(&c, today).unwrap();
    let lows: Vec<_> = first.iter().filter(|r| r.kind == "low").collect();
    assert_eq!(lows.len(), 2);
    assert_eq!(lows.iter().find(|r| r.name == "Salary account").unwrap().detail.as_deref(), Some("House rent"));
    assert!(reminders::take_due(&c, today).unwrap().iter().all(|r| r.kind != "low"), "once");

    // Topping up the cash and spending it again is a new dip, so a new reminder.
    let cash = wallet["id"].as_str().unwrap();
    transactions::create(&c, &json!({ "account_id": cash, "type": "income", "amount": "2000", "date": "2026-10-23" })).unwrap();
    assert!(reminders::take_due(&c, today).unwrap().iter().all(|r| r.kind != "low"));
    transactions::create(&c, &json!({ "account_id": cash, "type": "expense", "amount": "2500", "date": "2026-10-24" })).unwrap();
    let again = reminders::take_due(&c, d("2026-10-24")).unwrap();
    assert!(again.iter().any(|r| r.kind == "low" && r.name == "Cash" && r.amount == Some(300.0)), "{again:?}");

    // Salary arriving first keeps the account above its level.
    recurring::create(&c, &json!({ "name": "Salary", "type": "income", "amount": "90000", "account_id": salary["id"], "frequency": "monthly", "start_date": "2026-10-31" }), today).unwrap();
    assert!(reports::low_balances(&c, today).unwrap().iter().all(|l| l.name != "Salary account"));
}
