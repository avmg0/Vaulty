use chrono::NaiveDate;
use serde_json::{json, Value};
use vaulty_core::{accounts, db, export, goals, investments};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn find<'a>(l: &'a [Value], name: &str) -> &'a Value { l.iter().find(|g| g["name"] == name).unwrap() }

#[test]
fn progress_plan_and_links() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-23");
    let fd = accounts::create(&c, &json!({ "name": "Savings", "type": "savings", "currency": "INR", "initial_balance": "200000" })).unwrap();
    let card = accounts::create(&c, &json!({ "name": "Card", "type": "credit", "currency": "INR", "initial_balance": "0" })).unwrap();
    let fund = investments::create(&c, &json!({ "name": "Example Index Fund", "kind": "mutual_fund", "currency": "INR", "units": "1000", "price": "150" }), today).unwrap();

    // A house down payment: ₹10 lakh in 3 years, half the savings account and the fund set aside.
    let house = goals::create(&c, &json!({ "name": "House down payment", "currency": "INR", "target": "1000000", "target_date": "2029-10-23",
        "monthly": "15000", "return_rate": 8, "links": [{ "account_id": fd["id"], "share": 50 }, { "holding_id": fund["id"] }] })).unwrap();
    // An emergency fund: the other half of savings plus 60% more of it: over-claimed.
    goals::create(&c, &json!({ "name": "Emergency fund", "currency": "INR", "target": "300000", "saved": "50000",
        "links": [{ "account_id": fd["id"], "share": 60 }] })).unwrap();
    // A trip that's past its date.
    goals::create(&c, &json!({ "name": "Trip", "currency": "INR", "target": "100000", "target_date": "2026-09-01", "saved": "20000" })).unwrap();

    let l = goals::list(&c, today).unwrap();
    let h = find(&l, "House down payment");
    assert_eq!(h["saved"], 250_000.0, "half of ₹2 lakh + the fund's ₹1.5 lakh");
    assert_eq!(h["months_left"], 36);
    assert_eq!(h["progress"], 0.25);
    let need = h["needed_monthly"].as_f64().unwrap();
    assert!(need > 15_000.0 && need < 17_000.0, "{need}");
    assert_eq!(h["status"], "behind", "₹15,000 a month falls short");
    assert!(h["links"][0]["over"].as_bool().unwrap(), "savings is claimed 110%");
    assert!(!h["links"][1]["over"].as_bool().unwrap());

    let e = find(&l, "Emergency fund");
    assert_eq!(e["saved"], 170_000.0);
    assert_eq!(e["status"], "no_date");
    assert_eq!(find(&l, "Trip")["status"], "overdue");

    // Raising the plan puts it on track.
    let mut v = h.clone();
    v["monthly"] = json!("17000");
    v.as_object_mut().unwrap().remove("links");
    goals::update(&c, house["id"].as_str().unwrap(), &v).unwrap();
    let h = goals::list(&c, today).unwrap().into_iter().find(|g| g["name"] == "House down payment").unwrap();
    assert_eq!(h["status"], "on_track");
    assert_eq!(h["links"].as_array().unwrap().len(), 2, "links kept when not given");

    // Inflation grows a target set in today's money.
    goals::create(&c, &json!({ "name": "College", "currency": "INR", "target": "1000000", "target_date": "2036-10-23", "inflation": 8 })).unwrap();
    let col = goals::list(&c, today).unwrap().into_iter().find(|g| g["name"] == "College").unwrap();
    assert!((col["target_then"].as_f64().unwrap() - 2_158_925.0).abs() < 1.0, "{}", col["target_then"]);

    // Done goals stop counting towards what's claimed.
    goals::set_done(&c, house["id"].as_str().unwrap(), true, today).unwrap();
    let l = goals::list(&c, today).unwrap();
    assert_eq!(find(&l, "House down payment")["status"], "done");
    assert!(!find(&l, "Emergency fund")["links"][0]["over"].as_bool().unwrap());
    assert_eq!(goals::summary(&c, today).unwrap()[0]["goals"], 3);

    // Refused: a card, another currency, nonsense.
    assert!(goals::create(&c, &json!({ "name": "X", "currency": "INR", "target": "10", "links": [{ "account_id": card["id"] }] })).is_err());
    assert!(goals::create(&c, &json!({ "name": "X", "currency": "USD", "target": "10", "links": [{ "account_id": fd["id"] }] })).is_err());
    assert!(goals::create(&c, &json!({ "name": "", "currency": "INR", "target": "10" })).is_err());
    assert!(goals::create(&c, &json!({ "name": "X", "currency": "INR", "target": "0" })).is_err());

    // In backups.
    let b = export::to_json(&c).unwrap();
    assert_eq!(b["goals"].as_array().unwrap().len(), 4);
    assert_eq!(b["goal_links"].as_array().unwrap().len(), 3);
    let c2 = db::open_in_memory().unwrap();
    export::from_json(&c2, &b).unwrap();
    assert_eq!(goals::list(&c2, today).unwrap().len(), 4);
}
