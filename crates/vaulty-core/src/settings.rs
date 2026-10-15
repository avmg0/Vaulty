use crate::{error::{Error, Result}, rows::*};
use rusqlite::Connection;
use serde_json::{json, Value};

/// Columns the UI may change. Keys come from the renderer, so they are never interpolated unchecked.
const EDITABLE: &[&str] = &[
    "theme", "currency", "locale", "date_format", "start_of_week", "fiscal_year_start",
    "negative_format", "transaction_sort", "auto_clear_clipboard", "spellcheck_enabled",
    "font_size", "interface_font", "monospace_font",
];

pub fn get(conn: &Connection) -> Result<Value> {
    Ok(query_one(conn, "SELECT * FROM user_settings WHERE id = 'default'", &[])?.unwrap_or_else(|| json!({})))
}

pub fn update(conn: &Connection, patch: &Value) -> Result<Value> {
    let obj = patch.as_object().ok_or_else(|| Error::Invalid("settings must be an object".into()))?;
    for (k, v) in obj {
        if !EDITABLE.contains(&k.as_str()) {
            if k == "id" { continue; }
            return Err(Error::Invalid(format!("unknown or protected setting `{k}`")));
        }
        // `k` is whitelisted above, so interpolating the column name is safe.
        conn.execute(&format!("UPDATE user_settings SET {k} = ? WHERE id = 'default'"), [bind(v)])?;
    }
    get(conn)
}
