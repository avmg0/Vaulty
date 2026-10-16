use serde_json::json;
use vaulty_core::{db, market, settings};

#[test]
fn defaults_follow_the_profile_region() {
    let conn = db::open_in_memory().unwrap();
    settings::update(&conn, &json!({ "locale": "hi-IN", "currency": "INR" })).unwrap();
    let p = market::prefs(&conn).unwrap();
    assert!(!p.enabled, "the ticker is off until the person turns it on");
    assert_eq!(p.region, "IN");
    assert_eq!(p.crypto_currency, "INR");
    let v = market::prefs_view(&conn).unwrap();
    assert_eq!(v["regional"][0]["symbol"], "NIFTYBEES:NSE");
    assert_eq!(v["has_stock_key"], false);
}

#[test]
fn prefs_validate_everything_that_reaches_a_url() {
    let conn = db::open_in_memory().unwrap();
    let v = market::update_prefs(&conn, &json!({ "enabled": true, "watchlist": ["aapl", { "symbol": "tcs:nse", "label": "TCS" }],
        "crypto": ["bitcoin", "dogecoin"], "show": { "regional": false } })).unwrap();
    assert_eq!(v["prefs"]["watchlist"][0]["symbol"], "AAPL");
    assert_eq!(v["prefs"]["watchlist"][1]["label"], "TCS");
    assert_eq!(v["prefs"]["show"]["crypto"], true);
    for bad in [json!({ "crypto": ["bit coin&x=1"] }), json!({ "watchlist": ["A?B"] }), json!({ "stock_provider": "evil" }),
                json!({ "refresh_minutes": 1 }), json!({ "region": "ZZ" }), json!({ "unknown": 1 })] {
        assert!(market::update_prefs(&conn, &bad).is_err(), "{bad}");
    }
    // Rejected updates leave stored prefs alone.
    assert_eq!(market::prefs(&conn).unwrap().crypto, vec!["bitcoin", "dogecoin"]);
}

#[test]
fn keys_are_stored_but_never_shown() {
    let conn = db::open_in_memory().unwrap();
    market::set_key(&conn, "stock", Some(" abc123 ")).unwrap();
    assert_eq!(market::key(&conn, "stock").unwrap().as_deref(), Some("abc123"));
    let v = market::prefs_view(&conn).unwrap();
    assert_eq!(v["has_stock_key"], true);
    assert!(!v.to_string().contains("abc123"));
    market::set_key(&conn, "stock", Some("")).unwrap();
    assert_eq!(market::key(&conn, "stock").unwrap(), None);
    assert!(market::set_key(&conn, "stock", Some("has space")).is_err());
}

#[test]
fn parses_coingecko() {
    let body = json!({ "bitcoin": { "inr": 5612345.5, "inr_24h_change": -1.25 }, "ethereum": { "inr": 210000 } });
    let ids = vec!["bitcoin".to_string(), "ethereum".to_string(), "missing".to_string()];
    let q = market::parse_coingecko(&body, &ids, "inr", "2026-10-16T10:00:00Z");
    assert_eq!(q.len(), 2);
    assert_eq!(q[0].label, "BTC");
    assert_eq!(q[0].change_pct, Some(-1.25));
    assert_eq!(q[1].change_pct, None);
    assert_eq!(q[1].currency.as_deref(), Some("INR"));
    assert!(market::coingecko_url(&ids, "inr").ends_with("ids=bitcoin,ethereum,missing&vs_currencies=inr&include_24hr_change=true"));
}

#[test]
fn parses_twelvedata_batch_single_and_errors() {
    let items = vec![
        market::Item { symbol: "AAPL".into(), label: "Apple".into() },
        market::Item { symbol: "TCS:NSE".into(), label: "TCS".into() },
        market::Item { symbol: "NOPE".into(), label: "NOPE".into() },
    ];
    let body = json!({
        "AAPL": { "symbol": "AAPL", "close": "231.40", "percent_change": "0.84", "currency": "USD" },
        "TCS:NSE": { "symbol": "TCS", "close": "4102.5", "percent_change": "-0.3", "currency": "INR" },
        "NOPE": { "code": 404, "message": "symbol not found", "status": "error" }
    });
    let (ok, failed) = market::parse_twelvedata(&body, &items, "t");
    assert_eq!(ok.len(), 2);
    assert_eq!(ok[1].price, 4102.5);
    assert_eq!(failed, vec![("NOPE".to_string(), "symbol not found".to_string())]);

    let single = json!({ "symbol": "AAPL", "close": "231.40", "percent_change": "0.84", "currency": "USD" });
    assert_eq!(market::parse_twelvedata(&single, &items[..1], "t").0.len(), 1);

    let denied = json!({ "code": 401, "message": "Invalid API key", "status": "error" });
    let (ok, failed) = market::parse_twelvedata(&denied, &items, "t");
    assert!(ok.is_empty());
    assert_eq!(failed.len(), 3);
    assert_eq!(market::twelvedata_url(&items[..2]), "https://api.twelvedata.com/quote?symbol=AAPL,TCS:NSE");
}

#[test]
fn cache_keeps_ticker_order() {
    let conn = db::open_in_memory().unwrap();
    market::update_prefs(&conn, &json!({ "region": "US", "stock_provider": "twelvedata", "watchlist": ["TSLA"], "crypto": ["ethereum", "bitcoin"] })).unwrap();
    let q = |kind: &str, sym: &str| market::Quote { kind: kind.into(), symbol: sym.into(), label: sym.into(), price: 1.0, change_pct: None, currency: None, source: "x".into(), as_of: "t".into() };
    market::save_quotes(&conn, &[q("crypto", "bitcoin"), q("stock", "TSLA"), q("crypto", "ethereum"), q("stock", "SPY"), q("stock", "GONE")]).unwrap();
    let order: Vec<String> = market::cached(&conn).unwrap().into_iter().map(|q| q.symbol).collect();
    assert_eq!(order, vec!["SPY", "TSLA", "ethereum", "bitcoin"]);
    // Updating some quotes keeps the others; symbols no longer followed are dropped.
    market::save_quotes(&conn, &[q("crypto", "bitcoin")]).unwrap();
    assert_eq!(market::cached(&conn).unwrap().len(), 4);
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM market_quotes WHERE symbol = 'GONE'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn rate_limited_refreshes_cycle_through_long_lists() {
    let conn = db::open_in_memory().unwrap();
    market::update_prefs(&conn, &json!({ "region": "US", "stock_provider": "twelvedata", "watchlist": ["TSLA", "AMD"] })).unwrap();
    let p = market::prefs(&conn).unwrap();
    let stocks = market::plan(&p).stocks;
    assert_eq!(stocks.len(), 12);
    let first = market::next_stocks(&conn, &stocks, 8).unwrap();
    assert_eq!(first[0].symbol, "SPY");
    let fetched: Vec<market::Quote> = first.iter().map(|s| market::Quote { kind: "stock".into(), symbol: s.symbol.clone(), label: s.label.clone(),
        price: 1.0, change_pct: None, currency: None, source: "x".into(), as_of: "2026-10-16 10:00:00".into() }).collect();
    market::save_quotes(&conn, &fetched).unwrap();
    let second: Vec<String> = market::next_stocks(&conn, &stocks, 8).unwrap().into_iter().map(|s| s.symbol).collect();
    assert_eq!(&second[..4], &["META", "JPM", "TSLA", "AMD"]);
}
