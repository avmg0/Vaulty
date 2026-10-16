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
    atomic(conn, |conn| apply(conn, obj))?;
    get(conn)
}

fn apply(conn: &Connection, obj: &serde_json::Map<String, Value>) -> Result<()> {
    for (k, v) in obj {
        if !EDITABLE.contains(&k.as_str()) {
            if k == "id" { continue; }
            return Err(Error::Invalid(format!("unknown or protected setting `{k}`")));
        }
        check(k, v)?;
        // `k` is whitelisted above, so interpolating the column name is safe.
        conn.execute(&format!("UPDATE user_settings SET {k} = ? WHERE id = 'default'"), [bind(v)])?;
    }
    Ok(())
}

/// Checks values whose shape other code relies on.
fn check(key: &str, v: &Value) -> Result<()> {
    let bad = |what: &str| Err(Error::Invalid(format!("{key}: {what}")));
    match key {
        "locale" => match v.as_str() {
            Some(s) if is_locale(s) => Ok(()),
            _ => bad("expected a language tag such as en-US or hi-IN"),
        },
        "currency" => match v.as_str() {
            Some(s) if s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase()) => Ok(()),
            _ => bad("expected a 3-letter ISO 4217 code such as INR"),
        },
        "start_of_week" => match v.as_str() {
            Some("sunday" | "monday" | "saturday") => Ok(()),
            _ => bad("expected sunday, monday or saturday"),
        },
        _ => Ok(()),
    }
}

/// `language[-Script][-REGION]`, e.g. `hi`, `hi-IN`, `zh-Hant-TW`, `es-419`.
pub fn is_locale(s: &str) -> bool {
    let mut parts = s.split('-');
    let lang = parts.next().unwrap_or("");
    if !(2..=3).contains(&lang.len()) || !lang.bytes().all(|b| b.is_ascii_lowercase()) { return false; }
    let rest: Vec<&str> = parts.collect();
    let (script, region) = match rest.as_slice() {
        [] => (None, None),
        [a] if a.len() == 4 => (Some(*a), None),
        [a] => (None, Some(*a)),
        [a, b] => (Some(*a), Some(*b)),
        _ => return false,
    };
    let script_ok = script.is_none_or(|s| s.len() == 4 && s.as_bytes()[0].is_ascii_uppercase() && s.bytes().skip(1).all(|b| b.is_ascii_lowercase()));
    let region_ok = region.is_none_or(|r| (r.len() == 2 && r.bytes().all(|b| b.is_ascii_uppercase())) || (r.len() == 3 && r.bytes().all(|b| b.is_ascii_digit())));
    script_ok && region_ok
}

/// The country part of a locale (`hi-IN` → `IN`), used to pick region packs.
pub fn country_of(locale: &str) -> Option<&str> {
    locale.rsplit('-').next().filter(|r| r.len() == 2 && r.bytes().all(|b| b.is_ascii_uppercase()))
}
