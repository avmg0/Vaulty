//! User-facing accounts (bank, cash, card, investment…). Each one is backed by a ledger account;
//! balances are the sum of its postings, never stored.
use crate::{
    error::{Error, Result},
    ledger::{self, Kind, Posting, System},
    money,
    rows::*,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const SELECT: &str = "
  SELECT a.*, l.id AS ledger_account_id, l.kind AS ledger_kind,
    COALESCE((SELECT SUM(p.amount_minor) FROM postings p JOIN entries e ON e.id = p.entry_id
              WHERE p.ledger_account_id = l.id AND e.is_cleared = 1), 0) AS current_balance_minor,
    COALESCE((SELECT SUM(p.amount_minor) FROM postings p JOIN entries e ON e.id = p.entry_id
              WHERE p.ledger_account_id = l.id AND e.kind = 'opening'), 0) AS opening_minor
  FROM accounts a JOIN ledger_accounts l ON l.account_id = a.id";

/// Adds decimal fields for the UI:
/// - `current_balance`: what the account adds to net worth (cards and loans are negative).
/// - `initial_balance`: the opening balance as the user entered it (for cards and loans, the amount owed).
fn decorate(mut row: Value) -> Value {
    let currency = row["currency"].as_str().unwrap_or("USD").to_string();
    let liability = row["ledger_kind"] == "liability";
    let current = row["current_balance_minor"].as_i64().unwrap_or(0);
    let opening = row["opening_minor"].as_i64().unwrap_or(0);
    let initial = if liability { -opening } else { opening };
    if let Some(o) = row.as_object_mut() {
        o.insert("current_balance".into(), json!(money::to_decimal(current, &currency)));
        o.insert("initial_balance".into(), json!(money::to_decimal(initial, &currency)));
        o.insert("initial_balance_minor".into(), json!(initial));
        o.remove("opening_minor");
        // Badge info from the institution directory (monogram text and optional bundled logo file).
        let inst = o.get("institution_id").and_then(Value::as_str).and_then(crate::institutions::get);
        o.insert("institution_short".into(), json!(inst.map(|i| i.short.clone())));
        o.insert("institution_logo".into(), json!(inst.and_then(|i| i.logo.clone())));
    }
    row
}

pub fn get_all(conn: &Connection) -> Result<Vec<Value>> {
    Ok(query(conn, &format!("{SELECT} WHERE a.is_active = 1 ORDER BY a.created_at DESC"), &[])?
        .into_iter()
        .map(decorate)
        .collect())
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    query_one(conn, &format!("{SELECT} WHERE a.id = ?"), &[json!(id)])?
        .map(decorate)
        .ok_or_else(|| Error::Invalid("no such account".into()))
}

fn currency_code(a: &Value) -> Result<String> {
    let c = str_field(a, "currency").unwrap_or("USD").trim().to_ascii_uppercase();
    if c.len() != 3 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(Error::Invalid(format!("`{c}` is not a 3-letter currency code")));
    }
    Ok(c)
}

/// Opening balance as entered, converted to the signed posting on the account
/// (liabilities: the amount owed, stored as a credit).
fn opening_minor(a: &Value, currency: &str, kind: Kind) -> Result<Option<i64>> {
    match a.get("initial_balance") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(Some(0)),
        Some(v) => {
            let m = money::parse_minor(v, currency)?;
            Ok(Some(if kind == Kind::Liability { -m } else { m }))
        }
    }
}

/// Largest logo accepted, as a data URL (the app resizes uploads to a small square first).
pub const MAX_LOGO_LEN: usize = 200_000;

/// `None` = key absent (keep current), `Some(None)` = clear, `Some(Some(v))` = set.
type Change<T> = Option<Option<T>>;

fn change_str(a: &Value, key: &str) -> Change<String> {
    match a.get(key) {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) if s.trim().is_empty() => Some(None),
        Some(v) => Some(v.as_str().map(|s| s.trim().to_string())),
    }
}

fn institution_change(a: &Value) -> Result<Change<&'static crate::institutions::Institution>> {
    match change_str(a, "institution_id") {
        None => Ok(None),
        Some(None) => Ok(Some(None)),
        Some(Some(id)) => crate::institutions::get(&id)
            .map(|i| Some(Some(i)))
            .ok_or_else(|| Error::Invalid(format!("unknown bank or institution `{id}`"))),
    }
}

fn logo_change(a: &Value) -> Result<Change<String>> {
    let change = change_str(a, "logo");
    if let Some(Some(logo)) = &change {
        let ok_type = ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/webp;base64,"]
            .iter()
            .any(|p| logo.starts_with(p));
        if !ok_type {
            return Err(Error::Invalid("logo must be a PNG, JPEG or WebP image".into()));
        }
        if logo.len() > MAX_LOGO_LEN {
            return Err(Error::Invalid("logo image is too large".into()));
        }
    }
    Ok(change)
}

pub fn create(conn: &Connection, a: &Value) -> Result<Value> {
    let id = str_field(a, "id").map(String::from).unwrap_or_else(new_id);
    let name = require_str(a, "name")?;
    let account_type = require_str(a, "type")?;
    let currency = currency_code(a)?;
    let kind = Kind::for_account_type(account_type);
    let opening = opening_minor(a, &currency, kind)?.unwrap_or(0);
    let institution = institution_change(a)?.flatten();
    let logo = logo_change(a)?.flatten();

    atomic(conn, |tx| {
        tx.execute(
            "INSERT INTO accounts (id, name, type, institution, institution_id, logo, currency, color, icon, notes, include_in_total)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                name,
                account_type,
                str_field(a, "institution").or(institution.map(|i| i.name.as_str())),
                institution.map(|i| i.id.as_str()),
                logo,
                currency,
                str_field(a, "color").unwrap_or("#3b82f6"),
                str_field(a, "icon").unwrap_or("💰"),
                str_field(a, "notes"),
                bool_field(a, "include_in_total", true),
            ],
        )?;
        let ledger_id = ledger::create_account_ledger(tx, &id, name, kind)?;
        // Every credit-card account has a card (details can be added later on the Cards page).
        if account_type == "credit" {
            tx.execute(
                "INSERT INTO cards (id, kind, name, account_id, owns_account) VALUES (?, 'credit', ?, ?, 1)",
                params![format!("card_{id}"), name, id],
            )?;
        }
        set_opening(tx, &id, &ledger_id, &currency, opening, str_field(a, "opening_date"))?;
        Ok(())
    })?;
    get(conn, &id)
}

/// Partial update: only the keys present in `a` change.
pub fn update(conn: &Connection, id: &str, a: &Value) -> Result<Value> {
    let (ledger_id, old_kind, old_currency) = ledger::for_account(conn, id)?;
    let current = query_one(conn, "SELECT * FROM accounts WHERE id = ?", &[json!(id)])?
        .ok_or_else(|| Error::Invalid("no such account".into()))?;
    let keep = |key: &str| current[key].as_str().map(String::from);
    let pick = |key: &str| -> Option<String> {
        match change_str(a, key) {
            None => keep(key),
            Some(v) => v,
        }
    };

    let name = pick("name").ok_or_else(|| Error::Invalid("`name` is required".into()))?;
    let account_type = pick("type").ok_or_else(|| Error::Invalid("`type` is required".into()))?;
    let currency = match str_field(a, "currency") {
        Some(_) => currency_code(a)?,
        None => old_currency.clone(),
    };
    let has_postings: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM postings WHERE ledger_account_id = ?)",
        [&ledger_id],
        |r| r.get(0),
    )?;
    if currency != old_currency && has_postings {
        return Err(Error::Invalid("can't change the currency of an account that already has transactions".into()));
    }
    let kind = Kind::for_account_type(&account_type);
    if kind != old_kind && has_postings {
        return Err(Error::Invalid(
            "can't switch between a card/loan and a bank/cash account once it has transactions".into(),
        ));
    }
    let (institution_id, institution_name) = match institution_change(a)? {
        None => (keep("institution_id"), pick("institution")),
        Some(None) => (None, pick("institution")),
        Some(Some(i)) => (Some(i.id.clone()), Some(change_str(a, "institution").flatten().unwrap_or_else(|| i.name.clone()))),
    };
    let logo = match logo_change(a)? {
        None => keep("logo"),
        Some(v) => v,
    };

    atomic(conn, |tx| {
        tx.execute(
            "UPDATE accounts SET name = ?, type = ?, currency = ?, notes = ?, color = ?, icon = ?,
                    institution = ?, institution_id = ?, logo = ? WHERE id = ?",
            params![
                name,
                account_type,
                currency,
                pick("notes"),
                pick("color"),
                pick("icon"),
                institution_name,
                institution_id,
                logo,
                id
            ],
        )?;
        tx.execute(
            "UPDATE ledger_accounts SET name = ?, kind = ? WHERE id = ?",
            params![name, ledger::kind_str(kind), ledger_id],
        )?;
        if let Some(opening) = opening_minor(a, &currency, kind)? {
            set_opening(tx, id, &ledger_id, &currency, opening, str_field(a, "opening_date"))?;
        }
        Ok(())
    })?;
    get(conn, id)
}

/// Replaces the account's opening-balance entry (none when zero).
fn set_opening(conn: &Connection, account_id: &str, ledger_id: &str, currency: &str, minor: i64, date: Option<&str>) -> Result<()> {
    let entry_id = format!("open_{account_id}");
    conn.execute("DELETE FROM entries WHERE id = ?", [&entry_id])?;
    if minor == 0 {
        return Ok(());
    }
    let today = chrono::Utc::now().date_naive().to_string();
    conn.execute(
        "INSERT INTO entries (id, kind, date, description) VALUES (?, 'opening', ?, 'Opening balance')",
        params![entry_id, date.unwrap_or(&today)],
    )?;
    let equity = ledger::system_account(conn, ledger::PERSONAL_WORKSPACE, System::OpeningBalance)?;
    ledger::post(
        conn,
        &entry_id,
        &[
            Posting { ledger_account_id: ledger_id.into(), amount_minor: minor, currency: currency.into(), memo: None },
            Posting { ledger_account_id: equity, amount_minor: -minor, currency: currency.into(), memo: None },
        ],
    )
}

/// Soft delete: the account disappears from lists but its history stays in the ledger.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE accounts SET is_active = 0 WHERE id = ?", [id])?;
    Ok(())
}
