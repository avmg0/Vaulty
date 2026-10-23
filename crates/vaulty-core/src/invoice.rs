//! Reads a bill or invoice into draft bill items: who it's from (name and GSTIN), the bill number
//! and date, each line (item, qty, unit, rate, amount, GST rate) and the totals, so the person can
//! check and add them to a project instead of typing them.
//!
//! It works on text lines (from a PDF with text in it today; from photos later), so it doesn't
//! depend on one shop's layout. Item lines are found where quantity × rate comes to the amount on
//! the same line, or, inside the item table, where a description has an amount. Nothing is saved
//! here; the result says whether the lines add up to the bill's total.
use crate::{cas::scaled, error::Result, pdf_text};
use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DraftItem {
    pub item: String,
    /// Numbers as plain decimal text ("1850.50"), ready for the item form.
    pub qty: Option<String>,
    pub unit: Option<String>,
    pub rate: Option<String>,
    pub amount: String,
    pub tax_percent: Option<String>,
    pub hsn: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Draft {
    pub vendor: Option<String>,
    pub gstin: Option<String>,
    pub bill_no: Option<String>,
    pub date: Option<NaiveDate>,
    pub items: Vec<DraftItem>,
    /// Before tax, as the bill states it.
    pub taxable: Option<String>,
    /// Tax as the bill states it (CGST + SGST, or IGST), and its overall rate.
    pub tax: Option<String>,
    pub tax_rate: Option<String>,
    pub total: Option<String>,
    /// The lines found, with their tax, in paise-exact decimal text.
    pub items_total: String,
    /// Whether the lines (with tax) come to the bill's total (within ₹1).
    pub matches: Option<bool>,
}

struct P { rate: Regex, gstin: Regex, bill_no: Regex, date: Regex, tax: Regex, total: Regex, taxable: Regex, header: Regex, end: Regex, skip_vendor: Regex, number: Regex, percent: Regex, hsn: Regex }

fn re() -> &'static P {
    static R: OnceLock<P> = OnceLock::new();
    R.get_or_init(|| {
        let r = |s: &str| Regex::new(s).expect("pattern");
        P {
            rate: r(r"(\d{1,2}(?:\.\d+)?)\s*%"),
            gstin: r(r"\b(\d{2}[A-Z]{5}\d{4}[A-Z][1-9A-Z]Z[0-9A-Z])\b"),
            bill_no: r(r"(?i)\b(?:invoice|bill|inv|receipt|memo)\.?\s*(?:no|number|num|#)\.?\s*[:#\-]?\s*([A-Z0-9][A-Z0-9/\-_.]*)"),
            date: r(r"(?i)\bdate[d]?\s*[:\-]?\s*(\d{1,2}[\-/. ](?:\d{1,2}|[A-Za-z]{3,9})[\-/. ,]+\d{2,4}|\d{4}-\d{2}-\d{2})"),
            tax: r(r"(?i)\b(cgst|sgst|utgst|igst)\b"),
            total: r(r"(?i)\b(grand\s*total|invoice\s*total|total\s*amount|amount\s*payable|net\s*(?:amount|payable)|bill\s*amount|total)\b"),
            taxable: r(r"(?i)\b(taxable\s*(?:value|amount)|sub\s*-?\s*total)\b"),
            header: r(r"(?i)\b(description|particulars|item|product|goods|services?)\b.*\b(amount|amt|total|value|rate|price)\b"),
            end: r(r"(?i)^\s*(taxable|sub\s*-?\s*total|total|grand|cgst|sgst|utgst|igst|tax\b|round|discount|amount in words|net\b|balance)"),
            skip_vendor: r(r"(?i)^(tax\s+invoice|invoice|bill( of supply)?|cash\s*memo|retail\s*invoice|receipt|estimate|quotation|original|duplicate|triplicate)\b"),
            number: r(r"^(?:₹|Rs\.?|INR)?\s*\(?-?[\d,]*\.?\d+\)?$"),
            percent: r(r"^(\d{1,2}(?:\.\d+)?)\s*%$"),
            hsn: r(r"^\d{4,8}$"),
        }
    })
}

fn clean_num(s: &str) -> &str { s.trim().trim_start_matches('₹').trim_start_matches("Rs.").trim_start_matches("Rs").trim_start_matches("INR").trim() }
fn num(s: &str, places: u32) -> Option<i64> { scaled(clean_num(s), places) }
/// Paise as decimal text: 185050 → "1850.50".
fn money(p: i64) -> String { format!("{}{}.{:02}", if p < 0 { "-" } else { "" }, (p.abs() / 100), p.abs() % 100) }
/// Quantity (×10,000) as short decimal text: 25000 → "2.5".
fn qty_text(q: i64) -> String {
    let s = format!("{}.{:04}", q / 10_000, q % 10_000);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim().replace(',', " ");
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    for f in ["%d/%m/%Y", "%d-%m-%Y", "%d.%m.%Y", "%d-%b-%Y", "%d %b %Y", "%d/%b/%Y", "%d-%B-%Y", "%d %B %Y", "%Y-%m-%d", "%d/%m/%y", "%d-%m-%y", "%d.%m.%y", "%d-%b-%y"] {
        if let Ok(d) = NaiveDate::parse_from_str(&s, f) { if chrono::Datelike::year(&d) >= 1990 { return Some(d); } }
    }
    None
}

/// The cells of a line: columns are joined with two spaces by `pdf_text`; a line with one cell is
/// split into words (text from photos has no columns).
fn cells(line: &str) -> Vec<String> {
    let c: Vec<String> = line.split("  ").map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
    if c.len() > 1 { c } else { line.split_whitespace().map(str::to_string).collect() }
}

/// An item line: a description, and numbers where qty × rate = amount (or, when `table` says the
/// line is inside the item table, just an amount).
fn item_line(line: &str, table: bool) -> Option<DraftItem> {
    let p = re();
    let mut cs = cells(line);
    // A leading serial number.
    if cs.len() > 2 && cs[0].trim_end_matches('.').parse::<u32>().is_ok_and(|n| n < 1000) { cs.remove(0); }
    let is_num = |s: &str| p.number.is_match(s.trim());
    let nums: Vec<usize> = (0..cs.len()).filter(|&i| is_num(&cs[i]) && !p.hsn.is_match(cs[i].trim())).collect();
    let last = *nums.last()?;
    if last + 1 != cs.len() && !cs[last + 1..].iter().all(|c| p.percent.is_match(c.trim())) { return None; }
    let amount = num(&cs[last], 2).filter(|a| *a > 0)?;
    // Find qty × rate = amount among the numbers before it.
    let mut found = None;
    'outer: for (a, &qi) in nums.iter().enumerate() {
        for &ri in &nums[a + 1..] {
            if ri >= last { break; }
            let (Some(q), Some(r)) = (num(&cs[qi], 4), num(&cs[ri], 2)) else { continue };
            if q <= 0 || r <= 0 { continue; }
            let calc = q as i128 * r as i128 / 10_000;
            if (calc - amount as i128).abs() <= (amount as i128 / 200).max(100) { found = Some((qi, ri, q, r)); break 'outer; }
        }
    }
    if found.is_none() && !table { return None; }
    let first_num = nums.first().copied().unwrap_or(last);
    let text: Vec<&str> = cs[..first_num].iter().map(String::as_str).filter(|c| !p.hsn.is_match(c.trim())).collect();
    let item = text.join(" ").trim().trim_matches(|c: char| c == '-' || c == ':' || c == '*').trim().to_string();
    if item.chars().filter(|c| c.is_alphabetic()).count() < 2 { return None; }
    if p.end.is_match(&item) || p.total.is_match(&item) && item.split_whitespace().count() <= 3 { return None; }
    let unit = found.and_then(|(qi, _, _, _)| cs.get(qi + 1)).map(|u| u.trim().to_string())
        .filter(|u| u.len() <= 8 && u.chars().all(|c| c.is_alphabetic() || c == '.') && !u.is_empty());
    let hsn = cs.iter().map(|c| c.trim()).find(|c| p.hsn.is_match(c)).map(str::to_string);
    let tax_percent = cs.iter().find_map(|c| p.percent.captures(c.trim()).map(|m| m[1].to_string()));
    Some(DraftItem {
        item: item.chars().take(200).collect(),
        qty: found.map(|f| qty_text(f.2)), unit, rate: found.map(|f| money(f.3)),
        amount: money(amount), tax_percent, hsn,
    })
}

fn last_number(line: &str) -> Option<i64> {
    cells(line).iter().rev().find(|c| re().number.is_match(c.trim())).and_then(|c| num(c, 2))
}

/// Reads a bill from its text lines.
pub fn parse(lines: &[String]) -> Draft {
    let p = re();
    let mut d = Draft::default();
    let mut in_table = false;
    let (mut tax_sum, mut tax_seen, mut rate_sum) = (0i64, false, 0i64);
    let mut total_rank = 0;
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        if d.vendor.is_none() && i < 6 && !p.skip_vendor.is_match(line) {
            let first = cells(line).into_iter().next().unwrap_or_default();
            let words = if line.contains("  ") { first } else { line.to_string() };
            if words.chars().filter(|c| c.is_alphabetic()).count() >= 3 && !p.bill_no.is_match(&words) && !p.date.is_match(&words) && !p.gstin.is_match(&words) {
                d.vendor = Some(words.chars().take(100).collect());
            }
        }
        if d.gstin.is_none() { d.gstin = p.gstin.captures(line).map(|c| c[1].to_string()); }
        if d.bill_no.is_none() {
            d.bill_no = p.bill_no.captures(line).map(|c| c[1].trim_end_matches(['.', '-']).to_string()).filter(|b| b.chars().any(|c| c.is_ascii_digit()));
        }
        if d.date.is_none() { d.date = p.date.captures(line).and_then(|c| parse_date(&c[1])); }

        if p.header.is_match(line) && !in_table && d.items.is_empty() { in_table = true; continue; }
        if p.tax.is_match(line) && !p.header.is_match(line) {
            if let Some(a) = last_number(line) { tax_sum += a; tax_seen = true; }
            if let Some(r) = p.rate.captures(line).and_then(|c| scaled(&c[1], 2)) { rate_sum += r; }
            in_table = false;
            continue;
        }
        if p.taxable.is_match(line) {
            if let Some(a) = last_number(line) { d.taxable = Some(money(a)); }
            in_table = false;
            continue;
        }
        if let Some(c) = p.total.captures(line) {
            let rank = match c[1].to_lowercase().split_whitespace().collect::<String>().as_str() { "total" => 1, _ => 2 };
            if let Some(a) = last_number(line) {
                if rank >= total_rank && line.split("  ").count() <= 3 { d.total = Some(money(a)); total_rank = rank; in_table = false; continue; }
            }
        }
        if in_table && p.end.is_match(line) { in_table = false; }
        if let Some(it) = item_line(line, in_table) { d.items.push(it); }
    }
    if tax_seen { d.tax = Some(money(tax_sum)); }
    if rate_sum > 0 { d.tax_rate = Some(qty_text(rate_sum * 100)); }
    // One GST rate for the whole bill: give it to items that didn't state their own.
    let items_amount: i64 = d.items.iter().filter_map(|i| num(&i.amount, 2)).sum();
    if let Some(rate) = &d.tax_rate {
        let taxable_ok = d.taxable.as_deref().and_then(|t| num(t, 2)).is_none_or(|t| (t - items_amount).abs() <= 100);
        if taxable_ok { for it in d.items.iter_mut().filter(|i| i.tax_percent.is_none()) { it.tax_percent = Some(rate.clone()); } }
    }
    let with_tax: i64 = d.items.iter().map(|i| {
        let a = num(&i.amount, 2).unwrap_or(0);
        let pct = i.tax_percent.as_deref().and_then(|t| num(t, 2)).unwrap_or(0);
        a + (a as i128 * pct as i128 / 10_000) as i64
    }).sum();
    d.items_total = money(with_tax);
    d.matches = d.total.as_deref().and_then(|t| num(t, 2)).filter(|_| !d.items.is_empty()).map(|t| (t - with_tax).abs() <= 100);
    d
}

/// Reads a bill from a PDF that has text in it (most bills sent by email or made by billing
/// software). A scanned PDF has no text: it gives no lines, and the draft says so by being empty.
pub fn read_pdf(bytes: &[u8], password: Option<&str>) -> Result<Draft> {
    let doc = pdf_text::open(bytes, password)?;
    let lines: Vec<String> = pdf_text::lines(&doc)?.into_iter().take(10).flatten().collect();
    Ok(parse(&lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_and_numbers() {
        assert_eq!(parse_date("02/10/2026"), NaiveDate::from_ymd_opt(2026, 10, 2));
        assert_eq!(parse_date("6-Oct-2026"), NaiveDate::from_ymd_opt(2026, 10, 6));
        assert_eq!(parse_date("06 October, 2026"), NaiveDate::from_ymd_opt(2026, 10, 6));
        assert_eq!(parse_date("02.10.26"), NaiveDate::from_ymd_opt(2026, 10, 2));
        assert_eq!((money(185050), qty_text(25000), qty_text(40000)), ("1850.50".into(), "2.5".into(), "4".into()));
        let it = item_line("Cement OPC 53 grade 50 kg bag  2523  10  Bag  ₹385.00  3,850.00", false).unwrap();
        assert_eq!((it.item.as_str(), it.qty.as_deref(), it.unit.as_deref(), it.rate.as_deref(), it.amount.as_str(), it.hsn.as_deref()),
            ("Cement OPC 53 grade 50 kg bag", Some("10"), Some("Bag"), Some("385.00"), "3850.00", Some("2523")));
        assert!(item_line("Phone: 98765 43210", false).is_none());
        assert!(item_line("Grand Total  19,564.40", true).is_none());
        // From a photo: words, not columns.
        let it = item_line("2 Tile adhesive 5 320.00 1600.00", false).unwrap();
        assert_eq!((it.item.as_str(), it.qty.as_deref(), it.amount.as_str()), ("Tile adhesive", Some("5"), "1600.00"));
    }
}
