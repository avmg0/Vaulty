//! Form 16 (India): the certificate an employer gives for tax deducted from salary. Part A (made
//! by TRACES) has the employer, TAN, assessment year and TDS for each quarter; Part B (filled in
//! by the employer) has salary, exemptions, deductions and the regime. Read from the PDF's text,
//! by the section each line refers to (17(1), 10(13A), 80C …), not by position, since employers
//! lay Part B out in many ways. Nothing from the file is kept: the figures are offered to the
//! tax planner and the person picks which to use.
use crate::{error::{Error, Result}, money, pdf_text, tax};
use chrono::NaiveDate;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::LazyLock;

static AMOUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[\s(])(-?\d[\d,]*\.\d{1,2})(?:$|[\s)])").unwrap());
static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:\d{1,2}\.|\([a-z]{1,2}\)|\(?[ivx]{1,4}\))\s").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(20\d\d)\s*-\s*(\d\d)\b").unwrap());
static PAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Z]{5}\d{4}[A-Z]\b").unwrap());
static TAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Z]{4}\d{5}[A-Z]\b").unwrap());
static QUARTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Q([1-4])\b").unwrap());

/// One line: its text and its cells with where they start.
struct Row { text: String, lower: String, cells: Vec<(f64, String)> }

/// Amounts on a line, in paise.
fn amounts(s: &str) -> Vec<i64> {
    // Overlapping separators (a space between two amounts) need the scan to restart at each match.
    let mut out = vec![];
    let mut at = 0;
    while let Some(c) = AMOUNT.captures(&s[at..]) {
        let m = c.get(1).unwrap();
        if let Ok(v) = money::parse_minor(&json!(m.as_str().replace(',', "")), "INR") { out.push(v); }
        at += m.end();
    }
    out
}

/// The amounts for the first line matching `pattern`: on that line, or on the lines just after
/// it while its label wraps (they don't start a new numbered item).
fn find(rows: &[Row], pattern: &str) -> Option<Vec<i64>> {
    let re = Regex::new(pattern).ok()?;
    let i = rows.iter().position(|r| re.is_match(&r.lower))?;
    let a = amounts(&rows[i].text);
    if !a.is_empty() { return Some(a); }
    // The section can be on a label's second line, with the amounts on its first.
    if !ITEM.is_match(&rows[i].lower) {
        for r in rows[i.saturating_sub(2)..i].iter().rev() {
            let a = amounts(&r.text);
            if !a.is_empty() { return Some(a); }
            if ITEM.is_match(&r.lower) { break; }
        }
    }
    for r in rows.iter().skip(i + 1).take(2) {
        if ITEM.is_match(&r.lower) { break; }
        let a = amounts(&r.text);
        if !a.is_empty() { return Some(a); }
    }
    None
}
/// The last amount (the deductible one where there are two).
fn last(rows: &[Row], pattern: &str) -> Option<i64> { find(rows, pattern).and_then(|a| a.last().copied()) }

/// The text under a heading cell: the cell on the next line starting nearest it.
fn under(rows: &[Row], heading: &str) -> Option<String> {
    let (i, x) = rows.iter().enumerate().find_map(|(i, r)| r.cells.iter().find(|(_, t)| t.to_lowercase().contains(heading)).map(|(x, _)| (i, *x)))?;
    let next = rows.get(i + 1)?;
    next.cells.iter().min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs())).filter(|(cx, _)| (cx - x).abs() < 60.0).map(|(_, t)| t.trim().to_string())
}

fn rows_of(pages: Vec<Vec<pdf_text::Line>>) -> Vec<Row> {
    pages.into_iter().flatten().map(|l| {
        let text = l.text();
        Row { lower: text.to_lowercase(), cells: l.cells.into_iter().map(|c| (c.x, c.text)).collect(), text }
    }).collect()
}

/// What the form says. Amounts in paise; `None` where the form doesn't have the line.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Form16 {
    pub employer: Option<String>,
    pub tan: Option<String>,
    pub employee: Option<String>,
    pub pan: Option<String>,
    /// "2027-28".
    pub assessment_year: Option<String>,
    /// (quarter 1-4, amount paid, tax deducted).
    pub quarters: Vec<(u8, i64, i64)>,
    pub salary: Option<i64>,
    pub hra: Option<i64>,
    pub exempt_total: Option<i64>,
    pub standard_deduction: Option<i64>,
    pub professional_tax: Option<i64>,
    pub house_property: Option<i64>,
    pub other_sources: Option<i64>,
    /// Chapter VI-A, deductible amounts, by the planner's deduction ids.
    pub deductions: Vec<(&'static str, i64)>,
    pub taxable: Option<i64>,
    pub tax: Option<i64>,
    /// "old" or "new", from the 115BAC question.
    pub regime: Option<&'static str>,
}

fn parse_rows(rows: &[Row]) -> Form16 {
    let mut f = Form16 { employer: under(rows, "name and address of the employer"), employee: under(rows, "name and address of the employee"), ..Default::default() };
    f.tan = under(rows, "tan of the deductor").filter(|t| TAN.is_match(t)).or_else(|| rows.iter().find_map(|r| TAN.find(&r.text).map(|m| m.as_str().to_string())));
    f.pan = under(rows, "pan of the employee").and_then(|t| PAN.find(&t).map(|m| m.as_str().to_string()));
    f.assessment_year = rows.iter().position(|r| r.lower.contains("assessment year"))
        .and_then(|i| rows[i..rows.len().min(i + 3)].iter().find_map(|r| YEAR.captures(&r.text)))
        .map(|c| format!("{}-{}", &c[1], &c[2]));
    for r in rows {
        let Some(q) = r.cells.first().and_then(|(_, t)| QUARTER.captures(t.trim())) else { continue };
        let a = amounts(&r.text);
        if a.len() >= 2 && !f.quarters.iter().any(|x| x.0 == q[1].parse::<u8>().unwrap_or(0)) {
            f.quarters.push((q[1].parse().unwrap_or(0), a[0], a[1]));
        }
    }
    // Salary: 17(1) + 17(2) + 17(3), or the total if the parts aren't there.
    let parts: Vec<i64> = [r"section\s*17\s*\(1\)", r"section\s*17\s*\(2\)", r"section\s*17\s*\(3\)"].iter().filter_map(|p| last(rows, p)).collect();
    f.salary = if parts.is_empty() { last(rows, r"^\(d\)\s*total\s*[\d,.]*$") } else { Some(parts.iter().sum()) };
    f.hra = last(rows, r"10\s*\(13a\)");
    f.exempt_total = last(rows, r"total amount of exemption");
    f.standard_deduction = last(rows, r"16\s*\(ia\)");
    f.professional_tax = last(rows, r"16\s*\(iii\)");
    f.house_property = last(rows, r"house property");
    f.other_sources = last(rows, r"other sources");
    let c80 = last(rows, r"total deduction under section 80c").or_else(|| last(rows, r"section 80c\b"));
    for (id, v) in [
        ("investments", c80),
        ("nps_own", last(rows, r"80ccd\s*\(1b\)")),
        ("employer_nps", last(rows, r"80ccd\s*\(2\)")),
        ("health_self", last(rows, r"section 80d\b")),
        ("education_loan", last(rows, r"section 80e\b")),
        ("donations", last(rows, r"section 80g\b")),
        ("savings_interest", last(rows, r"80tt[ab]\b")),
    ] { if let Some(v) = v.filter(|v| *v > 0) { f.deductions.push((id, v)); } }
    f.taxable = last(rows, r"total taxable income");
    f.tax = last(rows, r"net tax payable").or_else(|| last(rows, r"tax payable"));
    // "Whether opting out of taxation u/s 115BAC(1A)? Yes" means the old regime; older forms
    // asked "opting for taxation u/s 115BAC", where Yes means the new one.
    if let Some(i) = rows.iter().position(|r| r.lower.contains("115bac")) {
        let text = rows[i..rows.len().min(i + 2)].iter().map(|r| r.lower.as_str()).collect::<Vec<_>>().join(" ");
        let after = text.split("115bac").nth(1).unwrap_or_default();
        let yes = Regex::new(r"\byes\b").unwrap().is_match(after);
        let no = Regex::new(r"\bno\b").unwrap().is_match(after);
        if yes != no { f.regime = Some(if text.contains("opting out") == yes { "old" } else { "new" }); }
    }
    f
}

/// Reads a Form 16 PDF (password-protected ones too).
pub fn read_pdf(bytes: &[u8], password: Option<&str>) -> Result<Form16> {
    let doc = pdf_text::open(bytes, password)?;
    let f = parse_rows(&rows_of(pdf_text::positioned(&doc)?));
    if f.quarters.is_empty() && f.salary.is_none() { return Err(Error::Invalid("this doesn't look like a Form 16".into())); }
    Ok(f)
}

/// The tax year a Form 16 is for: assessment year 2027-28 is the financial year 2026-27.
pub fn year_id(f: &Form16) -> Option<String> {
    let ay: i32 = f.assessment_year.as_deref()?.get(..4)?.parse().ok()?;
    Some(format!("FY{}-{:02}", ay - 1, ay % 100))
}

/// The form as the planner offers it: the year, figures by planner field, TDS by quarter as
/// payments (dated at each quarter's end), and the lines that don't go anywhere.
pub fn offer(f: &Form16) -> Result<Value> {
    let d = |m: i64| money::to_decimal(m, "INR");
    let year = year_id(f);
    let (start, ids) = match year.as_deref().map(|y| tax::pack("IN").and_then(|p| tax::year_of(&p, Some(y)).cloned())) {
        Some(Ok(y)) => {
            let ids: Vec<String> = y.regimes.iter().flat_map(|r| r.deductions.iter().map(|d| d.id.clone())).collect();
            (NaiveDate::parse_from_str(&y.start, "%Y-%m-%d").ok(), ids)
        }
        _ => (None, vec![]),
    };
    let known = year.is_some() && start.is_some();
    let mut fields = vec![];
    if let Some(s) = f.salary { fields.push(json!({ "field": "salary", "amount": d(s) })); }
    if let Some(o) = f.other_sources.filter(|o| *o > 0) { fields.push(json!({ "field": "other_income", "amount": d(o) })); }
    let mut ded: Vec<(&str, i64)> = f.deductions.clone();
    if let Some(h) = f.hra.filter(|h| *h > 0) { ded.insert(0, ("hra", h)); }
    // A loss from house property is home-loan interest on the home the person lives in.
    if let Some(h) = f.house_property.filter(|h| *h < 0) { ded.push(("home_loan_interest", -h)); }
    for (id, v) in ded {
        if ids.iter().any(|i| i == id) { fields.push(json!({ "field": "deduction", "id": id, "amount": d(v) })); }
    }
    let payments: Vec<Value> = match start {
        Some(s) => {
            let y = s.format("%Y").to_string().parse::<i32>().unwrap_or(2000);
            f.quarters.iter().filter(|q| q.2 > 0).filter_map(|&(q, _, tds)| {
                let date = match q { 1 => NaiveDate::from_ymd_opt(y, 6, 30), 2 => NaiveDate::from_ymd_opt(y, 9, 30), 3 => NaiveDate::from_ymd_opt(y, 12, 31), _ => NaiveDate::from_ymd_opt(y + 1, 3, 31) }?;
                Some(json!({ "quarter": q, "date": date.to_string(), "amount": d(tds) }))
            }).collect()
        }
        None => vec![],
    };
    Ok(json!({
        "year": if known { year.clone() } else { None }, "assessment_year": f.assessment_year,
        "employer": f.employer, "tan": f.tan, "employee": f.employee, "pan": f.pan,
        "tds": d(f.quarters.iter().map(|q| q.2).sum()), "paid": d(f.quarters.iter().map(|q| q.1).sum()),
        "fields": fields, "payments": payments, "regime": f.regime,
        "standard_deduction": f.standard_deduction.map(d), "professional_tax": f.professional_tax.filter(|p| *p > 0).map(d),
        "taxable": f.taxable.map(d), "tax": f.tax.map(d),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amounts_in_a_line() {
        assert_eq!(amounts("(a) under section 80C  1,65,000.00  1,50,000.00"), vec![16_500_000, 15_000_000]);
        assert_eq!(amounts("(a) Income from house property -2,00,000.00"), vec![-20_000_000]);
        assert_eq!(amounts("section 17(1) 1452000.00"), vec![145_200_000]);
        assert!(amounts("2027-28 01-Apr-2026").is_empty());
    }
}
