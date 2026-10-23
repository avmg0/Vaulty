use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, db, estate, export, history, insurance, investments, nominees};

#[test]
fn nominees_everywhere() {
    let c = db::open_in_memory().unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Savings", "type": "savings", "currency": "INR", "initial_balance": "1000", "nominee": "John Doe" })).unwrap();
    assert_eq!(bank["nominee"], "John Doe");
    accounts::create(&c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR", "initial_balance": "100" })).unwrap();
    accounts::create(&c, &json!({ "name": "Card", "type": "credit", "currency": "INR", "initial_balance": "0" })).unwrap();
    let fund = investments::create(&c, &json!({ "name": "Example Fund", "kind": "mutual_fund", "currency": "INR", "units": "10", "price": "100" }), today).unwrap();
    let pol = insurance::create(&c, &json!({ "kind": "term_life", "name": "Term plan", "currency": "INR", "cover": "10000000", "nominee": "John Doe" })).unwrap();

    let o = nominees::overview(&c, today).unwrap();
    let items = o["items"].as_array().unwrap();
    assert_eq!(items.len(), 3, "no cash wallet or card: {items:?}");
    assert_eq!(o["missing"], 1);
    assert_eq!(items[0]["name"], "Example Fund", "missing ones first");

    // Set from the overview (an undoable step), and when an account is edited.
    history::record(&c, "nominee.set", |c| nominees::set(c, "holding", fund["id"].as_str().unwrap(), Some("Jenny Doe, 100%"))).unwrap();
    assert_eq!(nominees::overview(&c, today).unwrap()["missing"], 0);
    accounts::update(&c, bank["id"].as_str().unwrap(), &json!({ "nominee": "" })).unwrap();
    assert!(accounts::get(&c, bank["id"].as_str().unwrap()).unwrap()["nominee"].is_null());
    accounts::update(&c, bank["id"].as_str().unwrap(), &json!({ "name": "Savings account" })).unwrap();
    assert!(accounts::get(&c, bank["id"].as_str().unwrap()).unwrap()["nominee"].is_null(), "left alone when not given");
    assert!(nominees::set(&c, "car", "x", Some("y")).is_err());
    assert!(nominees::set(&c, "account", "nope", Some("y")).is_err());

    // In the emergency summary.
    let e = estate::summary(&c, today).unwrap();
    assert_eq!(e.investments[0].nominee.as_deref(), Some("Jenny Doe, 100%"));
    assert_eq!(e.policies.len(), 1);
    assert_eq!(e.policies[0].nominee.as_deref(), Some("John Doe"));
    assert_eq!(e.policies[0].cover, Some(10_000_000.0));
    let _ = pol;

    // Carried in backups.
    let b = export::to_json(&c).unwrap();
    let c2 = db::open_in_memory().unwrap();
    export::from_json(&c2, &b).unwrap();
    assert_eq!(nominees::overview(&c2, today).unwrap()["missing"], 1);
}
