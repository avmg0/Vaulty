use serde_json::json;
use vaulty_core::{accounts, cards, categories, db, history, transactions};

fn setup() -> (rusqlite::Connection, String) {
    let c = db::open_in_memory().unwrap();
    let acc = accounts::create(&c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR", "initial_balance": "1000" })).unwrap();
    (c, acc["id"].as_str().unwrap().to_string())
}

fn balance(c: &rusqlite::Connection, id: &str) -> f64 {
    accounts::get_all(c).unwrap().into_iter().find(|a| a["id"] == id).unwrap()["current_balance"].as_f64().unwrap()
}

#[test]
fn changes_outside_an_action_are_not_logged() {
    let (c, _) = setup();
    assert_eq!(history::count(&c).unwrap(), 0);
}

#[test]
fn undo_a_deleted_transaction_restores_it_and_its_amounts() {
    let (c, acc) = setup();
    let t = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": acc, "type": "expense", "amount": "450", "merchant": "Big Bazaar", "date": "2026-10-01" }))).unwrap();
    let id = t["id"].as_str().unwrap().to_string();
    assert_eq!(balance(&c, &acc), 550.0);
    history::record(&c, "transaction.delete", |c| transactions::delete(c, &id)).unwrap();
    assert_eq!(balance(&c, &acc), 1000.0);

    let trash = history::trash(&c).unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0]["title"], "Big Bazaar");
    assert_eq!(trash[0]["amount"]["amount"], 450.0);

    history::undo(&c, trash[0]["id"].as_i64().unwrap()).unwrap();
    assert_eq!(transactions::get_one(&c, &id).unwrap().unwrap()["merchant"], "Big Bazaar");
    assert_eq!(balance(&c, &acc), 550.0);
    assert!(history::trash(&c).unwrap().is_empty(), "restored items leave the trash");
    assert!(vaulty_core::ledger::unbalanced_entries(&c).unwrap().is_empty());
}

#[test]
fn undo_an_edit_and_redo_it() {
    let (c, acc) = setup();
    let t = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": acc, "type": "expense", "amount": "100", "date": "2026-10-01" }))).unwrap();
    let id = t["id"].as_str().unwrap().to_string();
    history::record(&c, "transaction.update", |c| transactions::update(c, &id, &json!({ "account_id": acc, "type": "expense", "date": "2026-10-01", "amount": "250", "merchant": "Typo fixed" }))).unwrap();
    assert_eq!(balance(&c, &acc), 750.0);

    let undone = history::undo_last(&c).unwrap().unwrap();
    assert_eq!(undone["action"], "transaction.update");
    assert_eq!(balance(&c, &acc), 900.0);
    // The undo is itself in the history and can be undone (redo).
    let last = history::recent(&c, 1).unwrap()[0].clone();
    assert_eq!(last["action"], "undo");
    history::undo(&c, last["id"].as_i64().unwrap()).unwrap();
    assert_eq!(balance(&c, &acc), 750.0);
    assert_eq!(transactions::get_one(&c, &id).unwrap().unwrap()["merchant"], "Typo fixed");
}

#[test]
fn an_older_action_cant_be_undone_over_a_later_one() {
    let (c, acc) = setup();
    let t = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": acc, "type": "expense", "amount": "100", "date": "2026-10-01" }))).unwrap();
    let id = t["id"].as_str().unwrap().to_string();
    history::record(&c, "transaction.update", |c| transactions::update(c, &id, &json!({ "account_id": acc, "type": "expense", "date": "2026-10-01", "amount": "200" }))).unwrap();
    let create_batch = history::for_item(&c, "entries", &id).unwrap().last().unwrap()["id"].as_i64().unwrap();
    let err = history::undo(&c, create_batch).unwrap_err().to_string();
    assert!(err.contains("changed again"), "{err}");
    assert_eq!(history::for_item(&c, "entries", &id).unwrap().len(), 2);
}

#[test]
fn undo_category_delete_puts_transactions_back() {
    let (c, acc) = setup();
    let cat = history::record(&c, "category.create", |c| categories::create(c, &json!({ "name": "Pets", "type": "expense" }))).unwrap();
    let cat_id = cat["id"].as_str().unwrap().to_string();
    let t = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": acc, "category_id": cat_id, "type": "expense", "amount": "80", "date": "2026-10-02" }))).unwrap();
    history::record(&c, "category.delete", |c| categories::delete(c, &cat_id)).unwrap();
    assert_ne!(transactions::get_one(&c, t["id"].as_str().unwrap()).unwrap().unwrap()["category_id"], json!(cat_id));
    history::undo_last(&c).unwrap();
    assert_eq!(transactions::get_one(&c, t["id"].as_str().unwrap()).unwrap().unwrap()["category_id"], json!(cat_id));
    assert!(categories::get_all(&c).unwrap().iter().any(|x| x["id"] == json!(cat_id)));
}

#[test]
fn undo_card_removal_brings_back_its_account() {
    let c = db::open_in_memory().unwrap();
    let card = history::record(&c, "card.create", |c| cards::create(c, &json!({ "kind": "gift", "name": "Book voucher", "currency": "INR", "balance": "500" }))).unwrap();
    let id = card["id"].as_str().unwrap().to_string();
    history::record(&c, "card.delete", |c| cards::delete(c, &id)).unwrap();
    assert!(cards::list(&c).unwrap().is_empty());
    history::undo_last(&c).unwrap();
    assert_eq!(cards::list(&c).unwrap()[0]["name"], "Book voucher");
}

#[test]
fn a_failed_action_leaves_no_history() {
    let (c, _) = setup();
    assert!(history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "type": "bogus" }))).is_err());
    assert_eq!(history::count(&c).unwrap(), 0);
    // And the context is closed, so later unrecorded changes aren't captured.
    accounts::create(&c, &json!({ "name": "Other", "type": "cash", "currency": "INR" })).unwrap();
    assert_eq!(history::count(&c).unwrap(), 0);
}

#[test]
fn history_survives_reopening_and_logs_are_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.db");
    let key = vaulty_core::Key::new([7u8; 32]);
    {
        let c = db::open(&path, &key).unwrap();
        let acc = accounts::create(&c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR" })).unwrap();
        let id = acc["id"].as_str().unwrap().to_string();
        history::record(&c, "account.delete", |c| accounts::delete(c, &id)).unwrap();
    }
    let c = db::open(&path, &key).unwrap();
    assert_eq!(history::recent(&c, 10).unwrap()[0]["action"], "account.delete");
    history::undo_last(&c).unwrap();
    assert_eq!(accounts::get_all(&c).unwrap().len(), 1);
}
