use serde_json::json;
use vaulty_core::{accounts, db, history, household::{Household, SharingPrefs, Visibility}, transactions};

fn setup() -> (rusqlite::Connection, String) {
    let c = db::open_in_memory().unwrap();
    let acc = accounts::create(&c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR", "initial_balance": "10000" })).unwrap();
    (c, acc["id"].as_str().unwrap().to_string())
}

#[test]
fn hidden_transactions_are_masked_but_keep_their_amount() {
    let (c, acc) = setup();
    let t = transactions::create(&c, &json!({ "account_id": acc, "type": "expense", "amount": "2500", "merchant": "Surprise gift shop",
        "notes": "anniversary", "date": "2026-10-10", "hidden": true })).unwrap();
    transactions::create(&c, &json!({ "account_id": acc, "type": "expense", "amount": "300", "merchant": "Groceries", "date": "2026-10-11" })).unwrap();

    let shown = transactions::list(&c, None, false).unwrap();
    let h = shown.iter().find(|r| r["id"] == t["id"]).unwrap();
    assert_eq!(h["merchant"], "XXXXX");
    assert_eq!(h["category_name"], "Hidden");
    assert!(h["notes"].is_null());
    assert_eq!(h["amount"], 2500.0, "amount stays visible");
    assert_eq!(h["masked"], true);
    assert!(!serde_json::to_string(&shown).unwrap().contains("Surprise"), "details never leave unmasked");
    // Balances still include it.
    let bal = accounts::get_all(&c).unwrap()[0]["current_balance"].as_f64().unwrap();
    assert_eq!(bal, 10000.0 - 2500.0 - 300.0);

    // Revealed only on request (the app checks the Hidden section is unlocked).
    let only = transactions::list(&c, Some(&json!({ "hidden": "only" })), true).unwrap();
    assert_eq!(only.len(), 1);
    assert_eq!(only[0]["merchant"], "Surprise gift shop");
    assert_eq!(transactions::list(&c, Some(&json!({ "hidden": "exclude" })), false).unwrap().len(), 1);
    assert_eq!(transactions::hidden_count(&c).unwrap(), 1);
}

#[test]
fn hiding_later_also_masks_its_history_and_edits_keep_it_hidden() {
    let (c, acc) = setup();
    let t = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": acc, "type": "expense", "amount": "900", "merchant": "Secret plans", "date": "2026-10-10" }))).unwrap();
    let id = t["id"].as_str().unwrap().to_string();
    assert_eq!(history::recent(&c, 1).unwrap()[0]["title"], "Secret plans");
    history::record(&c, "transaction.hide", |c| transactions::set_hidden(c, &id, true)).unwrap();
    for h in history::recent(&c, 10).unwrap() { assert_ne!(h["title"], "Secret plans"); }
    // Editing (once unlocked) keeps it hidden.
    history::record(&c, "transaction.update", |c| transactions::update(c, &id, &json!({ "account_id": acc, "type": "expense", "amount": "950", "merchant": "Secret plans", "date": "2026-10-10" }))).unwrap();
    assert!(transactions::is_hidden(&c, &id).unwrap());
    let v = history::verify(&c).unwrap();
    assert!(v.ok, "{v:?}");
}

#[test]
fn hidden_transactions_are_never_shared() {
    let (c, acc) = setup();
    let h = Household::open_in_memory().unwrap();
    h.set_prefs("me", &SharingPrefs { enabled: true, default_visibility: Visibility::Public, shared_with: vec![] }).unwrap();
    let t = transactions::create(&c, &json!({ "account_id": acc, "type": "expense", "amount": "100", "merchant": "Public lunch", "date": "2026-10-10", "visibility": "public" })).unwrap();
    h.publish("me", &t).unwrap();
    assert_eq!(transactions::shareable(&c).unwrap().len(), 1);
    let t = transactions::set_hidden(&c, t["id"].as_str().unwrap(), true).unwrap();
    h.publish("me", &t).unwrap();
    assert!(transactions::shareable(&c).unwrap().is_empty());
    let n: i64 = h.conn_for_tests().query_row("SELECT COUNT(*) FROM shared_items", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "hiding withdraws it from the family view");
}
