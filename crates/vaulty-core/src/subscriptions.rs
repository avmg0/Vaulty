//! Spotting subscriptions and regular bills that aren't tracked yet, and price rises.
//!
//! A subscription candidate is the same place (by merchant key) charged at a steady rhythm:
//! weekly (3+ times), monthly or quarterly (3+ times) or yearly (2+ times), with amounts within
//! about a quarter of each other. Places already covered by a recurring payment are left out, as
//! are ones the member said aren't subscriptions.
//!
//! A price rise is the latest charge being at least 3% above the one before, for a candidate or
//! for a recurring payment paid at a different amount.
use crate::{error::Result, money, rules};
use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;

/// How far back to look.
pub const LOOK_BACK_DAYS: i64 = 400;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PriceChange { pub from: f64, pub to: f64, pub percent: f64, pub date: String }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Candidate {
    /// The merchant key: what "Not a subscription" remembers.
    pub key: String,
    /// The name as last written.
    pub name: String,
    /// weekly, monthly, quarterly or yearly (as a recurring payment: frequency + every).
    pub rhythm: String,
    pub frequency: String,
    pub every: u32,
    pub amount: f64,
    pub currency: String,
    pub count: usize,
    pub last_date: String,
    pub next_date: String,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub price_change: Option<PriceChange>,
}

struct Charge { date: NaiveDate, amount: i64, name: String, account: Option<String>, category: Option<String> }

/// (rhythm, frequency, every, typical gap in days, allowed drift in days, minimum charges)
const RHYTHMS: &[(&str, &str, u32, i64, i64, usize)] = &[
    ("weekly", "weekly", 1, 7, 1, 3),
    ("monthly", "monthly", 1, 30, 4, 3),
    ("quarterly", "monthly", 3, 91, 7, 3),
    ("yearly", "yearly", 1, 365, 10, 2),
];

fn price_change(prev: i64, last: i64, date: NaiveDate, cur: &str) -> Option<PriceChange> {
    (prev > 0 && last * 100 >= prev * 103).then(|| PriceChange {
        from: money::to_decimal(prev, cur), to: money::to_decimal(last, cur),
        percent: ((last - prev) as f64 / prev as f64 * 1000.0).round() / 10.0, date: date.to_string(),
    })
}

fn median(v: &mut [i64]) -> i64 { v.sort(); v[v.len() / 2] }

/// Regular charges that look like subscriptions and aren't tracked or dismissed.
pub fn detect(conn: &Connection, today: NaiveDate) -> Result<Vec<Candidate>> {
    let since = today - Duration::days(LOOK_BACK_DAYS);
    // One row per expense (the money out of an account), not hidden, not already a recurring payment.
    let mut stmt = conn.prepare(
        "SELECT t.date, t.amount_minor, t.currency, COALESCE(NULLIF(TRIM(t.merchant), ''), t.description), t.account_id, t.category_id
           FROM transactions t
          WHERE t.type = 'expense' AND t.date >= ?1 AND t.date <= ?2 AND t.recurring_id IS NULL AND COALESCE(t.hidden, 0) = 0")?;
    let mut groups: BTreeMap<(String, String), Vec<Charge>> = BTreeMap::new();
    let rows = stmt.query_map([since.to_string(), today.to_string()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?))
    })?;
    for row in rows {
        let (date, amount, cur, name, account, category) = row?;
        let (Some(name), Ok(date)) = (name, NaiveDate::parse_from_str(&date, "%Y-%m-%d")) else { continue };
        let Some(key) = rules::key(&name) else { continue };
        groups.entry((key, cur)).or_default().push(Charge { date, amount, name: name.trim().to_string(), account, category });
    }

    // Places already tracked (by a recurring payment's name or merchant), and dismissed ones.
    let mut skip: Vec<String> = conn.prepare("SELECT COALESCE(merchant, name) FROM recurring_payments WHERE is_active = 1")?
        .query_map([], |r| r.get::<_, String>(0))?.filter_map(|x| x.ok()).filter_map(|n| rules::key(&n)).collect();
    skip.extend(conn.prepare("SELECT key FROM subscription_dismissed")?.query_map([], |r| r.get::<_, String>(0))?.filter_map(|x| x.ok()));

    let mut out = vec![];
    for ((key, cur), mut charges) in groups {
        if skip.iter().any(|s| s == &key || key.starts_with(&format!("{s} ")) || s.starts_with(&format!("{key} "))) { continue; }
        charges.sort_by_key(|c| c.date);
        // Two charges the same day are one purchase split up, not a rhythm.
        charges.dedup_by(|b, a| { if a.date == b.date { a.amount += b.amount; true } else { false } });
        let gaps: Vec<i64> = charges.windows(2).map(|w| (w[1].date - w[0].date).num_days()).collect();
        let Some(&(rhythm, frequency, every, gap, drift, _)) = RHYTHMS.iter().find(|(_, _, _, gap, drift, min)| {
            charges.len() >= *min && !gaps.is_empty() && gaps.iter().all(|g| (g - gap).abs() <= *drift)
        }) else { continue };
        // Earlier amounts alike; the latest may be a price rise (up to 75% more) or a little less.
        let (earlier, latest) = charges.split_at(charges.len() - 1);
        let mut amounts: Vec<i64> = earlier.iter().map(|c| c.amount).collect();
        let mid = median(&mut amounts);
        if earlier.iter().any(|c| c.amount * 4 < mid * 3 || c.amount * 4 > mid * 5) { continue; }
        if latest[0].amount * 4 < mid * 3 || latest[0].amount * 4 > mid * 7 { continue; }
        let last = charges.last().unwrap();
        // Stale: the next charge should have come long ago (cancelled).
        let next = last.date + Duration::days(gap);
        if (today - next).num_days() > drift * 2 + 3 { continue; }
        let prev = charges[charges.len() - 2].amount;
        out.push(Candidate {
            key, name: last.name.clone(), rhythm: rhythm.into(), frequency: frequency.into(), every,
            amount: money::to_decimal(last.amount, &cur), count: charges.len(),
            last_date: last.date.to_string(), next_date: next.to_string(),
            account_id: last.account.clone(), category_id: last.category.clone(),
            price_change: price_change(prev, last.amount, last.date, &cur), currency: cur,
        });
    }
    out.sort_by(|a, b| b.amount.total_cmp(&a.amount));
    Ok(out)
}

/// "Not a subscription": stop suggesting this place.
pub fn dismiss(conn: &Connection, key: &str) -> Result<()> {
    let key: String = key.trim().chars().take(100).collect();
    conn.execute("INSERT OR IGNORE INTO subscription_dismissed (key) VALUES (?)", [key])?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecurringRise { pub recurring_id: String, pub name: String, pub currency: String, #[serde(flatten)] pub change: PriceChange }

/// Recurring payments whose latest payment cost at least 3% more than the one before.
pub fn recurring_rises(conn: &Connection) -> Result<Vec<RecurringRise>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.name, t.currency, t.amount_minor, t.date FROM recurring_payments r JOIN transactions t ON t.recurring_id = r.id
          WHERE r.is_active = 1 ORDER BY r.id, t.date")?;
    // recurring id -> (name, currency, payments as (amount, date) oldest first)
    type Paid = (String, String, Vec<(i64, NaiveDate)>);
    let mut by: BTreeMap<String, Paid> = BTreeMap::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?)))? {
        let (id, name, cur, amount, date) = row?;
        let Ok(date) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") else { continue };
        by.entry(id).or_insert_with(|| (name, cur, vec![])).2.push((amount, date));
    }
    Ok(by.into_iter().filter_map(|(id, (name, cur, paid))| {
        let [.., (prev, _), (last, date)] = paid.as_slice() else { return None };
        price_change(*prev, *last, *date, &cur).map(|change| RecurringRise { recurring_id: id, name, currency: cur, change })
    }).collect())
}
