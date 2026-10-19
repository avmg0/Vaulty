//! Size limits for anything the interface sends in.
//!
//! The interface is trusted code, but a bug or an injected script could still send something huge
//! or deeply nested. These checks keep every record a sensible size before it reaches the database,
//! so one bad request can't fill the disk, freeze the app or bloat every future backup.
use crate::error::{Error, Result};
use serde_json::Value;

/// Ordinary text: names, merchants, references, colours, ids.
pub const TEXT: usize = 500;
/// Longer free text written by a person.
pub const NOTES: usize = 10_000;
/// Images sent as data URLs. Each image field also has its own tighter limit where it's checked.
pub const IMAGE: usize = 1_000_000;
/// Items in one list (tags, split parts, selected ids...).
pub const LIST: usize = 5_000;
/// How deeply objects and lists may nest.
pub const DEPTH: usize = 8;
/// Fields in one object.
pub const FIELDS: usize = 200;
/// Passwords and recovery keys. Long passphrases are welcome; megabytes are not.
pub const PASSWORD: usize = 1_024;

fn max_for(key: &str) -> usize {
    match key {
        "notes" | "description" | "memo" | "comment" => NOTES,
        "logo" | "image" => IMAGE,
        _ => TEXT,
    }
}

fn too_long(key: &str, max: usize) -> Error {
    let what = if key.is_empty() { "a value".to_string() } else { format!("\"{key}\"") };
    Error::Invalid(format!("{what} is too long (at most {max} characters)"))
}

fn walk(v: &Value, key: &str, depth: usize) -> Result<()> {
    if depth > DEPTH {
        return Err(Error::Invalid("the request is nested too deeply".into()));
    }
    match v {
        Value::String(s) => {
            let max = max_for(key);
            // Byte length is a cheap upper bound on characters; only count when it might matter.
            if s.len() > max && s.chars().count() > max {
                return Err(too_long(key, max));
            }
        }
        Value::Array(items) => {
            if items.len() > LIST {
                return Err(Error::Invalid(format!("a list can have at most {LIST} items")));
            }
            for item in items {
                walk(item, key, depth + 1)?;
            }
        }
        Value::Object(map) => {
            if map.len() > FIELDS {
                return Err(Error::Invalid("the request has too many fields".into()));
            }
            for (k, item) in map {
                if k.len() > 100 {
                    return Err(Error::Invalid("a field name is too long".into()));
                }
                walk(item, k, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Checks a structured payload (an account, a transaction, settings...) from the interface.
pub fn check(v: &Value) -> Result<()> {
    walk(v, "", 0)
}

/// Checks one piece of text from the interface.
pub fn text(what: &str, s: &str, max: usize) -> Result<()> {
    if s.len() > max && s.chars().count() > max {
        return Err(too_long(what, max));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ordinary_records_pass() {
        check(&json!({ "name": "Everyday savings", "notes": "x".repeat(5000), "tags": ["a", "b"],
            "splits": [{ "category_id": "c", "amount": "10" }], "look": { "image": format!("data:image/png;base64,{}", "A".repeat(500_000)) } })).unwrap();
        // Non-ASCII text is measured in characters, not bytes.
        check(&json!({ "merchant": "चाय".repeat(160) })).unwrap();
    }

    #[test]
    fn oversized_input_is_refused() {
        assert!(check(&json!({ "merchant": "x".repeat(TEXT + 1) })).is_err());
        assert!(check(&json!({ "notes": "x".repeat(NOTES + 1) })).is_err());
        assert!(check(&json!({ "tags": vec!["t"; LIST + 1] })).is_err());
        let mut deep = json!(1);
        for _ in 0..20 { deep = json!([deep]); }
        assert!(check(&deep).is_err());
        let wide: serde_json::Map<String, Value> = (0..FIELDS + 1).map(|i| (format!("f{i}"), json!(1))).collect();
        assert!(check(&Value::Object(wide)).is_err());
        assert!(text("password", &"p".repeat(PASSWORD + 1), PASSWORD).is_err());
        assert!(text("password", "correct horse battery staple", PASSWORD).is_ok());
    }
}
