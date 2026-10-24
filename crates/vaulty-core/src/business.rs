//! Businesses, each in its own encrypted book (decision 8, settled October 2026 as separate
//! books): the business's accounts, categories, transactions and reports live in their own
//! database, opened by switching to it. The member's personal book lists their businesses with
//! each book's key wrapped under the member's data key, and keeps a personal account per
//! business showing the money put in, less the money taken out.
//!
//! Money between owner and business is recorded on both sides: in the business book against
//! the owner's capital (put in) or drawings (taken out), which are equity, not income or
//! spending; in the personal book as a transfer to or from the business's capital account.
use crate::{accounts, crypto::{self, Key}, db, error::{Error, Result}, ledger::{self, Kind, Posting}, money, rows::*};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;

pub const KINDS: &[&str] = &["sole_proprietor", "partnership", "llp", "company", "other"];
pub const MAX_BUSINESSES: i64 = 20;
const OWNER_CAPITAL: &str = "owner_capital";
const OWNER_DRAWINGS: &str = "owner_drawings";

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

/// Categories a new business book starts with (the personal ones are removed).
const CATEGORIES: &[(&str, &str, &str, &str)] = &[
    ("Sales", "income", "🧾", "#16a34a"), ("Services", "income", "🛠️", "#0d9488"), ("Interest received", "income", "🏦", "#65a30d"), ("Other income", "income", "➕", "#4d7c0f"),
    ("Purchases and stock", "expense", "📦", "#ea580c"), ("Rent", "expense", "🏢", "#b45309"), ("Salaries and wages", "expense", "👥", "#dc2626"),
    ("Utilities", "expense", "💡", "#0891b2"), ("Software and subscriptions", "expense", "💻", "#7c3aed"), ("Travel", "expense", "✈️", "#2563eb"),
    ("Professional fees", "expense", "⚖️", "#4f46e5"), ("Marketing", "expense", "📣", "#db2777"), ("Office supplies", "expense", "🖇️", "#64748b"),
    ("Repairs and maintenance", "expense", "🔧", "#a16207"), ("Bank charges", "expense", "🏧", "#475569"), ("Taxes and fees", "expense", "🏛️", "#991b1b"),
    ("Other expenses", "expense", "📁", "#78716c"),
];

/// Checks a GSTIN's shape and check digit (15 characters: state code, PAN, entity, Z, check).
pub fn valid_gstin(g: &str) -> bool {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let g = g.as_bytes();
    if g.len() != 15 || !g[..2].iter().all(u8::is_ascii_digit) { return false; }
    let Some(vals) = g.iter().map(|c| CHARS.iter().position(|x| x == c)).collect::<Option<Vec<_>>>() else { return false };
    let sum: usize = vals[..14].iter().enumerate().map(|(i, v)| { let p = v * if i % 2 == 0 { 1 } else { 2 }; p / 36 + p % 36 }).sum();
    vals[14] == (36 - sum % 36) % 36
}

struct Details { name: String, kind: String, currency: String, gstin: Option<String>, pan: Option<String>, state: Option<String>, address: Option<String>, fy: i64 }

fn details(v: &Value, currency_default: Option<&str>) -> Result<Details> {
    let text = |k: &str, max: usize| -> Result<Option<String>> {
        match v[k].as_str().map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) if s.chars().count() > max => Err(bad("one of the details is too long")),
            s => Ok(s.map(String::from)),
        }
    };
    let name = text("name", 100)?.ok_or_else(|| bad("give the business a name"))?;
    let kind = v["kind"].as_str().unwrap_or("sole_proprietor");
    if !KINDS.contains(&kind) { return Err(bad("unknown kind of business")); }
    let currency = v["currency"].as_str().or(currency_default).filter(|c| c.len() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase())).ok_or_else(|| bad("choose a currency"))?.to_string();
    let gstin = text("gstin", 15)?.map(|g| g.to_uppercase());
    if let Some(g) = &gstin { if !valid_gstin(g) { return Err(bad("that GSTIN doesn't look right (15 characters, with a check digit at the end)")); } }
    let pan = text("pan", 10)?.map(|p| p.to_uppercase());
    if let Some(p) = &pan { if !(p.len() == 10 && p[..5].chars().all(|c| c.is_ascii_uppercase()) && p[5..9].chars().all(|c| c.is_ascii_digit()) && p[9..].chars().all(|c| c.is_ascii_uppercase())) { return Err(bad("that PAN doesn't look right (like ABCDE1234F)")); } }
    // The state comes from the GSTIN when there is one.
    let state = gstin.as_ref().map(|g| g[..2].to_string()).or(text("state_code", 2)?);
    let fy = v["fy_start_month"].as_i64().unwrap_or(4);
    if !(1..=12).contains(&fy) { return Err(bad("the financial year starts in a month from 1 to 12")); }
    Ok(Details { name, kind: kind.into(), currency, gstin, pan, state, address: text("address", 300)?, fy })
}

/// The member's businesses (in their personal book), newest last.
pub fn list(personal: &Connection) -> Result<Vec<Value>> {
    query(personal, "SELECT b.id, b.name, b.kind, b.currency, b.gstin, b.pan, b.state_code, b.address, b.fy_start_month, b.capital_account_id, b.created_at, b.archived_at,
                            a.name AS capital_account_name
                       FROM businesses b LEFT JOIN accounts a ON a.id = b.capital_account_id ORDER BY b.archived_at IS NOT NULL, b.created_at", &[])
}

/// Creates a business: its own book at `path`, encrypted with a new key that's kept wrapped
/// under `data_key` in the personal book, and a personal capital account for it. Returns the
/// business's id and its open book.
pub fn create(personal: &Connection, data_key: &Key, path_for: impl Fn(&str) -> std::path::PathBuf, v: &Value) -> Result<(String, Connection)> {
    let n: i64 = personal.query_row("SELECT count(*) FROM businesses WHERE archived_at IS NULL", [], |r| r.get(0))?;
    if n >= MAX_BUSINESSES { return Err(bad("you can keep up to 20 businesses")); }
    let main: Option<String> = personal.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get(0)).optional()?.flatten();
    let d = details(v, main.as_deref())?;
    let id = new_id();
    let key = crypto::new_key();
    let path = path_for(&id);
    if path.exists() { return Err(bad("a book for that business already exists")); }
    let book = db::open(&path, &key)?;
    let setup = || -> Result<()> {
        atomic(&book, |b| {
            b.execute("UPDATE workspaces SET name = ?, kind = 'business' WHERE id = ?", params![d.name, ledger::PERSONAL_WORKSPACE])?;
            b.execute("UPDATE user_settings SET currency = ? WHERE id = 'default'", [&d.currency])?;
            b.execute("INSERT INTO business_profile (id, business_id, name, kind, gstin, pan, state_code, address, fy_start_month) VALUES ('self', ?, ?, ?, ?, ?, ?, ?, ?)",
                params![id, d.name, d.kind, d.gstin, d.pan, d.state, d.address, d.fy])?;
            // A business's own categories in place of the personal ones (nothing uses them yet).
            b.execute("DELETE FROM categories", [])?;
            for (name, kind, icon, color) in CATEGORIES {
                crate::categories::create(b, &json!({ "name": name, "type": kind, "icon": icon, "color": color }))?;
            }
            for (key, name) in [(OWNER_CAPITAL, "Owner's capital"), (OWNER_DRAWINGS, "Owner's drawings")] {
                b.execute("INSERT INTO ledger_accounts (id, workspace_id, kind, name, system_key) VALUES (?, ?, 'equity', ?, ?)",
                    params![new_id(), ledger::PERSONAL_WORKSPACE, name, key])?;
            }
            Ok(())
        })?;
        atomic(personal, |p| {
            let cap = accounts::create(p, &json!({ "name": format!("{} (capital)", d.name), "type": "business", "currency": d.currency, "initial_balance": "0", "icon": "🏢" }))?;
            p.execute("INSERT INTO businesses (id, name, kind, currency, gstin, pan, state_code, address, fy_start_month, wrapped_key, capital_account_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![id, d.name, d.kind, d.currency, d.gstin, d.pan, d.state, d.address, d.fy, crypto::wrap(data_key, &key)?, cap["id"].as_str()])?;
            Ok(())
        })
    };
    if let Err(e) = setup() {
        drop(book);
        for suffix in ["", "-wal", "-shm"] { db::shred(Path::new(&format!("{}{suffix}", path.display()))); }
        return Err(e);
    }
    Ok((id, book))
}

/// The key of a business's book.
pub fn key_of(personal: &Connection, data_key: &Key, id: &str) -> Result<Key> {
    let wrapped: String = personal.query_row("SELECT wrapped_key FROM businesses WHERE id = ?", [id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such business"))?;
    crypto::unwrap(data_key, &wrapped)
}

/// Opens a business's book.
pub fn open(personal: &Connection, data_key: &Key, path: &Path, id: &str) -> Result<Connection> {
    let key = key_of(personal, data_key, id)?;
    if !path.exists() { return Err(bad("this business's book is missing from this device")); }
    db::open(path, &key)
}

/// Changes a business's details, in the personal list and in its own book.
pub fn update(personal: &Connection, book: &Connection, id: &str, v: &Value) -> Result<()> {
    let cur: String = personal.query_row("SELECT currency FROM businesses WHERE id = ?", [id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such business"))?;
    let d = details(v, Some(&cur))?;
    if d.currency != cur { return Err(bad("a business's currency can't be changed")); }
    personal.execute("UPDATE businesses SET name = ?, kind = ?, gstin = ?, pan = ?, state_code = ?, address = ?, fy_start_month = ? WHERE id = ?",
        params![d.name, d.kind, d.gstin, d.pan, d.state, d.address, d.fy, id])?;
    book.execute("UPDATE business_profile SET name = ?, kind = ?, gstin = ?, pan = ?, state_code = ?, address = ?, fy_start_month = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 'self'",
        params![d.name, d.kind, d.gstin, d.pan, d.state, d.address, d.fy])?;
    book.execute("UPDATE workspaces SET name = ? WHERE id = ?", params![d.name, ledger::PERSONAL_WORKSPACE])?;
    Ok(())
}

/// The business's own details, from inside its book (`null` in a personal book).
pub fn profile(book: &Connection) -> Result<Option<Value>> {
    query_one(book, "SELECT business_id AS id, name, kind, gstin, pan, state_code, address, fy_start_month FROM business_profile WHERE id = 'self'", &[])
}

fn equity(book: &Connection, key: &str) -> Result<String> {
    book.query_row("SELECT id FROM ledger_accounts WHERE system_key = ?", [key], |r| r.get(0)).optional()?
        .ok_or_else(|| bad("this book isn't a business's"))
}

/// Money between owner and business: `direction` "in" (the owner put money in) or "out" (took
/// it out), `account_id` (the business's bank or cash account), `amount`, `date`, `note`, and
/// `personal_account_id` (the personal account it came from or went to; optional). Recorded in
/// the business book against capital or drawings and, with a personal account, in the personal
/// book as a transfer with the business's capital account. Returns both entries' ids.
pub fn owner_money(personal: &Connection, book: &Connection, business_id: &str, v: &Value) -> Result<Value> {
    let inward = match v["direction"].as_str() { Some("in") => true, Some("out") => false, _ => return Err(bad("say whether money went in or out")) };
    let account = v["account_id"].as_str().ok_or_else(|| bad("choose the business account"))?;
    let (acc_ledger, kind, cur) = ledger::for_account(book, account).map_err(|_| bad("no such account"))?;
    if kind != Kind::Asset { return Err(bad("choose a bank or cash account")); }
    let amount = money::parse_minor(v.get("amount").unwrap_or(&Value::Null), &cur)?;
    if amount <= 0 { return Err(bad("give the amount")); }
    let date = v["date"].as_str().and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()).ok_or_else(|| bad("give the date as YYYY-MM-DD"))?.to_string();
    let note = v["note"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(200).collect::<String>());
    let (cap_account, biz_name): (Option<String>, String) = personal.query_row("SELECT capital_account_id, name FROM businesses WHERE id = ?", [business_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?.ok_or_else(|| bad("no such business"))?;
    let label = if inward { "Owner put money in" } else { "Owner took money out" };
    let p = |l: String, m: i64| Posting { ledger_account_id: l, amount_minor: m, currency: cur.clone(), memo: None };
    let biz_id = new_id();
    atomic(book, |b| {
        let eq = equity(b, if inward { OWNER_CAPITAL } else { OWNER_DRAWINGS })?;
        b.execute("INSERT INTO entries (id, kind, date, merchant, description, notes) VALUES (?, 'transfer', ?, ?, ?, ?)", params![biz_id, date, "Owner", label, note])?;
        ledger::post(b, &biz_id, &if inward { vec![p(acc_ledger.clone(), amount), p(eq, -amount)] } else { vec![p(eq, amount), p(acc_ledger.clone(), -amount)] })
    })?;
    let mut personal_id = None;
    if let Some(pa) = v["personal_account_id"].as_str().filter(|s| !s.is_empty()) {
        let cap = cap_account.ok_or_else(|| bad("this business has no capital account in your personal book"))?;
        let (from, to) = if inward { (pa, cap.as_str()) } else { (cap.as_str(), pa) };
        let made = crate::transactions::create(personal, &json!({ "type": "transfer", "account_id": from, "to_account_id": to, "amount": money::to_string(amount, &cur),
            "date": date, "merchant": biz_name, "description": if inward { "Put into the business" } else { "Taken from the business" }, "notes": note }));
        match made {
            Ok(t) => personal_id = t["id"].as_str().map(str::to_string),
            // Keep the two books in step: undo the business side if the personal side fails.
            Err(e) => { book.execute("DELETE FROM entries WHERE id = ?", [&biz_id])?; return Err(e); }
        }
    }
    Ok(json!({ "business_entry": biz_id, "personal_entry": personal_id }))
}

/// Capital put in, drawings taken out and the net, in the business book.
pub fn owner_totals(book: &Connection) -> Result<Value> {
    let cur: String = book.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get::<_, Option<String>>(0))?.unwrap_or_else(|| "INR".into());
    let sum = |key: &str| -> Result<i64> {
        Ok(book.query_row("SELECT coalesce(sum(p.amount_minor), 0) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id WHERE l.system_key = ?", [key], |r| r.get(0))?)
    };
    let (capital, drawings) = (-sum(OWNER_CAPITAL)?, sum(OWNER_DRAWINGS)?);
    Ok(json!({ "currency": cur, "capital": money::to_decimal(capital, &cur), "drawings": money::to_decimal(drawings, &cur), "net": money::to_decimal(capital - drawings, &cur) }))
}

/// Puts a business aside (it disappears from the switcher; its book is kept), or back.
pub fn set_archived(personal: &Connection, id: &str, archived: bool) -> Result<()> {
    if personal.execute("UPDATE businesses SET archived_at = CASE WHEN ? THEN CURRENT_TIMESTAMP ELSE NULL END WHERE id = ?", params![archived, id])? == 0 { return Err(bad("no such business")); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gstin_check_digit() {
        assert!(valid_gstin("27AAPFU0939F1ZV"));
        assert!(!valid_gstin("27AAPFU0939F1ZW"));
        assert!(!valid_gstin("27AAPFU0939F1Z"));
    }
}
