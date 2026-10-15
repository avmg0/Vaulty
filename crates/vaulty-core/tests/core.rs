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
