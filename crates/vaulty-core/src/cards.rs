//! Cards of every kind: credit, debit, prepaid (incl. forex), gift and other (membership,
//! loyalty, store). Each card has an optional network, last 4 digits, expiry and a look
//! (preset finish, solid colour, gradient or the user's own image).
//!
//! Money lives in accounts, so cards point at one:
//! - credit: its own liability account (what you owe); limit and billing cycle live on the card
//! - debit: an existing bank account (several cards may share it)
//! - prepaid / gift: their own asset account holding the remaining balance
//! - other: no account
//!
//! Full card numbers are never stored or accepted. The UI detects the network as the user types
//! and saves only `network` and `last4`.
use crate::{
    accounts,
    error::{Error, Result},
    money,
    rows::*,
};
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};

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

pub const KINDS: &[&str] = &["credit", "debit", "prepaid", "gift", "other"];
/// Largest custom background image accepted, as a data URL (the app crops and compresses first).
pub const IMAGE_MAX_LEN: usize = 900_000;

fn is_hex_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Validates a card look and returns it as canonical JSON.
///
/// `{"preset": "midnight"}`, or a custom fill:
/// `{"fill": "solid", "colors": ["#1e3a8a"]}`,
/// `{"fill": "gradient", "colors": ["#0f766e", "#7c3aed", "#db2777"], "angle": 135}`,
/// `{"fill": "image", "image": "data:image/jpeg;base64,…", "dim": 0.2}`,
/// each with optional `"ink": "light" | "dark"` (text colour) and `"pattern": bool` (engraved lines).
pub fn normalize_look(v: &Value) -> Result<String> {
    let bad = |m: &str| Error::Invalid(format!("card background: {m}"));
    if let Some(p) = v.get("preset").and_then(Value::as_str) {
        if !DESIGNS.contains(&p) {
            return Err(bad(&format!("unknown preset `{p}`")));
        }
        return Ok(json!({ "preset": p }).to_string());
    }
    let fill = v.get("fill").and_then(Value::as_str).ok_or_else(|| bad("choose a preset, colour, gradient or image"))?;
    let ink = match v.get("ink").and_then(Value::as_str).unwrap_or("light") {
        i @ ("light" | "dark") => i,
        _ => return Err(bad("text colour must be light or dark")),
    };
    let pattern = v.get("pattern").and_then(Value::as_bool).unwrap_or(fill != "image");
    let colours = || -> Result<Vec<String>> {
        let list = v.get("colors").and_then(Value::as_array).ok_or_else(|| bad("colours are missing"))?;
        list.iter()
            .map(|c| c.as_str().filter(|s| is_hex_colour(s)).map(|s| s.to_ascii_lowercase()).ok_or_else(|| bad("colours must look like #1e3a8a")))
            .collect()
    };
    let mut out = Map::new();
    out.insert("fill".into(), json!(fill));
    out.insert("ink".into(), json!(ink));
    out.insert("pattern".into(), json!(pattern));
    match fill {
        "solid" => {
            let c = colours()?;
            if c.len() != 1 {
                return Err(bad("a solid fill takes one colour"));
            }
            out.insert("colors".into(), json!(c));
        }
        "gradient" => {
            let c = colours()?;
            if !(2..=3).contains(&c.len()) {
                return Err(bad("a gradient takes two or three colours"));
            }
            let angle = v.get("angle").and_then(Value::as_f64).unwrap_or(135.0);
            if !(0.0..=360.0).contains(&angle) {
                return Err(bad("angle must be between 0 and 360"));
            }
            out.insert("colors".into(), json!(c));
            out.insert("angle".into(), json!(angle.round()));
        }
        "image" => {
            let img = v.get("image").and_then(Value::as_str).ok_or_else(|| bad("image is missing"))?;
            let ok = ["data:image/jpeg;base64,", "data:image/png;base64,", "data:image/webp;base64,"].iter().any(|p| img.starts_with(p));
            if !ok {
                return Err(bad("image must be a JPEG, PNG or WebP"));
            }
            if img.len() > IMAGE_MAX_LEN {
                return Err(bad("image is too large"));
            }
            let dim = v.get("dim").and_then(Value::as_f64).unwrap_or(0.15);
            if !(0.0..=0.8).contains(&dim) {
                return Err(bad("dimming must be between 0 and 0.8"));
            }
            out.insert("image".into(), json!(img));
            out.insert("dim".into(), json!((dim * 100.0).round() / 100.0));
        }
        other => return Err(bad(&format!("unknown fill `{other}`"))),
    }
    Ok(Value::Object(out).to_string())
}

fn day(v: &Value, key: &str) -> Result<Option<i64>> {
    int_in(v, key, 1, 31, "a day of the month (1–31)")
}

fn int_in(v: &Value, key: &str, lo: i64, hi: i64, what: &str) -> Result<Option<i64>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(x) => match x.as_i64().or_else(|| x.as_str().and_then(|s| s.trim().parse().ok())) {
            Some(d) if (lo..=hi).contains(&d) => Ok(Some(d)),
            _ => Err(Error::Invalid(format!("`{key}` must be {what}"))),
        },
    }
}

fn kind_of(v: &Value) -> Result<&str> {
    let k = require_str(v, "kind")?;
    if KINDS.contains(&k) { Ok(k) } else { Err(Error::Invalid(format!("unknown card type `{k}`"))) }
}

fn default_name(kind: &str) -> &'static str {
    match kind {
        "credit" => "Credit card",
        "debit" => "Debit card",
        "prepaid" => "Prepaid card",
        "gift" => "Gift card",
        _ => "Card",
    }
}

/// A debit card must point at an active bank / cash style (asset) account.
fn check_debit_account(conn: &Connection, account_id: &str) -> Result<()> {
    let a = accounts::get(conn, account_id)?;
    if a["ledger_kind"] != "asset" || a["is_active"] == json!(0) {
        return Err(Error::Invalid("a debit card must be linked to an active bank account".into()));
    }
    Ok(())
}

/// Adds a card. Credit, prepaid and gift cards get their own account (with `owed` or
/// `balance` as the opening amount); debit cards link to `account_id`; other cards have none.
pub fn create(conn: &Connection, v: &Value) -> Result<Value> {
    let kind = kind_of(v)?.to_string();
    if v.get("number").is_some() || v.get("card_number").is_some() {
        return Err(Error::Invalid("full card numbers are never stored; send only `last4` and `network`".into()));
    }
    let name = str_field(v, "name").unwrap_or(default_name(&kind)).to_string();
    let currency = str_field(v, "currency").unwrap_or("INR").to_string();
    let institution = str_field(v, "institution_id");

    let id = atomic(conn, |tx| {
        let id = match kind.as_str() {
            "credit" => {
                // Creating a credit account also creates its card (see accounts::create).
                let acc = accounts::create(
                    tx,
                    &json!({"name": name, "type": "credit", "currency": currency, "institution_id": institution, "initial_balance": v.get("owed")}),
                )?;
                format!("card_{}", acc["id"].as_str().unwrap_or_default())
            }
            "prepaid" | "gift" => {
                let acc = accounts::create(
                    tx,
                    &json!({"name": name, "type": if kind == "gift" { "gift_card" } else { "prepaid" }, "currency": currency,
                            "institution_id": institution, "initial_balance": v.get("balance")}),
                )?;
                let id = new_id();
                tx.execute(
                    "INSERT INTO cards (id, kind, name, account_id, owns_account) VALUES (?, ?, ?, ?, 1)",
                    params![id, kind, name, acc["id"].as_str()],
                )?;
                id
            }
            "debit" => {
                let account_id = require_str(v, "account_id").map_err(|_| Error::Invalid("choose the bank account this debit card uses".into()))?;
                check_debit_account(tx, account_id)?;
                let id = new_id();
                tx.execute(
                    "INSERT INTO cards (id, kind, name, account_id, owns_account) VALUES (?, 'debit', ?, ?, 0)",
                    params![id, name, account_id],
                )?;
                id
            }
            _ => {
                let id = new_id();
                tx.execute("INSERT INTO cards (id, kind, name) VALUES (?, 'other', ?)", params![id, name])?;
                id
            }
        };
        // Everything else (network, last 4, expiry, look, limit…) goes through the same validation as edits.
        let mut rest = v.clone();
        if let Some(o) = rest.as_object_mut() {
            for k in ["kind", "name", "owed", "balance", "currency", "account_id"] {
                o.remove(k);
            }
        }
        update(tx, &id, &rest)?;
        Ok(id)
    })?;
    get(conn, &id)
}

/// Partial update: only the keys present change.
pub fn update(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    if v.get("number").is_some() || v.get("card_number").is_some() {
        return Err(Error::Invalid("full card numbers are never stored; send only `last4` and `network`".into()));
    }
    let card = query_one(conn, "SELECT * FROM cards WHERE id = ?", &[json!(id)])?.ok_or_else(|| Error::Invalid("no such card".into()))?;
    let kind = card["kind"].as_str().unwrap_or("other").to_string();
    let owned_account = if card["owns_account"] == json!(1) { card["account_id"].as_str().map(String::from) } else { None };
    let currency = match card["account_id"].as_str() {
        Some(a) => accounts::get(conn, a)?["currency"].as_str().unwrap_or("USD").to_string(),
        None => "USD".to_string(),
    };

    let mut sets: Vec<(&'static str, Value)> = Vec::new();
    let has = |k: &str| v.get(k).is_some();
    let opt_str = |k: &str| -> Value {
        match v.get(k) {
            Some(Value::String(s)) if !s.trim().is_empty() => json!(s.trim()),
            _ => Value::Null,
        }
    };

    if has("name") {
        let name = str_field(v, "name").ok_or_else(|| Error::Invalid("card name can't be empty".into()))?;
        sets.push(("name", json!(name)));
    }
    if has("institution_id") {
        let inst = opt_str("institution_id");
        if let Some(i) = inst.as_str() {
            if crate::institutions::get(i).is_none() {
                return Err(Error::Invalid(format!("unknown bank or institution `{i}`")));
            }
        }
        sets.push(("institution_id", inst));
    }
    if has("issuer_name") {
        sets.push(("issuer_name", opt_str("issuer_name")));
    }
    if has("notes") {
        sets.push(("notes", opt_str("notes")));
    }
    if has("network") {
        let n = opt_str("network");
        if let Some(s) = n.as_str() {
            if !NETWORKS.contains(&s) {
                return Err(Error::Invalid(format!("unknown card network `{s}`")));
            }
        }
        sets.push(("network", n));
    }
    if has("last4") {
        let l = opt_str("last4");
        if let Some(s) = l.as_str() {
            if s.len() != 4 || !s.chars().all(|c| c.is_ascii_digit()) {
                return Err(Error::Invalid("`last4` must be exactly 4 digits".into()));
            }
        }
        sets.push(("last4", l));
    }
    if has("expiry_month") {
        sets.push(("expiry_month", json!(int_in(v, "expiry_month", 1, 12, "a month (1–12)")?)));
    }
    if has("expiry_year") {
        let y = int_in(v, "expiry_year", 0, 2100, "a year")?.map(|y| if y < 100 { 2000 + y } else { y });
        if y.is_some_and(|y| y < 2000) {
            return Err(Error::Invalid("`expiry_year` must be a year".into()));
        }
        sets.push(("expiry_year", json!(y)));
    }
    if has("look") {
        sets.push(("look", json!(normalize_look(&v["look"])?)));
    }
    for key in ["credit_limit", "statement_day", "due_day"] {
        if has(key) && kind != "credit" && !v[key].is_null() {
            return Err(Error::Invalid(format!("`{key}` only applies to credit cards")));
        }
    }
    if has("credit_limit") {
        let limit = match &v["credit_limit"] {
            Value::Null => None,
            Value::String(s) if s.trim().is_empty() => None,
            x => {
                let m = money::parse_minor(x, &currency)?;
                if m < 0 {
                    return Err(Error::Invalid("credit limit can't be negative".into()));
                }
                Some(m)
            }
        };
        sets.push(("credit_limit_minor", json!(limit)));
    }
    if has("statement_day") {
        sets.push(("statement_day", json!(day(v, "statement_day")?)));
    }
    if has("due_day") {
        sets.push(("due_day", json!(day(v, "due_day")?)));
    }
    if has("account_id") {
        if kind != "debit" {
            return Err(Error::Invalid("only a debit card's linked account can be changed".into()));
        }
        let a = require_str(v, "account_id")?;
        check_debit_account(conn, a)?;
        sets.push(("account_id", json!(a)));
    }

    atomic(conn, |tx| {
        if !sets.is_empty() {
            // Column names are fixed literals above; values are bound.
            let clause = sets.iter().map(|(c, _)| format!("{c} = ?")).collect::<Vec<_>>().join(", ");
            let mut params: Vec<Value> = sets.iter().map(|(_, v)| v.clone()).collect();
            params.push(json!(id));
            run(tx, &format!("UPDATE cards SET {clause} WHERE id = ?"), &params)?;
        }
        // Keep the card's own account in step with its name and bank.
        if let Some(acc) = &owned_account {
            let mut patch = Map::new();
            if let Some((_, n)) = sets.iter().find(|(c, _)| *c == "name") {
                patch.insert("name".into(), n.clone());
            }
            if let Some((_, i)) = sets.iter().find(|(c, _)| *c == "institution_id") {
                patch.insert("institution_id".into(), i.clone());
            }
            if !patch.is_empty() {
                accounts::update(tx, acc, &Value::Object(patch))?;
            }
        }
        Ok(())
    })?;
    get(conn, id)
}

/// Removes a card. A card's own account (credit, prepaid, gift) is archived with it; a debit
/// card's bank account is left alone.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let card = query_one(conn, "SELECT account_id, owns_account FROM cards WHERE id = ?", &[json!(id)])?
        .ok_or_else(|| Error::Invalid("no such card".into()))?;
    atomic(conn, |tx| {
        if card["owns_account"] == json!(1) {
            if let Some(a) = card["account_id"].as_str() {
                accounts::delete(tx, a)?;
            }
        }
        tx.execute("DELETE FROM cards WHERE id = ?", [id])?;
        Ok(())
    })
}

/// Card with derived fields for the UI:
/// - all: `look` (object), `currency`, institution name/short/logo, `account_name`
/// - credit: `owed`, `credit_limit`, `available`, `utilization` (0–1, null without a limit)
/// - debit / prepaid / gift: `balance` (the linked or own account's balance)
fn decorate(conn: &Connection, mut card: Value) -> Result<Value> {
    let account = match card["account_id"].as_str() {
        Some(a) => accounts::get(conn, a).ok(),
        None => None,
    };
    let currency = account.as_ref().and_then(|a| a["currency"].as_str()).unwrap_or("USD").to_string();
    let institution_id = card["institution_id"]
        .as_str()
        .or_else(|| account.as_ref().and_then(|a| a["institution_id"].as_str()))
        .map(String::from);
    let inst = institution_id.as_deref().and_then(crate::institutions::get);
    let look: Value = serde_json::from_str(card["look"].as_str().unwrap_or("{}")).unwrap_or_else(|_| json!({"preset": "midnight"}));
    let o = card.as_object_mut().expect("row is an object");
    o.insert("look".into(), look);
    o.insert("currency".into(), json!(currency));
    o.insert("institution_id".into(), json!(institution_id));
    o.insert("institution".into(), json!(inst.map(|i| i.name.clone())));
    o.insert("institution_short".into(), json!(inst.map(|i| i.short.clone())));
    o.insert("institution_logo".into(), json!(inst.and_then(|i| i.logo.clone())));
    o.insert("account_name".into(), json!(account.as_ref().and_then(|a| a["name"].as_str())));
    o.insert("account_logo".into(), account.as_ref().map_or(Value::Null, |a| a["logo"].clone()));

    let balance_minor = account.as_ref().and_then(|a| a["current_balance_minor"].as_i64()).unwrap_or(0);
    if o["kind"] == "credit" {
        let owed = (-balance_minor).max(0);
        let limit = o["credit_limit_minor"].as_i64();
        let available = limit.map(|l| (l - owed).max(0));
        o.insert("owed_minor".into(), json!(owed));
        o.insert("owed".into(), json!(money::to_decimal(owed, &currency)));
        o.insert("credit_limit".into(), json!(limit.map(|l| money::to_decimal(l, &currency))));
        o.insert("available_minor".into(), json!(available));
        o.insert("available".into(), json!(available.map(|a| money::to_decimal(a, &currency))));
        o.insert("utilization".into(), json!(limit.filter(|l| *l > 0).map(|l| (owed as f64 / l as f64).min(9.99))));
    } else if account.is_some() {
        o.insert("balance_minor".into(), json!(balance_minor));
        o.insert("balance".into(), json!(money::to_decimal(balance_minor, &currency)));
    }
    Ok(card)
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    let row = query_one(conn, "SELECT * FROM cards WHERE id = ?", &[json!(id)])?.ok_or_else(|| Error::Invalid("no such card".into()))?;
    decorate(conn, row)
}

/// All cards, newest first. Cards whose own account was archived are hidden.
pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    query(
        conn,
        "SELECT c.* FROM cards c LEFT JOIN accounts a ON a.id = c.account_id
         WHERE NOT (c.owns_account = 1 AND COALESCE(a.is_active, 0) = 0)
         ORDER BY c.created_at DESC, c.rowid DESC",
        &[],
    )?
    .into_iter()
    .map(|c| decorate(conn, c))
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
