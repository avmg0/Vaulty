//! Money calculators: SIP (with a yearly step-up), lump sum, loan EMI (with extra payments),
//! fixed and recurring deposits, PPF, retirement, inflation and "how much a month for a goal".
//! Plain compound-interest maths in f64, rounded to whole units at the end; results are
//! estimates and are labelled so. Conventions follow the common Indian calculators: SIP
//! instalments at the start of each month, deposits compounded quarterly, PPF deposits at the
//! start of each year.
use crate::error::{Error, Result};
use serde_json::{json, Value};

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

/// A number from the input: a JSON number or a decimal string; missing is `default`.
fn num(v: &Value, key: &str, default: f64) -> Result<f64> {
    let n = match &v[key] {
        Value::Null => return Ok(default),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) if s.trim().is_empty() => return Ok(default),
        Value::String(s) => s.trim().replace(',', "").parse::<f64>().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    if !n.is_finite() { return Err(bad("give each figure as a number")); }
    Ok(n)
}
fn amount(v: &Value, key: &str) -> Result<f64> {
    let n = num(v, key, 0.0)?;
    if !(0.0..=1e13).contains(&n) { return Err(bad("amounts must be between 0 and 10 trillion")); }
    Ok(n)
}
fn rate(v: &Value, key: &str, default: f64) -> Result<f64> {
    let n = num(v, key, default)?;
    if !(0.0..=50.0).contains(&n) { return Err(bad("rates must be between 0% and 50%")); }
    Ok(n)
}
fn years(v: &Value, key: &str, default: f64, max: f64) -> Result<f64> {
    let n = num(v, key, default)?;
    if !(0.0..=max).contains(&n) { return Err(bad("that period is too long")); }
    Ok(n)
}
fn r2(x: f64) -> f64 { (x * 100.0).round() / 100.0 }
fn r0(x: f64) -> f64 { x.round() }

/// SIP: `monthly` for `years` at `rate`% a year, raised by `step_up`% each year.
pub fn sip(v: &Value) -> Result<Value> {
    let (pmt, yrs, r, step) = (amount(v, "monthly")?, years(v, "years", 10.0, 60.0)?, rate(v, "rate", 12.0)?, rate(v, "step_up", 0.0)?);
    let i = r / 1200.0;
    let months = (yrs * 12.0).round() as i64;
    let (mut value, mut invested, mut p) = (0.0, 0.0, pmt);
    let mut series = vec![];
    for m in 0..months {
        if m > 0 && m % 12 == 0 { p *= 1.0 + step / 100.0; }
        value = (value + p) * (1.0 + i);
        invested += p;
        if (m + 1) % 12 == 0 || m + 1 == months { series.push(json!({ "year": (m + 12) / 12, "invested": r0(invested), "value": r0(value) })); }
    }
    Ok(json!({ "invested": r0(invested), "value": r0(value), "gains": r0(value - invested), "series": series }))
}

/// Lump sum: `amount` left for `years` at `rate`% a year, compounded yearly.
pub fn lumpsum(v: &Value) -> Result<Value> {
    let (a, yrs, r) = (amount(v, "amount")?, years(v, "years", 10.0, 60.0)?, rate(v, "rate", 12.0)?);
    let series: Vec<Value> = (1..=yrs.ceil() as i64).map(|y| json!({ "year": y, "invested": r0(a), "value": r0(a * (1.0 + r / 100.0).powf((y as f64).min(yrs))) })).collect();
    let value = a * (1.0 + r / 100.0).powf(yrs);
    Ok(json!({ "invested": r0(a), "value": r0(value), "gains": r0(value - a), "series": series }))
}

/// The monthly instalment for `principal` over `n` months at monthly rate `i`.
fn emi_of(principal: f64, i: f64, n: f64) -> f64 {
    if n <= 0.0 { return principal; }
    if i == 0.0 { principal / n } else { principal * i * (1.0 + i).powf(n) / ((1.0 + i).powf(n) - 1.0) }
}

/// Loan: EMI, total interest, a year-by-year schedule, and the effect of paying `extra` each month.
pub fn emi(v: &Value) -> Result<Value> {
    let (principal, r, yrs, extra) = (amount(v, "principal")?, rate(v, "rate", 9.0)?, years(v, "years", 20.0, 40.0)?, amount(v, "extra")?);
    let n = (yrs * 12.0).round();
    if n < 1.0 { return Err(bad("give the loan's length")); }
    let i = r / 1200.0;
    let e = emi_of(principal, i, n);
    // Month by month, so extra payments shorten the loan.
    let run = |extra: f64| {
        let (mut bal, mut interest, mut months) = (principal, 0.0, 0);
        let mut rows = vec![];
        let (mut yp, mut yi) = (0.0, 0.0);
        while bal > 0.005 && months < 1200 {
            let int = bal * i;
            let pay = (e + extra).min(bal + int);
            bal -= pay - int;
            interest += int;
            yp += pay - int; yi += int;
            months += 1;
            if months % 12 == 0 || bal <= 0.005 {
                rows.push(json!({ "year": (months + 11) / 12, "principal": r0(yp), "interest": r0(yi), "balance": r0(bal.max(0.0)) }));
                yp = 0.0; yi = 0.0;
            }
        }
        (interest, months, rows)
    };
    let (interest, months, schedule) = run(0.0);
    let mut out = json!({ "emi": r2(e), "interest": r0(interest), "total": r0(principal + interest), "months": months, "schedule": schedule });
    if extra > 0.0 {
        let (i2, m2, _) = run(extra);
        out["with_extra"] = json!({ "months": m2, "interest": r0(i2), "months_saved": months - m2, "interest_saved": r0(interest - i2) });
    }
    Ok(out)
}

/// Fixed deposit: `principal` for `months` at `rate`%, compounded quarterly (or `compounding`:
/// monthly, quarterly, half_yearly, yearly, simple).
pub fn fd(v: &Value) -> Result<Value> {
    let (p, r, months) = (amount(v, "principal")?, rate(v, "rate", 7.0)?, years(v, "months", 12.0, 600.0)?);
    let per_year = match v["compounding"].as_str().unwrap_or("quarterly") {
        "monthly" => 12.0, "quarterly" => 4.0, "half_yearly" => 2.0, "yearly" => 1.0, "simple" => 0.0,
        _ => return Err(bad("unknown compounding")),
    };
    let t = months / 12.0;
    let value = if per_year == 0.0 { p * (1.0 + r / 100.0 * t) } else { p * (1.0 + r / 100.0 / per_year).powf(per_year * t) };
    Ok(json!({ "invested": r0(p), "value": r0(value), "gains": r0(value - p) }))
}

/// Recurring deposit: `monthly` for `months` at `rate`%, compounded quarterly (the bank formula:
/// each instalment grows for the months it's in).
pub fn rd(v: &Value) -> Result<Value> {
    let (p, r, months) = (amount(v, "monthly")?, rate(v, "rate", 7.0)?, years(v, "months", 12.0, 600.0)?.round());
    let q = r / 400.0;
    let value: f64 = (1..=months as i64).map(|k| p * (1.0 + q).powf(k as f64 / 3.0)).sum();
    Ok(json!({ "invested": r0(p * months), "value": r0(value), "gains": r0(value - p * months) }))
}

/// PPF: `yearly` deposited at the start of each year for `years` (15 or more) at `rate`%,
/// compounded yearly. Deposits are capped at ₹1.5 lakh a year.
pub fn ppf(v: &Value) -> Result<Value> {
    let (p, r, yrs) = (amount(v, "yearly")?, rate(v, "rate", 7.1)?, years(v, "years", 15.0, 50.0)?.round());
    if p > 150_000.0 { return Err(bad("PPF takes at most ₹1,50,000 a year")); }
    if yrs < 15.0 { return Err(bad("a PPF account runs for at least 15 years")); }
    let mut value = 0.0;
    let mut series = vec![];
    for y in 1..=yrs as i64 {
        value = (value + p) * (1.0 + r / 100.0);
        series.push(json!({ "year": y, "invested": r0(p * y as f64), "value": r0(value) }));
    }
    Ok(json!({ "invested": r0(p * yrs), "value": r0(value), "gains": r0(value - p * yrs), "series": series }))
}

/// What `amount` today will cost after `years` of `inflation`%.
pub fn inflation(v: &Value) -> Result<Value> {
    let (a, inf, yrs) = (amount(v, "amount")?, rate(v, "inflation", 6.0)?, years(v, "years", 10.0, 80.0)?);
    let future = a * (1.0 + inf / 100.0).powf(yrs);
    Ok(json!({ "future": r0(future), "today_worth": r0(a / (1.0 + inf / 100.0).powf(yrs)) }))
}

/// Monthly saving needed to reach `target` in `months`, starting from `saved`, growing at `rate`%
/// a year (contributions at the end of each month). Zero when `saved` alone gets there.
pub fn monthly_needed(target: f64, saved: f64, months: f64, rate_pct: f64) -> f64 {
    let i = rate_pct / 1200.0;
    let grown = saved * (1.0 + i).powf(months);
    let gap = target - grown;
    if gap <= 0.0 { return 0.0; }
    if months <= 0.0 { return gap; }
    if i == 0.0 { gap / months } else { gap * i / ((1.0 + i).powf(months) - 1.0) }
}

/// What `saved` plus `monthly` (end of each month) grows to in `months` at `rate`% a year.
pub fn projected(saved: f64, monthly: f64, months: f64, rate_pct: f64) -> f64 {
    let i = rate_pct / 1200.0;
    if i == 0.0 { saved + monthly * months } else { saved * (1.0 + i).powf(months) + monthly * ((1.0 + i).powf(months) - 1.0) / i }
}

/// Goal: the monthly saving needed for `target` (in today's money, grown by `inflation`) in `years`.
pub fn goal(v: &Value) -> Result<Value> {
    let (t, saved, yrs, r, inf) = (amount(v, "target")?, amount(v, "saved")?, years(v, "years", 5.0, 60.0)?, rate(v, "rate", 10.0)?, rate(v, "inflation", 0.0)?);
    let future = t * (1.0 + inf / 100.0).powf(yrs);
    let m = monthly_needed(future, saved, yrs * 12.0, r);
    Ok(json!({ "future_target": r0(future), "monthly": r0(m), "invested": r0(saved + m * yrs * 12.0) }))
}

/// Retirement: the corpus needed at `retire_age` to pay today's `monthly_expenses` (grown by
/// `inflation`) until `life_age`, earning `rate_after`% meanwhile; and the monthly saving needed
/// to build it from `saved` at `rate_before`%.
pub fn retirement(v: &Value) -> Result<Value> {
    let age = years(v, "age", 30.0, 100.0)?;
    let retire = years(v, "retire_age", 60.0, 100.0)?;
    let life = years(v, "life_age", 85.0, 120.0)?;
    if retire <= age { return Err(bad("retirement age must be after your age")); }
    if life <= retire { return Err(bad("plan for life after retirement")); }
    let (exp, saved) = (amount(v, "monthly_expenses")?, amount(v, "saved")?);
    let (inf, before, after) = (rate(v, "inflation", 6.0)?, rate(v, "rate_before", 11.0)?, rate(v, "rate_after", 7.0)?);
    let to_go = retire - age;
    let first_year = exp * 12.0 * (1.0 + inf / 100.0).powf(to_go);
    // Yearly withdrawals at the start of each year, rising with inflation: present value of a
    // growing annuity due at the post-retirement return.
    let n = life - retire;
    let (g, r) = (inf / 100.0, after / 100.0);
    let corpus = if (r - g).abs() < 1e-9 { first_year * n } else { first_year * (1.0 - ((1.0 + g) / (1.0 + r)).powf(n)) / (r - g) * (1.0 + r) };
    let m = monthly_needed(corpus, saved, to_go * 12.0, before);
    Ok(json!({ "monthly_expenses_then": r0(first_year / 12.0), "corpus": r0(corpus), "monthly": r0(m), "years_to_go": to_go, "years_after": n }))
}

/// Runs a calculator by name.
pub fn run(kind: &str, input: &Value) -> Result<Value> {
    match kind {
        "sip" => sip(input), "lumpsum" => lumpsum(input), "emi" => emi(input), "fd" => fd(input), "rd" => rd(input),
        "ppf" => ppf(input), "inflation" => inflation(input), "goal" => goal(input), "retirement" => retirement(input),
        _ => Err(bad("unknown calculator")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(v: &Value, k: &str) -> f64 { v[k].as_f64().unwrap() }

    #[test]
    fn sip_matches_the_usual_calculators() {
        // ₹10,000 a month for 10 years at 12%: ₹23.23 lakh (start-of-month instalments).
        let v = sip(&json!({ "monthly": 10000, "years": 10, "rate": 12 })).unwrap();
        assert_eq!(f(&v, "invested"), 1_200_000.0);
        assert!((f(&v, "value") - 2_323_391.0).abs() <= 1.0, "{}", v["value"]);
        assert_eq!(v["series"].as_array().unwrap().len(), 10);
        // A 10% step-up puts in more and ends higher.
        let s = sip(&json!({ "monthly": 10000, "years": 10, "rate": 12, "step_up": 10 })).unwrap();
        assert!((f(&s, "invested") - 1_912_491.0).abs() <= 1.0, "{}", s["invested"]);
        assert!(f(&s, "value") > f(&v, "value"));
    }

    #[test]
    fn emi_and_extra_payments() {
        // ₹10 lakh over 20 years at 9%: ₹8,997.26 a month.
        let v = emi(&json!({ "principal": "1000000", "rate": 9, "years": 20 })).unwrap();
        assert_eq!(f(&v, "emi"), 8997.26);
        assert_eq!(v["months"], 240);
        assert!((f(&v, "interest") - 1_159_342.0).abs() <= 2.0, "{}", v["interest"]);
        assert_eq!(v["schedule"].as_array().unwrap().last().unwrap()["balance"], 0.0);
        let x = emi(&json!({ "principal": 1000000, "rate": 9, "years": 20, "extra": 2000 })).unwrap();
        assert!(x["with_extra"]["months_saved"].as_i64().unwrap() > 60);
        assert!(f(&x["with_extra"], "interest_saved") > 300_000.0);
    }

    #[test]
    fn deposits() {
        // ₹1 lakh for 5 years at 7%, quarterly: ₹1,41,478.
        assert_eq!(f(&fd(&json!({ "principal": 100000, "rate": 7, "months": 60 })).unwrap(), "value"), 141_478.0);
        assert_eq!(f(&fd(&json!({ "principal": 100000, "rate": 7, "months": 12, "compounding": "simple" })).unwrap(), "value"), 107_000.0);
        // ₹5,000 a month for a year at 7%: about ₹62,330.
        let r = rd(&json!({ "monthly": 5000, "rate": 7, "months": 12 })).unwrap();
        assert!((f(&r, "value") - 62_330.0).abs() < 50.0, "{}", r["value"]);
        // PPF: ₹1.5 lakh a year for 15 years at 7.1%: ₹40.68 lakh.
        let p = ppf(&json!({ "yearly": 150000, "rate": 7.1, "years": 15 })).unwrap();
        assert!((f(&p, "value") - 4_068_209.0).abs() <= 1.0, "{}", p["value"]);
        assert!(ppf(&json!({ "yearly": 200000 })).is_err());
    }

    #[test]
    fn goals_and_retirement() {
        assert_eq!(monthly_needed(120_000.0, 0.0, 12.0, 0.0), 10_000.0);
        assert_eq!(monthly_needed(100.0, 200.0, 12.0, 5.0), 0.0, "already there");
        let m = monthly_needed(1_000_000.0, 100_000.0, 60.0, 10.0);
        assert!((projected(100_000.0, m, 60.0, 10.0) - 1_000_000.0).abs() < 0.01, "the two agree");
        let g = goal(&json!({ "target": 1000000, "years": 5, "rate": 0, "inflation": 6 })).unwrap();
        assert_eq!(f(&g, "future_target"), 1_338_226.0);
        let r = retirement(&json!({ "age": 30, "retire_age": 60, "life_age": 85, "monthly_expenses": 50000, "inflation": 6, "rate_before": 11, "rate_after": 7 })).unwrap();
        assert!((f(&r, "monthly_expenses_then") - 287_175.0).abs() < 2.0, "{}", r["monthly_expenses_then"]);
        assert!(f(&r, "corpus") > 7.0e7 && f(&r, "corpus") < 8.0e7, "{}", r["corpus"]);
        assert!(retirement(&json!({ "age": 60, "retire_age": 55 })).is_err());
    }

    #[test]
    fn bad_input() {
        assert!(sip(&json!({ "monthly": "abc" })).is_err());
        assert!(sip(&json!({ "monthly": -5 })).is_err());
        assert!(emi(&json!({ "principal": 100, "rate": 80 })).is_err());
        assert!(run("nope", &json!({})).is_err());
    }
}
