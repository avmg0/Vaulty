//! Credit cards: details that sit on top of a credit-card account (network, last 4 digits,
//! limit, billing cycle, look), plus card-network detection from the leading digits.
//!
//! Full card numbers are never stored or sent to the backend. The UI detects the network as the
//! user types and saves only `network` and `last4`.
use crate::{
    accounts,
    error::{Error, Result},
    money,
    rows::*,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub const NETWORKS: &[&str] = &["visa", "mastercard", "amex", "discover", "rupay", "diners", "jcb", "unionpay", "maestro"];
pub const DESIGNS: &[&str] = &["midnight", "ocean", "aurora", "sunset", "graphite", "emerald", "rose", "gold"];

/// Card network from the leading digits (IIN/BIN ranges). Order matters where ranges overlap:
/// RuPay's 65xxxx ranges are checked before Discover's 65.
pub fn detect_network(number: &str) -> Option<&'static str> {
    let d: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    let p = |n: usize| -> Option<u32> { d.get(..n).and_then(|s| s.parse().ok()) };
    let in_range = |n: usize, lo: u32, hi: u32| p(n).is_some_and(|v| (lo..=hi).contains(&v));

    if d.is_empty() {
        return None;
    }
    if in_range(6, 508500, 508999) || in_range(6, 606985, 607984) || in_range(6, 608001, 608500) || in_range(6, 652150, 653149)
        || in_range(2, 81, 82)
    {
        return Some("rupay");
    }
    if in_range(2, 34, 34) || in_range(2, 37, 37) {
        return Some("amex");
    }
    if in_range(2, 51, 55) || in_range(4, 2221, 2720) {
        return Some("mastercard");
    }
    if d.starts_with('4') {
        return Some("visa");
    }
    if in_range(4, 6011, 6011) || in_range(3, 644, 649) || in_range(2, 65, 65) {
        return Some("discover");
    }
    if in_range(4, 3528, 3589) {
        return Some("jcb");
    }
    if in_range(3, 300, 305) || in_range(2, 36, 36) || in_range(2, 38, 39) {
        return Some("diners");
    }
    if in_range(2, 62, 62) {
        return Some("unionpay");
    }
    if in_range(2, 50, 50) || in_range(2, 56, 58) || in_range(4, 6304, 6304) || in_range(4, 6759, 6759) {
        return Some("maestro");
    }
    None
}

/// Luhn checksum, for catching typos in a full card number before using its last 4 digits.
pub fn luhn_ok(number: &str) -> bool {
    let digits: Vec<u32> = number.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 12 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| if i % 2 == 1 { let x = d * 2; if x > 9 { x - 9 } else { x } } else { d })
        .sum();
    sum.is_multiple_of(10)
}

fn day(v: &Value, key: &str) -> Result<Option<i64>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(x) => match x.as_i64().or_else(|| x.as_str().and_then(|s| s.trim().parse().ok())) {
            Some(d) if (1..=31).contains(&d) => Ok(Some(d)),
            _ => Err(Error::Invalid(format!("`{key}` must be a day of the month (1–31)"))),
        },
    }
}

/// Saves card details for a credit-card account. Never accepts a full card number.
pub fn set_details(conn: &Connection, account_id: &str, v: &Value) -> Result<Value> {
    let account = accounts::get(conn, account_id)?;
    if account["ledger_kind"] != "liability" {
        return Err(Error::Invalid("card details can only be added to a credit card account".into()));
    }
    if v.get("number").is_some() || v.get("card_number").is_some() {
        return Err(Error::Invalid("full card numbers are never stored; send only `last4` and `network`".into()));
    }
    let currency = account["currency"].as_str().unwrap_or("USD").to_string();
    let network = match str_field(v, "network") {
        None => None,
        Some(n) if NETWORKS.contains(&n) => Some(n),
        Some(n) => return Err(Error::Invalid(format!("unknown card network `{n}`"))),
    };
    let last4 = match str_field(v, "last4") {
        None => None,
        Some(s) if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) => Some(s),
        Some(_) => return Err(Error::Invalid("`last4` must be exactly 4 digits".into())),
    };
    let limit = match v.get("credit_limit") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(x) => {
            let m = money::parse_minor(x, &currency)?;
            if m < 0 {
                return Err(Error::Invalid("credit limit can't be negative".into()));
            }
            Some(m)
        }
    };
    let design = match str_field(v, "design") {
        None => "midnight",
        Some(d) if DESIGNS.contains(&d) => d,
        Some(d) => return Err(Error::Invalid(format!("unknown card design `{d}`"))),
    };
    conn.execute(
        "INSERT INTO card_details (account_id, network, last4, credit_limit_minor, statement_day, due_day, design, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
         ON CONFLICT(account_id) DO UPDATE SET network = ?2, last4 = ?3, credit_limit_minor = ?4, statement_day = ?5,
           due_day = ?6, design = ?7, updated_at = CURRENT_TIMESTAMP",
        params![account_id, network, last4, limit, day(v, "statement_day")?, day(v, "due_day")?, design],
    )?;
    get(conn, account_id)
}

/// A credit-card account with its details and derived numbers:
/// `owed` (≥ 0), `available`, `utilization` (0–1, null without a limit).
fn decorate(conn: &Connection, account: Value) -> Result<Value> {
    let id = account["id"].as_str().unwrap_or_default().to_string();
    let details = query_one(conn, "SELECT * FROM card_details WHERE account_id = ?", &[json!(id)])?;
    let currency = account["currency"].as_str().unwrap_or("USD").to_string();
    let owed_minor = (-account["current_balance_minor"].as_i64().unwrap_or(0)).max(0);
    let limit_minor = details.as_ref().and_then(|d| d["credit_limit_minor"].as_i64());
    let mut out = account;
    if let Some(o) = out.as_object_mut() {
        for key in ["network", "last4", "statement_day", "due_day", "design"] {
            o.insert(key.into(), details.as_ref().map_or(Value::Null, |d| d[key].clone()));
        }
        if o["design"].is_null() {
            o.insert("design".into(), json!("midnight"));
        }
        o.insert("owed_minor".into(), json!(owed_minor));
        o.insert("owed".into(), json!(money::to_decimal(owed_minor, &currency)));
        o.insert("credit_limit_minor".into(), json!(limit_minor));
        o.insert("credit_limit".into(), json!(limit_minor.map(|l| money::to_decimal(l, &currency))));
        let available = limit_minor.map(|l| (l - owed_minor).max(0));
        o.insert("available_minor".into(), json!(available));
        o.insert("available".into(), json!(available.map(|a| money::to_decimal(a, &currency))));
        o.insert(
            "utilization".into(),
            json!(limit_minor.filter(|l| *l > 0).map(|l| (owed_minor as f64 / l as f64).min(9.99))),
        );
    }
    Ok(out)
}

pub fn get(conn: &Connection, account_id: &str) -> Result<Value> {
    decorate(conn, accounts::get(conn, account_id)?)
}

/// Active credit-card accounts (type `credit`), newest first.
pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    accounts::get_all(conn)?
        .into_iter()
        .filter(|a| a["type"] == "credit")
        .map(|a| decorate(conn, a))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_networks_from_standard_test_numbers() {
        for (n, want) in [
            ("4111 1111 1111 1111", "visa"),
            ("5555555555554444", "mastercard"),
            ("2223003122003222", "mastercard"),
            ("378282246310005", "amex"),
            ("6011111111111117", "discover"),
            ("6510000000000000", "discover"),
            ("6521500000000000", "rupay"),
            ("6070000000000000", "rupay"),
            ("8100000000000000", "rupay"),
            ("30569309025904", "diners"),
            ("3530111333300000", "jcb"),
            ("6200000000000005", "unionpay"),
            ("6759000000000000", "maestro"),
        ] {
            assert_eq!(detect_network(n), Some(want), "{n}");
        }
        assert_eq!(detect_network("9"), None);
        assert_eq!(detect_network(""), None);
        assert_eq!(detect_network("4"), Some("visa"), "detects as the user types");
    }

    #[test]
    fn luhn() {
        assert!(luhn_ok("4111 1111 1111 1111"));
        assert!(luhn_ok("378282246310005"));
        assert!(!luhn_ok("4111111111111112"));
        assert!(!luhn_ok("4111"));
    }
}
