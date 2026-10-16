//! Income and expense categories. Each one is backed by a ledger account of the same kind.
use crate::{
    error::{Error, Result},
    ledger::{self, Kind, System},
    rows::*,
};
use rusqlite::{params, Connection};
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

pub fn create(conn: &Connection, c: &Value) -> Result<Value> {
    let id = str_field(c, "id").map(String::from).unwrap_or_else(new_id);
    let name = require_str(c, "name")?;
    let (type_str, k) = kind(c)?;
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

/// Built-in categories can't be edited.
pub fn update(conn: &Connection, id: &str, c: &Value) -> Result<Value> {
    let name = require_str(c, "name")?;
    let (type_str, k) = kind(c)?;
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
        tx.execute("DELETE FROM categories WHERE id = ?", [id])?; // ledger account goes with it (ON DELETE CASCADE)
        Ok(())
    })?;
    Ok(())
}
