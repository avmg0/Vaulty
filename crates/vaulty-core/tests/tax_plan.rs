use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, calendar, categories, db, reminders, tax_plan as tp, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
const Y: &str = "FY2026-27";

#[test]
fn hra_exemption_is_the_least_of_three() {
    // Salary 6 lakh, HRA 2.4 lakh, rent 2.4 lakh: rent − 10% salary = 1.8 lakh is least.
    assert_eq!(tp::hra_exempt(600_000, 240_000, 240_000, true), 180_000);
    // Outside a metro, 40% of salary (1.2 lakh) can be the least.
    assert_eq!(tp::hra_exempt(300_000, 200_000, 400_000, false), 120_000);
    assert_eq!(tp::hra_exempt(600_000, 240_000, 30_000, true), 0, "rent below 10% of salary");
}

#[test]
fn plan_is_kept_filled_from_records_and_paid_down() {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let acct = accounts::create(&c, &json!({ "name": "Salary account", "type": "savings", "currency": "INR", "initial_balance": "0" })).unwrap();
    let cats = categories::get_all(&c).unwrap();
    let health = categories::create(&c, &json!({ "name": "Health insurance", "type": "expense" })).unwrap();
    let salary = cats.iter().find(|x| x["type"] == "income").unwrap()["id"].as_str().unwrap().to_string();
    let hid = health["id"].as_str().unwrap().to_string();
    let tx = |kind: &str, cat: &str, amount: &str, date: &str| transactions::create(&c, &json!({ "type": kind, "account_id": acct["id"], "category_id": cat, "amount": amount, "date": date })).unwrap();
    tx("income", &salary, "150000", "2026-04-30");
    tx("income", &salary, "150000", "2026-05-31");
    tx("expense", &hid, "25000", "2026-06-10");
    tx("expense", &hid, "9999", "2026-03-31"); // last year: not counted

    // Link the categories; a spending category can't feed salary.
    assert!(tp::set_links(&c, "IN", Y, &json!([{ "category_id": hid, "target": "salary" }])).is_err());
    let links = tp::set_links(&c, "IN", Y, &json!([{ "category_id": hid, "target": "health_self" }, { "category_id": salary, "target": "salary" }])).unwrap();
    assert_eq!(links.len(), 2);
    let r = tp::records(&c, "IN", Y).unwrap();
    assert_eq!((r["by"]["salary"].as_f64(), r["by"]["health_self"].as_f64()), (Some(300000.0), Some(25000.0)));

    // A salary of ₹24 lakh in the new regime.
    tp::save(&c, "IN", Y, &json!({ "input": { "age": 35, "salary": "2400000", "deductions": { "health_self": "25000" } }, "regime": "new" })).unwrap();
    assert!(tp::save(&c, "IN", Y, &json!({ "regime": "flat-tax" })).is_err());
    let p = tp::plan(&c, "IN", Y, d("2026-06-01")).unwrap();
    assert_eq!(p["used"], "new");
    let total = p["total"].as_f64().unwrap();
    assert!(total > 100_000.0, "{total}");
    // TDS covers part; the rest is due in instalments.
    tp::add_payment(&c, "IN", Y, &json!({ "kind": "tds", "date": "2026-06-30", "amount": "100000" })).unwrap();
    tp::add_payment(&c, "IN", Y, &json!({ "kind": "advance", "date": "2026-06-14", "amount": "5000" })).unwrap();
    assert!(tp::add_payment(&c, "IN", Y, &json!({ "kind": "advance", "date": "2026-03-01", "amount": "5" })).is_err(), "before the year");
    let p = tp::plan(&c, "IN", Y, d("2026-09-10")).unwrap();
    let after_tds = total - 100_000.0;
    let a = p["advance"].as_array().unwrap();
    assert_eq!(a.len(), 4);
    assert_eq!((a[0]["date"].as_str(), a[0]["needed"].as_f64(), a[0]["paid"].as_f64()), (Some("2026-06-15"), Some((after_tds * 0.15).round()), Some(5000.0)));
    assert_eq!(a[3]["date"], "2027-03-15");
    assert_eq!(p["balance"].as_f64(), Some(total - 105_000.0));

    // Reminded a week before 15 September, and on the calendar.
    let due = reminders::due(&c, d("2026-09-10")).unwrap();
    let r = due.iter().find(|x| x.kind == "tax").unwrap();
    assert_eq!((r.date.as_str(), r.amount), ("2026-09-15", Some((after_tds * 0.45).round() - 5000.0)));
    let cal = calendar::range(&c, "2026-09-01", "2026-09-30", "IN", d("2026-09-10")).unwrap();
    assert!(cal.iter().any(|i| i.kind == "tax" && i.date == "2026-09-15"));
    // Paying it all stops the reminders.
    tp::add_payment(&c, "IN", Y, &json!({ "kind": "advance", "date": "2026-09-12", "amount": format!("{:.0}", after_tds) })).unwrap();
    assert!(!reminders::due(&c, d("2026-09-13")).unwrap().iter().any(|x| x.kind == "tax"));
    assert!(tp::plan(&c, "IN", Y, d("2026-09-13")).unwrap()["balance"].as_f64().unwrap() <= 0.0);
}

#[test]
fn small_tax_needs_no_advance_tax_and_hra_details_are_checked() {
    let c = db::open_in_memory().unwrap();
    tp::save(&c, "IN", Y, &json!({ "input": { "salary": "1000000" }, "hra": { "salary": "600000", "received": "240000", "monthly_rent": "20000", "metro": true, "from": "2026-04", "to": "2027-03" } })).unwrap();
    let p = tp::plan(&c, "IN", Y, d("2026-06-01")).unwrap();
    assert!(p["advance"].as_array().unwrap().iter().all(|a| a["needed"].as_f64() == Some(0.0)), "the new regime's rebate leaves no tax");
    assert_eq!((p["hra_result"]["months"].as_i64(), p["hra_result"]["rent"].as_f64(), p["hra_result"]["exempt"].as_f64(), p["hra_result"]["pan_needed"].as_bool()),
        (Some(12), Some(240000.0), Some(180000.0), Some(true)));
    assert!(tp::save(&c, "IN", Y, &json!({ "hra": { "monthly_rent": "abc" } })).is_err());
}
