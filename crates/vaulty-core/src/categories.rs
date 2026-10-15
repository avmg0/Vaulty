use crate::{error::Result, rows::*};
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn get_all(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, "SELECT * FROM categories ORDER BY type, name", &[])
}

pub fn create(conn: &Connection, c: &Value) -> Result<Value> {
    let id = new_id();
    conn.execute(
        "INSERT INTO categories (id, name, type, parent_id, color, icon, budget_amount) VALUES (?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            id,
            require_str(c, "name")?,
            require_str(c, "type")?,
            str_field(c, "parent_id"),
            str_field(c, "color").unwrap_or("#3b82f6"),
            str_field(c, "icon").unwrap_or("📁"),
            num_field(c, "budget_amount"),
        ],
    )?;
    Ok(query_one(conn, "SELECT * FROM categories WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

pub fn update(conn: &Connection, id: &str, c: &Value) -> Result<Value> {
    conn.execute(
        "UPDATE categories SET name = ?, type = ?, parent_id = ?, color = ?, icon = ?, budget_amount = ?
         WHERE id = ? AND is_system = 0",
        rusqlite::params![
            require_str(c, "name")?,
            require_str(c, "type")?,
            str_field(c, "parent_id"),
            str_field(c, "color"),
            str_field(c, "icon"),
            num_field(c, "budget_amount"),
            id
        ],
    )?;
    Ok(query_one(conn, "SELECT * FROM categories WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM categories WHERE id = ? AND is_system = 0", [id])?;
    Ok(())
}
