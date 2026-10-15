use crate::{error::{Error, Result}, rows::*};
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn get_all(conn: &Connection, filters: Option<&Value>) -> Result<Vec<Value>> {
    let mut sql = String::from(
        "SELECT t.*, c.name AS category_name, c.icon AS category_icon, c.color AS category_color, a.name AS account_name
         FROM transactions t
         LEFT JOIN categories c ON t.category_id = c.id
         LEFT JOIN accounts a ON t.account_id = a.id
         WHERE 1=1",
    );
    let mut params: Vec<Value> = Vec::new();
    if let Some(f) = filters {
        for (key, clause) in [
            ("accountId", " AND t.account_id = ?"),
            ("categoryId", " AND t.category_id = ?"),
            ("type", " AND t.type = ?"),
            ("startDate", " AND t.date >= ?"),
            ("endDate", " AND t.date <= ?"),
        ] {
            if let Some(v) = str_field(f, key) {
                sql.push_str(clause);
                params.push(json!(v));
            }
        }
    }
    sql.push_str(" ORDER BY t.date DESC, t.created_at DESC");
    if let Some(limit) = filters.and_then(|f| f.get("limit")).and_then(Value::as_i64) {
        sql.push_str(" LIMIT ?");
        params.push(json!(limit));
    }
    query(conn, &sql, &params)
}

fn validate(t: &Value) -> Result<()> {
    let kind = require_str(t, "type")?;
    if !matches!(kind, "income" | "expense" | "transfer") {
        return Err(Error::Invalid(format!("unknown transaction type `{kind}`")));
    }
    match num_field(t, "amount") {
        Some(a) if a.is_finite() && a >= 0.0 => Ok(()),
        _ => Err(Error::Invalid("`amount` must be a non-negative number".into())),
    }
}

pub fn create(conn: &Connection, t: &Value) -> Result<Value> {
    validate(t)?;
    let id = new_id();
    let account_id = require_str(t, "account_id")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO transactions (id, account_id, category_id, type, amount, date, time, merchant, description, notes, tags, is_pending, is_cleared)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            id,
            account_id,
            str_field(t, "category_id"),
            require_str(t, "type")?,
            num_field(t, "amount"),
            require_str(t, "date")?,
            str_field(t, "time"),
            str_field(t, "merchant"),
            str_field(t, "description"),
            str_field(t, "notes"),
            str_field(t, "tags"),
            bool_field(t, "is_pending", false),
            bool_field(t, "is_cleared", true),
        ],
    )?;
    recompute_balance(&tx, account_id)?;
    tx.commit()?;
    Ok(query_one(conn, "SELECT * FROM transactions WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

pub fn update(conn: &Connection, id: &str, t: &Value) -> Result<Value> {
    validate(t)?;
    let old_account: Option<String> =
        conn.query_row("SELECT account_id FROM transactions WHERE id = ?", [id], |r| r.get(0)).ok();
    let new_account = require_str(t, "account_id")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE transactions SET account_id = ?, category_id = ?, type = ?, amount = ?, date = ?, time = ?,
                merchant = ?, description = ?, notes = ?, tags = ?, is_pending = ?, is_cleared = ? WHERE id = ?",
        rusqlite::params![
            new_account,
            str_field(t, "category_id"),
            require_str(t, "type")?,
            num_field(t, "amount"),
            require_str(t, "date")?,
            str_field(t, "time"),
            str_field(t, "merchant"),
            str_field(t, "description"),
            str_field(t, "notes"),
            str_field(t, "tags"),
            bool_field(t, "is_pending", false),
            bool_field(t, "is_cleared", true),
            id
        ],
    )?;
    if let Some(old) = old_account.as_deref() {
        recompute_balance(&tx, old)?;
    }
    if old_account.as_deref() != Some(new_account) {
        recompute_balance(&tx, new_account)?;
    }
    tx.commit()?;
    Ok(query_one(conn, "SELECT * FROM transactions WHERE id = ?", &[json!(id)])?.unwrap_or(Value::Null))
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let account: Option<String> =
        conn.query_row("SELECT account_id FROM transactions WHERE id = ?", [id], |r| r.get(0)).ok();
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM transactions WHERE id = ?", [id])?;
    if let Some(a) = account {
        recompute_balance(&tx, &a)?;
    }
    tx.commit()?;
    Ok(())
}

/// current_balance = initial_balance + sum(cleared income) - sum(cleared expense)
pub fn recompute_balance(conn: &Connection, account_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET current_balance = initial_balance + COALESCE((
            SELECT SUM(CASE type WHEN 'income' THEN amount WHEN 'expense' THEN -amount ELSE 0 END)
            FROM transactions WHERE account_id = accounts.id AND is_cleared = 1), 0)
         WHERE id = ?",
        [account_id],
    )?;
    Ok(())
}
