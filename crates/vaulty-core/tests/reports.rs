use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, cards, categories, db, recurring, reports, settings, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn cat(c: &rusqlite::Connection, name: &str) -> String {
    categories::get_all(c).unwrap().into_iter().find(|x| x["name"] == name).unwrap_or_else(|| panic!("no {name}"))["id"].as_str().unwrap().to_string()
}

#[test]
fn a_period_by_month_category_and_place() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "100000" })).unwrap();
    let usd = accounts::create(&c, &json!({ "name": "US", "type": "checking", "currency": "USD", "initial_balance": "100" })).unwrap();
    let food = cat(&c, "Food & Dining");
    let tx = |amount: &str, date: &str, kind: &str, merchant: &str, extra: serde_json::Value| {
        let mut t = json!({ "account_id": bank["id"], "type": kind, "amount": amount, "date": date, "merchant": merchant });
        for (k, v) in extra.as_object().unwrap() { t[k] = v.clone(); }
        transactions::create(&c, &t).unwrap();
    };
    tx("85000", "2026-09-01", "income", "Acme", json!({ "category_id": cat(&c, "Salary") }));
    tx("85000", "2026-10-01", "income", "Acme", json!({ "category_id": cat(&c, "Salary") }));
    tx("450", "2026-09-10", "expense", "Big Bazaar", json!({ "category_id": food }));
    tx("320", "2026-10-03", "expense", "Swiggy", json!({ "category_id": food }));
    tx("280", "2026-10-09", "expense", "SWIGGY", json!({ "category_id": food }));
    tx("1000", "2026-10-12", "expense", "Split dinner", json!({ "splits": [{ "category_id": food, "amount": "600" }, { "category_id": cat(&c, "Transportation"), "amount": "400" }] }));
    tx("42000", "2026-10-15", "expense", "Anniversary ring", json!({ "category_id": cat(&c, "Shopping"), "hidden": true }));
    tx("50", "2026-10-16", "expense", "", json!({}));
    // Moving money between one's own accounts is neither income nor spending.
    let savings = accounts::create(&c, &json!({ "name": "Savings", "type": "savings", "currency": "INR" })).unwrap();
    tx("10000", "2026-10-02", "transfer", "", json!({ "to_account_id": savings["id"] }));
    transactions::create(&c, &json!({ "account_id": usd["id"], "type": "expense", "amount": "20", "date": "2026-10-05", "merchant": "Coffee NYC" })).unwrap();
    // A sub-category counts in its parent.
    let groceries = categories::create(&c, &json!({ "name": "Groceries", "type": "expense", "parent_id": food })).unwrap();
    tx("700", "2026-10-20", "expense", "Corner store", json!({ "category_id": groceries["id"] }));

    let r = reports::overview(&c, "2026-09-01", "2026-10-31").unwrap();
    assert_eq!(r.currency, "INR");
    assert_eq!(r.months.iter().map(|m| (m.month.as_str(), m.income, m.spending)).collect::<Vec<_>>(),
               [("2026-09", 85000.0, 450.0), ("2026-10", 85000.0, 320.0 + 280.0 + 1000.0 + 42000.0 + 50.0 + 700.0)]);
    assert_eq!(r.income, 170000.0);
    assert_eq!(r.other_currencies, 1, "the dollar coffee is counted apart, not mixed in");
    let slice = |name: &str| r.categories.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no {name}: {:?}", r.categories));
    assert_eq!(slice("Food & Dining").amount, 450.0 + 320.0 + 280.0 + 600.0 + 700.0, "split part and sub-category included");
    assert_eq!(slice("Transportation").amount, 400.0);
    assert_eq!(r.categories[0].name, "Shopping", "largest first");
    assert!(r.categories.iter().any(|s| s.id.is_none() && s.amount == 50.0), "uncategorised has its own slice");
    // Places: the same place in different case counts once; hidden spending never shows its name.
    let swiggy = r.places.iter().find(|p| p.name.eq_ignore_ascii_case("swiggy")).unwrap();
    assert_eq!((swiggy.amount, swiggy.count), (600.0, 2));
    assert!(!r.places.iter().any(|p| p.name.contains("Anniversary")));
    assert!(r.places.iter().any(|p| p.name == "\u{0}hidden" && p.amount == 42000.0));
    assert!((r.savings_rate.unwrap() - (170000.0 - r.spending) / 170000.0).abs() < 1e-9);

    // Empty months still appear, so charts have no gaps.
    let r = reports::overview(&c, "2026-07-01", "2026-10-31").unwrap();
    assert_eq!(r.months.len(), 4);
    assert_eq!(r.months[0].spending, 0.0);
    assert!(reports::overview(&c, "2026-10-31", "2026-10-01").is_err());
}

#[test]
fn forecast_and_safe_to_spend() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = d("2026-10-19");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "40000" })).unwrap();
    let cash = accounts::create(&c, &json!({ "name": "Wallet", "type": "cash", "currency": "INR", "initial_balance": "2000" })).unwrap();
    accounts::create(&c, &json!({ "name": "Shares", "type": "investment", "currency": "INR", "initial_balance": "500000" })).unwrap();
    recurring::create(&c, &json!({ "name": "Salary", "type": "income", "amount": "85000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-11-01" }), today).unwrap();
    recurring::create(&c, &json!({ "name": "Rent", "type": "expense", "amount": "22000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-11-05" }), today).unwrap();
    recurring::create(&c, &json!({ "name": "SIP", "type": "expense", "amount": "5000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-21" }), today).unwrap();
    // Moving money between two everyday accounts changes nothing.
    recurring::create(&c, &json!({ "name": "Pocket money", "type": "transfer", "amount": "500", "account_id": bank["id"], "to_account_id": cash["id"], "frequency": "weekly", "start_date": "2026-10-20" }), today).unwrap();
    cards::create(&c, &json!({ "kind": "credit", "name": "Rewards", "currency": "INR", "owed": "12000", "due_day": 25 })).unwrap();

    let f = reports::forecast(&c, today, 30).unwrap();
    assert_eq!(f.balance, 42000.0, "bank and cash, not investments");
    assert_eq!(f.points.len(), 31);
    let at = |date: &str| f.points.iter().find(|p| p.date == date).unwrap().scheduled;
    assert_eq!(at("2026-10-20"), 42000.0, "the transfer to the wallet changes nothing");
    assert_eq!(at("2026-10-21"), 37000.0, "SIP");
    assert_eq!(at("2026-10-25"), 25000.0, "card paid in full");
    assert_eq!(at("2026-11-01"), 110000.0, "salary");
    assert_eq!(at("2026-11-05"), 88000.0, "rent");
    assert_eq!(f.typical_day, 0.0, "no history yet");
    assert_eq!(f.lowest.as_ref().unwrap().date, "2026-10-25");
    assert!(f.changes.iter().any(|c| c.kind == "card" && c.amount == -12000.0));

    // Before the salary on the 1st: 42,000 minus the SIP and the card payment.
    let s = reports::safe_to_spend(&c, today).unwrap();
    assert_eq!((s.until.as_str(), s.days), ("2026-11-01", 13));
    assert_eq!(s.committed, 17000.0);
    assert_eq!(s.amount, 25000.0);
    assert_eq!(s.per_day, (2_500_000 / 13) as f64 / 100.0);
    assert_eq!(s.income_name.as_deref(), Some("Salary"));
    assert!(!s.short);

    // Everyday spending shows up as a typical day.
    for i in 0..30 {
        transactions::create(&c, &json!({ "account_id": bank["id"], "type": "expense", "amount": "300", "date": (today - chrono::Duration::days(i + 1)).to_string() })).unwrap();
    }
    let f = reports::forecast(&c, today, 30).unwrap();
    assert_eq!(f.typical_day, 300.0, "averaged over the 30 days with history");
    assert!(f.points[10].expected < f.points[10].scheduled);
    assert!(reports::forecast(&c, today, 3).is_err());
}
