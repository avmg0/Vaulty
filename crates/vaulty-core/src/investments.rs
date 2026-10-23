//! Investments held (the `holdings` table): what they're worth, what they cost, how they've done
//! (the gain, and XIRR when the transactions are known), typed in or read from a CAMS/KFintech
//! consolidated account statement.
//!
//! Reading a statement again (or a later one that overlaps) updates each folio and adds only the
//! transactions not already there. A folio is matched on ISIN and folio number.
use crate::{cas::{Cas, CasScheme, CasTxn, E4}, error::{Error, Result}, money, rows::*};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const KINDS: &[&str] = &["mutual_fund", "stock", "etf", "bond", "deposit", "gold", "other"];
const MAX_UNITS_E4: i64 = 1_000_000_000_000_000; // 10^11 units
const MAX_PRICE_E4: i64 = 100_000_000_000_000;   // 10^10 per unit

/// Units × price, in the currency's minor units, rounded.
pub fn value_minor(units_e4: i64, price_e4: i64, currency: &str) -> i64 {
    let num = units_e4 as i128 * price_e4 as i128 * money::scale(currency) as i128;
    let den = (E4 * E4) as i128;
    i64::try_from((num + den / 2) / den).unwrap_or(i64::MAX)
}

/// The yearly rate that makes the money put in and taken out (with today's value as the last
/// amount out) come to nothing: XIRR. `None` when it can't be worked out (no money in, or too short).
pub fn xirr(flows: &[(NaiveDate, f64)]) -> Option<f64> {
    if flows.len() < 2 || !flows.iter().any(|f| f.1 < 0.0) || !flows.iter().any(|f| f.1 > 0.0) { return None; }
    let first = flows.iter().map(|f| f.0).min()?;
    let last = flows.iter().map(|f| f.0).max()?;
    if (last - first).num_days() < 30 { return None; }
    let npv = |r: f64| flows.iter().map(|(d, a)| a / (1.0 + r).powf((*d - first).num_days() as f64 / 365.0)).sum::<f64>();
    let (mut lo, mut hi) = (-0.9999, 100.0);
    let (flo, fhi) = (npv(lo), npv(hi));
    if !flo.is_finite() || !fhi.is_finite() || flo.signum() == fhi.signum() { return None; }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        let f = npv(mid);
        if f.signum() == flo.signum() { lo = mid } else { hi = mid }
        if hi - lo < 1e-9 { break; }
    }
    Some((lo + hi) / 2.0)
}

/// Money from the investor's side: negative going in, positive coming out. `None` for rows that
/// move no money (reinvested dividends, taxes taken from a redemption).
fn flow(kind: &str, amount_minor: Option<i64>) -> Option<i64> {
    let a = amount_minor?.abs();
    match kind {
        "purchase" | "sip" | "switch_in" | "stamp_duty" => Some(-a),
        "redemption" | "switch_out" | "dividend_payout" => Some(a),
        _ => None,
    }
}

const COLS: &str = "id, kind, name, isin, symbol, folio, provider, registrar, currency, units_e4, price_e4, price_date, cost_minor, source, statement_to, notes, created_at, updated_at";

fn decorate(conn: &Connection, mut h: Value, today: NaiveDate) -> Result<Value> {
    let cur = h["currency"].as_str().unwrap_or("INR").to_string();
    let (units, price, cost) = (h["units_e4"].as_i64().unwrap_or(0), h["price_e4"].as_i64().unwrap_or(0), h["cost_minor"].as_i64().unwrap_or(0));
    let value = value_minor(units, price, &cur);
    let txns: Vec<(String, String, Option<i64>)> = {
        let mut st = conn.prepare("SELECT date, kind, amount_minor FROM holding_txns WHERE holding_id = ? ORDER BY date")?;
        let rows = st.query_map([h["id"].as_str().unwrap_or_default()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let scale = money::scale(&cur) as f64;
    let mut flows: Vec<(NaiveDate, f64)> = txns.iter().filter_map(|(d, k, a)| {
        Some((NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?, flow(k, *a)? as f64 / scale))
    }).collect();
    let as_of = h["price_date"].as_str().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).unwrap_or(today);
    if value > 0 { flows.push((as_of, value as f64 / scale)); }
    let xirr = if txns.is_empty() { None } else { xirr(&flows) };
    let paid_out: i64 = txns.iter().filter(|t| t.1 == "dividend_payout").filter_map(|t| t.2).map(i64::abs).sum();
    if let Some(o) = h.as_object_mut() {
        o.insert("units".into(), json!(units as f64 / E4 as f64));
        o.insert("price".into(), json!(price as f64 / E4 as f64));
        o.insert("value".into(), json!(money::to_decimal(value, &cur)));
        o.insert("cost".into(), json!(money::to_decimal(cost, &cur)));
        o.insert("gain".into(), json!(money::to_decimal(value - cost, &cur)));
        o.insert("gain_percent".into(), json!(if cost > 0 { Some(((value - cost) as f64 / cost as f64 * 10_000.0).round() / 100.0) } else { None }));
        o.insert("dividends".into(), json!(money::to_decimal(paid_out, &cur)));
        o.insert("xirr".into(), json!(xirr.map(|r| (r * 10_000.0).round() / 100.0)));
        o.insert("transactions".into(), json!(txns.len()));
        o.insert("closed".into(), json!(units == 0));
        o.insert("value_minor".into(), json!(value));
    }
    Ok(h)
}

/// Every holding, largest first; closed ones (no units left) last.
pub fn list(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let mut all: Vec<Value> = query(conn, &format!("SELECT {COLS} FROM holdings"), &[])?
        .into_iter().map(|h| decorate(conn, h, today)).collect::<Result<_>>()?;
    all.sort_by(|a, b| a["closed"].as_bool().cmp(&b["closed"].as_bool())
        .then(b["value_minor"].as_i64().cmp(&a["value_minor"].as_i64()))
        .then(a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase())));
    Ok(all)
}

pub fn get(conn: &Connection, id: &str, today: NaiveDate) -> Result<Value> {
    let h = query_one(conn, &format!("SELECT {COLS} FROM holdings WHERE id = ?"), &[json!(id)])?
        .ok_or_else(|| Error::Invalid("no such investment".into()))?;
    decorate(conn, h, today)
}

/// A holding's transactions, newest first.
pub fn transactions(conn: &Connection, id: &str) -> Result<Vec<Value>> {
    let cur: String = conn.query_row("SELECT currency FROM holdings WHERE id = ?", [id], |r| r.get(0)).optional()?
        .ok_or_else(|| Error::Invalid("no such investment".into()))?;
    let rows = query(conn, "SELECT id, date, kind, description, amount_minor, units_e4, price_e4, balance_e4 FROM holding_txns WHERE holding_id = ? ORDER BY date DESC, rowid DESC", &[json!(id)])?;
    Ok(rows.into_iter().map(|mut t| {
        let e4 = |k: &str| t[k].as_i64().map(|v| v as f64 / E4 as f64);
        let (amount, units, price, balance) = (t["amount_minor"].as_i64().map(|a| money::to_decimal(a, &cur)), e4("units_e4"), e4("price_e4"), e4("balance_e4"));
        if let Some(o) = t.as_object_mut() {
            o.insert("amount".into(), json!(amount));
            o.insert("units".into(), json!(units));
            o.insert("price".into(), json!(price));
            o.insert("balance".into(), json!(balance));
        }
        t
    }).collect())
}

/// Totals per currency: value, cost, gain, and XIRR over every holding with transactions.
pub fn summary(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let all = list(conn, today)?;
    let mut by: Vec<(String, i64, i64, usize)> = vec![];
    for h in &all {
        let cur = h["currency"].as_str().unwrap_or("INR").to_string();
        let (v, c) = (h["value_minor"].as_i64().unwrap_or(0), h["cost_minor"].as_i64().unwrap_or(0));
        match by.iter_mut().find(|b| b.0 == cur) {
            Some(b) => { b.1 += v; b.2 += c; b.3 += usize::from(!h["closed"].as_bool().unwrap_or(false)); }
            None => by.push((cur, v, c, usize::from(!h["closed"].as_bool().unwrap_or(false)))),
        }
    }
    by.into_iter().map(|(cur, value, cost, open)| {
        let scale = money::scale(&cur) as f64;
        let mut flows = vec![];
        for h in all.iter().filter(|h| h["currency"] == cur.as_str()) {
            let mut st = conn.prepare("SELECT date, kind, amount_minor FROM holding_txns WHERE holding_id = ?")?;
            let rows = st.query_map([h["id"].as_str().unwrap_or_default()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?)))?;
            for r in rows {
                let (d, k, a) = r?;
                if let (Ok(d), Some(f)) = (NaiveDate::parse_from_str(&d, "%Y-%m-%d"), flow(&k, a)) { flows.push((d, f as f64 / scale)); }
            }
        }
        let latest = all.iter().filter(|h| h["currency"] == cur.as_str()).filter_map(|h| h["price_date"].as_str()).max()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).unwrap_or(today);
        if value > 0 { flows.push((latest, value as f64 / scale)); }
        Ok(json!({
            "currency": cur, "holdings": open,
            "value": money::to_decimal(value, &cur), "cost": money::to_decimal(cost, &cur), "gain": money::to_decimal(value - cost, &cur),
            "gain_percent": if cost > 0 { Some(((value - cost) as f64 / cost as f64 * 10_000.0).round() / 100.0) } else { None },
            "xirr": xirr(&flows).map(|r| (r * 10_000.0).round() / 100.0),
        }))
    }).collect()
}

fn text(v: &Value, key: &str, max: usize) -> Result<Option<String>> {
    let t = v[key].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if t.as_ref().is_some_and(|s| s.chars().count() > max) { return Err(Error::Invalid(format!("one of the details is too long (up to {max} characters)"))); }
    Ok(t)
}

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

/// A typed number of units (or, with `price`, a price) with up to four decimals, as an exact
/// ×10,000 integer.
fn e4(v: &Value, key: &str, price: bool) -> Result<i64> {
    let not_number = || if price { bad("the price isn't a number") } else { bad("the number of units isn't a number") };
    let s = match &v[key] {
        Value::Null => return Ok(0),
        Value::Number(n) => n.to_string(),
        Value::String(s) if s.trim().is_empty() => return Ok(0),
        Value::String(s) => s.trim().to_string(),
        _ => return Err(not_number()),
    };
    if s.split_once('.').is_some_and(|(_, f)| f.len() > 4) {
        return Err(if price { bad("the price can have up to 4 decimal places") } else { bad("the number of units can have up to 4 decimal places") });
    }
    let n = crate::cas::scaled(&s, 4).ok_or_else(not_number)?;
    if !(0..=if price { MAX_PRICE_E4 } else { MAX_UNITS_E4 }).contains(&n) {
        return Err(if price { bad("the price must be 0 or more, and not huge") } else { bad("the number of units must be 0 or more, and not huge") });
    }
    Ok(n)
}

type Fields = (String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, i64, i64, Option<String>, i64, Option<String>);

fn fields(conn: &Connection, v: &Value) -> Result<Fields> {
    let name = text(v, "name", 160)?.ok_or_else(|| Error::Invalid("give the investment a name".into()))?;
    let kind = v["kind"].as_str().unwrap_or("mutual_fund");
    if !KINDS.contains(&kind) { return Err(Error::Invalid("unknown kind of investment".into())); }
    let isin = text(v, "isin", 12)?.map(|s| s.to_uppercase());
    if isin.as_ref().is_some_and(|s| s.len() != 12 || !s.chars().all(|c| c.is_ascii_alphanumeric())) { return Err(Error::Invalid("an ISIN is 12 letters and digits".into())); }
    let currency = match text(v, "currency", 3)? {
        Some(c) => c.to_uppercase(),
        None => conn.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get::<_, Option<String>>(0)).optional()?.flatten().unwrap_or_else(|| "INR".into()),
    };
    let price_date = match text(v, "price_date", 10)? {
        Some(d) => Some(NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("`{d}` isn't a date (YYYY-MM-DD)")))?.to_string()),
        None => None,
    };
    let cost = match &v["cost"] { Value::Null => 0, c => money::parse_minor(c, &currency)? };
    if cost < 0 { return Err(Error::Invalid("the cost must be 0 or more".into())); }
    Ok((name, kind.into(), isin, text(v, "symbol", 40)?, text(v, "folio", 40)?, text(v, "provider", 80)?, currency,
        e4(v, "units", false)?, e4(v, "price", true)?, price_date, cost, text(v, "notes", 2000)?))
}

fn unique(e: rusqlite::Error) -> Error {
    if e.to_string().contains("UNIQUE") { Error::Invalid("you already have this fund in that folio".into()) } else { e.into() }
}

pub fn create(conn: &Connection, v: &Value, today: NaiveDate) -> Result<Value> {
    let (name, kind, isin, symbol, folio, provider, currency, units, price, price_date, cost, notes) = fields(conn, v)?;
    let id = new_id();
    conn.execute(
        "INSERT INTO holdings (id, kind, name, isin, symbol, folio, provider, currency, units_e4, price_e4, price_date, cost_minor, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, kind, name, isin, symbol, folio, provider, currency, units, price, price_date.or(Some(today.to_string())), cost, notes]).map_err(unique)?;
    get(conn, &id, today)
}

pub fn update(conn: &Connection, id: &str, v: &Value, today: NaiveDate) -> Result<Value> {
    let before = get(conn, id, today)?;
    let (name, kind, isin, symbol, folio, provider, currency, units, price, price_date, cost, notes) = fields(conn, v)?;
    let price_date = if price != before["price_e4"].as_i64().unwrap_or(0) && price_date.as_deref() == before["price_date"].as_str() { Some(today.to_string()) } else { price_date };
    conn.execute(
        "UPDATE holdings SET kind = ?, name = ?, isin = ?, symbol = ?, folio = ?, provider = ?, currency = ?, units_e4 = ?, price_e4 = ?,
                price_date = ?, cost_minor = ?, notes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![kind, name, isin, symbol, folio, provider, currency, units, price, price_date, cost, notes, id]).map_err(unique)?;
    get(conn, id, today)
}

/// Sets just the latest price (NAV), as of a date.
pub fn set_price(conn: &Connection, id: &str, price: &Value, date: NaiveDate, today: NaiveDate) -> Result<Value> {
    get(conn, id, today)?;
    let p = e4(&json!({ "p": price }), "p", true)?;
    conn.execute("UPDATE holdings SET price_e4 = ?, price_date = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?", params![p, date.to_string(), id])?;
    get(conn, id, today)
}

/// Deletes a holding and its transactions (undo brings them back).
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM holdings WHERE id = ?", [id])? == 0 { return Err(Error::Invalid("no such investment".into())); }
    Ok(())
}

// ---- Statements ----

fn find(conn: &Connection, s: &CasScheme) -> Result<Option<(String, Option<String>)>> {
    let row = match &s.isin {
        Some(isin) => conn.query_row("SELECT id, statement_to FROM holdings WHERE isin = ? AND folio = ?", params![isin, s.folio], |r| Ok((r.get(0)?, r.get(1)?))).optional()?,
        None => conn.query_row("SELECT id, statement_to FROM holdings WHERE isin IS NULL AND folio = ? AND lower(name) = lower(?)", params![s.folio, s.name], |r| Ok((r.get(0)?, r.get(1)?))).optional()?,
    };
    Ok(row)
}

/// A key per transaction row: the same row in two statements gives the same key. Rows identical
/// in every way on one day are told apart by how many came before.
fn keys(s: &CasScheme) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    s.txns.iter().map(|t: &CasTxn| {
        let base = format!("{}|{}|{}|{}|{}|{:?}|{:?}|{:?}", s.isin.as_deref().unwrap_or(&s.name), s.folio, t.date, t.kind,
            t.description.to_lowercase(), t.amount_minor, t.units_e4, t.nav_e4);
        let n = seen.entry(base.clone()).or_default();
        *n += 1;
        let mut h = Sha256::new();
        h.update(format!("cas-v1|{base}|{n}"));
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }).collect()
}

fn existing(conn: &Connection, keys: &[String]) -> Result<usize> {
    let mut st = conn.prepare("SELECT 1 FROM holding_txns WHERE source_key = ?")?;
    let mut n = 0;
    for k in keys { if st.exists([k])? { n += 1; } }
    Ok(n)
}

/// What reading the statement would do, per scheme: new or already there, and how many
/// transactions are new.
pub fn preview(conn: &Connection, cas: &Cas) -> Result<Value> {
    let mut schemes = vec![];
    let (mut new_txns, mut value) = (0usize, 0i64);
    for s in &cas.schemes {
        let found = find(conn, s)?;
        let keys = keys(s);
        let fresh = keys.len() - existing(conn, &keys)?;
        new_txns += fresh;
        value += s.value_minor.unwrap_or(0);
        let older = found.as_ref().and_then(|f| f.1.clone()).zip(cas.to).is_some_and(|(had, to)| had.as_str() > to.to_string().as_str());
        schemes.push(json!({
            "name": s.name, "amc": s.amc, "folio": s.folio, "isin": s.isin, "registrar": s.registrar,
            "status": if found.is_none() { "new" } else if older { "older" } else { "update" },
            "units": s.close_units_e4.map(|u| u as f64 / E4 as f64), "nav": s.nav_e4.map(|n| n as f64 / E4 as f64), "nav_date": s.nav_date,
            "cost": s.cost_minor.map(|c| money::to_decimal(c, "INR")), "value": s.value_minor.map(|v| money::to_decimal(v, "INR")),
            "transactions": s.txns.len(), "new_transactions": fresh, "balanced": s.balanced,
        }));
    }
    Ok(json!({
        "from": cas.from, "to": cas.to, "investor": cas.investor, "schemes": schemes,
        "new_transactions": new_txns, "value": money::to_decimal(value, "INR"),
    }))
}

/// Reads the statement in: new folios are added, known ones updated (unless this statement is
/// older than the one already read), and transactions not already there are added.
pub fn import_cas(conn: &Connection, cas: &Cas) -> Result<Value> {
    let to = cas.to.map(|d| d.to_string());
    let (mut added, mut updated, mut txns) = (0, 0, 0);
    for s in &cas.schemes {
        let id = match find(conn, s)? {
            Some((id, had)) => {
                let newer = match (&had, &to) { (Some(h), Some(t)) => t >= h, _ => true };
                if newer {
                    conn.execute(
                        "UPDATE holdings SET name = ?, provider = ?, registrar = ?, units_e4 = ?, price_e4 = coalesce(?, price_e4), price_date = coalesce(?, price_date),
                                cost_minor = coalesce(?, cost_minor), statement_to = coalesce(?, statement_to), source = 'cas', updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                        params![s.name, s.amc, s.registrar, s.close_units_e4.unwrap_or(0).max(0), s.nav_e4.map(|n| n.max(0)), s.nav_date.map(|d| d.to_string()),
                                s.cost_minor.map(|c| c.max(0)), to, id])?;
                    updated += 1;
                }
                id
            }
            None => {
                let id = new_id();
                conn.execute(
                    "INSERT INTO holdings (id, kind, name, isin, folio, provider, registrar, currency, units_e4, price_e4, price_date, cost_minor, source, statement_to)
                     VALUES (?, 'mutual_fund', ?, ?, ?, ?, ?, 'INR', ?, ?, ?, ?, 'cas', ?)",
                    params![id, s.name, s.isin, s.folio, s.amc, s.registrar, s.close_units_e4.unwrap_or(0).max(0), s.nav_e4.unwrap_or(0).max(0),
                            s.nav_date.map(|d| d.to_string()), s.cost_minor.unwrap_or(0).max(0), to])?;
                added += 1;
                id
            }
        };
        let mut st = conn.prepare(
            "INSERT OR IGNORE INTO holding_txns (id, holding_id, date, kind, description, amount_minor, units_e4, price_e4, balance_e4, source_key)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?;
        for (t, k) in s.txns.iter().zip(keys(s)) {
            txns += st.execute(params![new_id(), id, t.date.to_string(), t.kind, t.description, t.amount_minor, t.units_e4, t.nav_e4, t.balance_e4, k])?;
        }
    }
    Ok(json!({ "added": added, "updated": updated, "transactions": txns }))
}
