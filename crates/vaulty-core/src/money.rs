//! Money is stored as whole minor units (paise, cents, fils…) in an `i64`, never as a float.
//!
//! Each currency has a fixed number of decimal places (its *exponent*): INR and USD have 2,
//! JPY has 0, BHD has 3. Anything not listed uses 2, which is right for most currencies.
//! The same table is seeded into SQLite (`currencies`, migration 0004) so SQL and Rust agree;
//! a test checks that they do.
use crate::error::{Error, Result};
use serde_json::Value;

/// Currencies without minor units (ISO 4217 exponent 0).
pub const ZERO_DECIMAL: &[&str] = &[
    "BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "PYG", "RWF", "UGX", "UYI", "VND", "VUV", "XAF", "XOF", "XPF",
];

/// Currencies with three decimal places (ISO 4217 exponent 3).
pub const THREE_DECIMAL: &[&str] = &["BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND"];

/// Largest amount accepted from input, in minor units (10^15 ≈ ₹10 lakh crore). Keeps sums far from i64 overflow.
pub const MAX_MINOR: i64 = 1_000_000_000_000_000;

pub fn exponent(currency: &str) -> u32 {
    let c = currency.trim().to_ascii_uppercase();
    if ZERO_DECIMAL.contains(&c.as_str()) {
        0
    } else if THREE_DECIMAL.contains(&c.as_str()) {
        3
    } else {
        2
    }
}

pub fn scale(currency: &str) -> i64 {
    10i64.pow(exponent(currency))
}

/// Parses a decimal amount ("1234.50", 1234.5, "1,234.50") into minor units, exactly.
///
/// Numbers are read from their shortest decimal representation, so `12.5` is 1250 paise with no
/// float rounding. More decimal places than the currency allows is an error rather than a
/// silent rounding, because this is money someone typed.
pub fn parse_minor(v: &Value, currency: &str) -> Result<i64> {
    let text = match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.trim().replace([',', '_'], ""),
        _ => return Err(Error::Invalid("amount must be a number".into())),
    };
    parse_str(&text, exponent(currency))
}

fn parse_str(text: &str, exp: u32) -> Result<i64> {
    let bad = || Error::Invalid(format!("`{text}` is not a valid amount"));
    let (neg, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if body.is_empty() || body.contains(['e', 'E']) {
        return Err(bad());
    }
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(bad());
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let frac_trimmed = frac_part.trim_end_matches('0');
    if frac_trimmed.len() > exp as usize {
        return Err(Error::Invalid(format!(
            "`{text}` has more than {exp} decimal place{} for this currency",
            if exp == 1 { "" } else { "s" }
        )));
    }
    let digits = format!("{}{:0<width$}", if int_part.is_empty() { "0" } else { int_part }, frac_trimmed, width = exp as usize);
    let minor: i64 = digits.parse().map_err(|_| Error::Invalid("amount is too large".into()))?;
    if minor > MAX_MINOR {
        return Err(Error::Invalid("amount is too large".into()));
    }
    Ok(if neg { -minor } else { minor })
}

/// Minor units → decimal number for display and for the UI's JSON. Exact for any amount below
/// 2^53 minor units, which `MAX_MINOR` guarantees.
pub fn to_decimal(minor: i64, currency: &str) -> f64 {
    minor as f64 / scale(currency) as f64
}

/// Minor units → canonical decimal string ("1234.50", "-0.05", "1200" for JPY).
pub fn to_string(minor: i64, currency: &str) -> String {
    let exp = exponent(currency) as usize;
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs().to_string();
    if exp == 0 {
        return format!("{sign}{abs}");
    }
    let padded = format!("{abs:0>width$}", width = exp + 1);
    let (i, f) = padded.split_at(padded.len() - exp);
    format!("{sign}{i}.{f}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_exactly() {
        assert_eq!(parse_minor(&json!(12.5), "INR").unwrap(), 1250);
        assert_eq!(parse_minor(&json!("1,234.05"), "INR").unwrap(), 123405);
        assert_eq!(parse_minor(&json!(0.1), "USD").unwrap(), 10);
        assert_eq!(parse_minor(&json!(".5"), "USD").unwrap(), 50);
        assert_eq!(parse_minor(&json!("-3.10"), "USD").unwrap(), -310);
        assert_eq!(parse_minor(&json!(1500), "JPY").unwrap(), 1500);
        assert_eq!(parse_minor(&json!("1.250"), "BHD").unwrap(), 1250);
        assert_eq!(parse_minor(&json!("2.500"), "INR").unwrap(), 250, "trailing zeros are fine");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_minor(&json!("10.005"), "INR").is_err());
        assert!(parse_minor(&json!(1.5), "JPY").is_err());
        assert!(parse_minor(&json!("1e3"), "INR").is_err());
        assert!(parse_minor(&json!("abc"), "INR").is_err());
        assert!(parse_minor(&json!(""), "INR").is_err());
        assert!(parse_minor(&json!(null), "INR").is_err());
        assert!(parse_minor(&json!("99999999999999999999"), "INR").is_err());
    }

    #[test]
    fn formats() {
        assert_eq!(to_string(123405, "INR"), "1234.05");
        assert_eq!(to_string(-5, "USD"), "-0.05");
        assert_eq!(to_string(1200, "JPY"), "1200");
        assert_eq!(to_string(1250, "KWD"), "1.250");
        assert_eq!(to_decimal(1250, "INR"), 12.5);
    }
}
