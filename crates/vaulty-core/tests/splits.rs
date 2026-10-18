use serde_json::json;
use vaulty_core::{accounts, db, ledger, transactions};

#[test]
fn a_purchase_split_across_categories() {
    let c = db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Card", "type": "checking", "currency": "INR", "initial_balance": "5000" })).unwrap();
    let base = json!({ "account_id": a["id"], "type": "expense", "amount": "1500", "date": "2026-10-10", "merchant": "Supermarket" });
    let mut split = base.clone();
    split["splits"] = json!([
        { "category_id": "cat_food", "amount": "900", "memo": "groceries" },
        { "category_id": "cat_home", "amount": "450.50" },
        { "amount": "149.50", "memo": "  " },
    ]);
    let t = transactions::create(&c, &split).unwrap();
    assert_eq!(t["category_name"], "Split");
    let parts = t["splits"].as_array().unwrap();
    assert_eq!(parts.len(), 3);
    assert_eq!((parts[0]["category_id"].as_str(), parts[0]["amount"].as_f64(), parts[0]["memo"].as_str()), (Some("cat_food"), Some(900.0), Some("groceries")));
    assert!(parts[2]["category_id"].is_null() && parts[2]["memo"].is_null(), "uncategorised part, blank memo dropped");
    assert_eq!(accounts::get_all(&c).unwrap()[0]["current_balance"].as_f64(), Some(3500.0));
    assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
    // Listed the same way, and a plain transaction has no splits.
    transactions::create(&c, &base).unwrap();
    let list = transactions::list(&c, None, true).unwrap();
    assert_eq!(list.iter().filter(|r| r.get("splits").is_some()).count(), 1);

    // Parts must add up, be positive, and match the type's categories.
    for bad in [json!([{ "category_id": "cat_food", "amount": "1000" }, { "amount": "100" }]),
                json!([{ "category_id": "cat_food", "amount": "1500" }]),
                json!([{ "category_id": "cat_food", "amount": "1600" }, { "amount": "-100" }]),
                json!([{ "category_id": "cat_salary", "amount": "1000" }, { "amount": "500" }])] {
        let mut b = base.clone(); b["splits"] = bad.clone();
        assert!(transactions::create(&c, &b).is_err(), "{bad} is rejected");
    }
    // Editing back to one category removes the split.
    let id = t["id"].as_str().unwrap();
    let mut one = base.clone(); one["category_id"] = json!("cat_food");
    let t = transactions::update(&c, id, &one).unwrap();
    assert!(t.get("splits").is_none());
    assert_eq!(t["category_id"], "cat_food");

    // A hidden split shows nothing about its parts.
    let t = transactions::create(&c, &{ let mut h = split.clone(); h["hidden"] = json!(true); h }).unwrap();
    let masked = transactions::list(&c, None, false).unwrap().into_iter().find(|r| r["id"] == t["id"]).unwrap();
    assert!(masked["splits"].is_null());
    assert_eq!(masked["amount"].as_f64(), Some(1500.0));
}

#[test]
fn split_income() {
    let c = db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR" })).unwrap();
    let t = transactions::create(&c, &json!({ "account_id": a["id"], "type": "income", "amount": "1000", "date": "2026-10-10",
        "splits": [{ "category_id": "cat_salary", "amount": "800" }, { "category_id": "cat_gift", "amount": "200" }] })).unwrap();
    assert_eq!(t["splits"].as_array().unwrap().len(), 2);
    assert_eq!(accounts::get_all(&c).unwrap()[0]["current_balance"].as_f64(), Some(1000.0));
    assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
}
