use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, calendar, categories, db, export, insurance as ins, reminders, tax_plan, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn policies_premiums_claims_and_cover() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-23");
    let acct = accounts::create(&c, &json!({ "name": "Salary account", "type": "savings", "currency": "INR", "initial_balance": "500000" })).unwrap();
    let cats = categories::get_all(&c).unwrap();
    let salary = cats.iter().find(|x| x["type"] == "income").unwrap()["id"].as_str().unwrap().to_string();
    let spend = cats.iter().find(|x| x["type"] == "expense").unwrap()["id"].as_str().unwrap().to_string();
    for m in ["2026-01-31", "2026-04-30", "2026-07-31", "2026-10-15"] {
        transactions::create(&c, &json!({ "type": "income", "account_id": acct["id"], "category_id": salary, "amount": "300000", "date": m })).unwrap();
    }

    let term = ins::create(&c, &json!({ "kind": "term_life", "name": "Term plan", "insurer": "Example Life", "currency": "INR", "cover": "10000000",
        "premium": "15000", "frequency": "yearly", "next_due": "2026-11-01", "nominee": "John Doe" })).unwrap();
    let health = ins::create(&c, &json!({ "kind": "health", "name": "Family floater", "insurer": "Example Health", "currency": "INR", "cover": "1000000",
        "premium": "2000", "frequency": "monthly", "next_due": "2026-10-10" })).unwrap();
    ins::create(&c, &json!({ "kind": "health", "name": "Parents' cover", "currency": "INR", "cover": "500000", "premium": "30000", "for_parents": true, "next_due": "2027-03-01" })).unwrap();
    ins::create(&c, &json!({ "kind": "motor", "name": "Car", "currency": "INR", "premium": "12000", "next_due": "2026-08-01" })).unwrap();

    let l = ins::list(&c, today).unwrap();
    let by = |n: &str| l.iter().find(|p| p["name"] == n).unwrap().clone();
    assert_eq!(by("Term plan")["due"], "soon");
    assert_eq!(by("Family floater")["due"], "late");
    assert_eq!(by("Family floater")["premium_yearly"], 24_000.0);
    assert_eq!(by("Car")["due"], "lapsing", "past the 30-day grace");

    // Cover against a year's income (₹12 lakh): ₹1 crore is 8.3×.
    let s = &ins::summary(&c, today).unwrap()[0];
    assert_eq!(s["life_cover"], 10_000_000.0);
    assert_eq!(s["health_cover"], 1_500_000.0);
    assert_eq!(s["income_year"], 1_200_000.0);
    assert_eq!(s["life_multiple"], 8.3);
    assert_eq!(s["premiums_yearly"], 15_000.0 + 24_000.0 + 30_000.0 + 12_000.0);

    // Reminders and calendar: the late health premium and the term premium due soon.
    let due = reminders::due(&c, today).unwrap();
    let names: Vec<&str> = due.iter().filter(|r| r.kind == "premium").map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Term plan") && names.contains(&"Family floater") && !names.contains(&"Car"), "{names:?}");
    let cal = calendar::range(&c, "2026-10-01", "2026-12-31", "IN", today).unwrap();
    let floater: Vec<_> = cal.iter().filter(|i| i.kind == "premium" && i.title == "Family floater").collect();
    assert_eq!(floater.len(), 3, "October, November, December");

    // Paying moves the due date on from the date that was due, and can add the transaction.
    let hid = health["id"].as_str().unwrap();
    ins::pay(&c, hid, &json!({ "date": "2026-10-23", "record": { "account_id": acct["id"], "category_id": spend } })).unwrap();
    let p = ins::list(&c, today).unwrap().into_iter().find(|p| p["id"] == hid).unwrap();
    assert_eq!(p["next_due"], "2026-11-10");
    assert_eq!(p["paid_this_year"], 2000.0);
    assert!(p["payments"][0]["entry_id"].is_string());
    ins::pay(&c, term["id"].as_str().unwrap(), &json!({ "date": "2026-10-20", "amount": "15000" })).unwrap();

    // The tax planner counts premiums: health (80D) and life (80C), not twice.
    let r = tax_plan::records(&c, "IN", "FY2026-27").unwrap();
    assert_eq!(r["by"]["health_self"], 2000.0);
    assert_eq!(r["by"]["investments"], 15_000.0);
    tax_plan::set_links(&c, "IN", "FY2026-27", &json!([{ "category_id": spend, "target": "health_self" }])).unwrap();
    assert_eq!(tax_plan::records(&c, "IN", "FY2026-27").unwrap()["by"]["health_self"], 2000.0, "the premium's transaction isn't counted again");

    // Claims.
    let cl = ins::add_claim(&c, hid, &json!({ "date": "2026-09-12", "what": "Hospital stay", "claimed": "85000" })).unwrap();
    assert_eq!(ins::list(&c, today).unwrap().into_iter().find(|p| p["id"] == hid).unwrap()["open_claims"], 1);
    ins::update_claim(&c, cl["id"].as_str().unwrap(), &json!({ "date": "2026-09-12", "what": "Hospital stay", "claimed": "85000", "settled": "72000", "status": "partly_settled", "settled_on": "2026-10-01" })).unwrap();
    let p = ins::list(&c, today).unwrap().into_iter().find(|p| p["id"] == hid).unwrap();
    assert_eq!(p["open_claims"], 0);
    assert_eq!(p["claims"][0]["settled"], 72_000.0);

    // Refused.
    assert!(ins::create(&c, &json!({ "kind": "pet", "name": "X", "currency": "INR" })).is_err());
    assert!(ins::create(&c, &json!({ "kind": "life", "name": "X", "currency": "INR", "start_date": "2026-01-01", "end_date": "2025-01-01" })).is_err());
    assert!(ins::add_claim(&c, hid, &json!({ "date": "2026-09-12", "what": "", "claimed": "1" })).is_err());

    // Backups carry it all.
    let b = export::to_json(&c).unwrap();
    let c2 = db::open_in_memory().unwrap();
    export::from_json(&c2, &b).unwrap();
    assert_eq!(ins::list(&c2, today).unwrap().len(), 4);
}
