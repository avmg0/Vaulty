use serde_json::json;
use vaulty_core::*;

fn conn() -> rusqlite::Connection {
    db::open_in_memory().unwrap()
}

#[test]
fn seeds_default_categories_and_settings() {
    let c = conn();
    assert_eq!(categories::get_all(&c).unwrap().len(), 15);
    assert_eq!(settings::get(&c).unwrap()["currency"], json!("USD"));
}

#[test]
fn balance_follows_transactions() {
    let c = conn();
    let acc = accounts::create(&c, &json!({"name":"Main","type":"checking","initial_balance":100.0})).unwrap();
    let id = acc["id"].as_str().unwrap();
    let t = transactions::create(&c, &json!({"account_id":id,"type":"expense","amount":30.0,"date":"2026-10-01"})).unwrap();
    transactions::create(&c, &json!({"account_id":id,"type":"income","amount":50.0,"date":"2026-10-02"})).unwrap();
    assert_eq!(accounts::get_all(&c).unwrap()[0]["current_balance"], json!(120.0));
    transactions::delete(&c, t["id"].as_str().unwrap()).unwrap();
    assert_eq!(accounts::get_all(&c).unwrap()[0]["current_balance"], json!(150.0));
}

#[test]
fn rejects_bad_transactions() {
    let c = conn();
    let acc = accounts::create(&c, &json!({"name":"A","type":"cash"})).unwrap();
    let id = acc["id"].as_str().unwrap();
    assert!(transactions::create(&c, &json!({"account_id":id,"type":"bogus","amount":1,"date":"2026-01-01"})).is_err());
    assert!(transactions::create(&c, &json!({"account_id":id,"type":"expense","amount":-5,"date":"2026-01-01"})).is_err());
}

#[test]
fn system_categories_are_protected() {
    let c = conn();
    categories::delete(&c, "cat_food").unwrap();
    assert_eq!(categories::get_all(&c).unwrap().len(), 15);
    let custom = categories::create(&c, &json!({"name":"Pets","type":"expense"})).unwrap();
    categories::delete(&c, custom["id"].as_str().unwrap()).unwrap();
    assert_eq!(categories::get_all(&c).unwrap().len(), 15);
}

#[test]
fn settings_reject_unknown_keys() {
    let c = conn();
    assert!(settings::update(&c, &json!({"id; DROP TABLE x":"x"})).is_err());
    assert_eq!(settings::update(&c, &json!({"theme":"light"})).unwrap()["theme"], json!("light"));
}

#[test]
fn raw_sql_shim_binds_params_and_blocks_multi_statement() {
    let c = conn();
    rows::run(&c, "INSERT INTO tasks (id, title) VALUES (?, ?)", &[json!("t1"), json!("x'; DROP TABLE tasks;--")]).unwrap();
    assert_eq!(rows::query(&c, "SELECT title FROM tasks", &[]).unwrap().len(), 1);
    assert!(rows::query(&c, "SELECT 1; DROP TABLE tasks", &[]).is_err());
}

#[test]
fn json_backup_roundtrip() {
    let a = conn();
    let acc = accounts::create(&a, &json!({"name":"Main","type":"checking","initial_balance":10.0})).unwrap();
    transactions::create(&a, &json!({"account_id":acc["id"],"type":"expense","amount":4.0,"date":"2026-10-01"})).unwrap();
    let dump = export::to_json(&a).unwrap();
    let b = conn();
    let counts = export::from_json(&b, &dump).unwrap();
    assert_eq!((counts.accounts, counts.transactions), (1, 1));
    assert_eq!(accounts::get_all(&b).unwrap()[0]["current_balance"], json!(6.0));
    let (csv, n) = export::to_csv(&b).unwrap();
    assert_eq!(n, 1);
    assert!(csv.starts_with("date,type,amount"));
}

#[test]
fn settings_validate_locale_and_currency() {
    use vaulty_core::settings;
    let conn = vaulty_core::db::open_in_memory().unwrap();
    let s = settings::update(&conn, &json!({ "locale": "hi-IN", "currency": "INR" })).unwrap();
    assert_eq!(s["locale"], "hi-IN");
    assert_eq!(s["currency"], "INR");
    assert!(settings::update(&conn, &json!({ "locale": "hindi" })).is_err());
    assert!(settings::update(&conn, &json!({ "currency": "inr" })).is_err());
    // A rejected patch changes nothing, even keys that were valid.
    assert!(settings::update(&conn, &json!({ "currency": "EUR", "locale": "x" })).is_err());
    assert_eq!(settings::get(&conn).unwrap()["currency"], "INR");
    for ok in ["en", "hi-IN", "zh-Hant-TW", "es-419", "fil-PH"] { assert!(settings::is_locale(ok), "{ok}"); }
    for bad in ["EN-us", "en_US", "en-US-x", "e", ""] { assert!(!settings::is_locale(bad), "{bad}"); }
    assert_eq!(settings::country_of("hi-IN"), Some("IN"));
    assert_eq!(settings::country_of("es-419"), None);
}

#[test]
fn display_format_settings_start_automatic_and_are_validated() {
    let c = vaulty_core::db::open_in_memory().unwrap();
    let s = vaulty_core::settings::get(&c).unwrap();
    for k in ["date_format", "start_of_week", "fiscal_year_start", "decimals"] {
        assert_eq!(s[k], "auto", "{k} follows the region until changed");
    }
    assert_eq!(s["negative_format"], "minus");
    let s = vaulty_core::settings::update(&c, &serde_json::json!({ "date_format": "dmy", "fiscal_year_start": "april", "negative_format": "parentheses", "decimals": "0", "start_of_week": "monday" })).unwrap();
    assert_eq!((s["date_format"].as_str(), s["fiscal_year_start"].as_str(), s["decimals"].as_str()), (Some("dmy"), Some("april"), Some("0")));
    for bad in [("date_format", "dd/mm"), ("fiscal_year_start", "spring"), ("negative_format", "red"), ("decimals", "3"), ("start_of_week", "friday")] {
        assert!(vaulty_core::settings::update(&c, &serde_json::json!({ bad.0: bad.1 })).is_err(), "{bad:?} is rejected");
    }
}

#[test]
fn tags_are_tidied_and_transfers_edit_cleanly() {
    use vaulty_core::{accounts, transactions};
    let c = vaulty_core::db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "1000" })).unwrap();
    let b = accounts::create(&c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR" })).unwrap();
    let t = transactions::create(&c, &json!({ "account_id": a["id"], "type": "expense", "amount": "100", "date": "2026-10-01",
        "tags": " #Trip, goa , trip,  ", "notes": "with friends" })).unwrap();
    assert_eq!(t["tags"], "Trip, goa");
    assert_eq!(t["notes"], "with friends");
    assert!(transactions::create(&c, &json!({ "account_id": a["id"], "type": "expense", "amount": "1", "date": "2026-10-01", "tags": "x".repeat(41) })).is_err());
    // Turn the expense into a transfer, then back.
    let id = t["id"].as_str().unwrap();
    let t = transactions::update(&c, id, &json!({ "account_id": a["id"], "to_account_id": b["id"], "type": "transfer", "amount": "250", "date": "2026-10-02", "tags": ["cash"] })).unwrap();
    assert_eq!((t["type"].as_str(), t["to_account_id"].as_str(), t["tags"].as_str()), (Some("transfer"), b["id"].as_str(), Some("cash")));
    let bal = |id: &serde_json::Value| accounts::get_all(&c).unwrap().into_iter().find(|x| x["id"] == *id).unwrap()["current_balance"].as_f64().unwrap();
    assert_eq!((bal(&a["id"]), bal(&b["id"])), (750.0, 250.0));
    transactions::update(&c, id, &json!({ "account_id": a["id"], "type": "expense", "amount": "100", "date": "2026-10-02" })).unwrap();
    assert_eq!((bal(&a["id"]), bal(&b["id"])), (900.0, 0.0));
    assert!(vaulty_core::ledger::unbalanced_entries(&c).unwrap().is_empty());
}

#[test]
fn bulk_changes_in_one_go() {
    use vaulty_core::{accounts, history, transactions};
    let c = vaulty_core::db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "5000" })).unwrap();
    let b = accounts::create(&c, &json!({ "name": "Cash", "type": "cash", "currency": "INR" })).unwrap();
    let mk = |extra: serde_json::Value| {
        let mut t = json!({ "account_id": a["id"], "type": "expense", "amount": "100", "date": "2026-10-10", "merchant": "Shop" });
        t.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        transactions::create(&c, &t).unwrap()["id"].as_str().unwrap().to_string()
    };
    let plain = mk(json!({ "tags": "a" }));
    let other = mk(json!({}));
    let transfer = mk(json!({ "type": "transfer", "to_account_id": b["id"] }));
    let split = mk(json!({ "splits": [{ "amount": "50" }, { "amount": "50", "category_id": "cat_home" }] }));
    let ids = vec![plain.clone(), other.clone(), transfer, split];

    let n = history::record(&c, "transaction.bulk", |c| transactions::bulk(c, &ids, &json!({ "category_id": "cat_food" }))).unwrap();
    assert_eq!(n, 2, "transfer and split skipped");
    assert_eq!(transactions::get_one(&c, &plain).unwrap().unwrap()["category_id"], "cat_food");
    assert_eq!(transactions::bulk(&c, &ids, &json!({ "add_tag": "#Trip" })).unwrap(), 4);
    assert_eq!(transactions::get_one(&c, &plain).unwrap().unwrap()["tags"], "a, Trip");
    assert_eq!(transactions::bulk(&c, &ids, &json!({ "add_tag": "trip" })).unwrap(), 0, "already tagged");
    assert_eq!(transactions::bulk(&c, &ids, &json!({ "remove_tag": "a" })).unwrap(), 1);
    assert_eq!(transactions::bulk(&c, std::slice::from_ref(&other), &json!({ "hidden": true })).unwrap(), 1);
    assert_eq!(transactions::bulk(&c, &ids, &json!({ "visibility": "public" })).unwrap(), 3, "hidden one stays private");
    assert!(transactions::bulk(&c, &ids, &json!({ "rename": "x" })).is_err());
    // Balances are untouched by all of that; the ledger stays balanced.
    assert_eq!(accounts::get_all(&c).unwrap().into_iter().find(|x| x["id"] == a["id"]).unwrap()["current_balance"].as_f64(), Some(4600.0));
    assert!(vaulty_core::ledger::unbalanced_entries(&c).unwrap().is_empty());
    // One undo reverses the whole category change.
    history::record(&c, "transaction.bulk", |c| transactions::bulk(c, &ids, &json!({ "delete": true }))).unwrap();
    assert!(transactions::get_all(&c, None).unwrap().is_empty());
    history::undo_last(&c).unwrap();
    assert_eq!(transactions::get_all(&c, None).unwrap().len(), 4);
}
