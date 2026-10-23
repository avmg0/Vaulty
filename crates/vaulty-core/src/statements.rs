//! Bank and card statement import from CSV and spreadsheets (XLSX, XLS, ODS).
//!
//! 1. `read` turns a file into rows of text and finds the header row (statements often start
//!    with a few lines about the account).
//! 2. `guess` maps columns from the header names; the member can correct it, and the mapping is
//!    saved per account (`save_template`) so the next statement from that bank needs no setup.
//! 3. `preview` parses every row with the mapping and compares it with the account's existing
//!    transactions: same type and amount within a day of each other counts as a duplicate.
//! 4. `import` creates the chosen rows. The app takes a safety copy first and records the whole
//!    import as one undoable action.
//!
//! Everything here treats the file as untrusted: sizes and row counts are capped, and every cell
//! is parsed defensively.

use crate::{
    error::{Error, Result},
    money,
    rows::*,
    transactions,
};
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Cursor;

pub const MAX_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ROWS: usize = 50_000;
const MAX_COLS: usize = 60;
const MAX_CELL: usize = 500;

/// The file as rows of text, with the row that looks like the header.
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub rows: Vec<Vec<String>>,
    pub header_row: usize,
}

impl Table {
    pub fn headers(&self) -> &[String] { self.rows.get(self.header_row).map(Vec::as_slice).unwrap_or(&[]) }
    /// Normalised header names, used to recognise the same bank layout again.
    pub fn signature(&self) -> String {
        self.headers().iter().map(|h| norm(h)).collect::<Vec<_>>().join("|")
    }
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > MAX_CELL { s.chars().take(MAX_CELL).collect() } else { s.to_string() }
}

/// Reads a statement. `name` (the file name) decides CSV versus spreadsheet; anything that isn't a
/// spreadsheet is read as delimited text. PDFs go through `read_pdf` (they may need a password).
pub fn read(bytes: &[u8], name: &str) -> Result<Table> {
    if bytes.starts_with(b"%PDF") { return read_pdf(bytes, None); }
    if bytes.len() > MAX_BYTES { return Err(Error::Invalid("that file is larger than 20 MB".into())); }
    let lower = name.to_lowercase();
    let rows = if [".xlsx", ".xlsm", ".xls", ".xlsb", ".ods"].iter().any(|e| lower.ends_with(e)) { read_sheet(bytes)? } else { read_csv(bytes)? };
    if rows.is_empty() { return Err(Error::Invalid("the file has no rows".into())); }
    let header_row = find_header(&rows);
    Ok(Table { rows, header_row })
}

fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    // The delimiter that splits the most of the first lines into the same number of fields.
    let sample: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(30).collect();
    let delim = (*b",;\t|").into_iter().max_by_key(|d| {
        let counts: Vec<usize> = sample.iter().map(|l| l.bytes().filter(|b| b == d).count()).collect();
        let common = counts.iter().copied().filter(|c| *c > 0).max().unwrap_or(0);
        counts.iter().filter(|c| **c == common && common > 0).count() * 100 + common
    }).unwrap_or(b',');
    let mut r = csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(delim).from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for rec in r.records() {
        let rec = rec.map_err(|e| Error::Invalid(format!("couldn't read the CSV: {e}")))?;
        let row: Vec<String> = rec.iter().take(MAX_COLS).map(clip).collect();
        if row.iter().all(String::is_empty) { continue; }
        rows.push(row);
        if rows.len() > MAX_ROWS { return Err(Error::Invalid(format!("the file has more than {MAX_ROWS} rows; split it into smaller statements"))); }
    }
    Ok(rows)
}

fn read_sheet(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    use calamine::{Data, DataType, Reader};
    let mut wb = calamine::open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|_| Error::Invalid("that spreadsheet couldn't be opened (is it password protected?)".into()))?;
    let range = wb.worksheet_range_at(0).ok_or_else(|| Error::Invalid("the spreadsheet has no sheets".into()))?
        .map_err(|_| Error::Invalid("the first sheet couldn't be read".into()))?;
    let mut rows = Vec::new();
    for r in range.rows() {
        let row: Vec<String> = r.iter().take(MAX_COLS).map(|c| match c {
            Data::Empty => String::new(),
            Data::String(s) => clip(s),
            Data::Float(f) => if f.fract() == 0.0 && f.abs() < 1e15 { format!("{f:.0}") } else { format!("{f}") },
            Data::Int(i) => i.to_string(),
            Data::Bool(b) => b.to_string(),
            Data::DateTime(_) | Data::DateTimeIso(_) => c.as_date().map(|d| d.to_string()).unwrap_or_default(),
            other => clip(&other.to_string()),
        }).collect();
        if row.iter().all(String::is_empty) { continue; }
        rows.push(row);
        if rows.len() > MAX_ROWS { return Err(Error::Invalid(format!("the sheet has more than {MAX_ROWS} rows"))); }
    }
    Ok(rows)
}

/// Reads a PDF statement (with its password, if it has one) into the same rows as a spreadsheet.
/// The table's header row gives the columns; every piece of text below it goes to the column it
/// sits under, so empty cells (a withdrawal with no deposit) stay empty. A line without a date
/// that sits right under a row continues it (a narration over two lines). The header repeated on
/// later pages, page headers and footers are left out.
pub fn read_pdf(bytes: &[u8], password: Option<&str>) -> Result<Table> {
    let doc = crate::pdf_text::open(bytes, password)?;
    let pages = crate::pdf_text::positioned(&doc)?;
    let texts = |l: &crate::pdf_text::Line| l.cells.iter().map(|c| c.text.clone()).collect::<Vec<_>>();
    let Some(header) = pages.iter().flatten().take(200).find(|l| is_header(&texts(l))).cloned() else {
        return Err(Error::Invalid("no transaction table was found in this PDF (a scanned statement has no text to read)".into()));
    };
    // Column names drawn close together can join into one cell: split a cell back into the
    // pieces it was drawn as when more than one of them is a column name.
    let known = |t: &str| [DATE_WORDS, DESC_WORDS, DEBIT_WORDS, CREDIT_WORDS, AMOUNT_WORDS, REF_WORDS, SIDE_WORDS, BALANCE_WORDS].iter().any(|w| matches(t, w));
    let mut cols: Vec<crate::pdf_text::Cell> = vec![];
    for c in &header.cells {
        if c.parts.len() > 1 && c.parts.iter().filter(|p| known(&p.2)).count() >= 2 {
            cols.extend(c.parts.iter().map(|p| crate::pdf_text::Cell { x: p.0, end: p.1, text: p.2.clone(), parts: vec![p.clone()] }));
        } else {
            cols.push(c.clone());
        }
    }
    let head: Vec<String> = cols.iter().map(|c| c.text.clone()).collect();
    let header_texts = texts(&header);
    let cols = &cols;
    let column = |c: &crate::pdf_text::Cell| -> usize {
        let overlap = |h: &crate::pdf_text::Cell| (c.end.min(h.end) - c.x.max(h.x)).max(0.0);
        let (best, ov) = cols.iter().enumerate().map(|(i, h)| (i, overlap(h))).fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        if ov > 0.0 { return best; }
        cols.iter().enumerate().map(|(i, h)| (i, (c.x - h.end).abs().min((c.end - h.x).abs()).min(((c.x + c.end) - (h.x + h.end)).abs() / 2.0)))
            .fold((0, f64::MAX), |a, b| if b.1 < a.1 { b } else { a }).0
    };
    let is_date = |s: &str| s.chars().any(|c| c.is_ascii_digit()) && s.len() <= 20 && [DateOrder::Dmy, DateOrder::Mdy, DateOrder::Ymd].iter().any(|o| parse_date(s, *o).is_some());
    let date_col = head.iter().position(|h| matches(h, DATE_WORDS)).unwrap_or(0);
    let mut rows: Vec<Vec<String>> = vec![head.iter().map(|h| clip(h)).collect()];
    let mut started = false;
    for page in &pages {
        // On a page with the header, the table starts after it.
        let from = page.iter().position(|l| texts(l) == header_texts).map(|i| i + 1);
        if from.is_none() && !started { continue; }
        started = true;
        let mut last_y: Option<(f64, f64)> = None;
        for line in &page[from.unwrap_or(0)..] {
            let mut row = vec![String::new(); cols.len()];
            // Each piece as drawn goes to its column, so two amounts drawn close together don't join.
            for c in &line.cells {
                let mut prev: Option<(usize, &str)> = None;
                for p in &c.parts {
                    let i = column(&crate::pdf_text::Cell { x: p.0, end: p.1, text: p.2.clone(), parts: vec![] });
                    // Pieces of one word (drawn apart, e.g. for kerning) join without a space.
                    let glued = prev.is_some_and(|(j, t)| j == i && c.text.contains(&format!("{t}{}", p.2)));
                    if !row[i].is_empty() && !glued { row[i].push(' '); }
                    row[i].push_str(&p.2);
                    prev = Some((i, p.2.as_str()));
                }
            }
            if is_date(row[date_col].trim()) {
                rows.push(row.iter().map(|c| clip(c)).collect());
                last_y = Some((line.y, line.size));
                if rows.len() > MAX_ROWS { return Err(Error::Invalid(format!("the file has more than {MAX_ROWS} rows; split it into smaller statements"))); }
                continue;
            }
            // A continuation sits close under the row and has no amounts of its own.
            let close = last_y.is_some_and(|(y, size)| (y - line.y) > 0.0 && (y - line.y) <= size * 2.2);
            let numbers = row.iter().enumerate().any(|(i, c)| i != date_col && parse_amount(c).is_some() && !c.chars().any(|ch| ch.is_alphabetic()) && c.contains('.'));
            if close && !numbers && rows.len() > 1 {
                let prev = rows.last_mut().expect("a row");
                for (i, c) in row.iter().enumerate().filter(|(_, c)| !c.is_empty()) {
                    let joined = if prev[i].is_empty() { c.clone() } else if prev[i].ends_with('-') { format!("{}{c}", prev[i]) } else { format!("{} {c}", prev[i]) };
                    prev[i] = clip(&joined);
                }
                last_y = Some((line.y, line.size));
            } else {
                last_y = None;
            }
        }
    }
    if rows.len() < 2 { return Err(Error::Invalid("no transactions were found in this PDF".into())); }
    Ok(Table { rows, header_row: 0 })
}

/// A header row: a date column and at least one other recognisable column.
fn is_header(r: &[String]) -> bool {
    r.iter().any(|c| matches(c, DATE_WORDS)) && r.iter().filter(|c| [DESC_WORDS, DEBIT_WORDS, CREDIT_WORDS, AMOUNT_WORDS].iter().any(|w| matches(c, w))).count() >= 1
}

const DATE_WORDS: &[&str] = &["date", "txn date", "transaction date", "tran date", "posting date", "value date", "value dt", "txn dt"];
const DESC_WORDS: &[&str] = &["narration", "description", "particulars", "details", "transaction details", "remarks", "transaction remarks", "merchant", "payee", "memo"];
const DEBIT_WORDS: &[&str] = &["withdrawal", "withdrawal amt", "withdrawal amount", "debit", "debit amount", "dr", "paid out", "money out", "withdrawals"];
const CREDIT_WORDS: &[&str] = &["deposit", "deposit amt", "deposit amount", "credit", "credit amount", "cr", "paid in", "money in", "deposits"];
const AMOUNT_WORDS: &[&str] = &["amount", "transaction amount", "amount inr", "txn amount", "amt"];
const BALANCE_WORDS: &[&str] = &["balance", "closing balance", "running balance", "available balance", "value dt", "value date"];
const SIDE_WORDS: &[&str] = &["dr cr", "drcr", "cr dr", "type", "debit credit", "transaction type", "txn type"];
const REF_WORDS: &[&str] = &["ref", "ref no", "reference", "chq no", "chq ref no", "cheque no", "chq ref number", "reference no", "utr"];

fn matches(h: &str, words: &[&str]) -> bool {
    let h = norm(h);
    words.iter().any(|w| h == *w || h.starts_with(&format!("{w} ")) || h.ends_with(&format!(" {w}")))
}

/// The first row with at least two recognisable column names (a date and something else).
fn find_header(rows: &[Vec<String>]) -> usize {
    rows.iter().take(40).position(|r| {
        let date = r.iter().any(|c| matches(c, DATE_WORDS));
        let other = r.iter().filter(|c| [DESC_WORDS, DEBIT_WORDS, CREDIT_WORDS, AMOUNT_WORDS].iter().any(|w| matches(c, w))).count();
        date && other >= 1
    }).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateOrder { Dmy, Mdy, Ymd }

/// Which column holds what. Amounts come from one signed column (optionally with a Dr/Cr
/// column), or from separate debit and credit columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapping {
    pub header_row: usize,
    pub date: usize,
    pub description: usize,
    #[serde(default)] pub amount: Option<usize>,
    #[serde(default)] pub debit: Option<usize>,
    #[serde(default)] pub credit: Option<usize>,
    /// A column saying "Dr"/"Cr" (or debit/credit) for a single amount column.
    #[serde(default)] pub side: Option<usize>,
    #[serde(default)] pub reference: Option<usize>,
    pub date_order: DateOrder,
    /// For one signed amount column: true when negative numbers are money going out (most
    /// banks); false for card statements that show purchases as positive.
    #[serde(default = "yes")] pub negative_is_expense: bool,
}
fn yes() -> bool { true }

/// Best guess at the mapping from the header names and the dates in the file.
pub fn guess(t: &Table, default_order: DateOrder) -> Result<Mapping> {
    let h = t.headers();
    let find = |words: &[&str]| words.iter().find_map(|w| h.iter().position(|c| norm(c) == *w)).or_else(|| h.iter().position(|c| matches(c, words)));
    // Prefer the transaction date over the value date.
    let date = ["txn date", "transaction date", "tran date", "date", "posting date"].iter().find_map(|w| h.iter().position(|c| norm(c) == *w))
        .or_else(|| find(DATE_WORDS)).ok_or_else(|| Error::Invalid("no date column found; choose one".into()))?;
    let description = find(DESC_WORDS).unwrap_or_else(|| (0..h.len()).find(|i| *i != date).unwrap_or(0));
    // A "Dr/Cr" column says which way the money went; it isn't a debit or credit amount column.
    let is_side = |i: usize| SIDE_WORDS.contains(&norm(&h[i]).as_str());
    let find_amount = |words: &[&str]| words.iter().find_map(|w| h.iter().enumerate().position(|(i, c)| !is_side(i) && norm(c) == *w))
        .or_else(|| h.iter().enumerate().position(|(i, c)| !is_side(i) && matches(c, words)));
    let (debit, credit) = (find_amount(DEBIT_WORDS), find_amount(CREDIT_WORDS));
    let amount = if debit.is_some() && credit.is_some() { None } else { find_amount(AMOUNT_WORDS).or(debit).or(credit) };
    let side = if amount.is_some() { (0..h.len()).find(|i| is_side(*i)).or_else(|| find(SIDE_WORDS)).filter(|i| Some(*i) != amount) } else { None };
    let col: Vec<&str> = t.rows.iter().skip(t.header_row + 1).filter_map(|r| r.get(date).map(String::as_str)).collect();
    Ok(Mapping {
        header_row: t.header_row, date, description, amount, debit: if amount.is_none() { debit } else { None },
        credit: if amount.is_none() { credit } else { None }, side, reference: find(REF_WORDS), date_order: detect_order(&col, default_order),
        negative_is_expense: true,
    })
}

/// Day-first or month-first, from values that can only be read one way (a part above 12).
pub fn detect_order(values: &[&str], default: DateOrder) -> DateOrder {
    let (mut dmy, mut mdy) = (0, 0);
    for v in values.iter().take(500) {
        let parts: Vec<&str> = v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).collect();
        if parts.len() < 3 { continue; }
        if parts[0].len() == 4 { return DateOrder::Ymd; }
        let (a, b) = (parts[0].parse::<u32>().unwrap_or(0), parts[1].parse::<u32>().unwrap_or(0));
        if a > 12 && b <= 12 { dmy += 1; }
        if b > 12 && a <= 12 { mdy += 1; }
    }
    if dmy > mdy { DateOrder::Dmy } else if mdy > dmy { DateOrder::Mdy } else { default }
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// Dates like 15/10/2026, 15-10-26, 2026-10-15, 15 Oct 2026, 15-Oct-26, Oct 15, 2026.
pub fn parse_date(s: &str, order: DateOrder) -> Option<NaiveDate> {
    let s = s.trim();
    let s = s.split([' ', 'T']).next().filter(|first| first.len() >= 8 && first.contains(['-', '/', '.'])).unwrap_or(s);
    let lower = s.to_lowercase();
    let month_word = MONTHS.iter().position(|m| lower.contains(m)).map(|i| i as u32 + 1);
    // Letters are only allowed as a month name ("15 Oct 2026"); "1O/10/26" is a typo, not a date.
    let letters: String = lower.chars().filter(|c| c.is_alphabetic()).collect();
    if !letters.is_empty() && !(month_word.is_some() && letters.len() <= 9 && MONTHS.iter().any(|m| letters.starts_with(m))) { return None; }
    let nums: Vec<u32> = lower.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
    let year = |y: u32| if y < 100 { 2000 + y as i32 } else { y as i32 };
    let (y, m, d) = match (month_word, nums.as_slice()) {
        (Some(m), [d, y]) if *d <= 31 => (year(*y), m, *d),
        (Some(m), [y, d]) if *y > 31 => (year(*y), m, *d),
        (None, [a, b, c]) => match order {
            _ if *a > 999 => (*a as i32, *b, *c),
            DateOrder::Dmy => (year(*c), *b, *a),
            DateOrder::Mdy => (year(*c), *a, *b),
            DateOrder::Ymd => (year(*a), *b, *c),
        },
        _ => return None,
    };
    NaiveDate::from_ymd_opt(y, m, d).filter(|d| (1990..=2100).contains(&chrono::Datelike::year(d)))
}

/// An amount as written in statements: "1,23,456.78", "(1,234.00)", "-500", "500.00 Dr",
/// "₹ 1,000", "1.234,56" (European). Returns the signed decimal text, or None if blank.
pub fn parse_amount(s: &str) -> Option<(bool, String)> {
    let t = s.trim().to_lowercase();
    if t.is_empty() || t == "-" || t == "0" || t == "0.00" { return None; }
    let mut negative = t.starts_with('-') || t.ends_with('-') || (t.starts_with('(') && t.ends_with(')')) || t.ends_with("dr") || t.ends_with("dr.");
    if t.ends_with("cr") || t.ends_with("cr.") { negative = false; }
    // Strip what may surround a number; anything else left over (a minus in the middle, a slash,
    // letters) means this isn't an amount, e.g. a date like "10-10" must not read as 1010.
    let mut core = t.trim_end_matches('.').trim_end_matches("dr").trim_end_matches("cr").trim();
    for p in ["rs.", "rs", "inr", "usd", "eur", "gbp", "aed"] {
        core = core.strip_prefix(p).or_else(|| core.strip_suffix(p)).unwrap_or(core).trim();
    }
    let core: String = core.chars().filter(|c| !c.is_whitespace() && !matches!(c, '₹' | '$' | '€' | '£' | '¥' | '(' | ')' | '+' | '\'')).collect();
    let core = core.strip_prefix('-').or_else(|| core.strip_suffix('-')).unwrap_or(&core);
    if !core.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') { return None; }
    let digits = core.to_string();
    if !digits.chars().any(|c| c.is_ascii_digit()) { return None; }
    // European style if the last separator is a comma followed by 1–2 digits.
    let euro = digits.rfind(',').is_some_and(|i| digits.len() - i - 1 <= 2 && digits[i + 1..].chars().all(|c| c.is_ascii_digit()) && !digits[i..].contains('.'))
        && digits.matches(',').count() == 1 && digits.contains('.');
    let clean = if euro { digits.replace('.', "").replace(',', ".") } else { digits.replace(',', "") };
    clean.parse::<f64>().ok().filter(|v| *v > 0.0)?;
    let clean = if clean.starts_with('.') { format!("0{clean}") } else { clean };
    Some((negative, clean.trim_end_matches('.').to_string()))
}

/// Shorter, readable merchant names from bank narrations, e.g. "UPI/412345678/SWIGGY/..." → "SWIGGY".
pub fn merchant_from(desc: &str) -> String {
    let d = desc.split_whitespace().collect::<Vec<_>>().join(" ");
    let up = d.to_uppercase();
    for prefix in ["UPI/", "UPI-", "IMPS/", "IMPS-", "NEFT/", "NEFT-", "POS/", "ECOM/", "ACH/", "NACH/"] {
        if up.starts_with(prefix) {
            let skip = ["UPI", "IMPS", "NEFT", "POS", "ECOM", "ACH", "NACH", "P2M", "P2A", "DR", "CR", "PAYMENT", "OK", "ICICI", "YBL", "PAYTM", "OKAXIS", "OKSBI", "OKHDFCBANK"];
            if let Some(part) = d.split(['/', '-']).map(str::trim)
                .find(|p| p.len() >= 3 && p.chars().any(char::is_alphabetic) && !p.contains('@') && !skip.contains(&p.to_uppercase().as_str()) && p.chars().filter(|c| c.is_ascii_digit()).count() * 2 < p.len()) {
                return part.chars().take(80).collect();
            }
        }
    }
    // "POS 4242 CORNER CAFE" → "CORNER CAFE": drop a leading channel word and card or reference numbers.
    let words: Vec<&str> = d.split_whitespace().collect();
    let channel = ["POS", "ECOM", "ATM", "VPS", "IPS", "MPS"];
    if words.len() > 2 && channel.contains(&words[0].to_uppercase().as_str()) {
        let rest: Vec<&str> = words[1..].iter().copied().skip_while(|w| w.chars().all(|c| c.is_ascii_digit() || c == 'X' || c == 'x' || c == '*')).collect();
        if !rest.is_empty() { return rest.join(" ").chars().take(80).collect(); }
    }
    d.chars().take(80).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedRow {
    /// Position in the file (0-based, counting from the first row after the header).
    pub index: usize,
    pub date: Option<String>,
    pub description: String,
    pub merchant: String,
    pub amount: Option<String>,
    pub kind: Option<&'static str>,
    pub reference: Option<String>,
    /// new, duplicate, error or skipped (blank or summary lines).
    pub status: &'static str,
    pub message: Option<String>,
    /// For duplicates: the date of the matching transaction already in Vaulty.
    pub matched_date: Option<String>,
    /// Category from the member's rules, if one matches the merchant.
    pub category_id: Option<String>,
}

fn cell(row: &[String], i: Option<usize>) -> &str { i.and_then(|i| row.get(i)).map(String::as_str).unwrap_or("") }

fn parse_row(index: usize, row: &[String], m: &Mapping) -> ParsedRow {
    let desc = cell(row, Some(m.description)).to_string();
    let mut p = ParsedRow { index, date: None, merchant: merchant_from(&desc), description: desc, amount: None, kind: None,
        reference: Some(cell(row, m.reference).to_string()).filter(|s| !s.is_empty()), status: "new", message: None, matched_date: None, category_id: None };
    let raw_date = cell(row, Some(m.date));
    let date = parse_date(raw_date, m.date_order);
    // Amount and direction.
    let (amount, out) = if m.amount.is_some() {
        match parse_amount(cell(row, m.amount)) {
            None => (None, false),
            Some((neg, v)) => {
                let side = norm(cell(row, m.side));
                let out = if side.starts_with("dr") || side.starts_with("debit") || side == "d" { true }
                    else if side.starts_with("cr") || side.starts_with("credit") || side == "c" { false }
                    else if m.negative_is_expense { neg } else { !neg };
                (Some(v), out)
            }
        }
    } else {
        match (parse_amount(cell(row, m.debit)), parse_amount(cell(row, m.credit))) {
            (Some((_, d)), None) => (Some(d), true),
            (None, Some((_, c))) => (Some(c), false),
            (Some(_), Some(_)) => { p.status = "error"; p.message = Some("both debit and credit have an amount".into()); (None, false) }
            (None, None) => (None, false),
        }
    };
    if p.status == "error" { return p; }
    match (date, amount) {
        // Lines without a date or amount are usually headings, opening balances or totals.
        (None, None) => { p.status = "skipped"; }
        (None, Some(_)) => { p.status = if raw_date.is_empty() { "skipped" } else { "error" }; if !raw_date.is_empty() { p.message = Some(format!("couldn't read the date `{raw_date}`")); } }
        (Some(_), None) => { p.status = "skipped"; }
        (Some(d), Some(a)) => {
            p.date = Some(d.to_string());
            p.amount = Some(a);
            p.kind = Some(if out { "expense" } else { "income" });
            let summary = ["opening balance", "closing balance", "total", "balance b f", "balance c f", "brought forward", "carried forward"];
            if summary.iter().any(|w| norm(&p.description).starts_with(w)) { p.status = "skipped"; }
        }
    }
    p
}

/// Parses every row after the header and marks duplicates of the account's existing transactions.
pub fn preview(conn: &Connection, t: &Table, m: &Mapping, account_id: &str) -> Result<Vec<ParsedRow>> {
    let width = t.headers().len().max(1);
    for c in [Some(m.date), Some(m.description), m.amount, m.debit, m.credit, m.side, m.reference].into_iter().flatten() {
        if c >= width.max(MAX_COLS) { return Err(Error::Invalid("the mapping points at a column the file doesn't have".into())); }
    }
    if m.amount.is_none() && m.debit.is_none() && m.credit.is_none() { return Err(Error::Invalid("choose the amount column, or the debit and credit columns".into())); }
    let currency: String = conn.query_row("SELECT currency FROM accounts WHERE id = ?", [account_id], |r| r.get(0))
        .optional()?.ok_or_else(|| Error::Invalid("no such account".into()))?;
    let mut rows: Vec<ParsedRow> = t.rows.iter().skip(m.header_row + 1).enumerate().map(|(i, r)| parse_row(i, r, m)).collect();
    // Amounts must be valid for the account's currency.
    for r in rows.iter_mut().filter(|r| r.status == "new") {
        if let Err(e) = money::parse_minor(&json!(r.amount.as_deref().unwrap_or("")), &currency) { r.status = "error"; r.message = Some(e.to_string()); }
    }
    mark_duplicates(conn, &mut rows, account_id, &currency)?;
    for r in rows.iter_mut().filter(|r| r.kind.is_some()) {
        r.category_id = crate::rules::suggest(conn, &r.merchant, r.kind.unwrap_or("expense"))?;
    }
    Ok(rows)
}

/// Each existing transaction can match one row: same type and amount, dates at most a day apart.
fn mark_duplicates(conn: &Connection, rows: &mut [ParsedRow], account_id: &str, currency: &str) -> Result<()> {
    let dates: Vec<NaiveDate> = rows.iter().filter_map(|r| r.date.as_deref()).filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).collect();
    let (Some(min), Some(max)) = (dates.iter().min(), dates.iter().max()) else { return Ok(()) };
    let mut existing: Vec<(NaiveDate, String, i64, bool)> = conn.prepare(
        "SELECT date, type, amount_minor FROM transactions WHERE account_id = ?1 AND date BETWEEN ?2 AND ?3 AND type IN ('income', 'expense')")?
        .query_map(params![account_id, (*min - Duration::days(1)).to_string(), (*max + Duration::days(1)).to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?
        .filter_map(|x| x.ok())
        .filter_map(|(d, k, a)| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok().map(|d| (d, k, a, false)))
        .collect();
    for r in rows.iter_mut().filter(|r| r.status == "new") {
        let (Some(d), Some(k), Some(a)) = (r.date.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()), r.kind, r.amount.as_deref()) else { continue };
        let Ok(minor) = money::parse_minor(&json!(a), currency) else { continue };
        // Prefer a same-day match, then a day either side.
        let hit = (0..=1).find_map(|gap| existing.iter().position(|(ed, ek, ea, used)| !used && ek == k && *ea == minor && (*ed - d).num_days().abs() == gap));
        if let Some(i) = hit {
            existing[i].3 = true;
            r.status = "duplicate";
            r.message = Some(format!("already recorded on {}", existing[i].0));
            r.matched_date = Some(existing[i].0.to_string());
        }
    }
    Ok(())
}

/// Creates the rows not listed in `skip` (duplicates are skipped unless listed in `include`).
/// Returns how many were added.
pub fn import(conn: &Connection, t: &Table, m: &Mapping, account_id: &str, skip: &[usize], include: &[usize]) -> Result<usize> {
    let rows = preview(conn, t, m, account_id)?;
    let chosen: Vec<&ParsedRow> = rows.iter().filter(|r| {
        (r.status == "new" && !skip.contains(&r.index)) || (r.status == "duplicate" && include.contains(&r.index))
    }).collect();
    atomic(conn, |tx| {
        for r in &chosen {
            transactions::create(tx, &json!({
                "account_id": account_id, "type": r.kind, "amount": r.amount, "date": r.date, "category_id": r.category_id,
                "merchant": r.merchant, "description": (r.description != r.merchant).then_some(&r.description),
                "notes": r.reference.as_ref().map(|x| format!("Ref {x}")),
            }))?;
        }
        Ok(())
    })?;
    save_template(conn, account_id, t, m)?;
    Ok(chosen.len())
}

/// Remembers the mapping for this account.
pub fn save_template(conn: &Connection, account_id: &str, t: &Table, m: &Mapping) -> Result<()> {
    conn.execute(
        "INSERT INTO import_templates (id, account_id, header_signature, mapping) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(account_id) DO UPDATE SET header_signature = excluded.header_signature, mapping = excluded.mapping, updated_at = CURRENT_TIMESTAMP",
        params![new_id(), account_id, t.signature(), serde_json::to_string(m)?],
    )?;
    Ok(())
}

/// A saved mapping for this account, or for another account whose statements look the same.
/// Only used if the header row is still where it was and the headers match.
pub fn saved_mapping(conn: &Connection, account_id: &str, t: &Table) -> Result<Option<Mapping>> {
    let sig = t.signature();
    let found: Option<String> = conn.query_row(
        "SELECT mapping FROM import_templates WHERE header_signature = ?2 ORDER BY account_id = ?1 DESC, updated_at DESC LIMIT 1",
        params![account_id, sig], |r| r.get(0)).optional()?;
    Ok(found.and_then(|j| serde_json::from_str::<Mapping>(&j).ok()).map(|mut m| { m.header_row = t.header_row; m }))
}

/// What the import screen needs to start: the rows around the header, the guessed or saved
/// mapping and whether it was saved.
pub fn inspect(conn: &Connection, t: &Table, account_id: &str, default_order: DateOrder) -> Result<Value> {
    let (mapping, saved) = match saved_mapping(conn, account_id, t)? {
        Some(m) => (m, true),
        None => (guess(t, default_order)?, false),
    };
    Ok(json!({
        "headers": t.headers(), "header_row": t.header_row, "row_count": t.rows.len().saturating_sub(t.header_row + 1),
        "sample": t.rows.iter().skip(t.header_row + 1).take(8).collect::<Vec<_>>(),
        "mapping": mapping, "saved": saved,
    }))
}
