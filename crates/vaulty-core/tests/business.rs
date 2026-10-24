use serde_json::json;
use vaulty_core::{accounts, business, db, reports, transactions, Key};

#[test]
fn a_business_book_beside_the_personal_one() {
    let dir = tempfile::tempdir().unwrap();
    let dk = Key::new([7u8; 32]);
    let personal = db::open(&dir.path().join("me.db"), &dk).unwrap();
    vaulty_core::settings::update(&personal, &json!({ "currency": "INR" })).unwrap();
    let savings = accounts::create(&personal, &json!({ "name": "Savings", "type": "checking", "currency": "INR", "initial_balance": "500000" })).unwrap();
    let path = |id: &str| dir.path().join(format!("biz-{id}.db"));

    assert!(business::create(&personal, &dk, path, &json!({ "name": "Doe Designs", "gstin": "27AAPFU0939F1ZW" })).is_err(), "bad check digit");
    let (id, book) = business::create(&personal, &dk, path, &json!({ "name": "Doe Designs", "kind": "sole_proprietor", "gstin": "27AAPFU0939F1ZV", "pan": "AAPFU0939F" })).unwrap();
    let list = business::list(&personal).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["state_code"], "27", "state from the GSTIN");
    assert_eq!(list[0]["capital_account_name"], "Doe Designs (capital)");

    // Its own book: its own categories, nothing personal in it.
    let p = business::profile(&book).unwrap().unwrap();
    assert_eq!(p["name"], "Doe Designs");
    let cats = vaulty_core::categories::get_all(&book).unwrap();
    assert!(cats.iter().any(|c| c["name"] == "Sales") && !cats.iter().any(|c| c["name"] == "Food & Dining"));
    assert!(business::profile(&personal).unwrap().is_none());

    // Reopened with the key kept in the personal book.
    drop(book);
    let book = business::open(&personal, &dk, &path(&id), &id).unwrap();
    assert!(business::open(&personal, &Key::new([9u8; 32]), &path(&id), &id).is_err(), "another member's key can't open it");

    // The owner puts ₹2 lakh in from savings, sells for ₹50,000, takes ₹30,000 out.
    let bank = accounts::create(&book, &json!({ "name": "Current account", "type": "checking", "currency": "INR", "initial_balance": "0" })).unwrap();
    let r = business::owner_money(&personal, &book, &id, &json!({ "direction": "in", "account_id": bank["id"], "amount": "200000", "date": "2026-10-01", "personal_account_id": savings["id"] })).unwrap();
    assert!(r["personal_entry"].is_string());
    let sales = vaulty_core::categories::get_all(&book).unwrap().into_iter().find(|c| c["name"] == "Sales").unwrap();
    transactions::create(&book, &json!({ "type": "income", "account_id": bank["id"], "category_id": sales["id"], "amount": "50000", "date": "2026-10-10", "merchant": "Jane Doe" })).unwrap();
    business::owner_money(&personal, &book, &id, &json!({ "direction": "out", "account_id": bank["id"], "amount": "30000", "date": "2026-10-20", "personal_account_id": savings["id"] })).unwrap();

    assert_eq!(accounts::get(&book, bank["id"].as_str().unwrap()).unwrap()["current_balance"], 220_000.0);
    let t = business::owner_totals(&book).unwrap();
    assert_eq!((t["capital"].as_f64(), t["drawings"].as_f64(), t["net"].as_f64()), (Some(200_000.0), Some(30_000.0), Some(170_000.0)));
    // Owner money isn't income or spending in either book.
    let o = reports::overview(&book, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!((o.income, o.spending), (50_000.0, 0.0));
    let po = reports::overview(&personal, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!((po.income, po.spending), (0.0, 0.0));
    // Personal side: savings down by the net, the capital account up by it.
    assert_eq!(accounts::get(&personal, savings["id"].as_str().unwrap()).unwrap()["current_balance"], 330_000.0);
    let cap = list[0]["capital_account_id"].as_str().unwrap();
    assert_eq!(accounts::get(&personal, cap).unwrap()["current_balance"], 170_000.0);

    // Owner money can't go to a card, and needs a direction.
    assert!(business::owner_money(&personal, &book, &id, &json!({ "direction": "sideways", "account_id": bank["id"], "amount": "1", "date": "2026-10-20" })).is_err());

    // Details change in both places; the currency can't.
    business::update(&personal, &book, &id, &json!({ "name": "Doe Design Studio", "kind": "llp", "gstin": "27AAPFU0939F1ZV" })).unwrap();
    assert_eq!(business::profile(&book).unwrap().unwrap()["name"], "Doe Design Studio");
    assert_eq!(business::list(&personal).unwrap()[0]["name"], "Doe Design Studio");
    assert!(business::update(&personal, &book, &id, &json!({ "name": "X", "currency": "USD" })).is_err());

    business::set_archived(&personal, &id, true).unwrap();
    assert!(business::list(&personal).unwrap()[0]["archived_at"].is_string());
}
