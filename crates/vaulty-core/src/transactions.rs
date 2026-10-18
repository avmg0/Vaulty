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

/// Adds `splits` (category, amount, memo per part) to transactions that are split across
/// several categories, and `category_name` "Split" so lists don't name just the first part.
fn attach_splits(conn: &Connection, rows: &mut [Value]) -> Result<()> {
    let mut stmt = conn.prepare_cached(
        "SELECT p.entry_id, l.category_id, c.name, c.icon, c.color, ABS(p.amount_minor), p.currency, p.memo
         FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id LEFT JOIN categories c ON c.id = l.category_id
         WHERE p.entry_id IN (SELECT p2.entry_id FROM postings p2 JOIN ledger_accounts l2 ON l2.id = p2.ledger_account_id
                              WHERE l2.kind IN ('income', 'expense') AND p2.entry_id IN (SELECT value FROM json_each(?1))
                              GROUP BY p2.entry_id HAVING COUNT(*) > 1)
           AND l.kind IN ('income', 'expense')
         ORDER BY p.entry_id, p.id")?;
    let ids: Vec<&str> = rows.iter().filter_map(|r| r["id"].as_str()).collect();
    if ids.is_empty() { return Ok(()); }
    let ids = serde_json::to_string(&ids)?;
    let mut by_entry: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    let found = stmt.query_map([&ids], |r| {
        let cur: String = r.get(6)?;
        let minor: i64 = r.get(5)?;
        Ok((r.get::<_, String>(0)?, json!({
            "category_id": r.get::<_, Option<String>>(1)?, "category_name": r.get::<_, Option<String>>(2)?,
            "category_icon": r.get::<_, Option<String>>(3)?, "category_color": r.get::<_, Option<String>>(4)?,
            "amount_minor": minor, "amount": money::to_decimal(minor, &cur), "memo": r.get::<_, Option<String>>(7)?,
        })))
    })?;
    for f in found { let (id, v) = f?; by_entry.entry(id).or_default().push(v); }
    if by_entry.is_empty() { return Ok(()); }
    for row in rows.iter_mut() {
        let Some(id) = row["id"].as_str() else { continue };
        if let Some(parts) = by_entry.remove(id) {
            if let Some(o) = row.as_object_mut() {
                o.insert("splits".into(), Value::Array(parts));
                o.insert("category_id".into(), Value::Null);
                o.insert("category_name".into(), json!("Split"));
                o.insert("category_icon".into(), Value::Null);
                o.insert("category_color".into(), Value::Null);
            }
        }
    }
    Ok(())
}

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
    let mut rows = query(conn, &sql, &params)?;
    attach_splits(conn, &mut rows)?;
    Ok(rows)
}

/// One transaction with the joined names the UI and the family view need.
pub fn get_one(conn: &Connection, id: &str) -> Result<Option<Value>> {
    let Some(row) = query_one(conn, &format!("{SELECT} WHERE t.id = ?"), &[json!(id)])? else { return Ok(None) };
    let mut rows = [row];
    attach_splits(conn, &mut rows)?;
    let [row] = rows;
    Ok(Some(row))
}

/// Everything the owner has marked shared or public, for (re)publishing. Hidden transactions are
/// never shared, whatever their visibility says.
pub fn shareable(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, &format!("{SELECT} WHERE t.visibility <> 'private' AND t.hidden = 0"), &[])
}

/// What a hidden transaction shows outside the Hidden section.
pub const MASK: &str = "XXXXX";

/// Hides a hidden transaction's details: merchant, description, notes, tags, category and
/// receipt. Amount, date, type and account stay, so balances and totals remain right.
pub fn mask(mut row: Value) -> Value {
    if row.get("hidden").and_then(Value::as_i64) != Some(1) { return row; }
    if let Some(o) = row.as_object_mut() {
        o.insert("merchant".into(), json!(MASK));
        for k in ["description", "notes", "tags", "receipt_path", "category_id", "category_icon", "category_color", "splits"] { o.insert(k.into(), Value::Null); }
        o.insert("category_name".into(), json!("Hidden"));
        o.insert("masked".into(), json!(true));
    }
    row
}

/// Transactions as the app shows them. Hidden ones are masked unless `reveal` is true (the
/// caller checks the member unlocked the Hidden section). Filter `hidden`: "only" or "exclude".
pub fn list(conn: &Connection, filters: Option<&Value>, reveal: bool) -> Result<Vec<Value>> {
    let mut rows = get_all(conn, filters)?;
    match filters.and_then(|f| str_field(f, "hidden")) {
        Some("only") => rows.retain(|r| r["hidden"] == 1),
        Some("exclude") => rows.retain(|r| r["hidden"] != 1),
        _ => {}
    }
    Ok(if reveal { rows } else { rows.into_iter().map(mask).collect() })
}

pub fn is_hidden(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.query_row("SELECT hidden FROM entries WHERE id = ?", [id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1)
}

pub fn hidden_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM entries WHERE hidden = 1 AND kind IN ('income', 'expense', 'transfer')", [], |r| r.get(0))?)
}

/// Moves a transaction into or out of the Hidden section.
pub fn set_hidden(conn: &Connection, id: &str, hidden: bool) -> Result<Value> {
    let n = conn.execute("UPDATE entries SET hidden = ? WHERE id = ? AND kind IN ('income', 'expense', 'transfer')", params![hidden as i64, id])?;
    if n == 0 { return Err(Error::Invalid("no such transaction".into())); }
    get_one(conn, id)?.ok_or_else(|| Error::Invalid("no such transaction".into()))
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
            let category_ledger = |cat: Option<&str>| -> Result<String> {
                match cat {
                    Some(cat) => {
                        let (id, k) = ledger::for_category(conn, cat)?;
                        if (k == Kind::Income) != income {
                            return Err(Error::Invalid(format!("that category is for {}", if income { "expenses" } else { "income" })));
                        }
                        Ok(id)
                    }
                    None => ledger::system_account(
                        conn,
                        ledger::PERSONAL_WORKSPACE,
                        if income { System::UncategorizedIncome } else { System::UncategorizedExpense },
                    ),
                }
            };
            // Category side: one posting, or one per split part. The sign is flipped for income.
            let sign = if income { -1 } else { 1 };
            let mut postings = match t.get("splits").and_then(Value::as_array).filter(|a| !a.is_empty()) {
                None => vec![p(category_ledger(str_field(t, "category_id"))?, sign * amount)],
                Some(parts) => {
                    if parts.len() < 2 { return Err(Error::Invalid("a split needs at least two parts".into())); }
                    if parts.len() > 50 { return Err(Error::Invalid("a split can have at most 50 parts".into())); }
                    let mut total = 0i64;
                    let mut out = Vec::with_capacity(parts.len());
                    for part in parts {
                        let a = money::parse_minor(part.get("amount").unwrap_or(&Value::Null), &currency)?;
                        if a <= 0 { return Err(Error::Invalid("every split part needs an amount greater than zero".into())); }
                        total += a;
                        let memo = str_field(part, "memo").map(str::trim).filter(|m| !m.is_empty()).map(|m| m.chars().take(200).collect::<String>());
                        out.push(Posting { memo, ..p(category_ledger(str_field(part, "category_id"))?, sign * a) });
                    }
                    if total != amount {
                        return Err(Error::Invalid(format!("the split parts add up to {}, not the total of {}",
                            money::to_decimal(total, &currency), money::to_decimal(amount, &currency))));
                    }
                    out
                }
            };
            postings.push(p(account_ledger, -sign * amount));
            postings
        }
    };
    Ok((kind, postings))
}

/// Tags as stored: trimmed, without `#`, duplicates (ignoring case) removed, joined by ", ".
/// Accepts a comma-separated string or an array of strings.
pub fn normalize_tags(v: Option<&Value>) -> Result<Option<String>> {
    let raw: Vec<String> = match v {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(s)) => s.split(',').map(str::to_owned).collect(),
        Some(Value::Array(a)) => a.iter().map(|x| x.as_str().map(str::to_owned).ok_or_else(|| Error::Invalid("tags must be text".into()))).collect::<Result<_>>()?,
        _ => return Err(Error::Invalid("tags must be text or a list".into())),
    };
    let mut out: Vec<String> = Vec::new();
    for t in raw {
        let t = t.trim().trim_start_matches('#').trim();
        if t.is_empty() { continue; }
        if t.chars().count() > 40 { return Err(Error::Invalid("a tag can be at most 40 characters".into())); }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(t)) { out.push(t.to_string()); }
    }
    if out.len() > 20 { return Err(Error::Invalid("at most 20 tags per transaction".into())); }
    Ok((!out.is_empty()).then(|| out.join(", ")))
}

pub fn create(conn: &Connection, t: &Value) -> Result<Value> {
    let id = str_field(t, "id").map(String::from).unwrap_or_else(new_id);
    atomic(conn, |tx| {
        let (kind, postings) = postings_for(tx, t)?;
        tx.execute(
            "INSERT INTO entries (id, kind, date, time, merchant, description, notes, tags, is_pending, is_cleared, visibility, hidden)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                kind,
                require_str(t, "date")?,
                str_field(t, "time"),
                str_field(t, "merchant"),
                str_field(t, "description"),
                str_field(t, "notes"),
                normalize_tags(t.get("tags"))?,
                bool_field(t, "is_pending", false),
                bool_field(t, "is_cleared", true),
                visibility(t)?,
                bool_field(t, "hidden", false),
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
                normalize_tags(t.get("tags"))?,
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
