//! Merchant → category rules, so the same shop lands in the same category without asking.
//!
//! A rule is a piece of text the merchant name must contain ("swiggy", "bescom"). Rules are
//! learned whenever the member gives a merchant a category (the latest choice wins) and can be
//! added or removed by hand. The longest matching pattern wins, so "swiggy instamart" can go
//! to Groceries while "swiggy" stays Food.

use crate::{error::{Error, Result}, rows::*};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

/// The part of a merchant name worth remembering: lower case, letters and spaces only, no
/// order numbers or city suffixes, at most four words. "SWIGGY*ORDER 12345 BANGALORE" → "swiggy order".
pub fn key(merchant: &str) -> Option<String> {
    let cleaned: String = merchant.to_lowercase().chars().map(|c| if c.is_alphabetic() || c == '&' { c } else { ' ' }).collect();
    let words: Vec<&str> = cleaned.split_whitespace()
        .filter(|w| w.chars().count() > 1 && !["pvt", "ltd", "private", "limited", "india", "the", "inc", "llp", "co"].contains(w))
        .take(4).collect();
    let k = words.join(" ");
    (k.chars().count() >= 3).then_some(k)
}

/// The category for a merchant, if a rule matches and its category fits `kind` (income/expense).
pub fn suggest(conn: &Connection, merchant: &str, kind: &str) -> Result<Option<String>> {
    let Some(text) = key(merchant) else { return Ok(None) };
    Ok(conn.query_row(
        "SELECT r.category_id FROM category_rules r JOIN categories c ON c.id = r.category_id
         WHERE c.type = ?2 AND instr(?1, r.pattern) > 0 ORDER BY length(r.pattern) DESC, r.updated_at DESC LIMIT 1",
        params![text, kind], |r| r.get(0)).optional()?)
}

/// Remembers (or corrects) the category for a merchant. Manual rules aren't overwritten.
pub fn learn(conn: &Connection, merchant: &str, category_id: &str) -> Result<()> {
    let Some(pattern) = key(merchant) else { return Ok(()) };
    conn.execute(
        "INSERT INTO category_rules (id, pattern, category_id, source) VALUES (?1, ?2, ?3, 'learned')
         ON CONFLICT(pattern) DO UPDATE SET category_id = excluded.category_id, updated_at = CURRENT_TIMESTAMP
         WHERE category_rules.source = 'learned' AND category_rules.category_id <> excluded.category_id",
        params![new_id(), pattern, category_id],
    )?;
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, "SELECT r.*, c.name AS category_name, c.icon AS category_icon, c.type AS category_type
                 FROM category_rules r JOIN categories c ON c.id = r.category_id ORDER BY r.pattern", &[])
}

pub fn add(conn: &Connection, pattern: &str, category_id: &str) -> Result<Value> {
    let p = pattern.trim().to_lowercase();
    if p.chars().count() < 2 || p.chars().count() > 60 { return Err(Error::Invalid("a rule needs 2 to 60 characters of text to look for".into())); }
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?)", [category_id], |r| r.get(0))?;
    if !exists { return Err(Error::Invalid("no such category".into())); }
    conn.execute(
        "INSERT INTO category_rules (id, pattern, category_id, source) VALUES (?1, ?2, ?3, 'manual')
         ON CONFLICT(pattern) DO UPDATE SET category_id = excluded.category_id, source = 'manual', updated_at = CURRENT_TIMESTAMP",
        params![new_id(), p, category_id],
    )?;
    query_one(conn, "SELECT * FROM category_rules WHERE pattern = ?", &[json!(p)])?.ok_or_else(|| Error::Invalid("rule not saved".into()))
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM category_rules WHERE id = ?", [id])?;
    Ok(())
}

/// Uncategorised income and expenses (not split, not hidden) that a rule would categorise.
pub fn pending(conn: &Connection) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String, String)> = conn.prepare(
        "SELECT id, COALESCE(merchant, description, ''), type FROM transactions
         WHERE category_id IS NULL AND type IN ('income', 'expense') AND hidden = 0")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<_, _>>()?;
    let mut out = Vec::new();
    for (id, merchant, kind) in rows {
        if let Some(cat) = suggest(conn, &merchant, &kind)? { out.push((id, cat)); }
    }
    // Split transactions show no single category but aren't uncategorised.
    let split: std::collections::HashSet<String> = conn.prepare(
        "SELECT p.entry_id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id WHERE l.kind IN ('income', 'expense') GROUP BY p.entry_id HAVING COUNT(*) > 1")?
        .query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    out.retain(|(id, _)| !split.contains(id));
    Ok(out)
}

/// Applies the rules to every uncategorised transaction they match. Returns how many changed.
pub fn apply_all(conn: &Connection) -> Result<usize> {
    let todo = pending(conn)?;
    atomic(conn, |tx| {
        for (id, cat) in &todo {
            let (ledger, _) = crate::ledger::for_category(tx, cat)?;
            // Move the category side of the entry from "uncategorised" to the category's ledger account.
            tx.execute(
                "UPDATE postings SET ledger_account_id = ?1 WHERE entry_id = ?2 AND ledger_account_id IN
                   (SELECT id FROM ledger_accounts WHERE system_key IN ('uncategorized_income', 'uncategorized_expense'))",
                params![ledger, id])?;
        }
        Ok(())
    })?;
    Ok(todo.len())
}
