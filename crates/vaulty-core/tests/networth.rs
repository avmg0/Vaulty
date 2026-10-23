use chrono::NaiveDate;
use serde_json::{json, Value};
use vaulty_core::{accounts, categories, db, investments, networth, people, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn at<'a>(pts: &'a [Value], date: &str) -> &'a Value { pts.iter().find(|p| p["date"] == date).unwrap() }

#[test]
fn month_by_month() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-23");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "savings", "currency": "INR", "initial_balance": "100000", "opening_date": "2026-01-01" })).unwrap();
    let card = accounts::create(&c, &json!({ "name": "Card", "type": "credit", "currency": "INR", "initial_balance": "0" })).unwrap();
    let cats = categories::get_all(&c).unwrap();
    let spend = cats.iter().find(|x| x["type"] == "expense").unwrap()["id"].as_str().unwrap().to_string();
    let salary = cats.iter().find(|x| x["type"] == "income").unwrap()["id"].as_str().unwrap().to_string();
    let tx = |v: Value| transactions::create(&c, &v).unwrap();
    tx(json!({ "type": "income", "account_id": bank["id"], "category_id": salary, "amount": "50000", "date": "2026-08-01" }));
    tx(json!({ "type": "expense", "account_id": card["id"], "category_id": spend, "amount": "8000", "date": "2026-09-10" }));
    let john = people::create(&c, &json!({ "name": "John Doe" })).unwrap();
    tx(json!({ "account_id": bank["id"], "type": "transfer", "person_id": john["id"], "flow": "out", "amount": "5000", "date": "2026-09-20" }));

    // A fund bought in August from a statement (units and NAV per row), and a typed-in stock.
    let fund = investments::create(&c, &json!({ "name": "Example Fund", "kind": "mutual_fund", "currency": "INR", "units": "150", "price": "120" }), today).unwrap();
    c.execute("UPDATE holdings SET price_date = '2026-10-20' WHERE id = ?", [fund["id"].as_str().unwrap()]).unwrap();
    for (date, bal, price) in [("2026-08-05", 1_000_000i64, 1_000_000i64), ("2026-09-05", 1_500_000, 1_100_000)] {
        c.execute("INSERT INTO holding_txns (id, holding_id, date, kind, units_e4, price_e4, balance_e4) VALUES (?, ?, ?, 'purchase', 0, ?, ?)",
            rusqlite::params![format!("t{date}"), fund["id"].as_str().unwrap(), date, price, bal]).unwrap();
    }
    investments::create(&c, &json!({ "name": "Example Stock", "kind": "stock", "currency": "INR", "units": "10", "price": "1000" }), today).unwrap();

    let t = networth::timeline(&c, 3, today).unwrap();
    assert_eq!(t.len(), 1);
    let pts = t[0]["points"].as_array().unwrap();
    assert_eq!(pts.len(), 4);
    // End of July: the opening balance, the stock (held throughout), no fund yet.
    let jul = at(pts, "2026-07-31");
    assert_eq!((jul["cash"].as_f64(), jul["investments"].as_f64()), (Some(100_000.0), Some(10_000.0)));
    // End of August: salary in; 100 units at ₹100.
    let aug = at(pts, "2026-08-31");
    assert_eq!((aug["cash"].as_f64(), aug["investments"].as_f64()), (Some(150_000.0), Some(20_000.0)));
    // End of September: 150 units at ₹110; card owes 8,000; John owes 5,000.
    let sep = at(pts, "2026-09-30");
    assert_eq!(sep["investments"], 26_500.0);
    assert_eq!((sep["debts"].as_f64(), sep["owed_to_you"].as_f64(), sep["cash"].as_f64()), (Some(8_000.0), Some(5_000.0), Some(145_000.0)));
    assert_eq!(sep["net"], 145_000.0 + 26_500.0 + 5_000.0 - 8_000.0);
    // Today: the fund at today's price (₹120).
    let now = at(pts, "2026-10-23");
    assert_eq!(now["investments"], 18_000.0 + 10_000.0);
    assert_eq!(t[0]["net"], now["net"]);
    assert_eq!(t[0]["change_month"].as_f64().unwrap(), now["net"].as_f64().unwrap() - sep["net"].as_f64().unwrap());
    assert_eq!(t[0]["change_period"].as_f64().unwrap(), now["net"].as_f64().unwrap() - jul["net"].as_f64().unwrap());
}

#[test]
fn nothing_recorded() {
    let c = db::open_in_memory().unwrap();
    assert!(networth::timeline(&c, 12, d("2026-10-23")).unwrap().is_empty());
}
