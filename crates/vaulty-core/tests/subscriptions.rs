use chrono::{Duration, NaiveDate};
use serde_json::json;
use vaulty_core::{accounts, db, recurring, reminders, subscriptions, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn regular_charges_are_spotted_and_odd_ones_are_not() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-22");
    let card = accounts::create(&c, &json!({ "name": "Card", "type": "credit", "currency": "INR" })).unwrap();
    let spend = |name: &str, amount: &str, date: NaiveDate| {
        transactions::create(&c, &json!({ "account_id": card["id"], "type": "expense", "amount": amount, "date": date.to_string(), "merchant": name })).unwrap();
    };
    // Monthly, with a price rise on the latest charge (dates drift a few days, as cards do).
    for (i, amount) in ["499", "499", "499", "649"].iter().enumerate() {
        spend(&format!("NETFLIX.COM {}", 1000 + i), amount, d("2026-07-05") + Duration::days(30 * i as i64 + (i as i64 % 2)));
    }
    // Yearly, twice.
    spend("Amazon Prime", "1499", d("2025-11-02"));
    spend("Amazon Prime", "1499", d("2026-11-01") - Duration::days(365));
    spend("Amazon Prime", "1499", d("2026-10-30") - Duration::days(4));
    // Weekly.
    for i in 0..4 { spend("Milkman", "420", today - Duration::days(7 * i + 2)); }
    // Not subscriptions: irregular, wildly different amounts, only twice monthly, cancelled long ago.
    for (i, gap) in [0, 9, 40, 52].iter().enumerate() { spend("Swiggy", "300", d("2026-07-01") + Duration::days(*gap + i as i64)); }
    for (i, amount) in ["200", "1900", "650"].iter().enumerate() { spend("Electricity", amount, d("2026-07-10") + Duration::days(30 * i as i64)); }
    spend("Gym", "1500", d("2026-09-01")); spend("Gym", "1500", d("2026-10-01"));
    for i in 0..4 { spend("Old magazine", "99", d("2025-12-01") + Duration::days(30 * i)); }

    let found = subscriptions::detect(&c, today).unwrap();
    let names: Vec<(&str, &str)> = found.iter().map(|f| (f.key.as_str(), f.rhythm.as_str())).collect();
    assert!(names.contains(&("netflix com", "monthly")), "{names:?}");
    assert!(names.contains(&("milkman", "weekly")), "{names:?}");
    assert!(!names.iter().any(|(k, _)| ["swiggy", "electricity", "gym", "old magazine"].contains(k)), "{names:?}");
    let netflix = found.iter().find(|f| f.key == "netflix com").unwrap();
    assert_eq!((netflix.amount, netflix.count), (649.0, 4));
    let rise = netflix.price_change.as_ref().unwrap();
    assert_eq!((rise.from, rise.to, rise.percent), (499.0, 649.0, 30.1));

    // A price rise is a reminder, once.
    let r = reminders::take_due(&c, today).unwrap();
    let price = r.iter().find(|x| x.kind == "price").unwrap();
    assert_eq!((price.previous, price.amount, price.percent), (Some(499.0), Some(649.0), Some(30)));
    assert!(reminders::take_due(&c, today).unwrap().iter().all(|x| x.kind != "price"));

    // "Not a subscription" sticks; tracking it as a recurring payment also stops the suggestion.
    subscriptions::dismiss(&c, "milkman").unwrap();
    recurring::create(&c, &json!({ "name": "Netflix", "merchant": "NETFLIX.COM", "type": "expense", "amount": "649", "account_id": card["id"], "frequency": "monthly", "start_date": "2026-11-05" }), today).unwrap();
    let left: Vec<String> = subscriptions::detect(&c, today).unwrap().into_iter().map(|f| f.key).collect();
    assert!(!left.contains(&"milkman".to_string()) && !left.contains(&"netflix com".to_string()), "{left:?}");
}

#[test]
fn recurring_payments_that_got_dearer() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-22");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    recurring::create(&c, &json!({ "name": "Broadband", "type": "expense", "amount": "799", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-08-10" }), d("2026-08-01")).unwrap();
    let id = recurring::list(&c, today).unwrap()[0]["id"].as_str().unwrap().to_string();
    recurring::mark_paid(&c, &id, None, None, d("2026-08-10")).unwrap();
    recurring::mark_paid(&c, &id, None, None, d("2026-09-10")).unwrap();
    assert!(subscriptions::recurring_rises(&c).unwrap().is_empty());
    recurring::mark_paid(&c, &id, None, Some(&json!("899")), d("2026-10-10")).unwrap();
    let rises = subscriptions::recurring_rises(&c).unwrap();
    assert_eq!((rises[0].name.as_str(), rises[0].change.from, rises[0].change.to), ("Broadband", 799.0, 899.0));
}
