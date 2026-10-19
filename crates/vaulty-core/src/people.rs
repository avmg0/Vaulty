//! People you lend to, borrow from or split bills with ("udhaar").
//!
//! Each person has a ledger account. Its balance is what they owe you (positive) or you owe them
//! (negative). Money moves through ordinary transactions: a transfer with `person_id` and `flow`
//! ("out": you lent or repaid; "in": you borrowed or were repaid), or an expense split where some
//! parts name a person (you paid the whole bill; they owe you their part).
use crate::{error::{Error, Result}, ledger, money, rows::*};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

fn name_of(v: &Value) -> Result<String> {
    let n = v["name"].as_str().map(str::trim).unwrap_or_default();
    if n.is_empty() || n.chars().count() > 60 { return Err(Error::Invalid("a name needs 1 to 60 characters".into())); }
    Ok(n.to_string())
}

fn text(v: &Value, key: &str, max: usize) -> Option<String> {
    v[key].as_str().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(max).collect())
}

/// Everyone, with what they owe (per currency; positive = they owe you) and their last activity.
pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    let people = query(conn, "SELECT p.id, p.name, p.phone, p.notes, p.is_active, l.id AS ledger_id FROM people p JOIN ledger_accounts l ON l.person_id = p.id
                               WHERE p.is_active = 1 ORDER BY p.name COLLATE NOCASE", &[])?;
    let mut out = vec![];
    for mut p in people {
        let ledger_id = p["ledger_id"].as_str().unwrap_or_default().to_string();
        let balances: Vec<Value> = conn.prepare(
            "SELECT p.currency, SUM(p.amount_minor) FROM postings p WHERE p.ledger_account_id = ? GROUP BY p.currency HAVING SUM(p.amount_minor) <> 0 ORDER BY p.currency")?
            .query_map([&ledger_id], |r| {
                let (cur, m): (String, i64) = (r.get(0)?, r.get(1)?);
                Ok(json!({ "currency": cur, "amount_minor": m, "amount": money::to_decimal(m, &cur) }))
            })?.collect::<rusqlite::Result<_>>()?;
        let last: Option<String> = conn.query_row(
            "SELECT MAX(e.date) FROM postings p JOIN entries e ON e.id = p.entry_id WHERE p.ledger_account_id = ?", [&ledger_id], |r| r.get(0))?;
        if let Some(o) = p.as_object_mut() {
            o.remove("ledger_id");
            o.insert("balances".into(), json!(balances));
            o.insert("last_date".into(), json!(last));
        }
        out.push(p);
    }
    Ok(out)
}

pub fn create(conn: &Connection, v: &Value) -> Result<Value> {
    let name = name_of(v)?;
    let id = uuid::Uuid::new_v4().to_string();
    atomic(conn, |tx| {
        tx.execute("INSERT INTO people (id, name, phone, notes) VALUES (?, ?, ?, ?)", params![id, name, text(v, "phone", 30), text(v, "notes", 500)])?;
        tx.execute("INSERT INTO ledger_accounts (id, workspace_id, kind, name, person_id) VALUES (?, ?, 'asset', ?, ?)",
            params![format!("la_person_{id}"), ledger::PERSONAL_WORKSPACE, format!("Person: {name}"), id])?;
        Ok(())
    })?;
    get(conn, &id)
}

pub fn update(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    let name = name_of(v)?;
    let n = conn.execute("UPDATE people SET name = ?, phone = ?, notes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND is_active = 1",
        params![name, text(v, "phone", 30), text(v, "notes", 500), id])?;
    if n == 0 { return Err(Error::Invalid("no such person".into())); }
    conn.execute("UPDATE ledger_accounts SET name = ? WHERE person_id = ?", params![format!("Person: {name}"), id])?;
    get(conn, id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    list(conn)?.into_iter().find(|p| p["id"] == id).ok_or_else(|| Error::Invalid("no such person".into()))
}

/// Removes someone with no history, or hides someone whose balance is settled. Refused while
/// money is still owed either way.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let ledger_id = ledger::for_person(conn, id)?;
    let (count, open): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN s <> 0 THEN 1 ELSE 0 END), 0) FROM
           (SELECT SUM(amount_minor) AS s FROM postings WHERE ledger_account_id = ?1 GROUP BY currency)",
        [&ledger_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if open > 0 { return Err(Error::Invalid("settle up first: there's still money owed".into())); }
    if count == 0 {
        conn.execute("DELETE FROM ledger_accounts WHERE person_id = ?", [id])?;
        conn.execute("DELETE FROM people WHERE id = ?", [id])?;
    } else {
        conn.execute("UPDATE people SET is_active = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?", [id])?;
    }
    Ok(())
}

/// Everything with this person, newest first, with what they owed after each (per currency).
/// `amount` is from your side: positive when they came to owe you more.
pub fn history(conn: &Connection, id: &str) -> Result<Vec<Value>> {
    let ledger_id = ledger::for_person(conn, id)?;
    let mut rows = query(conn,
        "SELECT e.id AS entry_id, e.kind, e.date, e.merchant, e.description, e.hidden, p.amount_minor, p.currency, p.memo
           FROM postings p JOIN entries e ON e.id = p.entry_id
          WHERE p.ledger_account_id = ? ORDER BY e.date, e.created_at, p.id", &[json!(ledger_id)])?;
    let mut running: std::collections::BTreeMap<String, i64> = Default::default();
    for r in rows.iter_mut() {
        let cur = r["currency"].as_str().unwrap_or("INR").to_string();
        let m = r["amount_minor"].as_i64().unwrap_or(0);
        let bal = running.entry(cur.clone()).or_default();
        *bal += m;
        let what = match (r["kind"].as_str(), m > 0) {
            (Some("transfer"), true) => "lent",
            (Some("transfer"), false) => "received",
            (_, true) => "split",
            (_, false) => "they_paid",
        };
        let hidden = r["hidden"] == 1;
        if let Some(o) = r.as_object_mut() {
            o.insert("what".into(), json!(what));
            o.insert("amount".into(), json!(money::to_decimal(m, &cur)));
            o.insert("balance".into(), json!(money::to_decimal(*bal, &cur)));
            if hidden { o.insert("merchant".into(), json!("XXXXX")); o.insert("description".into(), Value::Null); o.insert("memo".into(), Value::Null); }
            o.remove("hidden");
        }
    }
    rows.reverse();
    Ok(rows)
}

/// Total owed to you and by you, per currency, across everyone.
pub fn totals(conn: &Connection) -> Result<Vec<Value>> {
    query(conn,
        "SELECT currency,
                COALESCE(SUM(CASE WHEN s > 0 THEN s END), 0) AS owed_to_me_minor,
                COALESCE(-SUM(CASE WHEN s < 0 THEN s END), 0) AS i_owe_minor
           FROM (SELECT p.currency, SUM(p.amount_minor) AS s FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
                  JOIN people pe ON pe.id = l.person_id WHERE pe.is_active = 1 GROUP BY l.id, p.currency)
          GROUP BY currency ORDER BY currency", &[])
        .map(|rows| rows.into_iter().map(|mut r| {
            let cur = r["currency"].as_str().unwrap_or("INR").to_string();
            if let Some(o) = r.as_object_mut() {
                o.insert("owed_to_me".into(), json!(money::to_decimal(o["owed_to_me_minor"].as_i64().unwrap_or(0), &cur)));
                o.insert("i_owe".into(), json!(money::to_decimal(o["i_owe_minor"].as_i64().unwrap_or(0), &cur)));
            }
            r
        }).collect())
}

/// People someone hasn't settled with for a while: owed money and no activity in 30 days.
pub fn overdue(conn: &Connection, today: chrono::NaiveDate) -> Result<Vec<Value>> {
    let cutoff = (today - chrono::Duration::days(30)).to_string();
    Ok(list(conn)?.into_iter().filter(|p| {
        p["balances"].as_array().is_some_and(|b| b.iter().any(|x| x["amount_minor"].as_i64().unwrap_or(0) > 0))
            && p["last_date"].as_str().is_some_and(|d| d <= cutoff.as_str())
    }).collect())
}
