//! Reports: income and spending by month, spending by category and by place, a cash-flow
//! forecast for everyday money, and "safe to spend" until the next income.
//!
//! Everything is in the profile's main currency; amounts in other currencies are counted
//! separately so nothing is silently mixed. Transfers between one's own accounts are neither
//! income nor spending. Hidden transactions count in totals but never show their names.
use crate::{error::{Error, Result}, money, recurring};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::BTreeMap;

/// Accounts whose money can pay a bill today.
pub const LIQUID: &[&str] = &["checking", "savings", "cash", "business"];
/// How many days of past spending set the "typical day" in the forecast.
pub const TYPICAL_DAYS: i64 = 90;
pub const MAX_FORECAST_DAYS: i64 = 180;

fn main_currency(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get::<_, Option<String>>(0))?.unwrap_or_else(|| "INR".into()))
}

fn day(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("`{s}` isn't a date (YYYY-MM-DD)")))
}

fn checked_range(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate)> {
    let (f, t) = (day(from)?, day(to)?);
    if t < f { return Err(Error::Invalid("the end date is before the start".into())); }
    if (t - f).num_days() > 3700 { return Err(Error::Invalid("pick a range of at most ten years".into())); }
    Ok((f, t))
}

/// Spending (expense postings) and income (income postings, made positive) per category ledger,
/// in the main currency, between two dates. The SQL shared by the reports below.
const FLOWS: &str = "
    SELECT e.date, l.kind, l.category_id, l.system_key, p.amount_minor, e.merchant, e.description, COALESCE(e.hidden, 0)
      FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id JOIN entries e ON e.id = p.entry_id
     WHERE l.kind IN ('expense', 'income') AND p.currency = ?3 AND e.date BETWEEN ?1 AND ?2";

struct Flow { date: String, kind: String, category: Option<String>, amount: i64, place: String }

fn flows(conn: &Connection, from: NaiveDate, to: NaiveDate, cur: &str) -> Result<Vec<Flow>> {
    let mut stmt = conn.prepare(FLOWS)?;
    let rows = stmt.query_map(params![from.to_string(), to.to_string(), cur], |r| {
        let kind: String = r.get(1)?;
        let amount: i64 = r.get(4)?;
        let hidden: i64 = r.get(7)?;
        let place: Option<String> = r.get::<_, Option<String>>(5)?.filter(|s| !s.trim().is_empty()).or(r.get(6)?);
        Ok(Flow {
            date: r.get(0)?,
            category: r.get::<_, Option<String>>(2)?.or(r.get::<_, Option<String>>(3)?),
            // Expenses are positive postings, income negative: report both as positive amounts.
            amount: if kind == "income" { -amount } else { amount },
            kind,
            place: if hidden == 1 { "\u{0}hidden".into() } else { place.unwrap_or_default().trim().to_string() },
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn other_currencies(conn: &Connection, from: NaiveDate, to: NaiveDate, cur: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(DISTINCT e.id) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id JOIN entries e ON e.id = p.entry_id
          WHERE l.kind IN ('expense', 'income') AND p.currency <> ?3 AND e.date BETWEEN ?1 AND ?2",
        params![from.to_string(), to.to_string(), cur], |r| r.get(0))?)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Month { pub month: String, pub income: f64, pub spending: f64, pub net: f64 }

#[derive(Debug, Serialize, PartialEq)]
pub struct Slice { pub id: Option<String>, pub name: String, pub icon: Option<String>, pub color: Option<String>, pub amount: f64, pub count: i64 }

#[derive(Debug, Serialize)]
pub struct Overview {
    pub currency: String,
    pub months: Vec<Month>,
    pub income: f64,
    pub spending: f64,
    /// Share of income kept (0–1), when there was income.
    pub savings_rate: Option<f64>,
    /// Spending by top-level category (sub-categories included in their parent), largest first.
    pub categories: Vec<Slice>,
    /// Income by top-level category.
    pub income_sources: Vec<Slice>,
    /// Where the money went: the 10 places with the most spending.
    pub places: Vec<Slice>,
    /// Transactions in other currencies, left out of the figures above.
    pub other_currencies: i64,
}

/// Everything the Reports page shows for a date range.
pub fn overview(conn: &Connection, from: &str, to: &str) -> Result<Overview> {
    let (from, to) = checked_range(from, to)?;
    let cur = main_currency(conn)?;
    let all = flows(conn, from, to, &cur)?;

    let mut months: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    let mut d = NaiveDate::from_ymd_opt(from.year(), from.month(), 1).unwrap_or(from);
    while d <= to { months.insert(d.format("%Y-%m").to_string(), (0, 0)); d = d + chrono::Months::new(1); }
    for f in &all {
        let e = months.entry(f.date[..7].to_string()).or_default();
        if f.kind == "income" { e.0 += f.amount } else { e.1 += f.amount }
    }
    let (income, spending): (i64, i64) = months.values().fold((0, 0), |a, v| (a.0 + v.0, a.1 + v.1));
    let dec = |m: i64| money::to_decimal(m, &cur);

    // Categories rolled up to their top level.
    // id -> (name, icon, colour, parent)
    type Cat = (String, Option<String>, Option<String>, Option<String>);
    let cats: BTreeMap<String, Cat> = conn
        .prepare("SELECT id, name, icon, color, parent_id FROM categories")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))))?
        .collect::<rusqlite::Result<_>>()?;
    let top = |id: &Option<String>| -> Option<String> {
        let id = id.as_ref()?;
        Some(cats.get(id).and_then(|c| c.3.clone()).filter(|p| cats.contains_key(p)).unwrap_or_else(|| id.clone()))
    };
    let slices = |kind: &str| -> Vec<Slice> {
        let mut by: BTreeMap<Option<String>, (i64, i64)> = BTreeMap::new();
        for f in all.iter().filter(|f| f.kind == kind) {
            let key = f.category.as_ref().filter(|c| cats.contains_key(*c)).and(top(&f.category));
            let e = by.entry(key).or_default();
            e.0 += f.amount; e.1 += 1;
        }
        let mut out: Vec<Slice> = by.into_iter().filter(|(_, (a, _))| *a > 0).map(|(id, (amount, count))| {
            let c = id.as_ref().and_then(|i| cats.get(i));
            Slice { name: c.map(|c| c.0.clone()).unwrap_or_else(|| "\u{0}uncategorised".into()), icon: c.and_then(|c| c.1.clone()), color: c.and_then(|c| c.2.clone()), id, amount: dec(amount), count }
        }).collect();
        out.sort_by(|a, b| b.amount.total_cmp(&a.amount));
        out
    };
    let mut places: BTreeMap<String, (String, i64, i64)> = BTreeMap::new();
    for f in all.iter().filter(|f| f.kind == "expense" && !f.place.is_empty()) {
        // Same place written differently ("Swiggy", "SWIGGY") counts once, shown as first seen.
        let e = places.entry(f.place.to_lowercase()).or_insert_with(|| (f.place.clone(), 0, 0));
        e.1 += f.amount; e.2 += 1;
    }
    let mut places: Vec<Slice> = places.into_values().map(|(name, a, n)| Slice { id: None, name, icon: None, color: None, amount: dec(a), count: n }).collect();
    places.sort_by(|a, b| b.amount.total_cmp(&a.amount));
    places.truncate(10);

    Ok(Overview {
        months: months.into_iter().map(|(month, (i, s))| Month { month, income: dec(i), spending: dec(s), net: dec(i - s) }).collect(),
        income: dec(income), spending: dec(spending),
        savings_rate: (income > 0).then(|| (income - spending) as f64 / income as f64),
        categories: slices("expense"), income_sources: slices("income"), places,
        other_currencies: other_currencies(conn, from, to, &cur)?,
        currency: cur,
    })
}

// ------------------------------------------------------------ forecast

#[derive(Debug, Serialize, PartialEq)]
pub struct Point {
    pub date: String,
    /// Today's balance plus what's scheduled up to this day.
    pub scheduled: f64,
    /// The same, also taking out a typical day's spending.
    pub expected: f64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Change { pub date: String, pub title: String, pub amount: f64, pub kind: String }

#[derive(Debug, Serialize)]
pub struct Forecast {
    pub currency: String,
    /// Everyday money now: bank and cash accounts in the main currency.
    pub balance: f64,
    /// Average spending per day over the last 90 days, leaving out bills Vaulty already schedules.
    pub typical_day: f64,
    pub points: Vec<Point>,
    /// Known money in and out, by date.
    pub changes: Vec<Change>,
    /// Lowest expected balance and when.
    pub lowest: Option<Point>,
    /// Bank and cash accounts in other currencies, not included.
    pub other_currencies: i64,
}

struct Liquid { ids: Vec<String>, balance: i64, others: i64 }

fn liquid(conn: &Connection, cur: &str) -> Result<Liquid> {
    let mut out = Liquid { ids: vec![], balance: 0, others: 0 };
    for a in crate::accounts::get_all(conn)? {
        if !LIQUID.contains(&a["type"].as_str().unwrap_or_default()) { continue; }
        if a["currency"].as_str() != Some(cur) { out.others += 1; continue; }
        out.ids.push(a["id"].as_str().unwrap_or_default().to_string());
        out.balance += a["current_balance_minor"].as_i64().unwrap_or(0);
    }
    Ok(out)
}

/// Known money in and out of everyday accounts from `today` to `to`: bills, income and transfers
/// from recurring payments, and the next credit card payment (assumed paid in full).
fn scheduled(conn: &Connection, today: NaiveDate, to: NaiveDate, cur: &str, liquid_ids: &[String]) -> Result<Vec<(NaiveDate, i64, String, String)>> {
    let mut out = vec![];
    let accounts: BTreeMap<String, (Option<String>, Option<String>)> = recurring::list(conn, today)?.into_iter()
        .map(|r| (r["id"].as_str().unwrap_or_default().to_string(), (r["account_id"].as_str().map(str::to_string), r["to_account_id"].as_str().map(str::to_string))))
        .collect();
    let is_liquid = |id: &Option<String>| id.as_ref().is_some_and(|i| liquid_ids.contains(i));
    for r in recurring::upcoming(conn, today, to)? {
        if r["currency"].as_str() != Some(cur) { continue; }
        let date = day(r["date"].as_str().unwrap_or_default())?.max(today);
        let amount = money::parse_minor(&r["amount"], cur).unwrap_or(0);
        let (from_acc, to_acc) = accounts.get(r["recurring_id"].as_str().unwrap_or_default()).cloned().unwrap_or_default();
        let sign = match r["type"].as_str() {
            Some("income") if is_liquid(&from_acc) => 1,
            Some("transfer") => (is_liquid(&to_acc) as i64) - (is_liquid(&from_acc) as i64),
            Some("expense") if is_liquid(&from_acc) => -1,
            _ => 0,
        };
        if sign != 0 { out.push((date, sign * amount, r["name"].as_str().unwrap_or_default().to_string(), r["type"].as_str().unwrap_or("expense").to_string())); }
    }
    for c in crate::cards::list(conn)? {
        if c["kind"] != "credit" || c["currency"].as_str() != Some(cur) { continue; }
        // With statement days: the latest statement's amount by its due date.
        if let Some((d, left, _)) = crate::card_cycles::next_due(conn, c["id"].as_str().unwrap_or_default(), today)? {
            if d <= to { out.push((d.max(today), -left, c["name"].as_str().unwrap_or_default().to_string(), "card".into())); }
            continue;
        }
        if !c["statement_day"].is_null() { continue; }
        let (Some(due_day), Some(owed)) = (c["due_day"].as_i64(), c["owed_minor"].as_i64()) else { continue };
        if owed <= 0 { continue; }
        let next = (0..3).filter_map(|m| {
            let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)? + chrono::Months::new(m);
            (1..=due_day as u32).rev().find_map(|d| NaiveDate::from_ymd_opt(first.year(), first.month(), d))
        }).find(|d| *d >= today && *d <= to);
        if let Some(d) = next { out.push((d, -owed, c["name"].as_str().unwrap_or_default().to_string(), "card".into())); }
    }
    out.sort_by_key(|x| x.0);
    Ok(out)
}

/// Average day-to-day spending from everyday accounts over the last 90 days, leaving out payments
/// made for recurring bills (they're already in the schedule).
fn typical_day(conn: &Connection, today: NaiveDate, cur: &str) -> Result<i64> {
    let from = today - Duration::days(TYPICAL_DAYS);
    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(p.amount_minor), 0) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id JOIN entries e ON e.id = p.entry_id
          WHERE l.kind = 'expense' AND p.currency = ?3 AND e.date >= ?1 AND e.date < ?2 AND e.recurring_id IS NULL",
        params![from.to_string(), today.to_string(), cur], |r| r.get(0))?;
    // Only count the days since the first transaction, so a new profile isn't averaged over empty days.
    let first: Option<String> = conn.query_row("SELECT MIN(date) FROM entries WHERE kind <> 'opening'", [], |r| r.get(0))?;
    let since = first.and_then(|f| day(&f).ok()).map(|f| f.max(from)).unwrap_or(today);
    let days = (today - since).num_days().max(1);
    Ok(total.max(0) / days.clamp(14, TYPICAL_DAYS))
}

pub fn forecast(conn: &Connection, today: NaiveDate, days: i64) -> Result<Forecast> {
    if !(7..=MAX_FORECAST_DAYS).contains(&days) { return Err(Error::Invalid("forecast 7 to 180 days ahead".into())); }
    let cur = main_currency(conn)?;
    let liq = liquid(conn, &cur)?;
    let to = today + Duration::days(days);
    let sched = scheduled(conn, today, to, &cur, &liq.ids)?;
    let per_day = typical_day(conn, today, &cur)?;
    let dec = |m: i64| money::to_decimal(m, &cur);
    let (mut bal, mut i, mut points) = (liq.balance, 0, vec![]);
    for n in 0..=days {
        let d = today + Duration::days(n);
        while i < sched.len() && sched[i].0 == d { bal += sched[i].1; i += 1; }
        points.push(Point { date: d.to_string(), scheduled: dec(bal), expected: dec(bal - per_day * n) });
    }
    let lowest = points.iter().min_by(|a, b| a.expected.total_cmp(&b.expected)).map(|p| Point { date: p.date.clone(), scheduled: p.scheduled, expected: p.expected });
    Ok(Forecast {
        balance: dec(liq.balance), typical_day: dec(per_day),
        changes: sched.into_iter().map(|(d, a, title, kind)| Change { date: d.to_string(), title, amount: dec(a), kind }).collect(),
        points, lowest, other_currencies: liq.others, currency: cur,
    })
}

// ------------------------------------------------------------ safe to spend

#[derive(Debug, Serialize)]
pub struct SafeToSpend {
    pub currency: String,
    /// Everyday money now.
    pub balance: f64,
    /// Bills and card payments due before the next income.
    pub committed: f64,
    /// What's left: balance minus committed (never below zero).
    pub amount: f64,
    /// Per day until then.
    pub per_day: f64,
    /// The next income date (or the end of the month when none is scheduled).
    pub until: String,
    pub days: i64,
    pub income_name: Option<String>,
    /// Committed is more than the balance.
    pub short: bool,
}

pub fn safe_to_spend(conn: &Connection, today: NaiveDate) -> Result<SafeToSpend> {
    let cur = main_currency(conn)?;
    let liq = liquid(conn, &cur)?;
    let sched = scheduled(conn, today, today + Duration::days(45), &cur, &liq.ids)?;
    let next_income = sched.iter().find(|s| s.1 > 0 && s.3 == "income" && s.0 > today);
    let month_end = (NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today) + chrono::Months::new(1)) - Duration::days(1);
    let until = next_income.map(|s| s.0).unwrap_or(month_end.max(today + Duration::days(1)));
    // Money going out before (not on) the day income arrives.
    let committed: i64 = -sched.iter().filter(|s| s.0 < until && s.1 < 0).map(|s| s.1).sum::<i64>();
    let left = liq.balance - committed;
    let days = (until - today).num_days().max(1);
    let dec = |m: i64| money::to_decimal(m, &cur);
    Ok(SafeToSpend {
        balance: dec(liq.balance), committed: dec(committed), amount: dec(left.max(0)), per_day: dec(left.max(0) / days),
        until: until.to_string(), days, income_name: next_income.map(|s| s.2.clone()), short: left < 0, currency: cur,
    })
}

// ------------------------------------------------------------ low balance

/// How far ahead to look for a bill that would take an account below its level.
pub const LOW_BALANCE_DAYS: i64 = 14;

#[derive(Debug, Serialize, PartialEq)]
pub struct LowBalance {
    pub account_id: String,
    pub name: String,
    pub currency: String,
    pub balance: f64,
    /// The level the member set.
    pub level: f64,
    /// Already below it.
    pub now: bool,
    /// When it goes below (today when `now`).
    pub date: String,
    /// The scheduled payment that takes it below, when not `now`.
    pub because: Option<String>,
    /// The lowest balance in the next 14 days, counting scheduled payments.
    pub lowest: f64,
}

/// Accounts with a low-balance level that are below it, or that a bill, income or transfer
/// scheduled in the next 14 days would take below it. Only payments scheduled on the account
/// itself count (credit card payments don't say which account pays them).
pub fn low_balances(conn: &Connection, today: NaiveDate) -> Result<Vec<LowBalance>> {
    let to = today + Duration::days(LOW_BALANCE_DAYS);
    let watched: Vec<_> = crate::accounts::get_all(conn)?.into_iter()
        .filter(|a| a["ledger_kind"] != "liability" && a["low_balance_minor"].as_i64().is_some()).collect();
    if watched.is_empty() { return Ok(vec![]); }
    // recurring id -> (from account, to account, currency of the amount)
    let rules: BTreeMap<String, (Option<String>, Option<String>)> = recurring::list(conn, today)?.into_iter()
        .map(|r| (r["id"].as_str().unwrap_or_default().to_string(), (r["account_id"].as_str().map(str::to_string), r["to_account_id"].as_str().map(str::to_string))))
        .collect();
    let upcoming = recurring::upcoming(conn, today, to)?;
    let mut out = vec![];
    for a in watched {
        let (id, cur) = (a["id"].as_str().unwrap_or_default(), a["currency"].as_str().unwrap_or("INR"));
        let level = a["low_balance_minor"].as_i64().unwrap_or(0);
        let balance = a["current_balance_minor"].as_i64().unwrap_or(0);
        let (mut bal, mut lowest, mut crossed): (i64, i64, Option<(String, String)>) = (balance, balance, None);
        for u in &upcoming {
            if u["currency"].as_str() != Some(cur) { continue; }
            let (from, to_acc) = rules.get(u["recurring_id"].as_str().unwrap_or_default()).cloned().unwrap_or_default();
            let (out_of, into) = (from.as_deref() == Some(id), to_acc.as_deref() == Some(id));
            let sign = match u["type"].as_str() {
                Some("income") if out_of => 1,
                Some("transfer") => into as i64 - out_of as i64,
                Some("expense") if out_of => -1,
                _ => 0,
            };
            if sign == 0 { continue; }
            bal += sign * money::parse_minor(&u["amount"], cur).unwrap_or(0);
            lowest = lowest.min(bal);
            if crossed.is_none() && bal < level {
                crossed = Some((day(u["date"].as_str().unwrap_or_default())?.max(today).to_string(), u["name"].as_str().unwrap_or_default().to_string()));
            }
        }
        let now = balance < level;
        if !now && crossed.is_none() { continue; }
        let (date, because) = if now { (today.to_string(), None) } else { crossed.map(|(d, n)| (d, Some(n))).unwrap_or_default() };
        out.push(LowBalance {
            account_id: id.into(), name: a["name"].as_str().unwrap_or_default().into(), currency: cur.into(),
            balance: money::to_decimal(balance, cur), level: money::to_decimal(level, cur), now, date, because,
            lowest: money::to_decimal(lowest, cur),
        });
    }
    out.sort_by(|a, b| (!a.now, &a.date).cmp(&(!b.now, &b.date)));
    Ok(out)
}
