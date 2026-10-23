use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, categories, db, export, journal, transactions};

#[test]
fn entries_reviews_and_search() {
    let c = db::open_in_memory().unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000", "opening_date": "2026-01-01" })).unwrap();
    let cats = categories::get_all(&c).unwrap();
    let spend = cats.iter().find(|x| x["type"] == "expense").unwrap()["id"].as_str().unwrap().to_string();
    let salary = cats.iter().find(|x| x["type"] == "income").unwrap()["id"].as_str().unwrap().to_string();
    let tx = |kind: &str, cat: &str, m: &str, amt: &str, date: &str| transactions::create(&c, &json!({ "type": kind, "account_id": bank["id"], "category_id": cat, "merchant": m, "amount": amt, "date": date })).unwrap();
    tx("income", &salary, "Salary", "80000", "2026-08-01");
    tx("expense", &spend, "Rent", "25000", "2026-08-05");
    tx("income", &salary, "Salary", "80000", "2026-09-01");
    let laptop = tx("expense", &spend, "Example Electronics", "60000", "2026-09-14");
    tx("expense", &spend, "Rent", "25000", "2026-09-05");

    let l = journal::list(&c, None, None, today).unwrap();
    assert_eq!(l["review_due"], "2026-09", "September has spending and no review");

    let e = journal::create(&c, &json!({ "date": "2026-09-14", "title": "Bought a laptop", "content": "Needed it for work; paid in full.", "mood": "okay",
        "tags": "#Work, purchases", "linked": [laptop["id"]] })).unwrap();
    let r = journal::create(&c, &json!({ "date": "2026-10-01", "title": "September", "content": "Big month.", "mood": "worried", "review_of": "2026-09" })).unwrap();
    assert!(journal::create(&c, &json!({ "date": "2026-10-02", "title": "Again", "review_of": "2026-09" })).is_err(), "one review a month");

    let l = journal::list(&c, None, None, today).unwrap();
    assert!(l["review_due"].is_null());
    assert_eq!(l["tags"], json!(["purchases", "work"]));
    let entries = l["entries"].as_array().unwrap();
    assert_eq!(entries[0]["id"], r["id"], "newest first");
    let f = &entries[0]["facts"];
    assert_eq!((f["income"].as_f64(), f["spending"].as_f64(), f["kept"].as_f64()), (Some(80000.0), Some(85000.0), Some(-5000.0)));
    assert_eq!(f["change"], 240.0, "spending up 240% on August");
    assert_eq!(f["biggest"][0]["name"], "Example Electronics");
    assert_eq!(entries[1]["linked"][0]["name"], "Example Electronics");

    // Search and tag filters.
    assert_eq!(journal::list(&c, Some("LAPTOP"), None, today).unwrap()["entries"].as_array().unwrap().len(), 1);
    assert_eq!(journal::list(&c, None, Some("work"), today).unwrap()["entries"].as_array().unwrap().len(), 1);
    assert_eq!(journal::list(&c, Some("nothing like this"), None, today).unwrap()["entries"].as_array().unwrap().len(), 0);

    // A review keeps its figures unless asked to take them again.
    tx("expense", &spend, "Late bill", "1000", "2026-09-30");
    journal::update(&c, r["id"].as_str().unwrap(), &json!({ "date": "2026-10-01", "title": "September", "content": "Big month." })).unwrap();
    let l = journal::list(&c, None, None, today).unwrap();
    assert_eq!(l["entries"][0]["facts"]["spending"], 85000.0);
    journal::update(&c, r["id"].as_str().unwrap(), &json!({ "date": "2026-10-01", "title": "September", "refresh_facts": true })).unwrap();
    assert_eq!(journal::list(&c, None, None, today).unwrap()["entries"][0]["facts"]["spending"], 86000.0);

    // Refused.
    assert!(journal::create(&c, &json!({ "date": "2026-10-01", "title": "" })).is_err());
    assert!(journal::create(&c, &json!({ "date": "2026-10-01", "title": "x", "mood": "ecstatic" })).is_err());
    assert!(journal::create(&c, &json!({ "date": "2026-10-01", "title": "x", "linked": ["nope"] })).is_err());

    // Backups.
    let b = export::to_json(&c).unwrap();
    let c2 = db::open_in_memory().unwrap();
    export::from_json(&c2, &b).unwrap();
    assert_eq!(journal::list(&c2, None, None, today).unwrap()["entries"].as_array().unwrap().len(), 2);
    journal::delete(&c, e["id"].as_str().unwrap()).unwrap();
}
