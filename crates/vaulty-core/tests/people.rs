use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, categories, db, history, ledger, people, reminders, reports, settings, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn bal(c: &rusqlite::Connection, id: &str) -> f64 {
    people::get(c, id).unwrap()["balances"].as_array().unwrap().first().map(|b| b["amount"].as_f64().unwrap()).unwrap_or(0.0)
}
fn account_balance(c: &rusqlite::Connection, id: &serde_json::Value) -> f64 {
    accounts::get_all(c).unwrap().into_iter().find(|a| a["id"] == *id).unwrap()["current_balance"].as_f64().unwrap()
}

#[test]
fn lending_borrowing_and_splitting_keep_the_books_straight() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "10000" })).unwrap();
    let john = people::create(&c, &json!({ "name": "John Doe", "phone": "+91 98765 43210" })).unwrap();
    let jane = people::create(&c, &json!({ "name": "Jane Doe" })).unwrap();
    let (john_id, jane_id) = (john["id"].as_str().unwrap(), jane["id"].as_str().unwrap());
    assert!(people::create(&c, &json!({ "name": " " })).is_err());

    // I lent John 2,000.
    let lent = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": john_id, "flow": "out", "amount": "2000", "date": "2026-10-01", "description": "Concert ticket" }))).unwrap();
    assert_eq!((lent["person_name"].as_str(), lent["flow"].as_str(), lent["to_account_id"].is_null()), (Some("John Doe"), Some("out"), true));
    assert_eq!(bal(&c, john_id), 2000.0);
    assert_eq!(account_balance(&c, &bank["id"]), 8000.0);

    // Dinner for three, ₹3,000, I paid: my share is spending, theirs is owed to me.
    let food = categories::get_all(&c).unwrap().into_iter().find(|x| x["name"] == "Food & Dining").unwrap()["id"].as_str().unwrap().to_string();
    let dinner = transactions::create(&c, &json!({ "account_id": bank["id"], "type": "expense", "amount": "3000", "date": "2026-10-05", "merchant": "Dinner",
        "splits": [{ "category_id": food, "amount": "1000" }, { "person_id": john_id, "amount": "1000" }, { "person_id": jane_id, "amount": "1000" }] })).unwrap();
    assert_eq!(dinner["amount"].as_f64(), Some(3000.0), "the transaction is what left the bank");
    let parts = transactions::get_all(&c, None).unwrap().into_iter().find(|t| t["id"] == dinner["id"]).unwrap()["splits"].as_array().unwrap().clone();
    assert_eq!(parts.iter().filter(|p| p["person_name"] == "Jane Doe").count(), 1, "split parts name the person");
    assert_eq!((bal(&c, john_id), bal(&c, jane_id)), (3000.0, 1000.0));
    let r = reports::overview(&c, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!(r.spending, 1000.0, "only my share is spending; lending isn't spending");

    // Jane paid me back; I borrowed 500 from her.
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": jane_id, "flow": "in", "amount": "1000", "date": "2026-10-08" })).unwrap();
    let borrowed = transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": jane_id, "flow": "in", "amount": "500", "date": "2026-10-09" })).unwrap();
    assert_eq!(borrowed["flow"].as_str(), Some("in"));
    assert_eq!(bal(&c, jane_id), -500.0, "now I owe Jane");
    assert_eq!(account_balance(&c, &bank["id"]), 10000.0 - 2000.0 - 3000.0 + 1000.0 + 500.0);
    let totals = people::totals(&c).unwrap();
    assert_eq!((totals[0]["owed_to_me"].as_f64(), totals[0]["i_owe"].as_f64()), (Some(3000.0), Some(500.0)));

    let h = people::history(&c, jane_id).unwrap();
    assert_eq!(h.iter().map(|x| (x["what"].as_str().unwrap(), x["balance"].as_f64().unwrap())).collect::<Vec<_>>(),
               [("received", -500.0), ("received", 0.0), ("split", 1000.0)], "newest first, with the running balance");

    // Undo, editing and the ledger all still hold.
    history::undo_last(&c).unwrap();
    assert_eq!(bal(&c, john_id), 1000.0, "undoing the loan leaves his share of dinner");
    transactions::update(&c, dinner["id"].as_str().unwrap(), &json!({ "account_id": bank["id"], "type": "expense", "amount": "3000", "date": "2026-10-05",
        "splits": [{ "category_id": food, "amount": "1500" }, { "person_id": jane_id, "amount": "1500" }] })).unwrap();
    assert_eq!((bal(&c, john_id), bal(&c, jane_id)), (0.0, 0.0));
    assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
    assert!(transactions::create(&c, &json!({ "account_id": bank["id"], "type": "income", "amount": "100", "date": "2026-10-09",
        "splits": [{ "person_id": jane_id, "amount": "50" }, { "amount": "50" }] })).is_err(), "income isn't split with people");

    // Removing: refused while money is owed; someone settled is hidden, not deleted.
    let sam = people::create(&c, &json!({ "name": "Sam" })).unwrap();
    people::delete(&c, sam["id"].as_str().unwrap()).unwrap();
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": jane_id, "flow": "out", "amount": "100", "date": "2026-10-10" })).unwrap();
    assert!(people::delete(&c, jane_id).is_err());
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": jane_id, "flow": "in", "amount": "100", "date": "2026-10-11" })).unwrap();
    people::delete(&c, jane_id).unwrap();
    assert_eq!(people::list(&c).unwrap().len(), 1);
    assert!(transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": jane_id, "flow": "out", "amount": "1", "date": "2026-10-12" })).is_err());
}

#[test]
fn quiet_debts_get_a_reminder() {
    let c = db::open_in_memory().unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "10000" })).unwrap();
    let john = people::create(&c, &json!({ "name": "John Doe" })).unwrap();
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": john["id"], "flow": "out", "amount": "2000", "date": "2026-09-01" })).unwrap();
    assert!(reminders::take_due(&c, d("2026-09-20")).unwrap().iter().all(|r| r.kind != "owed"), "not yet");
    let due = reminders::take_due(&c, d("2026-10-05")).unwrap();
    let owed = due.iter().find(|r| r.kind == "owed").unwrap();
    assert_eq!((owed.name.as_str(), owed.amount, owed.days), ("John Doe", Some(2000.0), 34));
    assert!(reminders::take_due(&c, d("2026-10-06")).unwrap().iter().all(|r| r.kind != "owed"), "once");
}

#[test]
fn people_budgets_and_rules_survive_a_backup() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "10000" })).unwrap();
    let john = people::create(&c, &json!({ "name": "John Doe" })).unwrap();
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "transfer", "person_id": john["id"], "flow": "out", "amount": "2000", "date": "2026-10-01" })).unwrap();
    let food = categories::get_all(&c).unwrap().into_iter().find(|x| x["name"] == "Food & Dining").unwrap()["id"].as_str().unwrap().to_string();
    vaulty_core::budgets::set(&c, &food, &json!("8000")).unwrap();
    vaulty_core::rules::add(&c, "swiggy", &food).unwrap();
    vaulty_core::hotkeys::set(&c, "quick_add", Some("Mod+Shift+A")).unwrap();

    let fresh = db::open_in_memory().unwrap();
    vaulty_core::export::from_json(&fresh, &vaulty_core::export::to_json(&c).unwrap()).unwrap();
    assert_eq!(bal(&fresh, john["id"].as_str().unwrap()), 2000.0);
    assert_eq!(vaulty_core::budgets::month(&fresh, "2026-10").unwrap()["categories"].as_array().unwrap().iter()
        .find(|x| x["id"] == food.as_str()).unwrap()["budget"].as_f64(), Some(8000.0));
    assert_eq!(vaulty_core::rules::list(&fresh).unwrap().len(), 1);
    assert_eq!(vaulty_core::hotkeys::list(&fresh).unwrap().into_iter().find(|h| h.action == "quick_add").unwrap().keys, "Mod+Shift+A");
    assert!(ledger::unbalanced_entries(&fresh).unwrap().is_empty());
}
