use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{card_benefits, cards, categories, db, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn rewards_milestones_perks_and_the_best_card() {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = d("2026-10-23");
    let food = categories::get_all(&c).unwrap().into_iter().find(|x| x["type"] == "expense" && x["name"].as_str().unwrap().contains("Food")).unwrap();
    let food = food["id"].as_str().unwrap().to_string();
    let rewards = cards::create(&c, &json!({ "kind": "credit", "name": "Rewards", "currency": "INR", "statement_day": 15, "due_day": 5, "credit_limit": "200000" })).unwrap();
    let shop = cards::create(&c, &json!({ "kind": "credit", "name": "Shopping", "currency": "INR", "statement_day": 1, "due_day": 20, "credit_limit": "50000" })).unwrap();
    let (r, s) = (rewards["id"].as_str().unwrap(), shop["id"].as_str().unwrap());
    cards::update(&c, r, &json!({ "card_year_start": "03-15", "annual_fee": "2500" })).unwrap();

    card_benefits::create(&c, r, &json!({ "kind": "reward", "title": "5% on food", "category_id": food, "rate": 5, "cap": "500" }), today).unwrap();
    card_benefits::create(&c, r, &json!({ "kind": "reward", "title": "1% on everything", "rate": "1" }), today).unwrap();
    card_benefits::create(&c, r, &json!({ "kind": "milestone", "title": "Annual fee waived", "target": "150000" }), today).unwrap();
    let lounge = card_benefits::create(&c, r, &json!({ "kind": "perk", "title": "Airport lounge", "allowance": 4 }), today).unwrap();
    card_benefits::create(&c, s, &json!({ "kind": "reward", "title": "10% at Amazon", "merchant": "Amazon", "rate": 10 }), today).unwrap();
    card_benefits::create(&c, s, &json!({ "kind": "reward", "title": "1.5% everywhere", "rate": 1.5 }), today).unwrap();
    assert!(card_benefits::create(&c, r, &json!({ "kind": "reward", "title": "x", "rate": 0 }), today).is_err());
    assert!(card_benefits::create(&c, r, &json!({ "kind": "milestone", "title": "x" }), today).is_err());
    assert!(card_benefits::create(&c, r, &json!({ "kind": "jackpot", "title": "x" }), today).is_err());

    // Food spending this statement (after 15 Oct), and some earlier in the card year.
    transactions::create(&c, &json!({ "account_id": rewards["account_id"], "type": "expense", "amount": "8000", "date": "2026-10-18", "category_id": food, "merchant": "Swiggy" })).unwrap();
    transactions::create(&c, &json!({ "account_id": rewards["account_id"], "type": "expense", "amount": "42000", "date": "2026-06-02", "merchant": "Croma" })).unwrap();

    let list = card_benefits::list(&c, r, today).unwrap();
    let get = |t: &str| list.iter().find(|b| b["title"] == t).unwrap().clone();
    assert_eq!((get("5% on food")["progress"]["earned"].as_f64(), get("5% on food")["progress"]["since"].as_str()), (Some(400.0), Some("2026-10-16")));
    let m = get("Annual fee waived");
    assert_eq!((m["progress"]["spent"].as_f64(), m["progress"]["left"].as_f64(), m["progress"]["since"].as_str()), (Some(50000.0), Some(100000.0), Some("2026-03-15")));

    // Perks are counted per card year.
    let lid = lounge["id"].as_str().unwrap();
    card_benefits::use_perk(&c, lid, 1, today).unwrap();
    assert_eq!(card_benefits::use_perk(&c, lid, 1, today).unwrap(), 2);
    assert_eq!(card_benefits::list(&c, r, today).unwrap().iter().find(|b| b["title"] == "Airport lounge").unwrap()["progress"]["left"], 2);
    assert_eq!(card_benefits::list(&c, r, d("2027-03-20")).unwrap().iter().find(|b| b["title"] == "Airport lounge").unwrap()["progress"]["used"], 0, "a new card year");

    // Food for ₹4,000: 5% would be ₹200 but only ₹100 of the cap is left; Shopping gives ₹60.
    let picks = card_benefits::best_for(&c, today, Some(&json!("4000")), Some(&food), None).unwrap();
    let top: Vec<(&str, Option<f64>)> = picks.iter().map(|p| (p.name.as_str(), p.earns)).collect();
    assert_eq!(top, [("Rewards", Some(100.0)), ("Shopping", Some(60.0))]);
    assert_eq!((picks[0].interest_free_days, picks[1].interest_free_days), (Some(43), Some(28)));
    // At Amazon, the Shopping card.
    let picks = card_benefits::best_for(&c, today, Some(&json!("2000")), None, Some("Amazon Pay")).unwrap();
    assert_eq!((picks[0].name.as_str(), picks[0].earns, picks[0].reward.as_deref()), ("Shopping", Some(200.0), Some("10% at Amazon")));
    // Too big for the Shopping card's limit: it goes last, whatever it earns.
    let picks = card_benefits::best_for(&c, today, Some(&json!("60000")), None, Some("Amazon")).unwrap();
    assert_eq!((picks[0].name.as_str(), picks[1].name.as_str(), picks[1].fits), ("Rewards", "Shopping", false));
}
