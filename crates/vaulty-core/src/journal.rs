//! The money journal: dated notes on decisions, plans and how money felt, with a mood, tags
//! and the transactions they're about. A month review keeps that month's figures (income,
//! spending, what was kept, where it went, change from the month before) with the words, as
//! they stood when it was written.
use crate::{error::{Error, Result}, money, reports, rows::*};
use chrono::{Datelike, Months, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub const MOODS: &[&str] = &["great", "good", "okay", "worried", "stressed"];
const MAX_LINKED: usize = 20;
const MAX_TEXT: usize = 20_000;
fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

fn month_range(month: &str) -> Result<(NaiveDate, NaiveDate)> {
    let start = NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").map_err(|_| bad("give the month as YYYY-MM"))?;
    let end = start.checked_add_months(Months::new(1)).and_then(|d| d.pred_opt()).ok_or_else(|| bad("give the month as YYYY-MM"))?;
    Ok((start, end))
}

/// A month's figures: income, spending, kept, savings rate, top categories and places, the
/// biggest spends, and the change in spending from the month before.
pub fn month_facts(conn: &Connection, month: &str) -> Result<Value> {
    let (start, end) = month_range(month)?;
    let o = reports::overview(conn, &start.to_string(), &end.to_string())?;
    let (ps, pe) = month_range(&start.checked_sub_months(Months::new(1)).map(|d| d.format("%Y-%m").to_string()).unwrap_or_default())?;
    let prev = reports::overview(conn, &ps.to_string(), &pe.to_string())?;
    let cur = o.currency.clone();
    let scale = money::scale(&cur) as f64;
    let biggest: Vec<Value> = query(conn,
        "SELECT t.id, t.date, COALESCE(NULLIF(TRIM(t.merchant), ''), t.description) AS name, t.amount_minor FROM transactions t
          WHERE t.type = 'expense' AND t.currency = ? AND t.date BETWEEN ? AND ? AND COALESCE(t.hidden, 0) = 0
          ORDER BY t.amount_minor DESC LIMIT 3", &[json!(cur), json!(start.to_string()), json!(end.to_string())])?
        .into_iter().map(|r| json!({ "id": r["id"], "date": r["date"], "name": r["name"], "amount": r["amount_minor"].as_i64().unwrap_or(0) as f64 / scale })).collect();
    Ok(json!({
        "month": month, "currency": cur, "income": o.income, "spending": o.spending, "kept": ((o.income - o.spending) * 100.0).round() / 100.0,
        "savings_rate": o.savings_rate, "previous_spending": prev.spending,
        "change": if prev.spending > 0.0 { Some(((o.spending - prev.spending) / prev.spending * 1000.0).round() / 10.0) } else { None },
        "categories": o.categories.iter().filter(|c| !c.name.starts_with('\u{0}')).take(3).map(|c| json!({ "name": c.name, "icon": c.icon, "amount": c.amount })).collect::<Vec<_>>(),
        "places": o.places.iter().filter(|c| !c.name.starts_with('\u{0}')).take(3).map(|c| json!({ "name": c.name, "amount": c.amount })).collect::<Vec<_>>(),
        "biggest": biggest,
    }))
}

/// An entry's checked fields: date, title, content, mood, tags, linked (JSON).
type Fields = (String, String, String, Option<String>, Option<String>, String);
fn fields(conn: &Connection, v: &Value) -> Result<Fields> {
    let date = v["date"].as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()).ok_or_else(|| bad("give the date as YYYY-MM-DD"))?;
    let title = v["title"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad("give the entry a title"))?;
    if title.chars().count() > 120 { return Err(bad("the title is too long (up to 120 characters)")); }
    let content = v["content"].as_str().unwrap_or("").to_string();
    if content.chars().count() > MAX_TEXT { return Err(bad("the entry is too long (up to 20,000 characters)")); }
    let mood = v["mood"].as_str().filter(|s| !s.is_empty());
    if let Some(m) = mood { if !MOODS.contains(&m) { return Err(bad("unknown mood")); } }
    let tags: Vec<String> = match &v["tags"] {
        Value::Array(a) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
        Value::String(s) => s.split(',').map(String::from).collect(),
        _ => vec![],
    };
    let mut tags: Vec<String> = tags.iter().map(|t| t.trim().trim_start_matches('#').to_lowercase()).filter(|t| !t.is_empty()).collect();
    tags.dedup();
    if tags.len() > 12 || tags.iter().any(|t| t.chars().count() > 30) { return Err(bad("up to 12 tags of 30 characters")); }
    let linked: Vec<String> = v["linked"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
    if linked.len() > MAX_LINKED { return Err(bad("link up to 20 transactions")); }
    for id in &linked {
        let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE id = ?)", [id], |r| r.get(0))?;
        if !ok { return Err(bad("no such transaction")); }
    }
    Ok((date.to_string(), title.to_string(), content, mood.map(String::from), (!tags.is_empty()).then(|| tags.join(",")), serde_json::to_string(&linked).unwrap_or_else(|_| "[]".into())))
}

/// A new entry. With `review_of` (YYYY-MM) it's that month's review, and the month's figures are
/// kept with it; there's one review per month.
pub fn create(conn: &Connection, v: &Value) -> Result<Value> {
    let (date, title, content, mood, tags, linked) = fields(conn, v)?;
    let review = v["review_of"].as_str().filter(|s| !s.is_empty());
    let facts = match review { Some(m) => Some(month_facts(conn, m)?.to_string()), None => None };
    if let Some(m) = review {
        let had: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM journal_entries WHERE review_of = ?)", [m], |r| r.get(0))?;
        if had { return Err(bad("that month already has a review")); }
    }
    let id = new_id();
    conn.execute("INSERT INTO journal_entries (id, date, title, content, mood, tags, linked, review_of, facts) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, date, title, content, mood, tags, linked, review, facts])?;
    Ok(json!({ "id": id }))
}

/// Changes an entry's words, mood, tags and links (a review keeps its figures; `refresh_facts`
/// takes them again).
pub fn update(conn: &Connection, id: &str, v: &Value) -> Result<()> {
    let (date, title, content, mood, tags, linked) = fields(conn, v)?;
    let review: Option<Option<String>> = conn.query_row("SELECT review_of FROM journal_entries WHERE id = ?", [id], |r| r.get(0)).optional()?;
    let Some(review) = review else { return Err(bad("no such entry")) };
    conn.execute("UPDATE journal_entries SET date = ?, title = ?, content = ?, mood = ?, tags = ?, linked = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![date, title, content, mood, tags, linked, id])?;
    if let (Some(m), true) = (review, v["refresh_facts"].as_bool().unwrap_or(false)) {
        conn.execute("UPDATE journal_entries SET facts = ? WHERE id = ?", params![month_facts(conn, &m)?.to_string(), id])?;
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM journal_entries WHERE id = ?", [id])? == 0 { return Err(bad("no such entry")); }
    Ok(())
}

/// Entries newest first, matching `search` (words in the title, text or tags) and `tag`, with
/// linked transactions (hidden ones masked), the tags in use, and the latest finished month
/// without a review.
pub fn list(conn: &Connection, search: Option<&str>, tag: Option<&str>, today: NaiveDate) -> Result<Value> {
    let mut entries = query(conn, "SELECT * FROM journal_entries ORDER BY date DESC, created_at DESC", &[])?;
    let s = search.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let tag = tag.map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty());
    let mut all_tags: Vec<String> = vec![];
    for e in &entries { for t in e["tags"].as_str().unwrap_or("").split(',').filter(|t| !t.is_empty()) { if !all_tags.iter().any(|x| x == t) { all_tags.push(t.to_string()); } } }
    all_tags.sort();
    entries.retain(|e| {
        let tags: Vec<&str> = e["tags"].as_str().unwrap_or("").split(',').collect();
        tag.as_deref().is_none_or(|t| tags.contains(&t))
            && s.as_deref().is_none_or(|s| [&e["title"], &e["content"], &e["tags"]].iter().any(|f| f.as_str().is_some_and(|x| x.to_lowercase().contains(s))))
    });
    for e in entries.iter_mut() {
        let ids: Vec<String> = serde_json::from_str(e["linked"].as_str().unwrap_or("[]")).unwrap_or_default();
        let mut txns = vec![];
        for id in ids {
            if let Some(t) = query_one(conn, "SELECT id, date, type, COALESCE(NULLIF(TRIM(merchant), ''), description) AS name, amount, currency, COALESCE(hidden, 0) AS hidden FROM transactions WHERE id = ?", &[json!(id)])? {
                txns.push(if t["hidden"] == 1 { json!({ "id": t["id"], "date": t["date"], "type": t["type"], "name": crate::transactions::MASK, "amount": null, "currency": t["currency"], "hidden": true }) } else { t });
            }
        }
        let o = e.as_object_mut().ok_or_else(|| bad("bad entry"))?;
        o.insert("linked".into(), json!(txns));
        let tags: Vec<&str> = o.get("tags").and_then(Value::as_str).unwrap_or("").split(',').filter(|t| !t.is_empty()).collect::<Vec<_>>();
        let tags = json!(tags);
        o.insert("tags".into(), tags);
        let facts = o.get("facts").and_then(Value::as_str).and_then(|f| serde_json::from_str::<Value>(f).ok());
        o.insert("facts".into(), json!(facts));
    }
    // The last full month, if it has spending and no review yet.
    let last = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).and_then(|d| d.checked_sub_months(Months::new(1))).map(|d| d.format("%Y-%m").to_string()).unwrap_or_default();
    let reviewed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM journal_entries WHERE review_of = ?)", [&last], |r| r.get(0))?;
    let (ls, le) = month_range(&last)?;
    let any: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE kind IN ('income', 'expense') AND date BETWEEN ? AND ?)", params![ls.to_string(), le.to_string()], |r| r.get(0))?;
    Ok(json!({ "entries": entries, "tags": all_tags, "review_due": (!reviewed && any).then_some(last) }))
}
