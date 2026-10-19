use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, budgets, cards, db, profiles::Registry, recurring, reminders, settings, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn bills_cards_and_budgets_are_reminded_once() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = d("2026-10-19");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    // Due in 2 days, reminded 3 days ahead: yes. Due in 10 days: not yet. Auto-paid: never.
    recurring::create(&c, &json!({ "name": "Mutual fund SIP", "type": "expense", "amount": "5000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-21", "reminder_days": 3 }), today).unwrap();
    recurring::create(&c, &json!({ "name": "Gym", "type": "expense", "amount": "1500", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-29" }), today).unwrap();
    recurring::create(&c, &json!({ "name": "Netflix", "type": "expense", "amount": "649", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-20", "auto_pay": true }), d("2026-10-25")).unwrap();
    // Overdue by 4 days.
    recurring::create(&c, &json!({ "name": "Electricity", "type": "expense", "amount": "1850", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-15" }), d("2026-10-01")).unwrap();
    cards::create(&c, &json!({ "kind": "credit", "name": "Rewards", "currency": "INR", "owed": "12400", "due_day": 22 })).unwrap();
    cards::create(&c, &json!({ "kind": "credit", "name": "Paid off", "currency": "INR", "owed": "0", "due_day": 20 })).unwrap();
    let food = vaulty_core::categories::get_all(&c).unwrap().into_iter().find(|x| x["type"] == "expense").unwrap();
    budgets::set(&c, food["id"].as_str().unwrap(), &json!("1000")).unwrap();
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "expense", "amount": "850", "date": "2026-10-10", "category_id": food["id"] })).unwrap();

    let first = reminders::take_due(&c, today).unwrap();
    let names: Vec<(&str, &str)> = first.iter().map(|r| (r.kind.as_str(), r.name.as_str())).collect();
    assert!(names.contains(&("bill", "Mutual fund SIP")), "{names:?}");
    assert!(names.contains(&("bill", "Electricity")), "overdue bills too");
    assert!(names.contains(&("card", "Rewards")));
    assert!(!names.iter().any(|(_, n)| ["Gym", "Netflix", "Paid off"].contains(n)), "{names:?}");
    let budget = first.iter().find(|r| r.kind == "budget").unwrap();
    assert_eq!(budget.percent, Some(80));
    assert_eq!(first.iter().find(|r| r.name == "Electricity").unwrap().days, -4);

    assert!(reminders::take_due(&c, today).unwrap().is_empty(), "each shown once");
    // Going over budget is a new reminder; the due date arriving is a new one too.
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "expense", "amount": "300", "date": "2026-10-19", "category_id": food["id"] })).unwrap();
    let again = reminders::take_due(&c, d("2026-10-21")).unwrap();
    assert!(again.iter().any(|r| r.kind == "budget" && r.percent == Some(100)));
    assert!(again.iter().any(|r| r.name == "Mutual fund SIP" && r.days == 0));

    settings::update(&c, &json!({ "notifications": "off" })).unwrap();
    assert!(reminders::take_due(&c, d("2026-10-22")).unwrap().is_empty(), "turned off");
}

#[test]
fn reminders_while_locked_keep_only_dates() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-19");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    recurring::create(&c, &json!({ "name": "Rent", "type": "expense", "amount": "22000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-11-05" }), today).unwrap();
    assert!(reminders::locked_schedule(&c, today).unwrap().is_empty(), "opt-in");
    settings::update(&c, &json!({ "notify_locked": true })).unwrap();
    let dates = reminders::locked_schedule(&c, today).unwrap();
    assert_eq!(dates, vec![d("2026-11-05"), d("2026-12-05")]);

    let r = Registry::open_in_memory().unwrap();
    let admin = r.create_first_admin("John Doe", "#2563eb", "parent-password").unwrap();
    r.set_locked_reminders(&admin.user_id, &dates).unwrap();
    assert!(r.take_locked_reminders(d("2026-11-04")).unwrap().is_empty());
    let due = r.take_locked_reminders(d("2026-11-05")).unwrap();
    assert_eq!(due.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), ["John Doe"]);
    assert!(r.take_locked_reminders(d("2026-11-05")).unwrap().is_empty(), "once");
    assert_eq!(r.take_locked_reminders(d("2026-12-10")).unwrap().len(), 1, "a missed day is still reminded");
}
