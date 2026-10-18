//! Monthly budgets per expense category, and what was spent against them.
//!
//! Spending comes straight from the ledger: every posting to a category's ledger account in the
//! month, so split parts count under their own category and hidden transactions still count.
//! Sub-categories roll up into their parent; a parent's budget covers its children too.

use crate::{
    error::{Error, Result},
    money,
    rows::*,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

/// `YYYY-MM` → first and last day.
fn month_bounds(month: &str) -> Result<(String, String)> {
    let first = chrono::NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")
        .map_err(|_| Error::Invalid("month must look like 2026-10".into()))?;
    let next = first.checked_add_months(chrono::Months::new(1)).ok_or_else(|| Error::Invalid("month out of range".into()))?;
    Ok((first.to_string(), (next - chrono::Duration::days(1)).to_string()))
}

fn main_currency(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get::<_, Option<String>>(0))?.unwrap_or_else(|| "INR".into()))
}

/// Every category with its budget (if any) and what was spent or received in `month`, in the
/// profile's main currency. Amounts in other currencies are counted in `other_currencies`.
pub fn month(conn: &Connection, month: &str) -> Result<Value> {
    let (from, to) = month_bounds(month)?;
    let currency = main_currency(conn)?;
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.type, c.icon, c.color, c.parent_id, c.is_system,
                b.amount_minor, b.currency,
                COALESCE((SELECT SUM(p.amount_minor) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
                          JOIN entries e ON e.id = p.entry_id
                          WHERE l.category_id = c.id AND p.currency = ?3 AND e.date BETWEEN ?1 AND ?2), 0),
                (SELECT COUNT(*) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
                          JOIN entries e ON e.id = p.entry_id
                          WHERE l.category_id = c.id AND p.currency <> ?3 AND e.date BETWEEN ?1 AND ?2)
         FROM categories c LEFT JOIN budgets b ON b.category_id = c.id
         WHERE c.is_active = 1 ORDER BY c.type DESC, c.name")?;
    let rows = stmt.query_map(params![from, to, currency], |r| {
        let kind: String = r.get(2)?;
        let net: i64 = r.get(9)?;
        // Expense postings are positive, income postings negative; show both as positive amounts.
        let amount = if kind == "income" { -net } else { net };
        let budget: Option<i64> = r.get(7)?;
        Ok(json!({
            "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "type": kind, "icon": r.get::<_, Option<String>>(3)?,
            "color": r.get::<_, Option<String>>(4)?, "parent_id": r.get::<_, Option<String>>(5)?, "is_system": r.get::<_, bool>(6)?,
            "budget_minor": budget, "budget": budget.map(|b| money::to_decimal(b, &currency)),
            "budget_currency": r.get::<_, Option<String>>(8)?,
            "spent_minor": amount, "spent": money::to_decimal(amount, &currency),
            "other_currencies": r.get::<_, i64>(10)?,
        }))
    })?.collect::<std::result::Result<Vec<_>, _>>()?;
    let uncat = |key: &str| -> Result<i64> {
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(p.amount_minor), 0) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
             JOIN entries e ON e.id = p.entry_id WHERE l.system_key = ?4 AND p.currency = ?3 AND e.date BETWEEN ?1 AND ?2",
            params![from, to, currency, key], |r| r.get(0))?)
    };
    let uncat_expense = uncat("uncategorized_expense")?;
    let uncat_income = -uncat("uncategorized_income")?;
    Ok(json!({
        "month": month, "currency": currency, "categories": rows,
        "uncategorised_expense": money::to_decimal(uncat_expense, &currency),
        "uncategorised_income": money::to_decimal(uncat_income, &currency),
    }))
}

/// Sets or clears (`amount` null or 0) the monthly budget of an expense category.
pub fn set(conn: &Connection, category_id: &str, amount: &Value) -> Result<Option<Value>> {
    let kind: Option<String> = conn.query_row("SELECT type FROM categories WHERE id = ?", [category_id], |r| r.get(0)).optional()?;
    match kind.as_deref() {
        None => return Err(Error::Invalid("no such category".into())),
        Some("expense") => {}
        Some(_) => return Err(Error::Invalid("budgets are for expense categories".into())),
    }
    let currency = main_currency(conn)?;
    let minor = if amount.is_null() || amount.as_str() == Some("") { 0 } else { money::parse_minor(amount, &currency)? };
    if minor < 0 { return Err(Error::Invalid("a budget can't be negative".into())); }
    if minor == 0 {
        conn.execute("DELETE FROM budgets WHERE category_id = ?", [category_id])?;
        return Ok(None);
    }
    conn.execute(
        "INSERT INTO budgets (id, category_id, amount_minor, currency) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(category_id) DO UPDATE SET amount_minor = excluded.amount_minor, currency = excluded.currency, updated_at = CURRENT_TIMESTAMP",
        params![format!("bud_{category_id}"), category_id, minor, currency],
    )?;
    query_one(conn, "SELECT * FROM budgets WHERE category_id = ?", &[json!(category_id)])
}
