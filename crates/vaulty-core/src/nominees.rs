//! Nominees: who is registered to receive each bank account, investment and insurance policy.
//! Kept as written ("John Doe, 50%; Jenny Doe, 50%"), shown in the emergency summary, and
//! listed together so missing ones stand out.
use crate::{error::{Error, Result}, rows::*};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }
const MAX: usize = 200;

fn table(kind: &str) -> Result<&'static str> {
    match kind { "account" => Ok("accounts"), "holding" => Ok("holdings"), "policy" => Ok("policies"), _ => Err(bad("unknown kind of item")) }
}

/// Sets (or, empty, clears) the nominee on an account, investment or policy.
pub fn set(conn: &Connection, kind: &str, id: &str, nominee: Option<&str>) -> Result<()> {
    let t = table(kind)?;
    let n = nominee.map(str::trim).filter(|s| !s.is_empty());
    if n.is_some_and(|s| s.chars().count() > MAX) { return Err(bad("the nominee is too long (up to 200 characters)")); }
    if conn.execute(&format!("UPDATE {t} SET nominee = ? WHERE id = ?"), params![n, id])? == 0 { return Err(bad("no such item")); }
    Ok(())
}

/// When `v` has a `nominee` key, sets it (used when an account or investment is saved).
pub fn from_input(conn: &Connection, kind: &str, id: &str, v: &Value) -> Result<()> {
    if v.get("nominee").is_some() { set(conn, kind, id, v["nominee"].as_str())?; }
    Ok(())
}

/// Every bank, cash or wallet account, open investment and active policy with its nominee
/// (`null` when none is noted), missing ones first; and how many are missing. Cash in hand
/// and cards and loans aren't listed: there's nothing to nominate.
pub fn overview(conn: &Connection, today: NaiveDate) -> Result<Value> {
    let mut items = query(conn, "SELECT 'account' AS kind, a.id, a.name, a.type AS detail, a.institution AS provider, a.currency, a.nominee
                                   FROM accounts a JOIN ledger_accounts l ON l.account_id = a.id
                                  WHERE l.kind = 'asset' AND a.type NOT IN ('cash', 'prepaid', 'gift_card')", &[])?;
    for h in crate::investments::list(conn, today)?.into_iter().filter(|h| !h["closed"].as_bool().unwrap_or(false)) {
        items.push(json!({ "kind": "holding", "id": h["id"], "name": h["name"], "detail": h["kind"], "provider": h["provider"], "currency": h["currency"], "nominee": h["nominee"] }));
    }
    items.extend(query(conn, "SELECT 'policy' AS kind, id, name, kind AS detail, insurer AS provider, currency, nominee FROM policies WHERE status = 'active'", &[])?);
    items.sort_by(|a, b| a["nominee"].is_string().cmp(&b["nominee"].is_string())
        .then(a["kind"].as_str().cmp(&b["kind"].as_str()))
        .then(a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase())));
    let missing = items.iter().filter(|i| !i["nominee"].is_string()).count();
    Ok(json!({ "items": items, "missing": missing }))
}
