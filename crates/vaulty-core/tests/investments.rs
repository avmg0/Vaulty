use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{cas, db, estate, history, investments as inv};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn statement() -> cas::Cas {
    let bytes = std::fs::read(format!("{}/tests/fixtures/cas/cas-aes256.pdf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    cas::read(&bytes, Some("CAS2026pw")).unwrap()
}

#[test]
fn reading_a_statement_twice_adds_nothing_new() {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = d("2026-10-23");
    let s = statement();

    let p = inv::preview(&c, &s).unwrap();
    assert_eq!(p["schemes"].as_array().unwrap().iter().map(|x| x["status"].as_str().unwrap()).collect::<Vec<_>>(), ["new", "new", "new"]);
    let total = p["new_transactions"].as_u64().unwrap();
    assert!(total > 70);

    let r = history::record(&c, "Read a fund statement", |c| inv::import_cas(c, &s)).unwrap();
    assert_eq!((r["added"].as_u64(), r["transactions"].as_u64()), (Some(3), Some(total)));

    let again = inv::preview(&c, &s).unwrap();
    assert_eq!(again["new_transactions"], 0);
    assert!(again["schemes"].as_array().unwrap().iter().all(|x| x["status"] == "update"));
    let r = inv::import_cas(&c, &s).unwrap();
    assert_eq!((r["added"].as_u64(), r["updated"].as_u64(), r["transactions"].as_u64()), (Some(0), Some(3), Some(0)));

    let list = inv::list(&c, today).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0]["name"], "HDFC Flexi Cap Fund - Direct Plan - Growth (Non-Demat)");
    assert_eq!((list[0]["value"].as_f64(), list[0]["cost"].as_f64()), (Some(2_887_660.65), Some(315_000.0)));
    assert_eq!(list[2]["closed"], true, "the liquid fund was sold off, so it's last");
    let x = list[0]["xirr"].as_f64().unwrap();
    assert!(x > 100.0, "{x}: the made-up statement grew ninefold in a year and a half");
    let elss = list.iter().find(|h| h["isin"] == "INF846K01EW2").unwrap();
    assert_eq!(elss["dividends"].as_f64(), Some(1210.5));

    let txns = inv::transactions(&c, elss["id"].as_str().unwrap()).unwrap();
    assert_eq!(txns[0]["kind"], "dividend_payout", "newest first");
    assert_eq!(txns.last().unwrap()["units"].as_f64(), Some(253.807));

    let sum = inv::summary(&c, today).unwrap();
    assert_eq!((sum[0]["currency"].as_str(), sum[0]["holdings"].as_u64(), sum[0]["value"].as_f64()), (Some("INR"), Some(2), Some(2_939_472.99)));

    // The nominee's summary lists the funds still held.
    let e = estate::summary(&c, today).unwrap();
    assert_eq!(e.investments.len(), 2);
    assert!(e.investments[0].institution.as_deref().unwrap().contains("folio 1234567/89"));

    // Undo takes the whole first reading back out.
    history::undo_last(&c).unwrap();
    assert!(inv::list(&c, today).unwrap().is_empty());
}

#[test]
fn typed_in_holdings() {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = d("2026-10-23");
    let h = inv::create(&c, &json!({ "kind": "stock", "name": "Example Industries", "symbol": "EXIND", "units": "12", "price": "1520.55", "cost": "15000" }), today).unwrap();
    assert_eq!((h["value"].as_f64(), h["gain"].as_f64(), h["gain_percent"].as_f64(), h["xirr"].as_f64()), (Some(18_246.6), Some(3_246.6), Some(21.64), None));
    assert_eq!(h["price_date"], "2026-10-23");
    let id = h["id"].as_str().unwrap();
    let h = inv::set_price(&c, id, &json!("1600"), d("2026-10-24"), today).unwrap();
    assert_eq!((h["value"].as_f64(), h["price_date"].as_str()), (Some(19_200.0), Some("2026-10-24")));
    assert!(inv::create(&c, &json!({ "name": "x", "units": "1.123456" }), today).is_err(), "too many decimals");
    assert!(inv::create(&c, &json!({ "name": "x", "units": "-1" }), today).is_err());
    assert!(inv::create(&c, &json!({ "name": "x", "isin": "SHORT" }), today).is_err());
    assert!(inv::create(&c, &json!({ "name": "x", "kind": "tulips" }), today).is_err());
    assert!(inv::create(&c, &json!({ "name": "" }), today).is_err());
    inv::create(&c, &json!({ "name": "Fund", "isin": "INF000X01AB3", "folio": "1" }), today).unwrap();
    assert!(inv::create(&c, &json!({ "name": "Fund again", "isin": "inf000x01ab3", "folio": "1" }), today).unwrap_err().to_string().contains("already"));
    let h = inv::update(&c, id, &json!({ "kind": "stock", "name": "Example Industries Ltd", "units": "10", "price": "1600", "cost": "12500", "price_date": "2026-10-24" }), today).unwrap();
    assert_eq!((h["name"].as_str(), h["value"].as_f64()), (Some("Example Industries Ltd"), Some(16_000.0)));
    inv::delete(&c, id).unwrap();
    assert!(inv::get(&c, id, today).is_err());
}

#[test]
fn xirr_matches_a_known_answer() {
    // ₹1,00,000 in, ₹1,10,000 back a year later: 10%.
    let r = inv::xirr(&[(d("2025-01-01"), -100_000.0), (d("2026-01-01"), 110_000.0)]).unwrap();
    assert!((r - 0.10).abs() < 1e-6, "{r}");
    assert!(inv::xirr(&[(d("2025-01-01"), -100.0)]).is_none());
    assert!(inv::xirr(&[(d("2025-01-01"), -100.0), (d("2025-01-05"), 101.0)]).is_none(), "too short to say");
}
