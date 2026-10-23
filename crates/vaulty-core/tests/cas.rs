use chrono::NaiveDate;
use vaulty_core::cas;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/cas/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}
fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn reads_every_encryption_the_registrars_use() {
    for f in ["cas-aes128.pdf", "cas-aes256.pdf", "cas-rc4.pdf"] {
        let c = cas::read(&fixture(f), Some("CAS2026pw")).unwrap_or_else(|e| panic!("{f}: {e}"));
        assert_eq!((c.from, c.to), (Some(d("2025-04-01")), Some(d("2026-10-21"))), "{f}");
        assert_eq!((c.investor.as_deref(), c.pan.as_deref()), (Some("JOHN DOE"), Some("ABCDE1234F")));
        assert_eq!(c.schemes.len(), 3, "{f}");

        let flexi = &c.schemes[0];
        assert_eq!((flexi.amc.as_str(), flexi.folio.as_str(), flexi.isin.as_deref(), flexi.registrar.as_deref()),
            ("HDFC Mutual Fund", "1234567/89", Some("INF179K01UT0"), Some("CAMS")));
        assert_eq!(flexi.name, "HDFC Flexi Cap Fund - Direct Plan - Growth (Non-Demat)");
        assert_eq!((flexi.open_units_e4, flexi.close_units_e4), (12_345_670, Some(14_115_910)));
        assert_eq!((flexi.nav_e4, flexi.nav_date), (Some(20_456_780), Some(d("2026-10-21"))));
        assert_eq!((flexi.cost_minor, flexi.value_minor), (Some(31_500_000), Some(288_766_065)));
        assert!(flexi.balanced, "{f}: the units add up across the page break");
        let sips = flexi.txns.iter().filter(|t| t.kind == "sip").count();
        assert_eq!(sips, 63);
        assert!(flexi.txns.iter().any(|t| t.kind == "stamp_duty" && t.units_e4.is_none()));
        let red = flexi.txns.iter().find(|t| t.kind == "redemption").unwrap();
        assert_eq!((red.date, red.amount_minor, red.units_e4, red.nav_e4, red.balance_e4),
            (d("2025-09-15"), Some(-2_000_000), Some(-123_450), Some(16_201_100), Some(12_321_910)));

        let liquid = &c.schemes[1];
        assert_eq!((liquid.name.as_str(), liquid.close_units_e4, liquid.value_minor), ("HDFC Liquid Fund - Direct Plan - Growth Option", Some(0), Some(0)));

        let elss = &c.schemes[2];
        assert_eq!((elss.amc.as_str(), elss.folio.as_str(), elss.registrar.as_deref()), ("Axis Mutual Fund", "9101112131", Some("KFINTECH")));
        let kinds: Vec<&str> = elss.txns.iter().map(|t| t.kind).collect();
        assert!(kinds.starts_with(&["purchase"]) && kinds.contains(&"switch_in") && kinds.contains(&"dividend_payout"), "{kinds:?}");
        let idcw = elss.txns.iter().find(|t| t.kind == "dividend_payout").unwrap();
        assert_eq!((idcw.amount_minor, idcw.units_e4), (Some(121_050), None));
        assert_eq!((elss.close_units_e4, elss.cost_minor, elss.value_minor), (Some(4_841_990), Some(4_850_000), Some(5_181_240)));
        assert!(elss.balanced);
    }
}

#[test]
fn passwords_and_wrong_files() {
    let pdf = fixture("cas-aes128.pdf");
    assert!(cas::read(&pdf, None).unwrap_err().to_string().contains("needs its password"));
    assert!(cas::read(&pdf, Some("nope")).unwrap_err().to_string().contains("doesn't open"));
    assert!(cas::read(b"hello", None).unwrap_err().to_string().contains("isn't a PDF"));
    assert!(cas::parse(&[vec!["Bank statement".into(), "01-Jan-2026  Coffee  120.00".into()]]).is_err());
}

#[test]
fn wrapped_descriptions_and_missing_value() {
    let lines: Vec<String> = [
        "Consolidated Account Statement",
        "01-Apr-2025 To 31-Mar-2026",
        "Email Id: jane@example.com",
        "JANE DOE",
        "Example Mutual Fund",
        "Folio No: 55 / 1   PAN: ABCDE1234F",
        "JANE DOE",
        "X1-Example Small Cap Fund - Direct",
        "Growth - ISIN: INF000X01AB3 Registrar : KFINTECH",
        "Opening Unit Balance: 0.000",
        "10-Apr-2025  Systematic Investment  5,000.00  100.000  50.0000  100.000",
        "(Instalment 1)",
        "10-May-2025  *** Stamp Duty ***  0.25",
        "Closing Unit Balance: 100.000   NAV on 31-Mar-2026: INR 61.2500",
    ].iter().map(|s| s.to_string()).collect();
    let c = cas::parse(&[lines]).unwrap();
    let s = &c.schemes[0];
    assert_eq!((s.name.as_str(), s.isin.as_deref(), s.folio.as_str()), ("Example Small Cap Fund - Direct Growth", Some("INF000X01AB3"), "55/1"));
    assert_eq!(s.txns[0].description, "Systematic Investment (Instalment 1)");
    assert_eq!((s.txns[0].kind, s.txns[1].kind), ("sip", "stamp_duty"));
    assert_eq!(s.value_minor, Some(612_500), "worked out from units × NAV");
    assert_eq!(c.investor.as_deref(), Some("JANE DOE"));
}
