//! Worked examples for India FY 2026-27, checked by hand against the published slabs
//! (rebate and surcharge marginal relief included).
use serde_json::{json, Value};
use vaulty_core::tax;

fn total(regime: &str, req: Value) -> f64 {
    let r = tax::compare("IN", Some("FY2026-27"), &req).unwrap();
    r["estimates"].as_array().unwrap().iter().find(|e| e["regime"] == regime).unwrap()["total"].as_f64().unwrap()
}

#[test]
fn packs_parse_and_validate() {
    assert_eq!(tax::countries().unwrap(), vec!["IN"]);
    let p = tax::pack("in").unwrap();
    assert_eq!(p.currency, "INR");
    assert!(p.years[0].regimes.iter().any(|r| r.id == "new" && r.default));
}

#[test]
fn new_regime_salary_up_to_12_75_lakh_pays_nothing() {
    // 12,75,000 − 75,000 standard deduction = 12,00,000 → full rebate.
    assert_eq!(total("new", json!({ "salary": 1_275_000 })), 0.0);
}

#[test]
fn new_regime_13_75_lakh_salary() {
    // Taxable 13,00,000: 20,000 + 40,000 + 15,000 = 75,000; cess 3,000.
    assert_eq!(total("new", json!({ "salary": 1_375_000 })), 78_000.0);
}

#[test]
fn new_regime_rebate_marginal_relief() {
    // Taxable 12,10,000: slab tax 61,500 but capped at the 10,000 above 12 lakh; cess 400.
    assert_eq!(total("new", json!({ "other_income": 1_210_000 })), 10_400.0);
}

#[test]
fn new_regime_60_lakh_with_surcharge() {
    // 3,00,000 up to 24 lakh + 30% of 36 lakh = 13,80,000; surcharge 10% 1,38,000; cess 4%.
    assert_eq!(total("new", json!({ "other_income": 6_000_000 })), 1_578_720.0);
}

#[test]
fn new_regime_surcharge_marginal_relief() {
    // Taxable 50,10,000: tax 10,83,000; tax + surcharge capped at 10,80,000 + 10,000; cess 43,600.
    assert_eq!(total("new", json!({ "other_income": 5_010_000 })), 1_133_600.0);
}

#[test]
fn old_regime_10_lakh_salary_with_investments() {
    // 10,00,000 − 50,000 − 1,50,000 = 8,00,000: 12,500 + 60,000 = 72,500; cess 2,900.
    assert_eq!(total("old", json!({ "salary": 1_000_000, "deductions": { "investments": 150_000 } })), 75_400.0);
}

#[test]
fn old_regime_rebate_and_caps() {
    // Claims above a cap are limited to it: 2 lakh of investments counts as 1.5 lakh.
    assert_eq!(total("old", json!({ "salary": 1_000_000, "deductions": { "investments": 200_000 } })), 75_400.0);
    // Taxable 5,00,000 → 12,500 rebate → nothing to pay.
    assert_eq!(total("old", json!({ "other_income": 500_000 })), 0.0);
}

#[test]
fn old_regime_senior_slabs() {
    // Age 65, taxable 6,00,000: 5% of 2 lakh + 20% of 1 lakh = 30,000; cess 1,200.
    assert_eq!(total("old", json!({ "age": 65, "other_income": 600_000 })), 31_200.0);
    // Age 82: nil up to 5 lakh, then 20%.
    assert_eq!(total("old", json!({ "age": 82, "other_income": 600_000 })), 20_800.0);
}

#[test]
fn senior_and_alternative_caps() {
    let r = tax::compare("IN", None, &json!({ "age": 64, "salary": 900_000,
        "deductions": { "health_self": 60_000, "health_parents": 60_000 }, "alt": ["health_parents"] })).unwrap();
    let old = r["estimates"].as_array().unwrap().iter().find(|e| e["regime"] == "old").unwrap();
    let allowed = |id: &str| old["deductions"].as_array().unwrap().iter().find(|d| d["id"] == id).unwrap()["allowed"].as_f64().unwrap();
    assert_eq!(allowed("health_self"), 50_000.0);
    assert_eq!(allowed("health_parents"), 50_000.0);
}

#[test]
fn recommends_the_cheaper_regime() {
    let r = tax::compare("IN", None, &json!({ "salary": 1_375_000 })).unwrap();
    assert_eq!(r["recommended"], "new");
    // Old regime with heavy deductions: 25 lakh salary, every major deduction claimed.
    let r = tax::compare("IN", None, &json!({ "salary": 2_500_000, "deductions": {
        "hra": 600_000, "investments": 150_000, "nps_own": 50_000, "health_self": 25_000, "health_parents": 50_000, "home_loan_interest": 200_000 },
        "alt": ["health_parents"] })).unwrap();
    assert_eq!(r["recommended"], "old");
    assert!(r["saving"].as_f64().unwrap() > 0.0);
}

#[test]
fn rejects_bad_input() {
    assert!(tax::compare("IN", None, &json!({ "salary": -5 })).is_err());
    assert!(tax::compare("IN", None, &json!({ "salary": "12.345" })).is_err());
    assert!(tax::compare("ZZ", None, &json!({})).is_err());
}
