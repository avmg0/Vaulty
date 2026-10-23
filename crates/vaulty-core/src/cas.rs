//! Reads the Consolidated Account Statement (CAS) that CAMS and KFintech send for mutual funds:
//! every folio, scheme, transaction and closing balance, from the PDF's text.
//!
//! The statement is laid out for printing, so this works line by line on the text rebuilt by
//! `pdf_text`: an AMC heading ("… Mutual Fund"), then per folio a "Folio No:" line, the scheme's
//! name with its ISIN and registrar, the opening balance, dated transaction rows (amount, units,
//! NAV, unit balance; brackets for negatives), and a closing line with units, NAV, cost and value.
//! Numbers are read exactly (no floating point): money in paise, units and NAV to 4 decimals.
use crate::{error::{Error, Result}, pdf_text};
use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

/// Units and NAVs are kept multiplied by this (four decimal places).
pub const E4: i64 = 10_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CasTxn {
    pub date: NaiveDate,
    pub kind: &'static str,
    pub description: String,
    /// Paise; negative for money paid out (redemptions, switches out) as the statement prints it.
    pub amount_minor: Option<i64>,
    pub units_e4: Option<i64>,
    pub nav_e4: Option<i64>,
    pub balance_e4: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CasScheme {
    pub amc: String,
    pub folio: String,
    pub name: String,
    pub isin: Option<String>,
    pub registrar: Option<String>,
    pub open_units_e4: i64,
    pub close_units_e4: Option<i64>,
    pub nav_e4: Option<i64>,
    pub nav_date: Option<NaiveDate>,
    pub cost_minor: Option<i64>,
    pub value_minor: Option<i64>,
    pub txns: Vec<CasTxn>,
    /// Whether opening units plus every transaction's units come to the closing units.
    pub balanced: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cas {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub investor: Option<String>,
    pub pan: Option<String>,
    pub schemes: Vec<CasScheme>,
}

struct Patterns {
    period: Regex, amc: Regex, folio: Regex, pan: Regex, isin: Regex, registrar: Regex, code: Regex,
    open: Regex, close: Regex, nav: Regex, cost: Regex, value: Regex, txn: Regex, number: Regex, noise: Regex,
}

fn re() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let r = |s: &str| Regex::new(s).expect("pattern");
        const D: &str = r"\d{1,2}-[A-Za-z]{3}-\d{4}";
        const N: &str = r"\(?-?[\d,]*\.?\d+\)?";
        Patterns {
            period: r(&format!(r"({D})\s+(?i:to)\s+({D})")),
            amc: r(r"^([A-Za-z][A-Za-z0-9&.'() -]*Mutual Fund)$"),
            folio: r(r"(?i)Folio\s*No\s*:?\s*([0-9A-Za-z]+(?:\s*/\s*[0-9A-Za-z]+)?)"),
            pan: r(r"PAN\s*:\s*([A-Z]{5}\d{4}[A-Z])"),
            isin: r(r"ISIN\s*:?\s*([A-Z]{2}[A-Z0-9]{9}\d)"),
            registrar: r(r"(?i)Registrar\s*:\s*([A-Za-z]+)"),
            code: r(r"^[A-Z0-9]{2,12}-\s*"),
            open: r(&format!(r"(?i)Opening Unit Balance\s*:?\s*({N})")),
            close: r(&format!(r"(?i)Closing Unit Balance\s*:?\s*({N})")),
            nav: r(&format!(r"(?i)NAV on ({D})\s*:\s*(?:INR|Rs\.?|₹)?\s*({N})")),
            cost: r(&format!(r"(?i)(?:Total\s+)?Cost Value\s*:?\s*(?:INR|Rs\.?|₹)?\s*({N})")),
            value: r(&format!(r"(?i)(?:Market\s+)?Value on {D}\s*:\s*(?:INR|Rs\.?|₹)?\s*({N})")),
            txn: r(&format!(r"^({D})\s+(.*)$")),
            number: r(r"^\(?-?[\d,]*\.\d+\)?$"),
            noise: r(r"(?i)^(consolidated account statement|date\s+transaction|page \d+|\d{1,2}-[a-z]{3}-\d{4}\s+to\s+\d{1,2}-[a-z]{3}-\d{4}$)"),
        }
    })
}

/// A printed number as an exact integer with `places` decimals: "(1,234.5)" → -1234500 for 3.
/// Accepts Indian (1,23,456.78) and Western (123,456.78) grouping.
pub fn scaled(s: &str, places: u32) -> Option<i64> {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(inner) => (true, inner),
        None => match s.strip_prefix('-') { Some(x) => (true, x), None => (false, s) },
    };
    let s: String = s.chars().filter(|&c| c != ',').collect();
    let (whole, frac) = s.split_once('.').unwrap_or((&s, ""));
    if whole.is_empty() && frac.is_empty() { return None; }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) { return None; }
    let mut v: i128 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
    let digits: Vec<u32> = frac.chars().filter_map(|c| c.to_digit(10)).collect();
    for i in 0..places as usize { v = v * 10 + digits.get(i).copied().unwrap_or(0) as i128; }
    // Round half away from zero on the first dropped digit.
    if digits.get(places as usize).is_some_and(|&d| d >= 5) { v += 1; }
    let v = i64::try_from(v).ok()?;
    Some(if neg { -v } else { v })
}

fn date(s: &str) -> Option<NaiveDate> { NaiveDate::parse_from_str(s, "%d-%b-%Y").ok() }

/// What a transaction is, from its wording (and, failing that, which way the units went).
pub fn classify(description: &str, units: Option<i64>) -> &'static str {
    let d = description.to_lowercase();
    let word = |w: &str| d.split(|c: char| !c.is_ascii_alphanumeric()).any(|x| x == w);
    if d.contains("stamp duty") { "stamp_duty" }
    else if word("stt") { "stt" }
    else if word("tds") { "tds" }
    else if d.contains("reversal") || d.contains("rejection") || d.contains("rejected") { "other" }
    else if d.contains("switch in") || d.contains("switch-in") || d.contains("switchin") { "switch_in" }
    else if d.contains("switch out") || d.contains("switch-out") || d.contains("switchout") { "switch_out" }
    else if d.contains("reinvest") { "dividend_reinvest" }
    else if word("idcw") || d.contains("dividend") { "dividend_payout" }
    else if d.contains("redemption") || d.contains("redeem") || word("swp") { "redemption" }
    else if word("sip") || d.contains("systematic") { "sip" }
    else if d.contains("purchase") || d.contains("investment") || word("stp") { "purchase" }
    else {
        match units { Some(u) if u > 0 => "purchase", Some(u) if u < 0 => "redemption", _ => "other" }
    }
}

fn txn_row(date_s: &str, rest: &str) -> Option<CasTxn> {
    let p = re();
    let date = date(date_s)?;
    let mut words: Vec<&str> = rest.split_whitespace().collect();
    let mut nums = vec![];
    while nums.len() < 4 && words.last().is_some_and(|w| p.number.is_match(w)) {
        nums.push(words.pop().unwrap_or_default());
    }
    nums.reverse();
    let description = words.join(" ").trim_matches('*').trim().to_string();
    let money = |s: &str| scaled(s, 2);
    let e4 = |s: &str| scaled(s, 4);
    let (amount, units, nav, balance) = match nums.as_slice() {
        [a, u, n, b] => (money(a), e4(u), e4(n), e4(b)),
        [a, u, n] => (money(a), e4(u), e4(n), None),
        [a, u] => (money(a), e4(u), None, None),
        [a] => (money(a), None, None, None),
        _ => (None, None, None, None),
    };
    Some(CasTxn { date, kind: classify(&description, units), description, amount_minor: amount, units_e4: units, nav_e4: nav, balance_e4: balance })
}

fn tidy_name(raw: &str) -> String {
    let p = re();
    let mut name = raw.to_string();
    if let Some(i) = name.find("ISIN") { name.truncate(i); }
    let name = name.trim().trim_end_matches('-').trim();
    let name = p.code.replace(name, "");
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads a CAS from its lines (as `pdf_text::lines` gives them).
pub fn parse(pages: &[Vec<String>]) -> Result<Cas> {
    let p = re();
    let mut cas = Cas { from: None, to: None, investor: None, pan: None, schemes: vec![] };
    let (mut amc, mut folio) = (String::new(), String::new());
    // Lines between "Folio No:" and the opening balance: the scheme's name (it can wrap).
    let mut heading: Option<Vec<String>> = None;
    let mut cur: Option<CasScheme> = None;
    let mut last_was_txn = false;
    let mut after_email = false;

    let finish = |cur: &mut Option<CasScheme>, out: &mut Vec<CasScheme>| {
        if let Some(mut s) = cur.take() {
            let moved: i64 = s.txns.iter().filter_map(|t| t.units_e4).sum();
            s.balanced = s.close_units_e4.is_none_or(|c| (s.open_units_e4 + moved - c).abs() <= 10);
            if s.close_units_e4.is_none() { s.close_units_e4 = Some(s.open_units_e4 + moved); }
            if s.value_minor.is_none() {
                if let (Some(u), Some(n)) = (s.close_units_e4, s.nav_e4) {
                    s.value_minor = i64::try_from((u as i128 * n as i128 + (E4 * E4 / 200) as i128) / (E4 * E4 / 100) as i128).ok();
                }
            }
            out.push(s);
        }
    };

    for line in pages.iter().flatten() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if cas.from.is_none() {
            if let Some(c) = p.period.captures(line) { cas.from = date(&c[1]); cas.to = date(&c[2]); continue; }
        }
        if p.noise.is_match(line) { continue; }
        if line.to_lowercase().starts_with("email id") { after_email = true; continue; }
        if after_email {
            after_email = false;
            if cas.investor.is_none() && !line.chars().any(|c| c.is_ascii_digit()) { cas.investor = Some(line.to_string()); continue; }
        }
        if let Some(c) = p.amc.captures(line) {
            finish(&mut cur, &mut cas.schemes);
            amc = c[1].to_string();
            last_was_txn = false;
            continue;
        }
        if let Some(c) = p.folio.captures(line) {
            finish(&mut cur, &mut cas.schemes);
            folio = c[1].split_whitespace().collect::<String>();
            if cas.pan.is_none() { cas.pan = p.pan.captures(line).map(|c| c[1].to_string()); }
            heading = Some(vec![]);
            last_was_txn = false;
            continue;
        }
        if let Some(c) = p.open.captures(line) {
            finish(&mut cur, &mut cas.schemes);
            let text = heading.take().unwrap_or_default().join(" ");
            let name = tidy_name(&text);
            if name.is_empty() { continue; }
            cur = Some(CasScheme {
                amc: amc.clone(), folio: folio.clone(), name,
                isin: p.isin.captures(&text).map(|c| c[1].to_string()),
                registrar: p.registrar.captures(&text).map(|c| c[1].to_uppercase()),
                open_units_e4: scaled(&c[1], 4).unwrap_or(0),
                close_units_e4: None, nav_e4: None, nav_date: None, cost_minor: None, value_minor: None,
                txns: vec![], balanced: true,
            });
            last_was_txn = false;
            continue;
        }
        if let Some(h) = heading.as_mut() {
            let low = line.to_lowercase();
            let skip = Some(line) == cas.investor.as_deref() || low.starts_with("nominee") || low.starts_with("kyc")
                || (h.is_empty() && !line.contains("ISIN") && !line.contains('-') && line.chars().all(|c| c.is_ascii_uppercase() || c == ' ' || c == '.'));
            if !skip { h.push(line.to_string()); }
            continue;
        }
        let Some(s) = cur.as_mut() else { continue };
        let mut closing = false;
        if let Some(c) = p.close.captures(line) { s.close_units_e4 = scaled(&c[1], 4); closing = true; }
        if let Some(c) = p.nav.captures(line) { s.nav_date = date(&c[1]); s.nav_e4 = scaled(&c[2], 4); closing = true; }
        if let Some(c) = p.cost.captures(line) { s.cost_minor = scaled(&c[1], 2); closing = true; }
        if let Some(c) = p.value.captures(line) { s.value_minor = scaled(&c[1], 2); closing = true; }
        if closing { last_was_txn = false; continue; }
        if let Some(c) = p.txn.captures(line) {
            if let Some(t) = txn_row(&c[1], &c[2]) { s.txns.push(t); last_was_txn = true; }
            continue;
        }
        // A description that wrapped onto the next line.
        if last_was_txn {
            if let Some(t) = s.txns.last_mut() {
                t.description = format!("{} {}", t.description, line.trim_matches('*').trim()).trim().to_string();
                t.kind = classify(&t.description, t.units_e4);
            }
            last_was_txn = false;
        }
    }
    finish(&mut cur, &mut cas.schemes);
    if cas.schemes.is_empty() {
        return Err(Error::Invalid("this doesn't look like a CAMS or KFintech consolidated account statement".into()));
    }
    Ok(cas)
}

/// Opens the PDF (with its password; CAS files are protected with the PAN or a password you chose)
/// and reads it.
pub fn read(bytes: &[u8], password: Option<&str>) -> Result<Cas> {
    let doc = pdf_text::open(bytes, password)?;
    parse(&pdf_text::lines(&doc)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(scaled("1,23,456.78", 2), Some(12_345_678));
        assert_eq!(scaled("2,887,660.65", 2), Some(288_766_065));
        assert_eq!(scaled("(12.345)", 4), Some(-123_450));
        assert_eq!(scaled("-0.5", 2), Some(-50));
        assert_eq!(scaled("2,045.67805", 4), Some(20_456_781));
        assert_eq!(scaled("abc", 2), None);
        assert_eq!(scaled("", 2), None);
    }

    #[test]
    fn kinds() {
        assert_eq!(classify("Purchase-SIP - Instalment 3/120", Some(1)), "sip");
        assert_eq!(classify("*** Stamp Duty ***", None), "stamp_duty");
        assert_eq!(classify("Switch In - From Axis Liquid Fund", Some(1)), "switch_in");
        assert_eq!(classify("Switch-Out - To Axis Liquid Fund", Some(-1)), "switch_out");
        assert_eq!(classify("IDCW Paid - Rs. 2.50 per unit", None), "dividend_payout");
        assert_eq!(classify("IDCW Reinvestment", Some(1)), "dividend_reinvest");
        assert_eq!(classify("Redemption - via Internet", Some(-1)), "redemption");
        assert_eq!(classify("*** STT Paid ***", None), "stt");
        assert_eq!(classify("Purchase Reversal", Some(-1)), "other");
        assert_eq!(classify("Something new", Some(5)), "purchase");
    }
}
