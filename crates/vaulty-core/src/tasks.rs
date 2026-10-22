//! To-dos, optionally linked to something they're about: a bill (recurring payment), a
//! transaction, a person or a calendar event. Marking a bill paid ticks off its open to-dos
//! (see `recurring::mark_paid`). Due dates show on the calendar and as reminders.
use crate::{error::{Error, Result}, rows::*};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const PRIORITIES: &[&str] = &["low", "medium", "high"];
const LINKS: &[&str] = &["recurring", "transaction", "person", "event"];

/// The current name of what a to-do is linked to, or None if it no longer exists.
fn linked_name(conn: &Connection, kind: &str, id: &str) -> Result<Option<String>> {
    let sql = match kind {
        "recurring" => "SELECT name FROM recurring_payments WHERE id = ?",
        "transaction" => "SELECT COALESCE(NULLIF(merchant, ''), description, date) FROM entries WHERE id = ? AND hidden = 0",
        "person" => "SELECT name FROM people WHERE id = ? AND is_active = 1",
        "event" => "SELECT title FROM calendar_events WHERE id = ?",
        _ => return Ok(None),
    };
    Ok(conn.query_row(sql, [id], |r| r.get::<_, Option<String>>(0)).ok().flatten())
}

fn decorate(conn: &Connection, mut t: Value, today: NaiveDate) -> Result<Value> {
    let (kind, id) = (t["linked_type"].as_str().map(str::to_string), t["linked_id"].as_str().map(str::to_string));
    let live = match (&kind, &id) { (Some(k), Some(i)) => linked_name(conn, k, i)?, _ => None };
    let overdue = t["status"] == "pending" && t["due_date"].as_str().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).is_some_and(|d| d < today);
    if let Some(o) = t.as_object_mut() {
        // Keep the name saved with the to-do when the item has gone, so it still reads sensibly.
        o.insert("link_missing".into(), json!(kind.is_some() && live.is_none()));
        if let Some(n) = live { o.insert("linked_name".into(), json!(n)); }
        o.insert("overdue".into(), json!(overdue));
    }
    Ok(t)
}

/// `status`: "open" (default), "done" or "all". Open ones by due date (undated last), then priority.
pub fn list(conn: &Connection, status: &str, today: NaiveDate) -> Result<Vec<Value>> {
    let filter = match status { "done" => "status = 'done'", "all" => "1 = 1", _ => "status = 'pending'" };
    let rows = query(conn, &format!(
        "SELECT id, title, description AS notes, due_date, priority, status, linked_type, linked_id, linked_name, completed_at, created_at
           FROM tasks WHERE {filter}
          ORDER BY status = 'done', due_date IS NULL, due_date, CASE priority WHEN 'high' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END, created_at DESC
          LIMIT 2000"), &[])?;
    rows.into_iter().map(|t| decorate(conn, t, today)).collect()
}

pub fn get(conn: &Connection, id: &str, today: NaiveDate) -> Result<Value> {
    let t = query_one(conn, "SELECT id, title, description AS notes, due_date, priority, status, linked_type, linked_id, linked_name, completed_at, created_at FROM tasks WHERE id = ?", &[json!(id)])?
        .ok_or_else(|| Error::Invalid("no such to-do".into()))?;
    decorate(conn, t, today)
}

struct Fields { title: String, notes: Option<String>, due: Option<String>, priority: String, link: Option<(String, String, String)> }

fn checked(conn: &Connection, v: &Value) -> Result<Fields> {
    let title = v["title"].as_str().map(str::trim).unwrap_or_default().to_string();
    if title.is_empty() || title.chars().count() > 200 { return Err(Error::Invalid("a to-do needs 1 to 200 characters".into())); }
    let notes = v["notes"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(|n| n.chars().take(2000).collect());
    let due = match v["due_date"].as_str().filter(|d| !d.is_empty()) {
        Some(d) => Some(NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("`{d}` isn't a date (YYYY-MM-DD)")))?.to_string()),
        None => None,
    };
    let priority = v["priority"].as_str().unwrap_or("medium");
    if !PRIORITIES.contains(&priority) { return Err(Error::Invalid("priority is low, medium or high".into())); }
    let link = match (v["linked_type"].as_str().filter(|s| !s.is_empty()), v["linked_id"].as_str().filter(|s| !s.is_empty())) {
        (Some(kind), Some(id)) => {
            if !LINKS.contains(&kind) { return Err(Error::Invalid(format!("a to-do can't be linked to `{kind}`"))); }
            let name = linked_name(conn, kind, id)?.ok_or_else(|| Error::Invalid("the linked item doesn't exist".into()))?;
            Some((kind.to_string(), id.to_string(), name.chars().take(200).collect()))
        }
        _ => None,
    };
    Ok(Fields { title, notes, due, priority: priority.to_string(), link })
}

pub fn create(conn: &Connection, v: &Value, today: NaiveDate) -> Result<Value> {
    let f = checked(conn, v)?;
    let id = uuid::Uuid::new_v4().to_string();
    let (lt, li, ln) = f.link.map(|(a, b, c)| (Some(a), Some(b), Some(c))).unwrap_or_default();
    conn.execute(
        "INSERT INTO tasks (id, title, description, due_date, priority, status, linked_type, linked_id, linked_name) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, ?)",
        params![id, f.title, f.notes, f.due, f.priority, lt, li, ln],
    )?;
    get(conn, &id, today)
}

pub fn update(conn: &Connection, id: &str, v: &Value, today: NaiveDate) -> Result<Value> {
    get(conn, id, today)?;
    let f = checked(conn, v)?;
    let (lt, li, ln) = f.link.map(|(a, b, c)| (Some(a), Some(b), Some(c))).unwrap_or_default();
    conn.execute(
        "UPDATE tasks SET title = ?, description = ?, due_date = ?, priority = ?, linked_type = ?, linked_id = ?, linked_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![f.title, f.notes, f.due, f.priority, lt, li, ln, id],
    )?;
    get(conn, id, today)
}

pub fn set_done(conn: &Connection, id: &str, done: bool, today: NaiveDate) -> Result<Value> {
    let n = conn.execute(
        "UPDATE tasks SET status = ?1, completed_at = CASE WHEN ?1 = 'done' THEN CURRENT_TIMESTAMP END, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![if done { "done" } else { "pending" }, id],
    )?;
    if n == 0 { return Err(Error::Invalid("no such to-do".into())); }
    get(conn, id, today)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM tasks WHERE id = ?", [id])? == 0 { return Err(Error::Invalid("no such to-do".into())); }
    Ok(())
}

/// Ticks off the open to-dos about a bill once it's paid. Returns how many.
pub fn complete_for_recurring(conn: &Connection, recurring_id: &str) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE tasks SET status = 'done', completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
          WHERE linked_type = 'recurring' AND linked_id = ? AND status = 'pending'", [recurring_id])?)
}

/// Open to-dos with a due date between two dates (for the calendar).
pub fn due_between(conn: &Connection, from: &str, to: &str) -> Result<Vec<Value>> {
    query(conn, "SELECT id, title, due_date, priority FROM tasks WHERE status = 'pending' AND due_date BETWEEN ? AND ? ORDER BY due_date", &[json!(from), json!(to)])
}
