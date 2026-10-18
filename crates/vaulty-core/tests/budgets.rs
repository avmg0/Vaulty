use serde_json::json;
use vaulty_core::{accounts, budgets, categories, db, history, settings, transactions};

fn spent(m: &serde_json::Value, id: &str) -> (Option<f64>, f64) {
    let c = m["categories"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap();
    (c["budget"].as_f64(), c["spent"].as_f64().unwrap())
}

#[test]
fn budgets_and_spending_per_month() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    let tx = |amount: &str, date: &str, cat: serde_json::Value, extra: serde_json::Value| {
        let mut t = json!({ "account_id": a["id"], "type": "expense", "amount": amount, "date": date, "category_id": cat });
        t.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        transactions::create(&c, &t).unwrap();
    };
    tx("1200", "2026-10-03", json!("cat_food"), json!({}));
    tx("300.50", "2026-10-20", json!("cat_food"), json!({ "hidden": true }));
    tx("999", "2026-09-30", json!("cat_food"), json!({}));                     // last month
    tx("1000", "2026-10-05", json!(null), json!({ "splits": [{ "category_id": "cat_food", "amount": "400" }, { "category_id": "cat_home", "amount": "600" }] }));
    tx("50", "2026-10-06", json!(null), json!({}));                            // uncategorised
    history::record(&c, "budget.set", |c| budgets::set(c, "cat_food", &json!("2000"))).unwrap();

    let m = budgets::month(&c, "2026-10").unwrap();
    assert_eq!(spent(&m, "cat_food"), (Some(2000.0), 1900.5), "own + hidden + split part, not September");
    assert_eq!(spent(&m, "cat_home"), (None, 600.0));
    assert_eq!(m["uncategorised_expense"].as_f64(), Some(50.0));
    assert_eq!(spent(&budgets::month(&c, "2026-09").unwrap(), "cat_food").1, 999.0);

    // Income categories report what came in; budgets are expense-only.
    assert!(budgets::set(&c, "cat_salary", &json!("100")).is_err());
    // Clearing a budget, and undoing that.
    history::record(&c, "budget.set", |c| budgets::set(c, "cat_food", &json!(null))).unwrap();
    assert_eq!(spent(&budgets::month(&c, "2026-10").unwrap(), "cat_food").0, None);
    history::undo_last(&c).unwrap();
    assert_eq!(spent(&budgets::month(&c, "2026-10").unwrap(), "cat_food").0, Some(2000.0));
    assert!(history::verify(&c).unwrap().ok);
    assert!(budgets::month(&c, "October").is_err());
}

#[test]
fn sub_categories_one_level_same_type() {
    let c = db::open_in_memory().unwrap();
    let groceries = categories::create(&c, &json!({ "name": "Groceries", "type": "expense", "parent_id": "cat_food" })).unwrap();
    assert_eq!(groceries["parent_id"], "cat_food");
    let gid = groceries["id"].as_str().unwrap();
    assert!(categories::create(&c, &json!({ "name": "Veg", "type": "expense", "parent_id": gid })).is_err(), "two levels");
    assert!(categories::create(&c, &json!({ "name": "Bonus", "type": "income", "parent_id": "cat_food" })).is_err(), "type mismatch");
    assert!(categories::create(&c, &json!({ "name": "  ", "type": "expense" })).is_err());
    assert!(categories::update(&c, "cat_food", &json!({ "parent_id": "cat_home" })).is_err(), "has children");
    assert!(categories::update(&c, gid, &json!({ "name": "Groceries", "type": "expense", "parent_id": gid })).is_err(), "itself");
    // Built-in: icon and colour change, name doesn't.
    let food = categories::update(&c, "cat_food", &json!({ "name": "Grub", "icon": "🥘", "color": "#000000" })).unwrap();
    assert_eq!((food["name"].as_str(), food["icon"].as_str()), (Some("Food & Dining"), Some("🥘")));
    // Deleting a parent lifts its children to the top level.
    let snacks = categories::create(&c, &json!({ "name": "Snacks", "type": "expense" })).unwrap();
    let chips = categories::create(&c, &json!({ "name": "Chips", "type": "expense", "parent_id": snacks["id"] })).unwrap();
    categories::delete(&c, snacks["id"].as_str().unwrap()).unwrap();
    let all = categories::get_all(&c).unwrap();
    assert!(all.iter().find(|x| x["id"] == chips["id"]).unwrap()["parent_id"].is_null());
}
