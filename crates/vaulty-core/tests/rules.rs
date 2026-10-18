use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, categories, db, history, quick, rules, transactions};

#[test]
fn rules_learn_correct_and_apply() {
    let c = db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR" })).unwrap();
    assert_eq!(rules::key("SWIGGY*ORDER 12345 BANGALORE").as_deref(), Some("swiggy order bangalore"));
    assert_eq!(rules::key("Acme Pvt Ltd").as_deref(), Some("acme"));
    assert_eq!(rules::key("12"), None);

    rules::learn(&c, "Swiggy", "cat_food").unwrap();
    assert_eq!(rules::suggest(&c, "SWIGGY ORDER 7781", "expense").unwrap().as_deref(), Some("cat_food"));
    assert_eq!(rules::suggest(&c, "Swiggy", "income").unwrap(), None, "type must fit");
    // A longer rule beats a shorter one; a correction replaces a learned rule.
    let groceries = categories::create(&c, &json!({ "name": "Groceries", "type": "expense", "parent_id": "cat_food" })).unwrap();
    rules::learn(&c, "Swiggy Instamart", groceries["id"].as_str().unwrap()).unwrap();
    assert_eq!(rules::suggest(&c, "SWIGGY INSTAMART 99", "expense").unwrap(), groceries["id"].as_str().map(String::from));
    assert_eq!(rules::suggest(&c, "swiggy", "expense").unwrap().as_deref(), Some("cat_food"));
    rules::learn(&c, "Swiggy", "cat_entertainment").unwrap();
    assert_eq!(rules::suggest(&c, "swiggy", "expense").unwrap().as_deref(), Some("cat_entertainment"));
    // Manual rules aren't overwritten by learning.
    rules::add(&c, "swiggy", "cat_food").unwrap();
    rules::learn(&c, "Swiggy", "cat_shopping").unwrap();
    assert_eq!(rules::suggest(&c, "swiggy", "expense").unwrap().as_deref(), Some("cat_food"));

    // Uncategorised transactions get categorised in one undoable action; split and hidden ones are left alone.
    let mk = |m: &str, extra: serde_json::Value| {
        let mut t = json!({ "account_id": a["id"], "type": "expense", "amount": "100", "date": "2026-10-10", "merchant": m });
        t.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        transactions::create(&c, &t).unwrap()
    };
    let plain = mk("SWIGGY 123", json!({}));
    mk("Swiggy", json!({ "hidden": true }));
    mk("Swiggy", json!({ "splits": [{ "amount": "40" }, { "category_id": "cat_home", "amount": "60" }] }));
    mk("Unknown shop", json!({}));
    assert_eq!(rules::pending(&c).unwrap().len(), 1);
    assert_eq!(history::record(&c, "rules.apply", rules::apply_all).unwrap(), 1);
    assert_eq!(transactions::get_one(&c, plain["id"].as_str().unwrap()).unwrap().unwrap()["category_id"], "cat_food");
    assert!(vaulty_core::ledger::unbalanced_entries(&c).unwrap().is_empty());
    history::undo_last(&c).unwrap();
    assert!(transactions::get_one(&c, plain["id"].as_str().unwrap()).unwrap().unwrap()["category_id"].is_null());
}

#[test]
fn quick_add_understands_plain_language() {
    let c = db::open_in_memory().unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Everyday savings", "type": "checking", "currency": "INR" })).unwrap();
    let cash = accounts::create(&c, &json!({ "name": "Cash wallet", "type": "cash", "currency": "INR" })).unwrap();
    let card = accounts::create(&c, &json!({ "name": "Rewards", "type": "credit", "currency": "INR" })).unwrap();
    let accs = accounts::get_all(&c).unwrap();
    rules::learn(&c, "Corner Cafe", "cat_food").unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(); // a Monday
    let p = |s: &str| quick::parse(&c, s, today, &accs).unwrap();

    let r = p("250 swiggy yesterday");
    assert_eq!((r.kind, r.amount.as_deref(), r.date.as_str(), r.merchant.as_str(), r.category_id.as_deref(), r.account_id.as_deref()),
               ("expense", Some("250"), "2026-10-18", "Swiggy", Some("cat_food"), None));
    let r = p("+85,000 salary on 1st");
    assert_eq!((r.kind, r.amount.as_deref(), r.date.as_str(), r.category_id.as_deref()), ("income", Some("85000"), "2026-10-01", Some("cat_salary")));
    let r = p("petrol 2k from card");
    assert_eq!((r.amount.as_deref(), r.category_id.as_deref(), r.account_id.as_deref(), r.merchant.as_str()), (Some("2000"), Some("cat_transport"), card["id"].as_str(), "Petrol"));
    let r = p("corner cafe 180 cash last friday");
    assert_eq!((r.amount.as_deref(), r.date.as_str(), r.account_id.as_deref(), r.category_id.as_deref(), r.merchant.as_str()),
               (Some("180"), "2026-10-16", cash["id"].as_str(), Some("cat_food"), "Corner Cafe"));
    let r = p("rent 22000 5 oct everyday");
    assert_eq!((r.date.as_str(), r.account_id.as_deref(), r.category_id.as_deref()), ("2026-10-05", bank["id"].as_str(), Some("cat_home")));
    let r = p("कल 300 सब्ज़ी");
    assert_eq!((r.date.as_str(), r.amount.as_deref(), r.category_id.as_deref()), ("2026-10-18", Some("300"), Some("cat_food")));
    let r = p("gift from mom 1.5 lakh");
    assert_eq!((r.kind, r.amount.as_deref(), r.category_id.as_deref()), ("income", Some("150000"), Some("cat_gift")));
    assert_eq!(p("lunch").amount, None, "no amount: the app asks for one");
    assert_eq!(p("dec 25 gift 500").date, "2025-12-25", "a date later this year means last year");
}
