//! Unusual transactions, worth a second look: the same amount charged twice at the same place
//! within two days (a double charge), an amount well above what a place usually charges, and a
//! large first payment to a place never paid before. Only spending is checked, hidden
//! transactions aren't, and anything marked fine stays quiet.
use crate::{error::Result, money, rules};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;

/// How far back new transactions are checked.
pub const RECENT_DAYS: i64 = 30;
/// How much history a place's usual amount comes from.
const HISTORY_DAYS: i64 = 365;
/// A place needs this many earlier charges before an amount counts as unusual for it.
const MIN_HISTORY: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    /// What "It's fine" remembers.
    pub key: String,
    /// double, amount or new_place
    pub kind: String,
    pub entry_id: String,
    /// The other charge, for a double charge.
    pub other_id: Option<String>,
    pub date: String,
    pub name: String,
    pub account: Option<String>,
    pub amount: f64,
    pub currency: String,
    /// For an unusual amount: what the place usually charges (median).
    pub usual: Option<f64>,
    /// How many times the usual amount (unusual amounts), or how many times the typical spend
    /// (new places).
    pub times: Option<f64>,
}

struct Tx { id: String, date: NaiveDate, amount: i64, currency: String, key: String, name: String, account_id: Option<String>, account: Option<String>, recurring: bool }

fn median(v: &mut [i64]) -> i64 { v.sort_unstable(); v[v.len() / 2] }

/// Unusual spending in the last 30 days, newest first, without the ones marked fine.
pub fn find(conn: &Connection, today: NaiveDate) -> Result<Vec<Anomaly>> {
    let since = today - chrono::Days::new((HISTORY_DAYS + RECENT_DAYS) as u64);
    let recent = today - chrono::Days::new(RECENT_DAYS as u64);
    let mut st = conn.prepare(
        "SELECT t.id, t.date, t.amount_minor, t.currency, COALESCE(NULLIF(TRIM(t.merchant), ''), t.description), t.account_id, a.name, t.recurring_id IS NOT NULL
           FROM transactions t LEFT JOIN accounts a ON a.id = t.account_id
          WHERE t.type = 'expense' AND t.date >= ?1 AND t.date <= ?2 AND COALESCE(t.hidden, 0) = 0 AND t.amount_minor > 0
          ORDER BY t.date, t.created_at")?;
    let rows = st.query_map(params![since.to_string(), today.to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?,
        r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, bool>(7)?)))?;
    let mut all = vec![];
    for r in rows {
        let (id, date, amount, currency, name, account_id, account, recurring) = r?;
        let (Some(name), Ok(date)) = (name, NaiveDate::parse_from_str(&date, "%Y-%m-%d")) else { continue };
        let Some(key) = rules::key(&name) else { continue };
        all.push(Tx { id, date, amount, currency, key, name: name.trim().to_string(), account_id, account, recurring });
    }
    let dismissed: Vec<String> = conn.prepare("SELECT key FROM anomaly_dismissed")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut out: Vec<Anomaly> = vec![];
    let push = |out: &mut Vec<Anomaly>, a: Anomaly| { if !dismissed.contains(&a.key) && !out.iter().any(|x| x.entry_id == a.entry_id) { out.push(a); } };
    let mk = |t: &Tx, kind: &str, key: String, other: Option<String>, usual: Option<i64>, times: Option<f64>| Anomaly {
        key, kind: kind.into(), entry_id: t.id.clone(), other_id: other, date: t.date.to_string(), name: t.name.clone(), account: t.account.clone(),
        amount: money::to_decimal(t.amount, &t.currency), currency: t.currency.clone(), usual: usual.map(|u| money::to_decimal(u, &t.currency)), times,
    };

    // A typical spend per currency: the median of all spending in the past year, for "large".
    let mut typical: HashMap<&str, i64> = HashMap::new();
    {
        let mut by: HashMap<&str, Vec<i64>> = HashMap::new();
        for t in &all { by.entry(t.currency.as_str()).or_default().push(t.amount); }
        for (c, mut v) in by { if v.len() >= 20 { typical.insert(c, median(&mut v)); } }
    }

    for (i, t) in all.iter().enumerate() {
        if t.date < recent { continue; }
        // Double charge: the same place, account and amount within 2 days before it.
        if let Some(o) = all[..i].iter().rev().take_while(|o| (t.date - o.date).num_days() <= 2)
            .find(|o| o.key == t.key && o.amount == t.amount && o.currency == t.currency && o.account_id == t.account_id && !(o.recurring && t.recurring)) {
            push(&mut out, mk(t, "double", format!("double:{}:{}", o.id, t.id), Some(o.id.clone()), None, None));
            continue;
        }
        let earlier: Vec<&Tx> = all[..i].iter().filter(|o| o.key == t.key && o.currency == t.currency && (t.date - o.date).num_days() <= HISTORY_DAYS).collect();
        // Unusual for the place: over 3× its usual, and well beyond its spread.
        if earlier.len() >= MIN_HISTORY && !t.recurring {
            let mut amounts: Vec<i64> = earlier.iter().map(|o| o.amount).collect();
            let usual = median(&mut amounts);
            let mut dev: Vec<i64> = amounts.iter().map(|a| (a - usual).abs()).collect();
            let mad = median(&mut dev);
            let biggest = *amounts.iter().max().unwrap_or(&0);
            if usual > 0 && t.amount >= usual * 3 && t.amount > usual + 5 * mad && t.amount > biggest {
                push(&mut out, mk(t, "amount", format!("amount:{}", t.id), None, Some(usual), Some((t.amount as f64 / usual as f64 * 10.0).round() / 10.0)));
            }
            continue;
        }
        // A first payment to a place, much bigger than typical spending (needs a year's picture).
        let first_ever = !all[..i].iter().any(|o| o.key == t.key) && !t.recurring;
        if let (true, Some(&typ)) = (first_ever, typical.get(t.currency.as_str())) {
            if typ > 0 && t.amount >= typ * 10 && t.amount >= 1_000 * money::scale(&t.currency) {
                push(&mut out, mk(t, "new_place", format!("new:{}", t.id), None, None, Some((t.amount as f64 / typ as f64).round())));
            }
        }
    }
    out.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(out)
}

/// "It's fine": stops showing it.
pub fn dismiss(conn: &Connection, key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 200 { return Err(crate::error::Error::Invalid("unknown item".into())); }
    conn.execute("INSERT OR IGNORE INTO anomaly_dismissed (key) VALUES (?)", [key])?;
    Ok(())
}

/// Forgets every "It's fine", so everything is checked again.
pub fn reset(conn: &Connection) -> Result<usize> { Ok(conn.execute("DELETE FROM anomaly_dismissed", [])?) }

