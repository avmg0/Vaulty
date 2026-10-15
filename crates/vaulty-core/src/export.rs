use crate::{error::Result, rows::*};
use rusqlite::Connection;
use serde_json::{json, Value};

/// Tables included in a full JSON backup, in dependency order (parents first) for restore.
const TABLES: &[&str] = &[
    "accounts", "categories", "transactions", "credit_cards", "recurring_payments", "tasks",
    "investments", "investment_events", "journal_entries", "goals", "calendar_events",
];

pub fn to_json(conn: &Connection) -> Result<Value> {
    let mut out = serde_json::Map::new();
    out.insert("version".into(), json!(1));
    out.insert("exported_at".into(), json!(now()));
    for t in TABLES {
        out.insert((*t).into(), Value::Array(query(conn, &format!("SELECT * FROM {t}"), &[])?));
    }
    Ok(Value::Object(out))
}

pub fn to_csv(conn: &Connection) -> Result<(String, usize)> {
    let rows = query(
        conn,
        "SELECT t.date, t.type, t.amount, a.name AS account, c.name AS category, t.merchant, t.description, t.notes
         FROM transactions t LEFT JOIN accounts a ON a.id = t.account_id LEFT JOIN categories c ON c.id = t.category_id
         ORDER BY t.date DESC",
        &[],
    )?;
    let cols = ["date", "type", "amount", "account", "category", "merchant", "description", "notes"];
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(cols)?;
    for r in &rows {
        w.write_record(cols.iter().map(|c| match r.get(*c) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
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

/// Merge a JSON backup: existing ids are kept (INSERT OR IGNORE), so importing is never destructive.
pub fn from_json(conn: &Connection, data: &Value) -> Result<ImportCounts> {
    let tx = conn.unchecked_transaction()?;
    let mut counts = ImportCounts::default();
    for table in TABLES {
        let Some(rows) = data.get(*table).and_then(Value::as_array) else { continue };
        for row in rows {
            let Some(obj) = row.as_object() else { continue };
            let cols: Vec<&String> = obj.keys().collect();
            // Column names come from a file the user chose; accept only plain identifiers.
            if cols.iter().any(|c| !c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')) {
                continue;
            }
            let names = cols.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ");
            let marks = vec!["?"; cols.len()].join(", ");
            let params: Vec<Value> = cols.iter().map(|c| obj[*c].clone()).collect();
            let n = run(&tx, &format!("INSERT OR IGNORE INTO {table} ({names}) VALUES ({marks})"), &params)?.changes;
            match *table {
                "transactions" => counts.transactions += n,
                "accounts" => counts.accounts += n,
                "categories" => counts.categories += n,
                _ => {}
            }
        }
    }
    let accounts: Vec<String> = query(&tx, "SELECT id FROM accounts", &[])?
        .iter().filter_map(|r| r["id"].as_str().map(String::from)).collect();
    for a in accounts {
        crate::transactions::recompute_balance(&tx, &a)?;
    }
    tx.commit()?;
    Ok(counts)
}
