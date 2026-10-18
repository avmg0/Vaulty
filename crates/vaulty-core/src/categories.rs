//! Income and expense categories. Each one is backed by a ledger account of the same kind.
use crate::{
    error::{Error, Result},
    ledger::{self, Kind, System},
    rows::*,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub fn get_all(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, "SELECT * FROM categories ORDER BY type, name", &[])
}

fn kind(c: &Value) -> Result<(&str, Kind)> {
    match require_str(c, "type")? {
        "income" => Ok(("income", Kind::Income)),
        "expense" => Ok(("expense", Kind::Expense)),
        other => Err(Error::Invalid(format!("category type must be income or expense, not `{other}`"))),
    }
}

/// Sub-categories go one level deep: the parent must exist, be the same type, and not be a
/// sub-category itself; a category with sub-categories can't become one.
fn check_parent(conn: &Connection, id: Option<&str>, parent: Option<&str>, type_str: &str) -> Result<()> {
    let Some(parent) = parent else { return Ok(()) };
    if Some(parent) == id { return Err(Error::Invalid("a category can't be inside itself".into())); }
    let row: Option<(String, Option<String>)> = conn
        .query_row("SELECT type, parent_id FROM categories WHERE id = ?", [parent], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((ptype, pparent)) = row else { return Err(Error::Invalid("the parent category doesn't exist".into())) };
    if ptype != type_str { return Err(Error::Invalid(format!("a sub-category must be {type_str} like its parent"))); }
    if pparent.is_some() { return Err(Error::Invalid("sub-categories can't have their own sub-categories".into())); }
    if let Some(id) = id {
        let has_children: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE parent_id = ?)", [id], |r| r.get(0))?;
        if has_children { return Err(Error::Invalid("this category has sub-categories, so it can't become one".into())); }
    }
    Ok(())
}

fn check_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 { return Err(Error::Invalid("a category name needs 1 to 40 characters".into())); }
    Ok(name)
}

pub fn create(conn: &Connection, c: &Value) -> Result<Value> {
    let id = str_field(c, "id").map(String::from).unwrap_or_else(new_id);
    let name = check_name(require_str(c, "name")?)?;
    let (type_str, k) = kind(c)?;
    check_parent(conn, None, str_field(c, "parent_id"), type_str)?;
    atomic(conn, |tx| {
        tx.execute(
            "INSERT INTO categories (id, name, type, parent_id, color, icon, budget_amount) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                name,
                type_str,
                str_field(c, "parent_id"),
                str_field(c, "color").unwrap_or("#3b82f6"),
                str_field(c, "icon").unwrap_or("📁"),
                num_field(c, "budget_amount"),
            ],
        )?;
        ledger::create_category_ledger(tx, &id, name, k)?;
        Ok(())
    })?;
    Ok(query_one(conn, "SELECT * FROM categories WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

/// Built-in categories keep their name and type; their icon, colour and parent can change.
pub fn update(conn: &Connection, id: &str, c: &Value) -> Result<Value> {
    let is_system: bool = conn.query_row("SELECT is_system FROM categories WHERE id = ?", [id], |r| r.get(0))
        .optional()?.ok_or_else(|| Error::Invalid("no such category".into()))?;
    if is_system {
        let ty: String = conn.query_row("SELECT type FROM categories WHERE id = ?", [id], |r| r.get(0))?;
        check_parent(conn, Some(id), str_field(c, "parent_id"), &ty)?;
        conn.execute("UPDATE categories SET parent_id = ?, color = COALESCE(?, color), icon = COALESCE(?, icon) WHERE id = ?",
            params![str_field(c, "parent_id"), str_field(c, "color"), str_field(c, "icon"), id])?;
        return Ok(query_one(conn, "SELECT * FROM categories WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null));
    }
    let name = check_name(require_str(c, "name")?)?;
    let (type_str, k) = kind(c)?;
    check_parent(conn, Some(id), str_field(c, "parent_id"), type_str)?;
    let (ledger_id, old_kind) = ledger::for_category(conn, id)?;
    if k != old_kind {
        let used: bool =
            conn.query_row("SELECT EXISTS(SELECT 1 FROM postings WHERE ledger_account_id = ?)", [&ledger_id], |r| r.get(0))?;
        if used {
            return Err(Error::Invalid("can't switch a category between income and expense once it's in use".into()));
        }
    }
    atomic(conn, |tx| {
        let changed = tx.execute(
            "UPDATE categories SET name = ?, type = ?, parent_id = ?, color = ?, icon = ?, budget_amount = ?
             WHERE id = ? AND is_system = 0",
            params![name, type_str, str_field(c, "parent_id"), str_field(c, "color"), str_field(c, "icon"), num_field(c, "budget_amount"), id],
        )?;
        if changed > 0 {
            tx.execute("UPDATE ledger_accounts SET name = ?, kind = ? WHERE id = ?", params![name, ledger::kind_str(k), ledger_id])?;
        }
        Ok(())
    })?;
    Ok(query_one(conn, "SELECT * FROM categories WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

/// Built-in categories can't be deleted. Transactions in a deleted category become uncategorised.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let is_system: Option<bool> = conn
        .query_row("SELECT is_system FROM categories WHERE id = ?", [id], |r| r.get(0))
        .ok();
    if is_system != Some(false) {
        return Ok(());
    }
    let (ledger_id, k) = ledger::for_category(conn, id)?;
    let uncategorised = ledger::system_account(
        conn,
        ledger::PERSONAL_WORKSPACE,
        if k == Kind::Income { System::UncategorizedIncome } else { System::UncategorizedExpense },
    )?;
    atomic(conn, |tx| {
        tx.execute("UPDATE postings SET ledger_account_id = ? WHERE ledger_account_id = ?", params![uncategorised, ledger_id])?;
        tx.execute("UPDATE categories SET parent_id = NULL WHERE parent_id = ?", [id])?;
        tx.execute("DELETE FROM categories WHERE id = ?", [id])?; // ledger account goes with it (ON DELETE CASCADE)
        Ok(())
    })?;
    Ok(())
}
