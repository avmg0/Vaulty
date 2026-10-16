//! Tamper evidence: the change history is a hash chain, and data edited outside Vaulty is flagged.
use serde_json::json;
use vaulty_core::{accounts, db, history, transactions};

fn setup() -> (rusqlite::Connection, String) {
    let c = db::open_in_memory().unwrap();
    let acc = history::record(&c, "account.create", |c| accounts::create(c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR", "initial_balance": "1000" }))).unwrap();
    let id = acc["id"].as_str().unwrap().to_string();
    for (i, amt) in ["120", "450", "80"].iter().enumerate() {
        history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": id, "type": "expense", "amount": amt, "merchant": format!("Shop {i}"), "date": "2026-10-01" }))).unwrap();
    }
    (c, id)
}

#[test]
fn untouched_history_verifies() {
    let (c, _) = setup();
    let v = history::verify(&c).unwrap();
    assert!(v.ok, "{v:?}");
    assert_eq!(v.checked, 4);
    assert_eq!(v.head.len(), 64);
    // Undo and redo stay on the chain.
    history::undo_last(&c).unwrap();
    let v2 = history::verify(&c).unwrap();
    assert!(v2.ok && v2.checked == 5);
    assert_ne!(v2.head, v.head, "every new action moves the head");
}

#[test]
fn editing_a_recorded_change_is_detected() {
    let (c, _) = setup();
    c.execute("UPDATE change_log SET after = replace(after, 'Shop 1', 'Shop X') WHERE after LIKE '%Shop 1%'", []).unwrap();
    let v = history::verify(&c).unwrap();
    assert!(!v.ok);
    assert!(v.reason.unwrap().contains("altered"));
    assert_eq!(v.broken_at, Some(3));
}

#[test]
fn deleting_a_change_or_an_action_is_detected() {
    let (c, _) = setup();
    c.execute("DELETE FROM change_log WHERE seq = (SELECT MAX(seq) FROM change_log WHERE batch_id = 2)", []).unwrap();
    assert!(!history::verify(&c).unwrap().ok);

    let (c, _) = setup();
    c.execute("DELETE FROM change_batches WHERE id = 2", []).unwrap();
    let v = history::verify(&c).unwrap();
    assert!(!v.ok);
    assert!(v.reason.unwrap().contains("missing"));
}

#[test]
fn stripping_a_fingerprint_is_detected() {
    let (c, _) = setup();
    c.execute("UPDATE change_batches SET hash = NULL, prev_hash = NULL WHERE id = 3", []).unwrap();
    assert!(!history::verify(&c).unwrap().ok);
}

#[test]
fn data_edited_outside_vaulty_is_flagged() {
    let (c, acc) = setup();
    // Someone with the key edits the database directly: no history entry is written.
    c.execute("UPDATE entries SET merchant = 'Not what happened' WHERE merchant = 'Shop 2'", []).unwrap();
    c.execute("UPDATE accounts SET name = 'Renamed quietly' WHERE id = ?", [&acc]).unwrap();
    let v = history::verify(&c).unwrap();
    assert!(!v.ok);
    let tables: Vec<&str> = v.changed_outside.iter().map(|x| x["table"].as_str().unwrap()).collect();
    assert!(tables.contains(&"entries") && tables.contains(&"accounts"), "{:?}", v.changed_outside);
    // Changes made through Vaulty are not flagged.
    let (c, acc) = setup();
    history::record(&c, "account.update", |c| accounts::update(c, &acc, &json!({ "name": "Renamed properly" }))).unwrap();
    assert!(history::verify(&c).unwrap().ok);
}

#[test]
fn clean_up_keeps_a_checkpoint_so_the_rest_still_verifies() {
    let (c, _) = setup();
    let before = history::verify(&c).unwrap();
    history::purge_before(&c, "2999-01-01T00:00:00Z").unwrap();
    assert_eq!(history::count(&c).unwrap(), 0);
    let acc = accounts::get_all(&c).unwrap()[0]["id"].as_str().unwrap().to_string();
    history::record(&c, "account.update", |c| accounts::update(c, &acc, &json!({ "name": "After clean-up" }))).unwrap();
    let v = history::verify(&c).unwrap();
    assert!(v.ok, "{v:?}");
    assert_eq!(v.checked, 1);
    assert_ne!(v.head, before.head);
}

#[test]
fn app_updates_are_recorded_not_flagged() {
    let (c, acc) = setup();
    // Simulate a migration reshaping data with no batch open (as migrations run).
    c.execute("UPDATE accounts SET notes = 'filled in by an app update' WHERE id = ?", [&acc]).unwrap();
    assert!(!history::verify(&c).unwrap().ok);
    assert_eq!(history::record_app_update(&c).unwrap(), 1);
    let v = history::verify(&c).unwrap();
    assert!(v.ok, "{v:?}");
    let last = history::recent(&c, 1).unwrap()[0].clone();
    assert_eq!(last["action"], "app.update");
    assert_eq!(last["can_undo"], false, "format updates aren't undoable");
    assert!(history::undo(&c, last["id"].as_i64().unwrap()).is_err());
}
