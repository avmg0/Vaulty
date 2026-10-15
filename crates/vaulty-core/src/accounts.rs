use crate::{error::Result, rows::*};
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn get_all(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, "SELECT * FROM accounts WHERE is_active = 1 ORDER BY created_at DESC", &[])
}

pub fn create(conn: &Connection, a: &Value) -> Result<Value> {
    let id = new_id();
    let initial = num_field(a, "initial_balance").unwrap_or(0.0);
    conn.execute(
        "INSERT INTO accounts (id, name, type, currency, initial_balance, current_balance, color, icon, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            require_str(a, "name")?,
            require_str(a, "type")?,
            str_field(a, "currency").unwrap_or("USD"),
            initial,
            str_field(a, "color").unwrap_or("#3b82f6"),
            str_field(a, "icon").unwrap_or("💰"),
            str_field(a, "notes"),
        ],
    )?;
    Ok(query_one(conn, "SELECT * FROM accounts WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

pub fn update(conn: &Connection, id: &str, a: &Value) -> Result<Value> {
    conn.execute(
        "UPDATE accounts SET name = ?, type = ?, currency = ?, notes = ?, color = ?, icon = ? WHERE id = ?",
        rusqlite::params![
            require_str(a, "name")?,
            require_str(a, "type")?,
            str_field(a, "currency").unwrap_or("USD"),
            str_field(a, "notes"),
            str_field(a, "color"),
            str_field(a, "icon"),
            id
        ],
    )?;
    Ok(query_one(conn, "SELECT * FROM accounts WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

/// Soft delete, as before.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE accounts SET is_active = 0 WHERE id = ?", [id])?;
    Ok(())
}
