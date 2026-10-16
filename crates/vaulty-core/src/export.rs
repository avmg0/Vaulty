//! Backups (JSON) and spreadsheet export (CSV).
//!
//! Backup format version 2 contains the ledger tables as they are stored. Version 1 backups
//! (from before the ledger, with REAL amounts and a `transactions` table) are still accepted and
//! converted on import, exactly like migration 0004 converts an existing database.
use crate::{
    error::Result,
    ledger::{self, Kind, Posting, System},
    money,
    rows::*,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;

pub const BACKUP_VERSION: i64 = 2;

/// Tables in a backup, parents first so a restore satisfies foreign keys.
const TABLES: &[&str] = &[
    "workspaces", "accounts", "card_details", "categories", "ledger_accounts", "entries", "postings",
    "credit_cards", "recurring_payments", "tasks", "investments", "investment_events",
    "journal_entries", "goals", "calendar_events",
];

pub fn to_json(conn: &Connection) -> Result<Value> {
    let mut out = serde_json::Map::new();
    out.insert("version".into(), json!(BACKUP_VERSION));
    out.insert("exported_at".into(), json!(now()));
    for t in TABLES {
        out.insert((*t).into(), Value::Array(query(conn, &format!("SELECT * FROM {t}"), &[])?));
    }
    Ok(Value::Object(out))
}

pub fn to_csv(conn: &Connection) -> Result<(String, usize)> {
    let rows = query(
        conn,
        "SELECT t.date, t.type, t.amount_minor, t.currency, a.name AS account, ta.name AS to_account, c.name AS category,
                t.merchant, t.description, t.notes, t.visibility
         FROM transactions t
         LEFT JOIN accounts a ON a.id = t.account_id
         LEFT JOIN accounts ta ON ta.id = t.to_account_id
         LEFT JOIN categories c ON c.id = t.category_id
         ORDER BY t.date DESC",
        &[],
    )?;
    let cols = ["date", "type", "amount", "currency", "account", "to_account", "category", "merchant", "description", "notes", "visibility"];
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(cols)?;
    for r in &rows {
        let currency = r["currency"].as_str().unwrap_or("USD");
        // Exact decimal text (e.g. "1234.50"), never a float.
        let amount = money::to_string(r["amount_minor"].as_i64().unwrap_or(0), currency);
        w.write_record(cols.iter().map(|c| match *c {
            "amount" => amount.clone(),
            _ => match r.get(*c) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(v) => v.to_string(),
            },
        }))?;
    }
    let bytes = w.into_inner().map_err(|e| crate::error::Error::Invalid(e.to_string()))?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), rows.len()))
}

#[derive(Debug, Default, serde::Serialize)]
pub struct ImportCounts {
    pub transactions: usize,
    pub accounts: usize,
    pub categories: usize,
}

/// Merges a backup into the current database. Rows whose id already exists are left alone,
/// so importing the same backup twice changes nothing.
pub fn from_json(conn: &Connection, data: &Value) -> Result<ImportCounts> {
    let legacy = data.get("version").and_then(Value::as_i64).unwrap_or(1) < 2 || data.get("entries").is_none();
    atomic(conn, |tx| if legacy { import_v1(tx, data) } else { import_v2(tx, data) })
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// INSERT OR IGNORE one backup row. Column names come from a file the user chose, so only
/// plain identifiers are accepted. Returns whether a row was inserted.
fn insert_row(conn: &Connection, table: &str, row: &Value, skip: &[&str]) -> Result<bool> {
    let Some(obj) = row.as_object() else { return Ok(false) };
    let cols: Vec<&String> = obj.keys().filter(|c| !skip.contains(&c.as_str())).collect();
    if cols.is_empty() || cols.iter().any(|c| !is_identifier(c)) {
        return Ok(false);
    }
    let names = cols.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ");
    let marks = vec!["?"; cols.len()].join(", ");
    let params: Vec<Value> = cols.iter().map(|c| obj[*c].clone()).collect();
    Ok(run(conn, &format!("INSERT OR IGNORE INTO {table} ({names}) VALUES ({marks})"), &params)?.changes > 0)
}

fn import_v2(conn: &Connection, data: &Value) -> Result<ImportCounts> {
    let mut counts = ImportCounts::default();
    let mut new_entries: HashSet<String> = HashSet::new();
    for table in TABLES {
        let Some(rows) = data.get(*table).and_then(Value::as_array) else { continue };
        for row in rows {
            match *table {
                // Postings have no stable id; import them only for entries this import created,
                // so an entry that already existed is never doubled.
                "postings" => {
                    if row["entry_id"].as_str().is_some_and(|e| new_entries.contains(e)) {
                        insert_row(conn, "postings", row, &["id"])?;
                    }
                }
                "entries" => {
                    if insert_row(conn, "entries", row, &[])? {
                        let id = row["id"].as_str().unwrap_or_default().to_string();
                        if matches!(row["kind"].as_str(), Some("income" | "expense" | "transfer")) {
                            counts.transactions += 1;
                        }
                        new_entries.insert(id);
                    }
                }
                t => {
                    if insert_row(conn, t, row, &[])? {
                        match t {
                            "accounts" => counts.accounts += 1,
                            "categories" => counts.categories += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    let bad = ledger::unbalanced_entries(conn)?;
    if !bad.is_empty() {
        return Err(crate::error::Error::Invalid(format!("backup contains {} unbalanced entries; nothing was imported", bad.len())));
    }
    Ok(counts)
}

fn exists(conn: &Connection, table: &str, id: &str) -> Result<bool> {
    Ok(conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?)"), [id], |r| r.get(0))?)
}

/// Version 1 backups: REAL amounts, stored balances, single-row transactions.
fn import_v1(conn: &Connection, data: &Value) -> Result<ImportCounts> {
    let mut counts = ImportCounts::default();
    let rows = |t: &str| data.get(t).and_then(Value::as_array).cloned().unwrap_or_default();

    for c in rows("categories") {
        let Some(id) = c["id"].as_str() else { continue };
        if !exists(conn, "categories", id)? {
            crate::categories::create(conn, &c)?;
            counts.categories += 1;
        }
    }
    for a in rows("accounts") {
        let Some(id) = a["id"].as_str() else { continue };
        if exists(conn, "accounts", id)? {
            continue;
        }
        // v1 stored the opening balance with the same sign as its effect on the balance; for
        // cards and loans the new API takes the amount owed, so flip it.
        let mut a = a.clone();
        let account_type = a["type"].as_str().unwrap_or("other");
        if Kind::for_account_type(account_type) == Kind::Liability {
            if let Some(n) = a["initial_balance"].as_f64() {
                a["initial_balance"] = json!(format!("{}", -n));
            }
        }
        if let Some(n) = a["initial_balance"].as_f64() {
            let currency = a["currency"].as_str().unwrap_or("USD");
            a["initial_balance"] = json!(round_legacy(n, currency));
        }
        crate::accounts::create(conn, &a)?;
        counts.accounts += 1;
    }
    for t in rows("transactions") {
        let Some(id) = t["id"].as_str() else { continue };
        if exists(conn, "entries", id)? {
            continue;
        }
        import_v1_transaction(conn, &t)?;
        counts.transactions += 1;
    }
    Ok(counts)
}

/// A legacy REAL amount as an exact decimal string for `currency` (rounded once, half away from zero).
fn round_legacy(n: f64, currency: &str) -> String {
    let minor = (n * money::scale(currency) as f64).round() as i64;
    money::to_string(minor, currency)
}

/// Same rules as migration 0004: zero amounts are kept; transfers without a destination stay
/// balance-neutral; a missing category becomes "uncategorised".
fn import_v1_transaction(conn: &Connection, t: &Value) -> Result<()> {
    let kind = match t["type"].as_str() {
        Some(k @ ("income" | "expense" | "transfer")) => k,
        _ => return Ok(()),
    };
    let Some(account_id) = t["account_id"].as_str() else { return Ok(()) };
    let Ok((acc, _, currency)) = ledger::for_account(conn, account_id) else { return Ok(()) };
    let amount = (t["amount"].as_f64().unwrap_or(0.0).abs() * money::scale(&currency) as f64).round() as i64;
    let p = |id: String, m: i64, memo: Option<&str>| Posting {
        ledger_account_id: id,
        amount_minor: m,
        currency: currency.clone(),
        memo: memo.map(String::from),
    };
    let postings = match kind {
        "transfer" => vec![p(acc.clone(), -amount, None), p(acc, amount, Some("migrated: transfer destination unknown"))],
        _ => {
            let income = kind == "income";
            let cat = t["category_id"]
                .as_str()
                .and_then(|c| ledger::for_category(conn, c).ok())
                .filter(|(_, k)| (*k == Kind::Income) == income)
                .map(|(id, _)| id);
            let cat = match cat {
                Some(c) => c,
                None => ledger::system_account(
                    conn,
                    ledger::PERSONAL_WORKSPACE,
                    if income { System::UncategorizedIncome } else { System::UncategorizedExpense },
                )?,
            };
            if income { vec![p(acc, amount, None), p(cat, -amount, None)] } else { vec![p(cat, amount, None), p(acc, -amount, None)] }
        }
    };
    let s = |k: &str| t.get(k).and_then(Value::as_str);
    conn.execute(
        "INSERT INTO entries (id, kind, date, time, merchant, description, notes, tags, is_pending, is_cleared, visibility)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            s("id"),
            kind,
            s("date").unwrap_or("1970-01-01"),
            s("time"),
            s("merchant"),
            s("description"),
            s("notes"),
            s("tags"),
            bool_field(t, "is_pending", false),
            bool_field(t, "is_cleared", true),
            s("visibility").filter(|v| matches!(*v, "private" | "shared" | "public")).unwrap_or("private"),
        ],
    )?;
    ledger::post(conn, s("id").unwrap_or_default(), &postings)
}
