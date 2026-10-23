//! Net worth over time: at the end of each month (and today), per currency, what's in bank,
//! cash and wallet accounts, investments, money people owe you, and what you owe (cards, loans,
//! people). Account and people balances come from the ledger as it stood on each date.
//! Investments use units held on the date and the latest known price by then: from statement
//! rows where there are any (a CAMS/KFintech statement prints units and NAV on every row), the
//! current price from the day it was set, and for holdings typed in without history, today's
//! units and price for the whole period (a flat line rather than a jump on the day it was
//! entered).
use crate::{error::Result, investments, money};
use chrono::{Datelike, Months, NaiveDate};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default, Clone, Copy)]
struct Parts { cash: i64, investments: i64, owed_to_you: i64, debts: i64, you_owe: i64 }
impl Parts { fn net(&self) -> i64 { self.cash + self.investments + self.owed_to_you - self.debts - self.you_owe } }

/// Account and people balances on `date` (cleared entries), by currency.
fn ledger_on(conn: &Connection, date: &str, out: &mut BTreeMap<String, Parts>) -> Result<()> {
    let mut st = conn.prepare(
        "SELECT l.kind, l.account_id IS NOT NULL, p.currency, sum(p.amount_minor)
           FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id JOIN entries e ON e.id = p.entry_id
          WHERE e.is_cleared = 1 AND e.date <= ?1 AND (l.account_id IS NOT NULL OR l.person_id IS NOT NULL)
          GROUP BY l.id, p.currency")?;
    let rows = st.query_map(params![date], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)))?;
    for r in rows {
        let (kind, is_account, cur, bal) = r?;
        let e = out.entry(cur).or_default();
        match (is_account, kind.as_str()) {
            (true, "liability") => e.debts += -bal,
            (true, _) => e.cash += bal,
            // A person's balance: positive when they owe you.
            (false, _) if bal >= 0 => e.owed_to_you += bal,
            (false, _) => e.you_owe += -bal,
        }
    }
    Ok(())
}

struct Holding { currency: String, units: i64, price: i64, price_date: Option<String>, cost: i64, rows: Vec<(String, Option<i64>, Option<i64>)> }

fn holdings(conn: &Connection) -> Result<Vec<Holding>> {
    let mut st = conn.prepare("SELECT id, currency, units_e4, price_e4, price_date, cost_minor FROM holdings")?;
    let list: Vec<(String, String, i64, i64, Option<String>, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut rows = conn.prepare("SELECT date, balance_e4, price_e4 FROM holding_txns WHERE holding_id = ? AND (balance_e4 IS NOT NULL OR price_e4 IS NOT NULL) ORDER BY date, rowid")?;
    list.into_iter().map(|(id, currency, units, price, price_date, cost)| {
        let r = rows.query_map([&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(Holding { currency, units, price, price_date, cost, rows: r })
    }).collect()
}

/// A holding's value on `date`, in minor units.
fn holding_on(h: &Holding, date: &str) -> i64 {
    if h.rows.is_empty() {
        let price = if h.price > 0 { h.price } else if h.units > 0 { h.cost * 10_000 / h.units.max(1) } else { 0 };
        return investments::value_minor(h.units, price, &h.currency);
    }
    let before: Vec<_> = h.rows.iter().filter(|r| r.0.as_str() <= date).collect();
    let Some(units) = before.iter().rev().find_map(|r| r.1) else { return 0 };
    // Today's price once it's known; otherwise the last price on a statement row by then.
    let price = match &h.price_date {
        Some(pd) if pd.as_str() <= date && h.price > 0 => h.price,
        _ => before.iter().rev().find_map(|r| r.2).unwrap_or(h.price),
    };
    investments::value_minor(units, price, &h.currency)
}

/// Month ends from `months` back up to last month, then today.
fn dates(today: NaiveDate, months: u32) -> Vec<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let mut out: Vec<NaiveDate> = (1..=months).rev().filter_map(|m| first.checked_sub_months(Months::new(m - 1)).and_then(|d| d.pred_opt())).collect();
    out.push(today);
    out
}

/// Net worth at each month end for `months` months and today, per currency: `points` with
/// cash, investments, owed_to_you, debts, you_owe and net; plus `now`, the change over a month
/// and over the period, and the earliest date with anything recorded.
pub fn timeline(conn: &Connection, months: u32, today: NaiveDate) -> Result<Vec<Value>> {
    let months = months.clamp(1, 240);
    let hs = holdings(conn)?;
    let first: Option<String> = conn.query_row("SELECT min(date) FROM entries", [], |r| r.get(0))?;
    let mut series: BTreeMap<String, Vec<(NaiveDate, Parts)>> = BTreeMap::new();
    let ds = dates(today, months);
    for d in &ds {
        let ds_ = d.to_string();
        let mut parts: BTreeMap<String, Parts> = BTreeMap::new();
        ledger_on(conn, &ds_, &mut parts)?;
        for h in &hs { parts.entry(h.currency.clone()).or_default().investments += holding_on(h, &ds_); }
        for (cur, p) in parts { series.entry(cur).or_default().push((*d, p)); }
    }
    Ok(series.into_iter().filter_map(|(cur, pts)| {
        // A currency that appears only part-way through starts at zero before that.
        let full: Vec<(NaiveDate, Parts)> = ds.iter().map(|d| pts.iter().find(|(x, _)| x == d).map(|(_, p)| (*d, *p)).unwrap_or((*d, Parts::default()))).collect();
        if full.iter().all(|(_, p)| p.net() == 0 && p.debts == 0 && p.cash == 0) { return None; }
        let m = |x: i64| money::to_decimal(x, &cur);
        let points: Vec<Value> = full.iter().map(|(d, p)| json!({ "date": d.to_string(), "cash": m(p.cash), "investments": m(p.investments),
            "owed_to_you": m(p.owed_to_you), "debts": m(p.debts), "you_owe": m(p.you_owe), "net": m(p.net()) })).collect();
        let now = full.last().map(|x| x.1).unwrap_or_default();
        let month_ago = full.len().checked_sub(2).map(|i| full[i].1.net());
        let start = full.first().map(|x| x.1.net()).unwrap_or(0);
        Some(json!({ "currency": cur, "points": points, "net": m(now.net()),
            "change_month": month_ago.map(|x| m(now.net() - x)), "change_period": m(now.net() - start), "since": first.clone() }))
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn month_ends() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let v = dates(d("2026-10-23"), 3);
        assert_eq!(v, vec![d("2026-07-31"), d("2026-08-31"), d("2026-09-30"), d("2026-10-23")]);
    }
}
