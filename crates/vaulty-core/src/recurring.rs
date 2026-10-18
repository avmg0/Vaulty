//! Recurring payments and bills: rent, subscriptions, salary, SIPs, a standing transfer.
//!
//! A schedule is "every N days / weeks / months / years" from a start date, optionally ending.
//! Monthly and yearly dates keep the start date's day, clamped to short months (a bill on the
//! 31st falls on 30 April and 28 or 29 February, then back to the 31st). Marking one paid creates
//! a normal transaction linked by `recurring_id` and moves the next due date on; auto-pay items
//! are caught up when the member signs in.

use crate::{
    error::{Error, Result},
    money,
    rows::*,
    transactions,
};
use chrono::{Datelike, Duration, Months, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

/// At most this many missed occurrences are created at once by auto-pay.
const CATCH_UP_LIMIT: usize = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency { Daily, Weekly, Monthly, Yearly }

impl Frequency {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "daily" => Self::Daily, "weekly" => Self::Weekly, "monthly" => Self::Monthly, "yearly" => Self::Yearly,
            _ => return Err(Error::Invalid("frequency must be daily, weekly, monthly or yearly".into())),
        })
    }
}

/// The `n`th occurrence (0 = the start date).
pub fn occurrence(start: NaiveDate, freq: Frequency, every: u32, n: u32) -> Option<NaiveDate> {
    let steps = every.checked_mul(n)?;
    match freq {
        Frequency::Daily => start.checked_add_signed(Duration::days(steps as i64)),
        Frequency::Weekly => start.checked_add_signed(Duration::weeks(steps as i64)),
        Frequency::Monthly | Frequency::Yearly => {
            let months = if freq == Frequency::Yearly { steps.checked_mul(12)? } else { steps };
            let first = start.with_day(1)?.checked_add_months(Months::new(months))?;
            let last = first.checked_add_months(Months::new(1))? - Duration::days(1);
            first.with_day(start.day().min(last.day()))
        }
    }
}

/// First occurrence on or after `from`, if the schedule hasn't ended by then.
pub fn next_on_or_after(start: NaiveDate, freq: Frequency, every: u32, end: Option<NaiveDate>, from: NaiveDate) -> Option<NaiveDate> {
    if from <= start { return Some(start).filter(|d| end.is_none_or(|e| *d <= e)); }
    // Jump close, then step: approximate how many periods have passed.
    let days = (from - start).num_days().max(0) as u32;
    let per = match freq { Frequency::Daily => 1, Frequency::Weekly => 7, Frequency::Monthly => 28, Frequency::Yearly => 365 } * every;
    let mut n = (days / per.max(1)).saturating_sub(2);
    loop {
        let d = occurrence(start, freq, every, n)?;
        if d >= from { return Some(d).filter(|d| end.is_none_or(|e| *d <= e)); }
        n += 1;
    }
}

fn date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("`{s}` isn't a date (YYYY-MM-DD)")))
}

struct Rule { start: NaiveDate, freq: Frequency, every: u32, end: Option<NaiveDate> }

fn rule_of(row: &Value) -> Result<Rule> {
    Ok(Rule {
        start: date(row["start_date"].as_str().unwrap_or_default())?,
        freq: Frequency::parse(row["frequency"].as_str().unwrap_or_default())?,
        every: row["every"].as_u64().unwrap_or(1) as u32,
        end: row["end_date"].as_str().map(date).transpose()?,
    })
}

const SELECT: &str = "SELECT r.*, a.name AS account_name, ta.name AS to_account_name, c.name AS category_name, c.icon AS category_icon,
                         c.color AS category_color
                  FROM recurring_payments r
                  LEFT JOIN accounts a ON a.id = r.account_id LEFT JOIN accounts ta ON ta.id = r.to_account_id
                  LEFT JOIN categories c ON c.id = r.category_id";

/// Adds `amount` (decimal) and, relative to `today`, `days_until` and `status`
/// (overdue, due, soon, upcoming, ended, paused).
fn decorate(mut row: Value, today: NaiveDate) -> Value {
    let cur = row["currency"].as_str().unwrap_or("INR").to_string();
    let minor = row["amount_minor"].as_i64().unwrap_or(0);
    let next = row["next_due_date"].as_str().and_then(|d| date(d).ok());
    let reminder = row["reminder_days"].as_i64().unwrap_or(3);
    let status = if row["is_active"] == 0 { "paused" } else {
        match next {
            None => "ended",
            Some(d) if d < today => "overdue",
            Some(d) if d == today => "due",
            Some(d) if (d - today).num_days() <= reminder => "soon",
            _ => "upcoming",
        }
    };
    if let Some(o) = row.as_object_mut() {
        o.insert("amount".into(), json!(money::to_decimal(minor, &cur)));
        o.insert("days_until".into(), json!(next.map(|d| (d - today).num_days())));
        o.insert("status".into(), json!(status));
    }
    row
}

pub fn list(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let rows = query(conn, &format!("{SELECT} ORDER BY r.is_active DESC, r.next_due_date IS NULL, r.next_due_date, r.name"), &[])?;
    Ok(rows.into_iter().map(|r| decorate(r, today)).collect())
}

pub fn get(conn: &Connection, id: &str, today: NaiveDate) -> Result<Value> {
    let row = query_one(conn, &format!("{SELECT} WHERE r.id = ?"), &[json!(id)])?.ok_or_else(|| Error::Invalid("no such recurring payment".into()))?;
    Ok(decorate(row, today))
}

/// Every occurrence of every active item between `from` and `to` (inclusive), for calendars and
/// the "coming up" list. Paid ones (before `next_due_date`) aren't included.
pub fn upcoming(conn: &Connection, from: NaiveDate, to: NaiveDate) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for row in query(conn, &format!("{SELECT} WHERE r.is_active = 1 AND r.next_due_date IS NOT NULL"), &[])? {
        let rule = rule_of(&row)?;
        let mut d = next_on_or_after(rule.start, rule.freq, rule.every, rule.end, date(row["next_due_date"].as_str().unwrap_or_default())?.max(from));
        while let Some(due) = d.filter(|d| *d <= to) {
            let cur = row["currency"].as_str().unwrap_or("INR");
            out.push(json!({ "recurring_id": row["id"], "name": row["name"], "type": row["type"], "date": due.to_string(),
                "amount": money::to_decimal(row["amount_minor"].as_i64().unwrap_or(0), cur), "currency": cur,
                "category_icon": row["category_icon"], "category_name": row["category_name"], "account_name": row["account_name"] }));
            d = next_on_or_after(rule.start, rule.freq, rule.every, rule.end, due + Duration::days(1));
        }
    }
    out.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    Ok(out)
}

/// Checks and normalises a recurring payment from the form.
fn fields(conn: &Connection, v: &Value) -> Result<Vec<(&'static str, Value)>> {
    let name = require_str(v, "name")?.trim();
    if name.is_empty() || name.chars().count() > 80 { return Err(Error::Invalid("a name needs 1 to 80 characters".into())); }
    let kind = match str_field(v, "type").unwrap_or("expense") { k @ ("expense" | "income" | "transfer") => k, _ => return Err(Error::Invalid("unknown type".into())) };
    let account = require_str(v, "account_id").map_err(|_| Error::Invalid("choose an account".into()))?;
    let currency: String = conn.query_row("SELECT currency FROM accounts WHERE id = ?", [account], |r| r.get(0))
        .optional()?.ok_or_else(|| Error::Invalid("no such account".into()))?;
    let amount = money::parse_minor(v.get("amount").unwrap_or(&Value::Null), &currency)?;
    if amount <= 0 { return Err(Error::Invalid("the amount must be greater than zero".into())); }
    let to = if kind == "transfer" { Some(require_str(v, "to_account_id").map_err(|_| Error::Invalid("a transfer needs a destination account".into()))?) } else { None };
    if to == Some(account) { return Err(Error::Invalid("can't transfer to the same account".into())); }
    let freq = str_field(v, "frequency").unwrap_or("monthly");
    Frequency::parse(freq)?;
    let every = v.get("every").and_then(Value::as_u64).unwrap_or(1);
    if !(1..=366).contains(&every) { return Err(Error::Invalid("repeat every 1 to 366 periods".into())); }
    let start = date(require_str(v, "start_date")?)?;
    let end = str_field(v, "end_date").filter(|s| !s.is_empty()).map(date).transpose()?;
    if end.is_some_and(|e| e < start) { return Err(Error::Invalid("the end date is before the start".into())); }
    let reminder = v.get("reminder_days").and_then(Value::as_i64).unwrap_or(3);
    if !(0..=60).contains(&reminder) { return Err(Error::Invalid("remind 0 to 60 days before".into())); }
    let vis = match str_field(v, "visibility").unwrap_or("private") { x @ ("private" | "shared" | "public") => x, _ => return Err(Error::Invalid("unknown visibility".into())) };
    Ok(vec![
        ("name", json!(name)), ("type", json!(kind)), ("amount_minor", json!(amount)), ("currency", json!(currency)),
        ("account_id", json!(account)), ("to_account_id", json!(to)),
        ("category_id", json!(if kind == "transfer" { None } else { str_field(v, "category_id") })),
        ("merchant", json!(str_field(v, "merchant"))), ("notes", json!(str_field(v, "notes"))),
        ("tags", json!(transactions::normalize_tags(v.get("tags"))?)),
        ("frequency", json!(freq)), ("every", json!(every)), ("start_date", json!(start.to_string())),
        ("end_date", json!(end.map(|e| e.to_string()))), ("reminder_days", json!(reminder)),
        ("auto_pay", json!(bool_field(v, "auto_pay", false))), ("visibility", json!(vis)),
    ])
}

pub fn create(conn: &Connection, v: &Value, today: NaiveDate) -> Result<Value> {
    let f = fields(conn, v)?;
    let id = new_id();
    let rule = rule_of(&Value::Object(f.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()))?;
    // New items start at their first date (which may be in the past, so it shows as overdue).
    let next = next_on_or_after(rule.start, rule.freq, rule.every, rule.end, rule.start);
    let cols: Vec<&str> = f.iter().map(|(k, _)| *k).collect();
    let sql = format!("INSERT INTO recurring_payments (id, next_due_date, {}) VALUES (?, ?, {})", cols.join(", "), vec!["?"; cols.len()].join(", "));
    let mut p: Vec<Value> = vec![json!(id), json!(next.map(|d| d.to_string()))];
    p.extend(f.into_iter().map(|(_, v)| v));
    conn.execute(&sql, rusqlite::params_from_iter(p.iter().map(bind)))?;
    get(conn, &id, today)
}

/// Edits an item. Changing the schedule recomputes the next due date from the last one paid.
pub fn update(conn: &Connection, id: &str, v: &Value, today: NaiveDate) -> Result<Value> {
    let old = get(conn, id, today)?;
    let f = fields(conn, v)?;
    let rule = rule_of(&Value::Object(f.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()))?;
    let after = old["last_paid_date"].as_str().map(date).transpose()?.map(|d| d + Duration::days(1)).unwrap_or(rule.start);
    let next = next_on_or_after(rule.start, rule.freq, rule.every, rule.end, after);
    let sets: Vec<String> = f.iter().map(|(k, _)| format!("{k} = ?")).collect();
    let sql = format!("UPDATE recurring_payments SET {}, next_due_date = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?", sets.join(", "));
    let mut p: Vec<Value> = f.into_iter().map(|(_, v)| v).collect();
    p.push(json!(next.map(|d| d.to_string())));
    p.push(json!(id));
    conn.execute(&sql, rusqlite::params_from_iter(p.iter().map(bind)))?;
    get(conn, id, today)
}

pub fn set_active(conn: &Connection, id: &str, active: bool, today: NaiveDate) -> Result<Value> {
    conn.execute("UPDATE recurring_payments SET is_active = ? WHERE id = ?", params![active as i64, id])?;
    get(conn, id, today)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    // Transactions already created stay; they just lose the link.
    conn.execute("UPDATE entries SET recurring_id = NULL WHERE recurring_id = ?", [id])?;
    conn.execute("DELETE FROM recurring_payments WHERE id = ?", [id])?;
    Ok(())
}

/// Moves the next due date past `due` without recording a payment.
fn advance(conn: &Connection, row: &Value, due: NaiveDate, paid: bool) -> Result<()> {
    let rule = rule_of(row)?;
    let next = next_on_or_after(rule.start, rule.freq, rule.every, rule.end, due + Duration::days(1));
    if paid {
        conn.execute("UPDATE recurring_payments SET next_due_date = ?, last_paid_date = ? WHERE id = ?",
            params![next.map(|d| d.to_string()), due.to_string(), row["id"].as_str()])?;
    } else {
        conn.execute("UPDATE recurring_payments SET next_due_date = ? WHERE id = ?", params![next.map(|d| d.to_string()), row["id"].as_str()])?;
    }
    Ok(())
}

/// Records the current due occurrence as paid: creates the transaction (on `date`, default the
/// due date; `amount` overrides this time only) and moves on to the next due date.
pub fn mark_paid(conn: &Connection, id: &str, date_override: Option<&str>, amount: Option<&Value>, today: NaiveDate) -> Result<Value> {
    atomic(conn, |tx| {
        let row = get(tx, id, today)?;
        let due = row["next_due_date"].as_str().map(date).transpose()?.ok_or_else(|| Error::Invalid("this has no more due dates".into()))?;
        let on = date_override.map(date).transpose()?.unwrap_or(due);
        let cur = row["currency"].as_str().unwrap_or("INR");
        let t = json!({
            "type": row["type"], "account_id": row["account_id"], "to_account_id": row["to_account_id"], "category_id": row["category_id"],
            "amount": amount.cloned().unwrap_or_else(|| json!(money::to_decimal(row["amount_minor"].as_i64().unwrap_or(0), cur).to_string())),
            "date": on.to_string(), "merchant": row["merchant"].as_str().or(row["name"].as_str()), "notes": row["notes"], "tags": row["tags"],
            "visibility": row["visibility"], "recurring_id": id,
        });
        if row["account_id"].is_null() { return Err(Error::Invalid("this recurring payment's account was archived; edit it to choose another".into())); }
        let created = transactions::create(tx, &t)?;
        advance(tx, &row, due, true)?;
        Ok(created)
    })
}

/// Skips the current due occurrence (no transaction).
pub fn skip(conn: &Connection, id: &str, today: NaiveDate) -> Result<Value> {
    let row = get(conn, id, today)?;
    let due = row["next_due_date"].as_str().map(date).transpose()?.ok_or_else(|| Error::Invalid("this has no more due dates".into()))?;
    advance(conn, &row, due, false)?;
    get(conn, id, today)
}

/// Creates the transactions for auto-pay items that fell due on or before `today` (up to a limit
/// per item) and returns them. Called when the member signs in.
pub fn run_auto_pay(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let due: Vec<String> = conn.prepare("SELECT id FROM recurring_payments WHERE auto_pay = 1 AND is_active = 1 AND next_due_date <= ?")?
        .query_map([today.to_string()], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    let mut made = Vec::new();
    for id in due {
        for _ in 0..CATCH_UP_LIMIT {
            let next: Option<String> = conn.query_row("SELECT next_due_date FROM recurring_payments WHERE id = ?", [&id], |r| r.get(0))?;
            if next.as_deref().is_none_or(|d| d > today.to_string().as_str()) { break; }
            match mark_paid(conn, &id, None, None, today) {
                Ok(t) => made.push(t),
                Err(_) => break, // e.g. its account was archived; it shows as overdue instead
            }
        }
    }
    Ok(made)
}

/// Transactions created from one recurring payment, newest first.
pub fn history(conn: &Connection, id: &str) -> Result<Vec<Value>> {
    transactions::get_all(conn, Some(&json!({ "recurringId": id, "limit": 24 })))
}
