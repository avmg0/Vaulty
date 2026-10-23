use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, anomalies, categories, db, reminders, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn unusual_spending() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-23");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "1000000", "opening_date": "2025-01-01" })).unwrap();
    let card = accounts::create(&c, &json!({ "name": "Card", "type": "credit", "currency": "INR", "initial_balance": "0" })).unwrap();
    let cat = categories::get_all(&c).unwrap().into_iter().find(|x| x["type"] == "expense").unwrap()["id"].as_str().unwrap().to_string();
    let spend = |acct: &serde_json::Value, m: &str, amt: &str, date: &str| transactions::create(&c, &json!({ "type": "expense", "account_id": acct["id"], "category_id": cat, "merchant": m, "amount": amt, "date": date })).unwrap();
    // A year of ordinary spending: groceries ~₹2,000 every two weeks, coffee ₹200.
    let mut day = d("2025-10-25");
    while day < d("2026-10-01") {
        spend(&bank, "Big Bazaar Pune", if day.format("%d").to_string().as_str() < "15" { "1900" } else { "2150" }, &day.to_string());
        spend(&card, "Corner Cafe", "200", &day.to_string());
        day = day + chrono::Days::new(14);
    }
    // This month: a double charge, a big grocery bill, a large first payment, a normal one.
    let a1 = spend(&card, "SWIGGY*ORDER 123", "640", "2026-10-18");
    let a2 = spend(&card, "Swiggy order 123", "640", "2026-10-19");
    let big = spend(&bank, "Big Bazaar Pune", "14500", "2026-10-12");
    let new = spend(&bank, "Example Electronics", "85000", "2026-10-20");
    spend(&bank, "Big Bazaar Pune", "2050", "2026-10-15");
    // Same amount at different places, or more than two days apart, isn't a double charge.
    spend(&card, "Corner Cafe", "200", "2026-10-05");
    spend(&card, "Corner Cafe", "200", "2026-10-09");

    let found = anomalies::find(&c, today).unwrap();
    let kinds: Vec<(&str, &str)> = found.iter().map(|a| (a.kind.as_str(), a.entry_id.as_str())).collect();
    assert!(kinds.contains(&("double", a2["id"].as_str().unwrap())), "{kinds:?}");
    assert!(!kinds.iter().any(|k| k.1 == a1["id"].as_str().unwrap()), "only the second of a pair");
    let b = found.iter().find(|a| a.entry_id == big["id"].as_str().unwrap()).expect("big grocery bill");
    assert_eq!(b.kind, "amount");
    assert!(b.usual.unwrap() > 1800.0 && b.times.unwrap() > 6.0);
    assert!(found.iter().any(|a| a.kind == "new_place" && a.entry_id == new["id"].as_str().unwrap()));
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(found[0].date, "2026-10-20", "newest first");

    // Reminders for the last three days only.
    let rem: Vec<_> = reminders::due(&c, today).unwrap().into_iter().filter(|r| r.kind == "unusual").collect();
    assert_eq!(rem.len(), 1, "the new place (20th); the double charge (19th) is four days old");

    // "It's fine" keeps it quiet; reset brings it back.
    anomalies::dismiss(&c, &b.key).unwrap();
    assert_eq!(anomalies::find(&c, today).unwrap().len(), 2);
    assert_eq!(anomalies::reset(&c).unwrap(), 1);
    assert_eq!(anomalies::find(&c, today).unwrap().len(), 3);
    // Older than 30 days isn't reported.
    assert!(anomalies::find(&c, d("2026-12-31")).unwrap().is_empty());
}
