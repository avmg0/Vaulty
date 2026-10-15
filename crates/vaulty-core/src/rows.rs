//! Helpers to move data between SQLite and JSON, so the existing React UI
//! (which works with loosely typed objects) keeps working unchanged.
use crate::error::{Error, Result};
use rusqlite::{params_from_iter, types::{Value as Sql, ValueRef}, Connection};
use serde_json::{Map, Value};

/// JSON -> SQLite value. (rusqlite's own impl for serde_json::Value would store strings as quoted JSON.)
pub fn bind(v: &Value) -> Sql {
    match v {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(*b as i64),
        Value::Number(n) => n.as_i64().map(Sql::Integer).unwrap_or_else(|| Sql::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

/// Rejects `a; b` batches (a `;` outside string literals, other than a trailing one).
pub fn single_statement(sql: &str) -> Result<()> {
    let mut in_str = false;
    let trimmed = sql.trim_end().trim_end_matches(';');
    for ch in trimmed.chars() {
        match ch {
            '\'' => in_str = !in_str,
            ';' if !in_str => return Err(Error::Invalid("multiple SQL statements are not allowed".into())),
            _ => {}
        }
    }
    Ok(())
}

pub fn query(conn: &Connection, sql: &str, params: &[Value]) -> Result<Vec<Value>> {
    single_statement(sql)?;
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let mut rows = stmt.query(params_from_iter(params.iter().map(bind)))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut obj = Map::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            let v = match row.get_ref(i)? {
                ValueRef::Null | ValueRef::Blob(_) => Value::Null,
                ValueRef::Integer(n) => Value::from(n),
                ValueRef::Real(f) => Value::from(f),
                ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
            };
            obj.insert(name.clone(), v);
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

pub fn query_one(conn: &Connection, sql: &str, params: &[Value]) -> Result<Option<Value>> {
    Ok(query(conn, sql, params)?.into_iter().next())
}

#[derive(Debug, serde::Serialize)]
pub struct RunResult {
    pub changes: usize,
    #[serde(rename = "lastInsertRowid")]
    pub last_insert_rowid: i64,
}

pub fn run(conn: &Connection, sql: &str, params: &[Value]) -> Result<RunResult> {
    single_statement(sql)?;
    let changes = conn.execute(sql, params_from_iter(params.iter().map(bind)))?;
    Ok(RunResult { changes, last_insert_rowid: conn.last_insert_rowid() })
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

pub fn require_str<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    str_field(v, key).ok_or_else(|| Error::Invalid(format!("`{key}` is required")))
}

pub fn num_field(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

pub fn bool_field(v: &Value, key: &str, default: bool) -> i64 {
    match v.get(key) {
        Some(Value::Bool(b)) => *b as i64,
        Some(Value::Number(n)) => (n.as_i64().unwrap_or(0) != 0) as i64,
        _ => default as i64,
    }
}
