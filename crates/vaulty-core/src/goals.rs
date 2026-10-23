//! Goals: a house down payment, a child's education, an emergency fund, a trip. Each has a
//! target (optionally in today's money, grown by inflation to the date), a date, what's saved
//! (entered by hand, plus a share of accounts and investments set aside for it), and the plan:
//! a monthly amount and the growth expected. From those: progress, the monthly saving needed,
//! where the plan lands by the date, and whether it's on track.
use crate::{calc, error::{Error, Result}, investments, ledger, money, rows::*};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashMap;

const MAX_GOALS: i64 = 200;
const MAX_LINKS: usize = 20;
fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

fn text(v: &Value, key: &str, max: usize) -> Result<Option<String>> {
    match v[key].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) if s.chars().count() > max => Err(bad("one of the details is too long")),
        s => Ok(s.map(String::from)),
    }
}
fn minor(v: &Value, key: &str, cur: &str) -> Result<i64> {
    match &v[key] {
        Value::Null => Ok(0),
        Value::String(s) if s.trim().is_empty() => Ok(0),
        x => { let m = money::parse_minor(x, cur)?; if m < 0 { Err(bad("amounts can't be negative")) } else { Ok(m) } }
    }
}
fn pct(v: &Value, key: &str) -> Result<f64> {
    let n = match &v[key] { Value::Null => 0.0, Value::Number(n) => n.as_f64().unwrap_or(-1.0), Value::String(s) if s.trim().is_empty() => 0.0, Value::String(s) => s.trim().parse().unwrap_or(-1.0), _ => -1.0 };
    if !(0.0..=30.0).contains(&n) { return Err(bad("rates must be between 0% and 30%")); }
    Ok(n)
}
fn date(v: &Value, key: &str) -> Result<Option<String>> {
    match v[key].as_str().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map(|d| Some(d.to_string())).map_err(|_| bad("give the date as YYYY-MM-DD")),
    }
}

/// Months from `today` to `to`, counting part of a month as a month; never negative.
fn months_between(today: NaiveDate, to: NaiveDate) -> i64 {
    let m = (to.year() - today.year()) as i64 * 12 + to.month() as i64 - today.month() as i64 - i64::from(to.day() < today.day());
    m.max(0)
}

/// What a link adds now: a share of an account's balance or an investment's value, in minor units.
fn link_value(conn: &Connection, l: &Value) -> Result<(i64, String, String)> {
    let share = l["share"].as_i64().unwrap_or(100);
    if let Some(a) = l["account_id"].as_str() {
        let (lid, kind, cur) = ledger::for_account(conn, a)?;
        let bal = if kind == ledger::Kind::Asset { ledger::balance(conn, &lid, true)?.max(0) } else { 0 };
        let name: String = conn.query_row("SELECT name FROM accounts WHERE id = ?", [a], |r| r.get(0))?;
        return Ok((bal * share / 100, cur, name));
    }
    let h = l["holding_id"].as_str().unwrap_or_default();
    let (units, price, cur, name): (i64, i64, String, String) = conn.query_row(
        "SELECT units_e4, price_e4, currency, name FROM holdings WHERE id = ?", [h], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok((investments::value_minor(units, price, &cur).max(0) * share / 100, cur, name))
}

fn check_links(conn: &Connection, cur: &str, links: &[Value]) -> Result<()> {
    if links.len() > MAX_LINKS { return Err(bad("link at most 20 accounts and investments")); }
    for l in links {
        let share = l["share"].as_i64().unwrap_or(100);
        if !(1..=100).contains(&share) { return Err(bad("a share is between 1% and 100%")); }
        let c: Option<String> = match (l["account_id"].as_str(), l["holding_id"].as_str()) {
            (Some(a), None) => {
                let (_, kind, c) = ledger::for_account(conn, a).map_err(|_| bad("no such account"))?;
                if kind != ledger::Kind::Asset { return Err(bad("money for a goal is kept in a bank, cash or savings account, not a card or loan")); }
                Some(c)
            }
            (None, Some(h)) => conn.query_row("SELECT currency FROM holdings WHERE id = ?", [h], |r| r.get(0)).optional()?,
            _ => return Err(bad("link an account or an investment")),
        };
        match c { None => return Err(bad("no such investment")), Some(c) if c != cur => return Err(bad("that's in another currency than the goal")), _ => {} }
    }
    Ok(())
}

fn write(conn: &Connection, id: &str, v: &Value, insert: bool) -> Result<()> {
    let name = text(v, "name", 80)?.ok_or_else(|| bad("give the goal a name"))?;
    let cur = v["currency"].as_str().filter(|c| c.len() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase())).ok_or_else(|| bad("choose a currency"))?.to_string();
    let target = minor(v, "target", &cur)?;
    if target <= 0 { return Err(bad("give the amount you're aiming for")); }
    let links = v["links"].as_array().cloned().unwrap_or_default();
    check_links(conn, &cur, &links)?;
    let priority = v["priority"].as_i64().unwrap_or(2);
    if !(1..=3).contains(&priority) { return Err(bad("unknown priority")); }
    let row = params![id, name, text(v, "icon", 8)?, cur, target, date(v, "target_date")?, minor(v, "saved", &cur)?, minor(v, "monthly", &cur)?,
        pct(v, "return_rate")?, pct(v, "inflation")?, priority, text(v, "notes", 1000)?];
    if insert {
        conn.execute("INSERT INTO goals (id, name, icon, currency, target_minor, target_date, saved_minor, monthly_minor, return_rate, inflation, priority, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", row)?;
    } else if conn.execute("UPDATE goals SET name = ?2, icon = ?3, currency = ?4, target_minor = ?5, target_date = ?6, saved_minor = ?7, monthly_minor = ?8,
            return_rate = ?9, inflation = ?10, priority = ?11, notes = ?12, updated_at = CURRENT_TIMESTAMP WHERE id = ?1", row)? == 0 {
        return Err(bad("no such goal"));
    }
    // Links are replaced only when given.
    if v.get("links").is_some() {
        conn.execute("DELETE FROM goal_links WHERE goal_id = ?", [id])?;
        for l in &links {
            conn.execute("INSERT INTO goal_links (id, goal_id, account_id, holding_id, share) VALUES (?, ?, ?, ?, ?)",
                params![new_id(), id, l["account_id"].as_str(), l["holding_id"].as_str(), l["share"].as_i64().unwrap_or(100)])?;
        }
    }
    Ok(())
}

pub fn create(conn: &Connection, v: &Value) -> Result<Value> {
    let n: i64 = conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
    if n >= MAX_GOALS { return Err(bad("you can keep up to 200 goals")); }
    let id = new_id();
    atomic(conn, |tx| write(tx, &id, v, true))?;
    Ok(json!({ "id": id }))
}

pub fn update(conn: &Connection, id: &str, v: &Value) -> Result<()> { atomic(conn, |tx| write(tx, id, v, false)) }

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM goals WHERE id = ?", [id])? == 0 { return Err(bad("no such goal")); }
    Ok(())
}

/// Marks a goal reached (or open again).
pub fn set_done(conn: &Connection, id: &str, done: bool, today: NaiveDate) -> Result<()> {
    let on = done.then(|| today.to_string());
    if conn.execute("UPDATE goals SET done_on = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?", params![on, id])? == 0 { return Err(bad("no such goal")); }
    Ok(())
}

/// Every goal with its progress and plan: `saved` (by hand plus linked money), `target_then`
/// (grown by inflation), `progress`, `months_left`, `needed_monthly`, `projected` (by the date on
/// the planned monthly amount), `status` (done, reached, on_track, behind, overdue, no_date),
/// and the links with what each adds. A link marked `over` shares an account or investment that
/// goals together claim more than 100% of.
pub fn list(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let goals = query(conn, "SELECT * FROM goals ORDER BY done_on IS NOT NULL, priority, coalesce(target_date, '9999'), name COLLATE NOCASE", &[])?;
    let links = query(conn, "SELECT l.* FROM goal_links l JOIN goals g ON g.id = l.goal_id ORDER BY l.rowid", &[])?;
    // How much of each account or investment open goals claim.
    let mut claimed: HashMap<String, i64> = HashMap::new();
    let open: Vec<&str> = goals.iter().filter(|g| g["done_on"].is_null()).filter_map(|g| g["id"].as_str()).collect();
    for l in links.iter().filter(|l| open.contains(&l["goal_id"].as_str().unwrap_or_default())) {
        let key = l["account_id"].as_str().or(l["holding_id"].as_str()).unwrap_or_default().to_string();
        *claimed.entry(key).or_default() += l["share"].as_i64().unwrap_or(100);
    }
    goals.into_iter().map(|mut g| {
        let id = g["id"].as_str().unwrap_or_default().to_string();
        let cur = g["currency"].as_str().unwrap_or("INR").to_string();
        let d = |m: f64| money::to_decimal(m.round() as i64, &cur);
        let mut linked = 0i64;
        let mut out_links = vec![];
        for l in links.iter().filter(|l| l["goal_id"] == id.as_str()) {
            let (value, _, name) = link_value(conn, l)?;
            linked += value;
            let key = l["account_id"].as_str().or(l["holding_id"].as_str()).unwrap_or_default();
            out_links.push(json!({ "account_id": l["account_id"], "holding_id": l["holding_id"], "share": l["share"], "name": name,
                "value": money::to_decimal(value, &cur), "over": claimed.get(key).copied().unwrap_or(0) > 100 }));
        }
        let manual = g["saved_minor"].as_i64().unwrap_or(0);
        let saved = manual + linked;
        let target = g["target_minor"].as_i64().unwrap_or(0) as f64;
        let monthly = g["monthly_minor"].as_i64().unwrap_or(0) as f64;
        let (rate, inflation) = (g["return_rate"].as_f64().unwrap_or(0.0), g["inflation"].as_f64().unwrap_or(0.0));
        let due = g["target_date"].as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let months = due.map(|d| months_between(today, d));
        let then = match months { Some(m) if inflation > 0.0 => target * (1.0 + inflation / 100.0).powf(m as f64 / 12.0), _ => target };
        let progress = if then > 0.0 { (saved as f64 / then).min(1.0) } else { 0.0 };
        let needed = months.map(|m| calc::monthly_needed(then, saved as f64, m as f64, rate));
        let projected = months.map(|m| calc::projected(saved as f64, monthly, m as f64, rate));
        let status = if !g["done_on"].is_null() { "done" }
            else if saved as f64 >= then { "reached" }
            else if due.is_some_and(|d| d < today) { "overdue" }
            else { match projected { None => "no_date", Some(p) if p + 0.5 >= then => "on_track", _ => "behind" } };
        let o = g.as_object_mut().ok_or_else(|| bad("bad goal"))?;
        o.insert("target".into(), json!(d(target)));
        o.insert("target_then".into(), json!(d(then)));
        o.insert("saved_by_hand".into(), json!(money::to_decimal(manual, &cur)));
        o.insert("saved".into(), json!(money::to_decimal(saved, &cur)));
        o.insert("monthly".into(), json!(d(monthly)));
        o.insert("progress".into(), json!(progress));
        o.insert("months_left".into(), json!(months));
        o.insert("needed_monthly".into(), json!(needed.map(d)));
        o.insert("projected".into(), json!(projected.map(d)));
        o.insert("status".into(), json!(status));
        o.insert("links".into(), json!(out_links));
        Ok(g)
    }).collect()
}

/// Totals per currency for the open goals: aimed for, saved, and planned each month.
pub fn summary(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let mut by: std::collections::BTreeMap<String, (f64, f64, f64, i64)> = Default::default();
    for g in list(conn, today)?.iter().filter(|g| g["done_on"].is_null()) {
        let e = by.entry(g["currency"].as_str().unwrap_or("INR").to_string()).or_default();
        e.0 += g["target_then"].as_f64().unwrap_or(0.0);
        e.1 += g["saved"].as_f64().unwrap_or(0.0).min(g["target_then"].as_f64().unwrap_or(0.0));
        e.2 += g["monthly"].as_f64().unwrap_or(0.0);
        e.3 += 1;
    }
    Ok(by.into_iter().map(|(c, (t, s, m, n))| json!({ "currency": c, "target": t, "saved": s, "monthly": m, "goals": n })).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn months() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        assert_eq!(months_between(d("2026-10-23"), d("2027-10-23")), 12);
        assert_eq!(months_between(d("2026-10-23"), d("2027-10-22")), 11);
        assert_eq!(months_between(d("2026-10-23"), d("2026-01-01")), 0);
    }
}
