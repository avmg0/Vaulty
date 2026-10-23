//! Credit card statements, worked out from the card's own transactions.
//!
//! Each statement covers the days up to the card's statement day. What it asks for is what was
//! owed on that day; payments (and refunds) after it count towards it. The minimum due follows
//! the card's rule (5% or ₹200, whichever is more, unless set otherwise), never more than the
//! statement. When the member enters the figures printed on a statement, those are used instead.
//!
//! Also: interest-free days for a purchase made today (until the due date of the statement it
//! will be on), which "best card for today" uses.
use crate::{error::{Error, Result}, ledger, money, rows::*};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

/// Statements shown in a card's history (the current one included).
pub const HISTORY: usize = 7;
pub const DEFAULT_MIN_PERCENT: f64 = 5.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cycle {
    pub statement_date: String,
    pub due_date: String,
    /// What the statement asks for.
    pub total: f64,
    pub min: f64,
    /// Paid since the statement (by the due date, for past statements).
    pub paid: f64,
    pub left: f64,
    pub min_left: f64,
    /// nothing_due, paid, min_paid, due, overdue (minimum missed), carried (minimum paid, not all:
    /// interest is likely). For past statements: nothing_due, paid, min_paid (by the due date) or missed.
    pub status: String,
    /// From the figures the member entered, rather than worked out.
    pub entered: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CardCycles {
    pub card_id: String,
    pub name: String,
    pub currency: String,
    pub statement_day: u32,
    pub due_day: u32,
    /// The latest statement, if one has been made since the card was added.
    pub current: Option<Cycle>,
    /// Spent since the latest statement: on the next one.
    pub unbilled: f64,
    pub next_statement: String,
    /// A purchase today is due on this date: this many days without interest.
    pub buy_today_due: String,
    pub interest_free_days: i64,
    /// Older statements, newest first.
    pub history: Vec<Cycle>,
}

fn clamp_day(y: i32, m: u32, day: u32) -> Option<NaiveDate> {
    (1..=day.min(31)).rev().find_map(|d| NaiveDate::from_ymd_opt(y, m, d))
}

fn add_months(d: NaiveDate, n: i32) -> (i32, u32) {
    let total = d.year() * 12 + d.month0() as i32 + n;
    (total.div_euclid(12), total.rem_euclid(12) as u32 + 1)
}

/// The statement date in the month `n` months from `d`'s.
fn statement_in(d: NaiveDate, n: i32, day: u32) -> Option<NaiveDate> {
    let (y, m) = add_months(d, n);
    clamp_day(y, m, day)
}

/// The first due day after a statement.
fn due_after(statement: NaiveDate, due_day: u32) -> Option<NaiveDate> {
    (0..3).filter_map(|n| statement_in(statement, n, due_day)).find(|d| *d > statement)
}

/// Statement days on or before `today`, newest first.
fn past_statements(today: NaiveDate, day: u32, count: usize) -> Vec<NaiveDate> {
    (-(count as i32) - 1..=0).rev().filter_map(|n| statement_in(today, n, day)).filter(|d| *d <= today).take(count).collect()
}

fn min_due(total: i64, percent: f64, floor: i64, unit: i64) -> i64 {
    if total <= 0 { return 0; }
    let pct = ((total as f64) * percent / 100.0).ceil() as i64;
    // Rounded up to a whole rupee (or dollar), as banks print it.
    let pct = (pct + unit - 1) / unit * unit;
    pct.max(floor).min(total)
}

/// The cycles of one credit card, or None if it isn't a credit card or lacks statement and due days.
pub fn for_card(conn: &Connection, card_id: &str, today: NaiveDate) -> Result<Option<CardCycles>> {
    let row = query_one(conn, "SELECT * FROM cards WHERE id = ?", &[json!(card_id)])?.ok_or_else(|| Error::Invalid("no such card".into()))?;
    let (Some(sd), Some(dd), Some(account)) = (row["statement_day"].as_i64(), row["due_day"].as_i64(), row["account_id"].as_str()) else { return Ok(None) };
    if row["kind"] != "credit" { return Ok(None); }
    let (sd, dd) = (sd as u32, dd as u32);
    let (ledger_id, _, cur) = ledger::for_account(conn, account)?;
    let dec = |m: i64| money::to_decimal(m, &cur);
    let unit = money::scale(&cur);
    let percent = row["min_due_percent"].as_f64().unwrap_or(DEFAULT_MIN_PERCENT);
    let floor = row["min_due_floor_minor"].as_i64().unwrap_or(if cur == "INR" { 200 * unit } else { 0 });

    // Every cleared posting on the card: (date, amount). Spending is negative, payments positive.
    let posts: Vec<(NaiveDate, i64)> = conn.prepare(
        "SELECT e.date, p.amount_minor FROM postings p JOIN entries e ON e.id = p.entry_id WHERE p.ledger_account_id = ? AND e.is_cleared = 1")?
        .query_map([&ledger_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .filter_map(|x| x.ok())
        .filter_map(|(d, a)| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok().map(|d| (d, a)))
        .collect();
    let owed_on = |d: NaiveDate| (-posts.iter().filter(|(pd, _)| *pd <= d).map(|(_, a)| a).sum::<i64>()).max(0);
    let credits = |after: NaiveDate, until: NaiveDate| posts.iter().filter(|(pd, a)| *pd > after && *pd <= until && *a > 0).map(|(_, a)| a).sum::<i64>();
    let first: Option<NaiveDate> = posts.iter().map(|(d, _)| *d).min();

    let entered: std::collections::HashMap<String, (i64, Option<i64>)> = conn.prepare("SELECT statement_date, total_minor, min_minor FROM card_statements WHERE card_id = ?")?
        .query_map([card_id], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?))))?
        .collect::<rusqlite::Result<_>>()?;

    let cycle = |s: NaiveDate| -> Option<Cycle> {
        let due = due_after(s, dd)?;
        let given = entered.get(&s.to_string());
        let total = given.map(|g| g.0).unwrap_or_else(|| owed_on(s));
        let min = given.and_then(|g| g.1).unwrap_or_else(|| min_due(total, percent, floor, unit)).min(total);
        let past = due < today;
        let paid = credits(s, if past { due } else { today });
        let (left, min_left) = ((total - paid).max(0), (min - paid).max(0));
        let status = if total == 0 { "nothing_due" }
            else if left == 0 { "paid" }
            else if past { if min_left == 0 { "min_paid" } else { "missed" } }
            else if min_left == 0 { "min_paid" } else { "due" };
        Some(Cycle {
            statement_date: s.to_string(), due_date: due.to_string(), total: dec(total), min: dec(min), paid: dec(paid),
            left: dec(left), min_left: dec(min_left), status: status.into(), entered: given.is_some(),
        })
    };

    // Statements since the card's first transaction (or ones the member entered).
    let made = |s: &NaiveDate| first.is_some_and(|f| f <= *s) || entered.contains_key(&s.to_string());
    let mut statements: Vec<Cycle> = past_statements(today, sd, HISTORY).into_iter().filter(made).filter_map(cycle).collect();
    // The latest statement, still being paid: overdue once its due date has passed unpaid.
    if let Some(c) = statements.first_mut() {
        if c.due_date.as_str() < today.to_string().as_str() {
            c.status = match c.status.as_str() { "missed" => "overdue", "min_paid" => "carried", s => s }.into();
            // What's left counts payments made after the due date too.
            if let (Ok(s), Ok(due)) = (NaiveDate::parse_from_str(&c.statement_date, "%Y-%m-%d"), NaiveDate::parse_from_str(&c.due_date, "%Y-%m-%d")) {
                let total = (c.total * unit as f64).round() as i64;
                let paid = credits(s, today.max(due));
                c.paid = dec(paid);
                c.left = dec((total - paid).max(0));
                c.min_left = dec(((c.min * unit as f64).round() as i64 - paid).max(0));
                if c.left == 0.0 { c.status = "paid".into(); }
            }
        }
    }
    let last = statements.first().and_then(|c| NaiveDate::parse_from_str(&c.statement_date, "%Y-%m-%d").ok());
    let unbilled = posts.iter().filter(|(d, a)| last.is_none_or(|l| *d > l) && *d <= today && *a < 0).map(|(_, a)| -a).sum::<i64>();
    let next_statement = (0..3).filter_map(|n| statement_in(today, n, sd)).find(|d| *d >= today).unwrap_or(today);
    let buy_due = due_after(next_statement, dd).unwrap_or(next_statement);
    let current = if statements.is_empty() { None } else { Some(statements.remove(0)) };

    Ok(Some(CardCycles {
        card_id: card_id.into(), name: row["name"].as_str().unwrap_or_default().into(), currency: cur.clone(), statement_day: sd, due_day: dd,
        current, unbilled: dec(unbilled), next_statement: next_statement.to_string(), buy_today_due: buy_due.to_string(),
        interest_free_days: (buy_due - today).num_days(), history: statements,
    }))
}

/// Every credit card with statement and due days.
pub fn all(conn: &Connection, today: NaiveDate) -> Result<Vec<CardCycles>> {
    let ids: Vec<String> = crate::cards::list(conn)?.into_iter().filter(|c| c["kind"] == "credit").filter_map(|c| c["id"].as_str().map(str::to_string)).collect();
    let mut out = vec![];
    for id in ids { if let Some(c) = for_card(conn, &id, today)? { out.push(c); } }
    Ok(out)
}

/// Records the figures printed on a statement (`min` optional). They replace the estimate.
pub fn enter(conn: &Connection, card_id: &str, v: &Value) -> Result<()> {
    let row = query_one(conn, "SELECT c.kind, c.statement_day, a.currency FROM cards c LEFT JOIN accounts a ON a.id = c.account_id WHERE c.id = ?", &[json!(card_id)])?
        .ok_or_else(|| Error::Invalid("no such card".into()))?;
    if row["kind"] != "credit" { return Err(Error::Invalid("statements are for credit cards".into())); }
    let cur = row["currency"].as_str().unwrap_or("INR");
    let date = v["statement_date"].as_str().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .ok_or_else(|| Error::Invalid("give the statement date".into()))?;
    let total = money::parse_minor(&v["total"], cur)?;
    let min = match &v["min"] { Value::Null => None, Value::String(s) if s.trim().is_empty() => None, x => Some(money::parse_minor(x, cur)?) };
    if total < 0 || min.is_some_and(|m| m < 0) { return Err(Error::Invalid("amounts can't be negative".into())); }
    if min.is_some_and(|m| m > total) { return Err(Error::Invalid("the minimum can't be more than the total".into())); }
    conn.execute("INSERT INTO card_statements (id, card_id, statement_date, total_minor, min_minor) VALUES (?1, ?2, ?3, ?4, ?5)
                  ON CONFLICT(card_id, statement_date) DO UPDATE SET total_minor = ?4, min_minor = ?5",
        params![new_id(), card_id, date.to_string(), total, min])?;
    Ok(())
}

/// Goes back to Vaulty's own figures for a statement.
pub fn forget(conn: &Connection, card_id: &str, statement_date: &str) -> Result<()> {
    conn.execute("DELETE FROM card_statements WHERE card_id = ? AND statement_date = ?", [card_id, statement_date])?;
    Ok(())
}

/// The amount due next on a card for reminders and the forecast: (due date, left, minimum left).
/// Falls back to everything owed when the card has no statement days.
pub fn next_due(conn: &Connection, card_id: &str, today: NaiveDate) -> Result<Option<(NaiveDate, i64, i64)>> {
    let Some(c) = for_card(conn, card_id, today)? else { return Ok(None) };
    let unit = money::scale(&c.currency) as f64;
    Ok(c.current.filter(|x| x.left > 0.0).and_then(|x| {
        let due = NaiveDate::parse_from_str(&x.due_date, "%Y-%m-%d").ok()?;
        Some((due, (x.left * unit).round() as i64, (x.min_left * unit).round() as i64))
    }))
}
