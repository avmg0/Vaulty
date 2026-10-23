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
    /// bill, income, transfer, card, budget, price (went up), owed (someone owes you; `days` since you
    /// last dealt), task, expiry (a document), low (an account below, or heading below, its low-balance level; `amount` is
    /// the balance now or the lowest coming)
    pub kind: String,
    pub name: String,
    pub date: String,
    /// Days from today (negative = overdue). For budgets, 0.
    pub days: i64,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    /// Budgets: 80 or 100. Price rises: the rise in percent, rounded.
    pub percent: Option<i64>,
    /// Price rises: what it cost before (`amount` is the new price). Low balance: the level set.
    /// Card payments: the minimum due, when less than the statement (`amount`).
    pub previous: Option<f64>,
    /// Low balance: the payment that would take it below (none when already below).
    pub detail: Option<String>,
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
            date: date.into(), days, amount: r["amount"].as_f64(), currency: r["currency"].as_str().map(str::to_string), percent: None, previous: None, detail: None,
        });
    }
    // Credit cards: what the latest statement asks for, by its due date (overdue ones too). Cards
    // without statement days: everything owed, by the next due day.
    for c in crate::cards::list(conn)? {
        if c["kind"] != "credit" { continue; }
        let id = c["id"].as_str().unwrap_or_default();
        let cur = c["currency"].as_str().unwrap_or("INR");
        let (date, left, min_left) = match crate::card_cycles::next_due(conn, id, today)? {
            Some(x) => x,
            None if c["statement_day"].is_null() => {
                let (Some(due_day), Some(owed)) = (c["due_day"].as_i64(), c["owed_minor"].as_i64()) else { continue };
                if owed <= 0 { continue; }
                let next = (0..3).filter_map(|m| {
                    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)? + chrono::Months::new(m);
                    (1..=due_day as u32).rev().find_map(|d| NaiveDate::from_ymd_opt(first.year(), first.month(), d))
                }).find(|d| *d >= today);
                let Some(date) = next else { continue };
                (date, owed, owed)
            }
            None => continue,
        };
        let days = (date - today).num_days();
        if !(-OVERDUE_DAYS..=CARD_DAYS).contains(&days) { continue; }
        out.push(Due {
            key: format!("card:{id}:{date}:{}", stage(days)), kind: "card".into(),
            name: c["name"].as_str().unwrap_or_default().into(), date: date.to_string(), days,
            amount: Some(money::to_decimal(left, cur)), currency: Some(cur.into()), percent: None,
            previous: (min_left < left).then(|| money::to_decimal(min_left, cur)), detail: None,
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
                amount: b["amount"].as_f64(), currency: b["currency"].as_str().map(str::to_string), percent: None, previous: None, detail: None,
            });
        }
    }
    // To-dos due today, or overdue (once each way).
    for t in crate::tasks::list(conn, "open", today)? {
        let Some(due) = t["due_date"].as_str().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()) else { continue };
        let days = (due - today).num_days();
        if !(-OVERDUE_DAYS..=0).contains(&days) { continue; }
        out.push(Due {
            key: format!("task:{}:{due}:{}", t["id"].as_str().unwrap_or_default(), if days == 0 { "today" } else { "overdue" }), kind: "task".into(),
            name: t["title"].as_str().unwrap_or_default().into(), date: due.to_string(), days, amount: None, currency: None, percent: None, previous: None, detail: None,
        });
    }
    // Documents: once when expiry comes within the member's chosen days, and once on or after the
    // day (for up to 30 days after it).
    for d in crate::documents::list(conn, today)? {
        let (Some(exp), Some(days)) = (d["expires_on"].as_str(), d["days_left"].as_i64()) else { continue };
        let ahead = d["remind_days"].as_i64().unwrap_or(30);
        let stage = if (-30..=0).contains(&days) { "due" } else if (1..=ahead).contains(&days) { "soon" } else { continue };
        out.push(Due {
            key: format!("expiry:{}:{exp}:{stage}", d["id"].as_str().unwrap_or_default()), kind: "expiry".into(),
            name: d["title"].as_str().unwrap_or_default().into(), date: exp.into(), days, amount: None, currency: None, percent: None, previous: None, detail: None,
        });
    }
    // Advance tax instalments with something still to pay: a week ahead, on the day, and late.
    for (label, date, short, cur) in crate::tax_plan::advance_due(conn, today)? {
        let days = (date - today).num_days();
        if !(-15..=7).contains(&days) { continue; }
        out.push(Due {
            key: format!("tax:{label}:{date}:{}", stage(days)), kind: "tax".into(), name: label, date: date.to_string(), days,
            amount: Some(money::to_decimal(short, &cur)), currency: Some(cur), percent: None, previous: None, detail: None,
        });
    }
    // Price rises in the last 30 days: subscriptions Vaulty spotted and recurring payments.
    let recent = |date: &str| NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok_and(|d| (today - d).num_days() <= 30);
    let rise = |key: String, name: &str, cur: &str, c: &crate::subscriptions::PriceChange| Due {
        key, kind: "price".into(), name: name.into(), date: c.date.clone(), days: 0, amount: Some(c.to), currency: Some(cur.into()),
        percent: Some(c.percent.round() as i64), previous: Some(c.from), detail: None,
    };
    for s in crate::subscriptions::detect(conn, today)? {
        if let Some(c) = s.price_change.as_ref().filter(|c| recent(&c.date)) { out.push(rise(format!("price:{}:{}:{}", s.key, c.date, c.to), &s.name, &s.currency, c)); }
    }
    for r in crate::subscriptions::recurring_rises(conn)? {
        if recent(&r.change.date) { out.push(rise(format!("price:{}:{}:{}", r.recurring_id, r.change.date, r.change.to), &r.name, &r.currency, &r.change)); }
    }
    // Accounts below their low-balance level now (once per dip), or a bill taking them below.
    for l in crate::reports::low_balances(conn, today)? {
        let days = NaiveDate::parse_from_str(&l.date, "%Y-%m-%d").map(|d| (d - today).num_days()).unwrap_or(0);
        out.push(Due {
            key: if l.now { format!("low:{}:now", l.account_id) } else { format!("low:{}:{}:{}", l.account_id, l.date, l.because.as_deref().unwrap_or_default()) },
            kind: "low".into(), name: l.name, date: l.date, days,
            amount: Some(if l.now { l.balance } else { l.lowest }), currency: Some(l.currency), percent: None, previous: Some(l.level), detail: l.because,
        });
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
            amount: c["spent"].as_f64(), currency: Some(cur.clone()), percent: Some(level), previous: None, detail: None,
        });
    }
    Ok(out)
}

/// The reminders not shown yet, marked as shown. A budget that already got its 100% reminder
/// doesn't also get the 80% one.
pub fn take_due(conn: &Connection, today: NaiveDate) -> Result<Vec<Due>> {
    if crate::settings::get(conn)?["notifications"] == "off" { return Ok(vec![]); }
    conn.execute("DELETE FROM reminders_sent WHERE at < datetime('now', '-120 days')", [])?;
    let all = due(conn, today)?;
    // An account back above its level is reminded again the next time it dips.
    let below: Vec<&str> = all.iter().filter(|d| d.kind == "low" && d.key.ends_with(":now")).map(|d| d.key.as_str()).collect();
    let shown: Vec<String> = conn.prepare("SELECT key FROM reminders_sent WHERE key LIKE 'low:%:now'")?
        .query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    for k in shown.iter().filter(|k| !below.contains(&k.as_str())) { conn.execute("DELETE FROM reminders_sent WHERE key = ?", [k])?; }
    let mut fresh = vec![];
    for d in all {
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
