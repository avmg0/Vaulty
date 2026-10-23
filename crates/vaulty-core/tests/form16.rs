use serde_json::{json, Value};
use vaulty_core::{db, form16, tax_plan as tp};

fn fixture(name: &str) -> Vec<u8> { std::fs::read(format!("{}/tests/fixtures/form16/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap() }
fn field(o: &Value, f: &str, id: Option<&str>) -> Option<f64> {
    o["fields"].as_array().unwrap().iter().find(|x| x["field"] == f && id.is_none_or(|i| x["id"] == i)).and_then(|x| x["amount"].as_f64())
}

#[test]
fn old_regime_form_reads_every_figure() {
    let f = form16::read_pdf(&fixture("form16-old.pdf"), None).unwrap();
    assert_eq!(f.employer.as_deref(), Some("EXAMPLE TECH PRIVATE LIMITED"));
    assert_eq!(f.employee.as_deref(), Some("JANE DOE"));
    assert_eq!(f.tan.as_deref(), Some("PNEE12345E"));
    assert_eq!(f.pan.as_deref(), Some("ABCPD1234K"));
    assert_eq!(f.assessment_year.as_deref(), Some("2027-28"));
    assert_eq!(form16::year_id(&f).as_deref(), Some("FY2026-27"));
    assert_eq!(f.quarters, vec![(1, 37_500_000, 1_850_000), (2, 37_500_000, 1_850_000), (3, 37_500_000, 1_850_000), (4, 37_500_000, 2_250_000)]);
    assert_eq!(f.salary, Some(150_000_000));
    assert_eq!(f.hra, Some(21_000_000));
    assert_eq!(f.standard_deduction, Some(5_000_000));
    assert_eq!(f.professional_tax, Some(250_000));
    assert_eq!(f.house_property, Some(-20_000_000), "a label over two lines, amount on the first");
    assert_eq!(f.other_sources, Some(1_200_000));
    assert_eq!(f.regime, Some("old"));
    assert_eq!(f.taxable, Some(75_860_000));
    assert_eq!(f.tax, Some(6_678_900));
    let ded: std::collections::HashMap<_, _> = f.deductions.iter().cloned().collect();
    assert_eq!(ded["investments"], 15_000_000, "deductible, not gross");
    assert_eq!(ded["nps_own"], 5_000_000);
    assert_eq!(ded["employer_nps"], 6_000_000);
    assert_eq!(ded["health_self"], 1_840_000);
    assert_eq!(ded["donations"], 250_000);
    assert_eq!(ded["savings_interest"], 1_000_000);
    assert!(!ded.contains_key("education_loan"), "zero lines are left out");

    let o = form16::offer(&f).unwrap();
    assert_eq!(o["year"], "FY2026-27");
    assert_eq!(field(&o, "salary", None), Some(1_500_000.0));
    assert_eq!(field(&o, "other_income", None), Some(12_000.0));
    assert_eq!(field(&o, "deduction", Some("hra")), Some(210_000.0));
    assert_eq!(field(&o, "deduction", Some("home_loan_interest")), Some(200_000.0));
    assert_eq!(o["tds"], 78_000.0);
    let dates: Vec<&str> = o["payments"].as_array().unwrap().iter().map(|p| p["date"].as_str().unwrap()).collect();
    assert_eq!(dates, ["2026-06-30", "2026-09-30", "2026-12-31", "2027-03-31"]);
}

#[test]
fn locked_new_regime_form() {
    assert!(form16::read_pdf(&fixture("form16-new.pdf"), None).is_err(), "needs the password");
    let f = form16::read_pdf(&fixture("form16-new.pdf"), Some("jane01011990")).unwrap();
    assert_eq!(f.regime, Some("new"));
    assert_eq!(f.salary, Some(180_000_000));
    assert_eq!(f.quarters.iter().map(|q| q.2).sum::<i64>(), 8_700_000);
    assert_eq!(f.deductions, vec![("employer_nps", 9_000_000)]);
    assert_eq!(f.hra, None);
}

#[test]
fn not_a_form16() {
    let bill = std::fs::read(format!("{}/tests/fixtures/invoices/shop.pdf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(form16::read_pdf(&bill, None).is_err());
}

#[test]
fn applying_fills_the_plan_once() {
    let c = db::open_in_memory().unwrap();
    let f = form16::read_pdf(&fixture("form16-old.pdf"), None).unwrap();
    let o = form16::offer(&f).unwrap();
    let pays: Vec<Value> = o["payments"].as_array().unwrap().iter().map(|p| json!({ "date": p["date"], "amount": p["amount"], "note": "Form 16" })).collect();
    let body = json!({ "input": { "age": 35, "salary": "1500000", "other_income": "12000", "deductions": { "hra": "210000", "investments": "150000" }, "alt": [] }, "regime": "old", "payments": pays });
    assert_eq!(tp::apply_form16(&c, "IN", "FY2026-27", &body).unwrap()["payments"], 4);
    assert_eq!(tp::apply_form16(&c, "IN", "FY2026-27", &body).unwrap()["payments"], 0, "the same quarters aren't added twice");
    let p = tp::plan(&c, "IN", "FY2026-27", chrono::NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()).unwrap();
    assert_eq!(p["tds"], 78_000.0);
    assert_eq!(p["regime"], "old");
    assert_eq!(p["input"]["deductions"]["hra"], "210000");
}
