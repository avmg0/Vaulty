//! Income, expenses and transfers as the app shows them. Each one is a ledger entry with
//! balanced postings; reads go through the `transactions` view (migration 0004).
use crate::{
    error::{Error, Result},
    ledger::{self, Kind, Posting, System},
    money,
    rows::*,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const SELECT: &str = "
  SELECT t.*, c.name AS category_name, c.icon AS category_icon, c.color AS category_color,
         a.name AS account_name, ta.name AS to_account_name
  FROM transactions t
  LEFT JOIN categories c ON t.category_id = c.id
  LEFT JOIN accounts a ON t.account_id = a.id
  LEFT JOIN accounts ta ON t.to_account_id = ta.id";

pub fn get_all(conn: &Connection, filters: Option<&Value>) -> Result<Vec<Value>> {
    let mut sql = format!("{SELECT} WHERE 1=1");
    let mut params: Vec<Value> = Vec::new();
    if let Some(f) = filters {
        if let Some(acc) = str_field(f, "accountId") {
            sql.push_str(" AND (t.account_id = ? OR t.to_account_id = ?)");
            params.push(json!(acc));
            params.push(json!(acc));
        }
        for (key, clause) in [
            ("categoryId", " AND t.category_id = ?"),
            ("type", " AND t.type = ?"),
            ("startDate", " AND t.date >= ?"),
            ("endDate", " AND t.date <= ?"),
            ("visibility", " AND t.visibility = ?"),
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

/// One transaction with the joined names the UI and the family view need.
pub fn get_one(conn: &Connection, id: &str) -> Result<Option<Value>> {
    query_one(conn, &format!("{SELECT} WHERE t.id = ?"), &[json!(id)])
}

/// Everything the owner has marked shared or public, for (re)publishing.
pub fn shareable(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, &format!("{SELECT} WHERE t.visibility <> 'private'"), &[])
}

/// `visibility` defaults to private when omitted.
fn visibility(t: &Value) -> Result<&str> {
    match str_field(t, "visibility").unwrap_or("private") {
        v @ ("private" | "shared" | "public") => Ok(v),
        other => Err(Error::Invalid(format!("unknown visibility `{other}`"))),
    }
}

/// Change only who can see a transaction.
pub fn set_visibility(conn: &Connection, id: &str, v: &str) -> Result<Value> {
    visibility(&json!({ "visibility": v }))?;
    let n = conn.execute(
        "UPDATE entries SET visibility = ? WHERE id = ? AND kind IN ('income', 'expense', 'transfer')",
        [v, id],
    )?;
    if n == 0 {
        return Err(Error::Invalid("no such transaction".into()));
    }
    get_one(conn, id)?.ok_or_else(|| Error::Invalid("no such transaction".into()))
}

/// Turns the app's transaction shape into balanced postings.
fn postings_for(conn: &Connection, t: &Value) -> Result<(&'static str, Vec<Posting>)> {
    let kind = match require_str(t, "type")? {
        "income" => "income",
        "expense" => "expense",
        "transfer" => "transfer",
        other => return Err(Error::Invalid(format!("unknown transaction type `{other}`"))),
    };
    let (account_ledger, _, currency) = ledger::for_account(conn, require_str(t, "account_id")?)?;
    let amount = money::parse_minor(t.get("amount").unwrap_or(&Value::Null), &currency)?;
    if amount <= 0 {
        return Err(Error::Invalid("`amount` must be greater than zero".into()));
    }
    let p = |id: String, m: i64| Posting { ledger_account_id: id, amount_minor: m, currency: currency.clone(), memo: None };

    let postings = match kind {
        "transfer" => {
            let to = require_str(t, "to_account_id").map_err(|_| Error::Invalid("a transfer needs a destination account".into()))?;
            let (to_ledger, _, to_currency) = ledger::for_account(conn, to)?;
            if to_ledger == account_ledger {
                return Err(Error::Invalid("can't transfer to the same account".into()));
            }
            if to_currency != currency {
                return Err(Error::Invalid("transfers between different currencies aren't supported yet".into()));
            }
            vec![p(to_ledger, amount), p(account_ledger, -amount)]
        }
        _ => {
            let income = kind == "income";
            let category_ledger = match str_field(t, "category_id") {
                Some(cat) => {
                    let (id, k) = ledger::for_category(conn, cat)?;
                    if (k == Kind::Income) != income {
                        return Err(Error::Invalid(format!("that category is for {}", if income { "expenses" } else { "income" })));
                    }
                    id
                }
                None => ledger::system_account(
                    conn,
                    ledger::PERSONAL_WORKSPACE,
                    if income { System::UncategorizedIncome } else { System::UncategorizedExpense },
                )?,
            };
            if income {
                vec![p(account_ledger, amount), p(category_ledger, -amount)]
            } else {
                vec![p(category_ledger, amount), p(account_ledger, -amount)]
            }
        }
    };
    Ok((kind, postings))
}

pub fn create(conn: &Connection, t: &Value) -> Result<Value> {
    let id = str_field(t, "id").map(String::from).unwrap_or_else(new_id);
    atomic(conn, |tx| {
        let (kind, postings) = postings_for(tx, t)?;
        tx.execute(
            "INSERT INTO entries (id, kind, date, time, merchant, description, notes, tags, is_pending, is_cleared, visibility)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                kind,
                require_str(t, "date")?,
                str_field(t, "time"),
                str_field(t, "merchant"),
                str_field(t, "description"),
                str_field(t, "notes"),
                str_field(t, "tags"),
                bool_field(t, "is_pending", false),
                bool_field(t, "is_cleared", true),
                visibility(t)?,
            ],
        )?;
        ledger::post(tx, &id, &postings)?;
        Ok(())
    })?;
    get_one(conn, &id)?.ok_or_else(|| Error::Invalid("transaction was not saved".into()))
}

pub fn update(conn: &Connection, id: &str, t: &Value) -> Result<Value> {
    atomic(conn, |tx| {
        let (kind, postings) = postings_for(tx, t)?;
        let n = tx.execute(
            "UPDATE entries SET kind = ?, date = ?, time = ?, merchant = ?, description = ?, notes = ?, tags = ?,
                    is_pending = ?, is_cleared = ?, visibility = ?
             WHERE id = ? AND kind IN ('income', 'expense', 'transfer')",
            params![
                kind,
                require_str(t, "date")?,
                str_field(t, "time"),
                str_field(t, "merchant"),
                str_field(t, "description"),
                str_field(t, "notes"),
                str_field(t, "tags"),
                bool_field(t, "is_pending", false),
                bool_field(t, "is_cleared", true),
                visibility(t)?,
                id
            ],
        )?;
        if n == 0 {
            return Err(Error::Invalid("no such transaction".into()));
        }
        tx.execute("DELETE FROM postings WHERE entry_id = ?", [id])?;
        ledger::post(tx, id, &postings)?;
        Ok(())
    })?;
    get_one(conn, id)?.ok_or_else(|| Error::Invalid("no such transaction".into()))
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    // Postings go with the entry (ON DELETE CASCADE). Opening balances are managed via the account.
    conn.execute("DELETE FROM entries WHERE id = ? AND kind IN ('income', 'expense', 'transfer')", [id])?;
    Ok(())
}
