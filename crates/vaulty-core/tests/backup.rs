use serde_json::{json, Value};
use vaulty_core::{accounts, backup, db, export, transactions};

#[test]
fn encrypted_backup_round_trip_and_tamper_checks() {
    let c = db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "1000" })).unwrap();
    transactions::create(&c, &json!({ "account_id": a["id"], "type": "expense", "amount": "250", "date": "2026-10-10", "merchant": "Secret shop" })).unwrap();

    assert!(backup::encrypt(&c, "short", None).is_err());
    assert!(backup::encrypt(&c, "correct horse battery", Some("my correct horse battery")).is_err(), "hint contains the password");
    let file = backup::encrypt(&c, "correct horse battery", Some("the usual one")).unwrap();
    let text = String::from_utf8(file.clone()).unwrap();
    assert!(!text.contains("Secret shop") && !text.contains("Bank"), "nothing readable inside");
    let h = backup::header(&file).unwrap();
    assert_eq!((h["format"].as_str(), h["hint"].as_str(), h.get("data")), (Some("vaulty-backup"), Some("the usual one"), None));
    assert!(backup::header(br#"{"version":2}"#).is_none());

    assert!(backup::decrypt(&file, "wrong password!").is_err());
    let plain = backup::decrypt(&file, "correct horse battery").unwrap();
    assert_eq!(plain["version"], 2);

    // Restores into a fresh profile.
    let fresh = db::open_in_memory().unwrap();
    export::from_json(&fresh, &plain).unwrap();
    assert_eq!(transactions::get_all(&fresh, None).unwrap()[0]["merchant"], "Secret shop");
    assert_eq!(accounts::get_all(&fresh).unwrap()[0]["current_balance"].as_f64(), Some(750.0));

    // Any change to the header breaks it.
    for (k, v) in [("hint", json!("changed")), ("created_at", json!("2020-01-01T00:00:00Z")), ("version", json!(1))] {
        let mut t: Value = serde_json::from_slice(&file).unwrap();
        if t[k] == v { continue; }
        t[k] = v;
        assert!(backup::decrypt(&serde_json::to_vec(&t).unwrap(), "correct horse battery").is_err(), "{k} changed");
    }
    let mut t: Value = serde_json::from_slice(&file).unwrap();
    t["kdf"]["iterations"] = json!(1);
    assert!(backup::decrypt(&serde_json::to_vec(&t).unwrap(), "correct horse battery").is_err());
}
