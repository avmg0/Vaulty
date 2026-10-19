//! What to remind the member about now: bills and income coming up (each bill's own "remind N
//! days before"), credit card payments due within a few days, and budgets reaching 80% and 100%.
//! Each reminder is handed out once (`take_due`); the window words it in the member's language
//! and shows it as a system notification.
use crate::{error::Result, money, recurring};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;

/// Card payments are reminded about this many days ahead.
pub const CARD_DAYS: i64 = 3;
/// Overdue bills are reminded about for this long, then left to the calendar.
pub const OVERDUE_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Due {
    /// Stable id: the same reminder is never shown twice.
    pub key: String,
    /// bill, income, transfer, card, budget, owed (someone owes you; `days` since you last dealt)
    pub kind: String,
    pub name: String,
    pub date: String,
    /// Days from today (negative = overdue). For budgets, 0.
    pub days: i64,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    /// Budgets: 80 or 100.
    pub percent: Option<i64>,
}

fn stage(days: i64) -> &'static str {
    match days { d if d < 0 => "overdue", 0 => "today", _ => "soon" }
}

/// Everything worth a reminder today, whether or not it was shown already.
pub fn due(conn: &Connection, today: NaiveDate) -> Result<Vec<Due>> {
    let mut out = vec![];
    for r in recurring::list(conn, today)? {
        if r["is_active"] != 1 { continue; }
        let (Some(date), Some(days)) = (r["next_due_date"].as_str(), r["days_until"].as_i64()) else { continue };
        let ahead = r["reminder_days"].as_i64().unwrap_or(3);
        // Auto-paid items are recorded by Vaulty; nobody needs reminding to pay them.
        if r["auto_pay"] == 1 || days > ahead || days < -OVERDUE_DAYS { continue; }
        let kind = match r["type"].as_str() { Some("income") => "income", Some("transfer") => "transfer", _ => "bill" };
        let id = r["id"].as_str().unwrap_or_default();
        out.push(Due {
            key: format!("{kind}:{id}:{date}:{}", stage(days)), kind: kind.into(), name: r["name"].as_str().unwrap_or_default().into(),
            date: date.into(), days, amount: r["amount"].as_f64(), currency: r["currency"].as_str().map(str::to_string), percent: None,
        });
    }
    // Credit cards: the next due date, if something is owed.
    for c in crate::cards::list(conn)? {
        if c["kind"] != "credit" { continue; }
        let (Some(due_day), Some(owed)) = (c["due_day"].as_i64(), c["owed_minor"].as_i64()) else { continue };
        if owed <= 0 { continue; }
        let next = (0..3).filter_map(|m| {
            let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)? + chrono::Months::new(m);
            (1..=due_day as u32).rev().find_map(|d| NaiveDate::from_ymd_opt(first.year(), first.month(), d))
        }).find(|d| *d >= today);
        let Some(date) = next else { continue };
        let days = (date - today).num_days();
        if days > CARD_DAYS { continue; }
        let cur = c["currency"].as_str().unwrap_or("INR");
        out.push(Due {
            key: format!("card:{}:{date}:{}", c["id"].as_str().unwrap_or_default(), stage(days)), kind: "card".into(),
            name: c["name"].as_str().unwrap_or_default().into(), date: date.to_string(), days,
            amount: Some(money::to_decimal(owed, cur)), currency: Some(cur.into()), percent: None,
        });
    }
    // Money owed to you with nothing happening for 30 days: once per quiet spell.
    for p in crate::people::overdue(conn, today)? {
        for b in p["balances"].as_array().into_iter().flatten().filter(|b| b["amount_minor"].as_i64().unwrap_or(0) > 0) {
            let last = p["last_date"].as_str().unwrap_or_default();
            out.push(Due {
                key: format!("owed:{}:{}:{last}", p["id"].as_str().unwrap_or_default(), b["currency"].as_str().unwrap_or_default()),
                kind: "owed".into(), name: p["name"].as_str().unwrap_or_default().into(), date: last.into(),
                days: NaiveDate::parse_from_str(last, "%Y-%m-%d").map(|d| (today - d).num_days()).unwrap_or(0),
                amount: b["amount"].as_f64(), currency: b["currency"].as_str().map(str::to_string), percent: None,
            });
        }
    }
    // Budgets this month: the highest level reached.
    let month = format!("{:04}-{:02}", today.year(), today.month());
    let b = crate::budgets::month(conn, &month)?;
    let cur = b["currency"].as_str().unwrap_or("INR").to_string();
    for c in b["categories"].as_array().into_iter().flatten() {
        let (Some(budget), Some(spent)) = (c["budget_minor"].as_i64(), c["spent_minor"].as_i64()) else { continue };
        if budget <= 0 || c["type"] != "expense" { continue; }
        let pct = spent * 100 / budget;
        let level = if pct >= 100 { 100 } else if pct >= 80 { 80 } else { continue };
        out.push(Due {
            key: format!("budget:{}:{month}:{level}", c["id"].as_str().unwrap_or_default()), kind: "budget".into(),
            name: c["name"].as_str().unwrap_or_default().into(), date: today.to_string(), days: 0,
            amount: c["spent"].as_f64(), currency: Some(cur.clone()), percent: Some(level),
        });
    }
    Ok(out)
}

/// The reminders not shown yet, marked as shown. A budget that already got its 100% reminder
/// doesn't also get the 80% one.
pub fn take_due(conn: &Connection, today: NaiveDate) -> Result<Vec<Due>> {
    if crate::settings::get(conn)?["notifications"] == "off" { return Ok(vec![]); }
    conn.execute("DELETE FROM reminders_sent WHERE at < datetime('now', '-120 days')", [])?;
    let mut fresh = vec![];
    for d in due(conn, today)? {
        if conn.execute("INSERT OR IGNORE INTO reminders_sent (key) VALUES (?)", [&d.key])? == 0 { continue; }
        if d.percent == Some(100) {
            let eighty = d.key.replace(":100", ":80");
            conn.execute("INSERT OR IGNORE INTO reminders_sent (key) VALUES (?)", [&eighty])?;
        }
        fresh.push(d);
    }
    Ok(fresh)
}

/// Payment due dates over the next two months, for reminders while locked (dates only).
pub fn locked_schedule(conn: &Connection, today: NaiveDate) -> Result<Vec<NaiveDate>> {
    let s = crate::settings::get(conn)?;
    if s["notifications"] == "off" || s["notify_locked"] != 1 { return Ok(vec![]); }
    let mut dates: Vec<NaiveDate> = crate::calendar::range(conn, &today.to_string(), &(today + Duration::days(60)).to_string(), "XX", today)?
        .into_iter().filter(|i| matches!(i.kind.as_str(), "bill" | "card_due") && !i.overdue)
        .filter_map(|i| NaiveDate::parse_from_str(&i.date, "%Y-%m-%d").ok()).collect();
    dates.sort();
    dates.dedup();
    Ok(dates)
}

/// For tests: how many reminders are recorded as shown.
#[doc(hidden)]
pub fn shown_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM reminders_sent", params![], |r| r.get(0))?)
}
