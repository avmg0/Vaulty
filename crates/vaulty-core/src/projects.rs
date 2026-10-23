//! Projects: a home renovation, a wedding, a trip, setting up a shop. Each has sections (electrical
//! work, plumbing…), bill items (what was bought or charged, qty × rate, tax, from whom) and
//! payments (money paid out, often as advances and instalments, with how it was paid and the name
//! the bank showed). From the two, what's still owed to each person is worked out.
//!
//! A project keeps its own columns too (text, number, date, a choice, yes/no), with values per item.
//! Payments can be recorded as a transaction at the same time, or linked to one already there
//! (such as a row from an imported bank statement, matched on amount, date and name).
use crate::{error::{Error, Result}, money, rows::*, transactions};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const KINDS: &[&str] = &["personal", "family", "group", "business"];
pub const STATUSES: &[&str] = &["planning", "ongoing", "on_hold", "done"];
pub const ITEM_KINDS: &[&str] = &["material", "labour", "fee", "transport", "tax", "other"];
pub const MODES: &[&str] = &["cash", "upi", "card", "bank", "cheque", "other"];
pub const COLUMN_TYPES: &[&str] = &["text", "number", "date", "choice", "yes_no"];
/// Standard item columns that can be hidden (date, item and amount always show).
pub const HIDEABLE: &[&str] = &["section", "kind", "qty", "rate", "tax", "vendor", "bill_no", "notes"];
/// Standard columns a project can give its own name.
pub const NAMEABLE: &[&str] = &["date", "item", "section", "kind", "qty", "unit", "rate", "amount", "tax", "total", "vendor", "bill_no", "notes"];
const MAX_COLUMNS: usize = 12;
const MAX_SECTIONS: usize = 60;
const MAX_QTY_E4: i64 = 1_000_000_000_000_000;

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

fn text(v: &Value, key: &str, max: usize) -> Result<Option<String>> {
    let t = v[key].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if t.as_ref().is_some_and(|s| s.chars().count() > max) { return Err(Error::Invalid(format!("one of the details is too long (up to {max} characters)"))); }
    Ok(t)
}

fn date(v: &Value, key: &str) -> Result<Option<String>> {
    match v[key].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => Ok(Some(NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("`{d}` isn't a date (YYYY-MM-DD)")))?.to_string())),
        None => Ok(None),
    }
}

/// An amount in the project's currency; empty means none.
fn amount(v: &Value, key: &str, currency: &str) -> Result<Option<i64>> {
    match &v[key] {
        Value::Null => Ok(None),
        Value::String(s) if s.trim().is_empty() => Ok(None),
        x => {
            let m = money::parse_minor(x, currency)?;
            if m < 0 { return Err(bad("amounts can't be negative")); }
            Ok(Some(m))
        }
    }
}

fn currency_of(conn: &Connection, project_id: &str) -> Result<String> {
    conn.query_row("SELECT currency FROM projects WHERE id = ?", [project_id], |r| r.get(0)).optional()?
        .ok_or_else(|| bad("no such project"))
}

fn dec(minor: i64, cur: &str) -> Value { json!(money::to_decimal(minor, cur)) }

/// Folds a name for matching: "  Jimmy  DOE " and "jimmy doe" are the same person.
fn fold(s: &str) -> String { s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase() }

// ---------------------------------------------------------------- columns

fn columns_of(v: &Value) -> Result<Vec<Value>> {
    let Some(list) = v.as_array() else { return Ok(vec![]) };
    if list.len() > MAX_COLUMNS { return Err(bad("a project can have at most 12 columns of its own")); }
    let mut out: Vec<Value> = vec![];
    for (i, c) in list.iter().enumerate() {
        let name = text(c, "name", 40)?.ok_or_else(|| bad("give each column a name"))?;
        let kind = c["type"].as_str().unwrap_or("text");
        if !COLUMN_TYPES.contains(&kind) { return Err(bad("unknown kind of column")); }
        let id = match c["id"].as_str().filter(|s| !s.is_empty()) {
            Some(id) if id.len() <= 40 && id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') => id.to_string(),
            Some(_) => return Err(bad("unknown kind of column")),
            None => format!("c{}{i}", &new_id().replace('-', "")[..10]),
        };
        if out.iter().any(|o| o["id"] == id.as_str()) { return Err(bad("two columns can't share an id")); }
        if out.iter().any(|o| o["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&name))) { return Err(bad("two columns can't have the same name")); }
        let options: Vec<String> = if kind == "choice" {
            let opts: Vec<String> = c["options"].as_array().into_iter().flatten()
                .filter_map(|o| o.as_str().map(str::trim).filter(|o| !o.is_empty()).map(|o| o.chars().take(40).collect())).collect();
            if opts.is_empty() || opts.len() > 30 { return Err(bad("a choice column needs 1 to 30 choices")); }
            opts
        } else { vec![] };
        out.push(json!({ "id": id, "name": name, "type": kind, "options": options }));
    }
    Ok(out)
}

/// The project's own names for standard columns; empty names are left out (the app's word is used).
fn labels_of(v: &Value) -> Result<String> {
    let mut out = Map::new();
    if let Some(m) = v.as_object() {
        for (k, name) in m {
            if !NAMEABLE.contains(&k.as_str()) { return Err(bad("that column can't be renamed")); }
            let name = name.as_str().map(str::trim).unwrap_or_default();
            if name.chars().count() > 40 { return Err(bad("a column name can be at most 40 characters")); }
            if !name.is_empty() { out.insert(k.clone(), json!(name)); }
        }
    }
    Ok(Value::Object(out).to_string())
}

fn hidden_of(v: &Value) -> Result<Vec<String>> {
    let mut out = vec![];
    for h in v.as_array().into_iter().flatten() {
        let h = h.as_str().unwrap_or_default();
        if !HIDEABLE.contains(&h) { return Err(bad("that column can't be hidden")); }
        if !out.iter().any(|o| o == h) { out.push(h.to_string()); }
    }
    Ok(out)
}

/// Checks an item's values for the project's own columns; unknown ones are dropped.
fn custom_values(columns: &[Value], v: &Value) -> Result<String> {
    let mut out = Map::new();
    for c in columns {
        let (id, name) = (c["id"].as_str().unwrap_or_default(), c["name"].as_str().unwrap_or_default());
        let x = &v[id];
        if x.is_null() || x.as_str().is_some_and(|s| s.trim().is_empty()) { continue; }
        let val = match c["type"].as_str().unwrap_or("text") {
            "number" => {
                let s = match x { Value::Number(n) => n.to_string(), Value::String(s) => s.trim().replace(',', ""), _ => String::new() };
                let n: f64 = s.parse().map_err(|_| Error::Invalid(format!("“{name}” needs a number")))?;
                if !n.is_finite() || n.abs() > 1e15 { return Err(Error::Invalid(format!("“{name}” needs a number"))); }
                json!(n)
            }
            "date" => {
                let s = x.as_str().unwrap_or_default().trim();
                json!(NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("“{name}” needs a date")))?.to_string())
            }
            "yes_no" => json!(x.as_bool().ok_or_else(|| Error::Invalid(format!("“{name}” is yes or no")))?),
            "choice" => {
                let s = x.as_str().unwrap_or_default().trim();
                if !c["options"].as_array().is_some_and(|o| o.iter().any(|o| o == s)) { return Err(Error::Invalid(format!("choose one of the choices for “{name}”"))); }
                json!(s)
            }
            _ => {
                let s = x.as_str().map(str::trim).unwrap_or_default();
                if s.chars().count() > 200 { return Err(bad("one of the details is too long (up to 200 characters)")); }
                json!(s)
            }
        };
        out.insert(id.to_string(), val);
    }
    Ok(Value::Object(out).to_string())
}

fn project_columns(conn: &Connection, project_id: &str) -> Result<Vec<Value>> {
    let raw: String = conn.query_row("SELECT columns FROM projects WHERE id = ?", [project_id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such project"))?;
    Ok(serde_json::from_str::<Vec<Value>>(&raw).unwrap_or_default())
}

// ---------------------------------------------------------------- projects

struct Totals { amount: i64, tax: i64, paid: i64, items: i64, payments: i64, last: Option<String> }

fn totals(conn: &Connection, id: &str) -> Result<Totals> {
    let (amount, tax, items, last_item): (i64, i64, i64, Option<String>) = conn.query_row(
        "SELECT coalesce(sum(amount_minor), 0), coalesce(sum(tax_minor), 0), count(*), max(date) FROM project_items WHERE project_id = ?", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let (paid, payments, last_paid): (i64, i64, Option<String>) = conn.query_row(
        "SELECT coalesce(sum(amount_minor), 0), count(*), max(date) FROM project_payments WHERE project_id = ?", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(Totals { amount, tax, paid, items, payments, last: last_item.max(last_paid) })
}

/// Per person paid or bought from: billed, paid, and the balance (positive: still owed to them;
/// negative: paid in advance).
fn vendors(conn: &Connection, id: &str, cur: &str) -> Result<Vec<Value>> {
    Ok(vendor_rows(conn, id, cur)?.into_iter().map(|x| x.1).collect())
}

fn vendor_rows(conn: &Connection, id: &str, cur: &str) -> Result<Vec<(i64, Value)>> {
    let mut by: BTreeMap<String, (String, i64, i64, i64, i64)> = BTreeMap::new();
    let mut st = conn.prepare("SELECT vendor, amount_minor + tax_minor FROM project_items WHERE project_id = ? AND vendor IS NOT NULL")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (v, a) = r?;
        let e = by.entry(fold(&v)).or_insert((v, 0, 0, 0, 0));
        e.1 += a; e.3 += 1;
    }
    let mut st = conn.prepare("SELECT vendor, amount_minor FROM project_payments WHERE project_id = ? AND vendor IS NOT NULL")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (v, a) = r?;
        let e = by.entry(fold(&v)).or_insert((v, 0, 0, 0, 0));
        e.2 += a; e.4 += 1;
    }
    let mut out: Vec<(i64, Value)> = by.into_values().map(|(name, billed, paid, items, payments)| (billed - paid, json!({
        "name": name, "billed": dec(billed, cur), "paid": dec(paid, cur), "balance": dec(billed - paid, cur),
        "items": items, "payments": payments,
    }))).collect();
    // Most owed first, then advances, then settled.
    out.sort_by_key(|b| std::cmp::Reverse(b.0));
    Ok(out)
}

fn summary(conn: &Connection, mut p: Value) -> Result<Value> {
    let id = p["id"].as_str().unwrap_or_default().to_string();
    let cur = p["currency"].as_str().unwrap_or("INR").to_string();
    let t = totals(conn, &id)?;
    let billed = t.amount + t.tax;
    let budget = p["budget_minor"].as_i64();
    let owed: i64 = vendor_rows(conn, &id, &cur)?.iter().map(|v| v.0).filter(|b| *b > 0).sum();
    if let Some(o) = p.as_object_mut() {
        for k in ["columns", "hidden", "labels"] {
            let empty = if k == "labels" { json!({}) } else { json!([]) };
            let parsed = o.get(k).and_then(Value::as_str).and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(empty);
            o.insert(k.into(), parsed);
        }
        o.insert("budget".into(), budget.map_or(Value::Null, |b| dec(b, &cur)));
        o.insert("amount".into(), dec(t.amount, &cur));
        o.insert("tax".into(), dec(t.tax, &cur));
        o.insert("billed".into(), dec(billed, &cur));
        o.insert("paid".into(), dec(t.paid, &cur));
        o.insert("owed".into(), dec(owed, &cur));
        o.insert("spent".into(), dec(billed.max(t.paid), &cur));
        o.insert("budget_left".into(), budget.map_or(Value::Null, |b| dec(b - billed.max(t.paid), &cur)));
        o.insert("items".into(), json!(t.items));
        o.insert("payments".into(), json!(t.payments));
        o.insert("last_date".into(), json!(t.last));
        o.insert("files".into(), json!(conn.query_row("SELECT count(*) FROM project_files WHERE project_id = ?", [&id], |r| r.get::<_, i64>(0))?));
    }
    Ok(p)
}

const COLS: &str = "id, name, kind, status, currency, budget_minor, start_date, end_date, notes, columns, hidden, labels, created_at, updated_at";

/// Every project: ongoing and planned first, then on hold, then done; newest activity first.
pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    let mut all: Vec<Value> = query(conn, &format!("SELECT {COLS} FROM projects"), &[])?
        .into_iter().map(|p| summary(conn, p)).collect::<Result<_>>()?;
    let rank = |s: &str| match s { "ongoing" => 0, "planning" => 1, "on_hold" => 2, _ => 3 };
    all.sort_by(|a, b| rank(a["status"].as_str().unwrap_or("")).cmp(&rank(b["status"].as_str().unwrap_or("")))
        .then(b["last_date"].as_str().unwrap_or(b["created_at"].as_str().unwrap_or("")).cmp(a["last_date"].as_str().unwrap_or(a["created_at"].as_str().unwrap_or("")))));
    Ok(all)
}

/// One project with its sections, totals by section, kind and mode, and the people paid.
pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    let p = query_one(conn, &format!("SELECT {COLS} FROM projects WHERE id = ?"), &[json!(id)])?.ok_or_else(|| bad("no such project"))?;
    let mut p = summary(conn, p)?;
    let cur = p["currency"].as_str().unwrap_or("INR").to_string();
    let mut sections = vec![];
    let mut st = conn.prepare(
        "SELECT s.id, s.name, s.budget_minor, s.position,
                (SELECT coalesce(sum(amount_minor + tax_minor), 0) FROM project_items WHERE section_id = s.id),
                (SELECT count(*) FROM project_items WHERE section_id = s.id),
                (SELECT coalesce(sum(amount_minor), 0) FROM project_payments WHERE section_id = s.id)
         FROM project_sections s WHERE s.project_id = ? ORDER BY s.position, s.name COLLATE NOCASE")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, i64>(6)?)))? {
        let (sid, name, budget, position, billed, items, paid) = r?;
        sections.push(json!({ "id": sid, "name": name, "position": position, "budget": budget.map(|b| money::to_decimal(b, &cur)),
            "billed": dec(billed, &cur), "items": items, "paid": dec(paid, &cur) }));
    }
    let (loose, loose_n): (i64, i64) = conn.query_row("SELECT coalesce(sum(amount_minor + tax_minor), 0), count(*) FROM project_items WHERE project_id = ? AND section_id IS NULL", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let group = |sql: &str| -> Result<Value> {
        let mut st = conn.prepare(sql)?;
        let rows = st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut m = Map::new();
        for r in rows { let (k, v) = r?; m.insert(k, dec(v, &cur)); }
        Ok(Value::Object(m))
    };
    let by_kind = group("SELECT kind, sum(amount_minor) FROM project_items WHERE project_id = ? GROUP BY kind")?;
    let by_mode = group("SELECT mode, sum(amount_minor) FROM project_payments WHERE project_id = ? GROUP BY mode")?;
    let vendors = vendors(conn, id, &cur)?;
    if let Some(o) = p.as_object_mut() {
        o.insert("sections".into(), json!(sections));
        o.insert("unsectioned".into(), json!({ "billed": money::to_decimal(loose, &cur), "items": loose_n }));
        o.insert("by_kind".into(), by_kind);
        o.insert("by_mode".into(), by_mode);
        o.insert("vendors".into(), json!(vendors));
    }
    Ok(p)
}

type ProjectFields = (String, String, String, String, Option<i64>, Option<String>, Option<String>, Option<String>, String, String);

fn project_fields(conn: &Connection, v: &Value, currency_now: Option<&str>) -> Result<ProjectFields> {
    let name = text(v, "name", 100)?.ok_or_else(|| bad("give the project a name"))?;
    let kind = v["kind"].as_str().unwrap_or("personal");
    if !KINDS.contains(&kind) { return Err(bad("unknown kind of project")); }
    let status = v["status"].as_str().unwrap_or("ongoing");
    if !STATUSES.contains(&status) { return Err(bad("unknown project status")); }
    let currency = match (text(v, "currency", 3)?, currency_now) {
        (_, Some(c)) => c.to_string(),
        (Some(c), None) => c.to_uppercase(),
        (None, None) => conn.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get::<_, Option<String>>(0)).optional()?.flatten().unwrap_or_else(|| "INR".into()),
    };
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) { return Err(bad("unknown currency")); }
    let (start, end) = (date(v, "start_date")?, date(v, "end_date")?);
    if let (Some(s), Some(e)) = (&start, &end) { if e < s { return Err(bad("the project can't end before it starts")); } }
    let columns = serde_json::to_string(&columns_of(&v["columns"])?)?;
    let hidden = serde_json::to_string(&hidden_of(&v["hidden"])?)?;
    Ok((name, kind.into(), status.into(), currency.clone(), amount(v, "budget", &currency)?, start, end, text(v, "notes", 2000)?, columns, hidden))
}

/// A new project; `sections` (names) gives it its sections, e.g. from a template.
pub fn create(conn: &Connection, v: &Value) -> Result<Value> {
    let (name, kind, status, currency, budget, start, end, notes, columns, hidden) = project_fields(conn, v, None)?;
    let labels = labels_of(&v["labels"])?;
    let id = new_id();
    atomic(conn, |tx| {
        tx.execute("INSERT INTO projects (id, name, kind, status, currency, budget_minor, start_date, end_date, notes, columns, hidden) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![id, name, kind, status, currency, budget, start, end, notes, columns, hidden])?;
        tx.execute("UPDATE projects SET labels = ? WHERE id = ?", params![labels, id])?;
        let names: Vec<&str> = v["sections"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if names.len() > MAX_SECTIONS { return Err(bad("a project can have at most 60 sections")); }
        for n in names { add_section(tx, &id, &json!({ "name": n })).or_else(|e| if e.to_string().contains("already") { Ok(Value::Null) } else { Err(e) })?; }
        Ok(())
    })?;
    get(conn, &id)
}

/// Changes the details. The currency stays what it was (amounts are kept in it).
pub fn update(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    let cur = currency_of(conn, id)?;
    let (name, kind, status, _, budget, start, end, notes, columns, hidden) = project_fields(conn, v, Some(&cur))?;
    let labels = labels_of(&v["labels"])?;
    conn.execute("UPDATE projects SET name = ?, kind = ?, status = ?, budget_minor = ?, start_date = ?, end_date = ?, notes = ?, columns = ?, hidden = ?, labels = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![name, kind, status, budget, start, end, notes, columns, hidden, labels, id])?;
    get(conn, id)
}

/// Deletes the project with its sections, items and payments (undo brings them back). Linked
/// transactions stay.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM projects WHERE id = ?", [id])? == 0 { return Err(bad("no such project")); }
    Ok(())
}

// ---------------------------------------------------------------- sections

pub fn add_section(conn: &Connection, project_id: &str, v: &Value) -> Result<Value> {
    let cur = currency_of(conn, project_id)?;
    let name = text(v, "name", 60)?.ok_or_else(|| bad("give the section a name"))?;
    let n: i64 = conn.query_row("SELECT count(*) FROM project_sections WHERE project_id = ?", [project_id], |r| r.get(0))?;
    if n as usize >= MAX_SECTIONS { return Err(bad("a project can have at most 60 sections")); }
    let id = new_id();
    conn.execute("INSERT INTO project_sections (id, project_id, name, budget_minor, position) VALUES (?, ?, ?, ?, ?)",
        params![id, project_id, name, amount(v, "budget", &cur)?, n])
        .map_err(|e| if e.to_string().contains("UNIQUE") { bad("this project already has a section with that name") } else { e.into() })?;
    Ok(json!({ "id": id, "name": name }))
}

pub fn update_section(conn: &Connection, id: &str, v: &Value) -> Result<()> {
    let project_id: String = conn.query_row("SELECT project_id FROM project_sections WHERE id = ?", [id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such section"))?;
    let cur = currency_of(conn, &project_id)?;
    let name = text(v, "name", 60)?.ok_or_else(|| bad("give the section a name"))?;
    conn.execute("UPDATE project_sections SET name = ?, budget_minor = ? WHERE id = ?", params![name, amount(v, "budget", &cur)?, id])
        .map_err(|e| if e.to_string().contains("UNIQUE") { bad("this project already has a section with that name") } else { e.into() })?;
    Ok(())
}

/// Removes a section; its items and payments stay in the project, without a section.
pub fn delete_section(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM project_sections WHERE id = ?", [id])? == 0 { return Err(bad("no such section")); }
    Ok(())
}

/// Puts the sections in this order.
pub fn order_sections(conn: &Connection, project_id: &str, ids: &[String]) -> Result<()> {
    atomic(conn, |tx| {
        for (i, id) in ids.iter().enumerate() {
            tx.execute("UPDATE project_sections SET position = ? WHERE id = ? AND project_id = ?", params![i as i64, id, project_id])?;
        }
        Ok(())
    })
}

fn section_in(conn: &Connection, project_id: &str, v: &Value) -> Result<Option<String>> {
    match v["section_id"].as_str().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => {
            let ok: bool = conn.query_row("SELECT 1 FROM project_sections WHERE id = ? AND project_id = ?", params![s, project_id], |_| Ok(true)).optional()?.unwrap_or(false);
            if !ok { return Err(bad("no such section")); }
            Ok(Some(s.to_string()))
        }
    }
}

// ---------------------------------------------------------------- items

fn qty(v: &Value) -> Result<Option<i64>> {
    let s = match &v["qty"] {
        Value::Null => return Ok(None),
        Value::Number(n) => n.to_string(),
        Value::String(s) if s.trim().is_empty() => return Ok(None),
        Value::String(s) => s.trim().replace(',', ""),
        _ => return Err(bad("the quantity isn't a number")),
    };
    if s.split_once('.').is_some_and(|(_, f)| f.trim_end_matches('0').len() > 4) { return Err(bad("the quantity can have up to 4 decimal places")); }
    let n = crate::cas::scaled(&s, 4).ok_or_else(|| bad("the quantity isn't a number"))?;
    if n <= 0 || n > MAX_QTY_E4 { return Err(bad("the quantity must be more than 0")); }
    Ok(Some(n))
}

type ItemFields = (Option<String>, String, String, String, Option<i64>, Option<String>, Option<i64>, i64, i64, Option<String>, Option<String>, Option<String>, String);

fn item_fields(conn: &Connection, project_id: &str, v: &Value) -> Result<ItemFields> {
    let cur = currency_of(conn, project_id)?;
    let section = section_in(conn, project_id, v)?;
    let d = date(v, "date")?.ok_or_else(|| bad("give the date"))?;
    let item = text(v, "item", 200)?.ok_or_else(|| bad("say what the item is"))?;
    let kind = v["kind"].as_str().unwrap_or("material");
    if !ITEM_KINDS.contains(&kind) { return Err(bad("unknown kind of item")); }
    let q = qty(v)?;
    let rate = amount(v, "rate", &cur)?;
    // The amount as typed, or else quantity × rate.
    let amt = match (amount(v, "amount", &cur)?, q, rate) {
        (Some(a), _, _) => a,
        (None, Some(q), Some(r)) => i64::try_from((q as i128 * r as i128 + 5_000) / 10_000).map_err(|_| bad("amount is too large"))?,
        (None, None, Some(r)) => r,
        _ => return Err(bad("give the amount, or the quantity and rate")),
    };
    if amt > money::MAX_MINOR { return Err(bad("amount is too large")); }
    // Tax as an amount, or as a percentage of the amount.
    let tax = match (amount(v, "tax", &cur)?, &v["tax_percent"]) {
        (Some(t), _) => t,
        (None, Value::Null) => 0,
        (None, Value::String(s)) if s.trim().is_empty() => 0,
        (None, p) => {
            let pct = p.as_f64().or_else(|| p.as_str().and_then(|s| s.trim().trim_end_matches('%').parse().ok())).ok_or_else(|| bad("the tax rate isn't a number"))?;
            if !(0.0..=100.0).contains(&pct) { return Err(bad("the tax rate is 0 to 100%")); }
            (amt as f64 * pct / 100.0).round() as i64
        }
    };
    let custom = custom_values(&project_columns(conn, project_id)?, &v["custom"])?;
    Ok((section, d, item, kind.into(), q, text(v, "unit", 20)?, rate, amt, tax, text(v, "vendor", 100)?, text(v, "bill_no", 60)?, text(v, "notes", 1000)?, custom))
}

fn item_row(conn: &Connection, id: &str) -> Result<Value> {
    let mut rows = items_where(conn, "i.id = ?", id)?;
    rows.pop().ok_or_else(|| bad("no such item"))
}

fn items_where(conn: &Connection, filter: &str, arg: &str) -> Result<Vec<Value>> {
    let rows = query(conn, &format!(
        "SELECT i.id, i.project_id, i.section_id, s.name AS section_name, i.date, i.item, i.kind, i.qty_e4, i.unit, i.rate_minor, i.amount_minor, i.tax_minor,
                i.vendor, i.bill_no, i.notes, i.custom, p.currency, i.created_at,
                (SELECT count(*) FROM project_files f WHERE f.item_id = i.id) AS files
         FROM project_items i JOIN projects p ON p.id = i.project_id LEFT JOIN project_sections s ON s.id = i.section_id
         WHERE {filter} ORDER BY i.date, i.created_at, i.rowid"), &[json!(arg)])?;
    Ok(rows.into_iter().map(|mut r| {
        let cur = r["currency"].as_str().unwrap_or("INR").to_string();
        let (a, t) = (r["amount_minor"].as_i64().unwrap_or(0), r["tax_minor"].as_i64().unwrap_or(0));
        let custom = r["custom"].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(json!({}));
        if let Some(o) = r.as_object_mut() {
            o.insert("qty".into(), json!(o["qty_e4"].as_i64().map(|q| q as f64 / 10_000.0)));
            o.insert("rate".into(), json!(o["rate_minor"].as_i64().map(|x| money::to_decimal(x, &cur))));
            o.insert("amount".into(), dec(a, &cur));
            o.insert("tax".into(), dec(t, &cur));
            o.insert("total".into(), dec(a + t, &cur));
            o.insert("custom".into(), custom);
        }
        r
    }).collect())
}

/// A project's bill items, oldest first (the order they're numbered in).
pub fn items(conn: &Connection, project_id: &str) -> Result<Vec<Value>> {
    currency_of(conn, project_id)?;
    items_where(conn, "i.project_id = ?", project_id)
}

pub fn add_item(conn: &Connection, project_id: &str, v: &Value) -> Result<Value> {
    let (section, d, item, kind, q, unit, rate, amt, tax, vendor, bill_no, notes, custom) = item_fields(conn, project_id, v)?;
    let id = new_id();
    conn.execute(
        "INSERT INTO project_items (id, project_id, section_id, date, item, kind, qty_e4, unit, rate_minor, amount_minor, tax_minor, vendor, bill_no, notes, custom)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, project_id, section, d, item, kind, q, unit, rate, amt, tax, vendor, bill_no, notes, custom])?;
    item_row(conn, &id)
}

pub fn update_item(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    let project_id: String = conn.query_row("SELECT project_id FROM project_items WHERE id = ?", [id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such item"))?;
    let (section, d, item, kind, q, unit, rate, amt, tax, vendor, bill_no, notes, custom) = item_fields(conn, &project_id, v)?;
    conn.execute(
        "UPDATE project_items SET section_id = ?, date = ?, item = ?, kind = ?, qty_e4 = ?, unit = ?, rate_minor = ?, amount_minor = ?, tax_minor = ?,
                vendor = ?, bill_no = ?, notes = ?, custom = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![section, d, item, kind, q, unit, rate, amt, tax, vendor, bill_no, notes, custom, id])?;
    item_row(conn, id)
}

pub fn delete_item(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM project_items WHERE id = ?", [id])? == 0 { return Err(bad("no such item")); }
    Ok(())
}

// ---------------------------------------------------------------- payments

fn payments_where(conn: &Connection, filter: &str, arg: &str) -> Result<Vec<Value>> {
    let rows = query(conn, &format!(
        "SELECT y.id, y.project_id, y.section_id, s.name AS section_name, y.date, y.amount_minor, y.vendor, y.mode, y.account_name, y.reference,
                y.entry_id, y.notes, p.currency, a.name AS from_account, t.date AS entry_date, y.created_at,
                (SELECT count(*) FROM project_files f WHERE f.payment_id = y.id) AS files
         FROM project_payments y JOIN projects p ON p.id = y.project_id LEFT JOIN project_sections s ON s.id = y.section_id
         LEFT JOIN transactions t ON t.id = y.entry_id LEFT JOIN accounts a ON a.id = t.account_id
         WHERE {filter} ORDER BY y.date, y.created_at, y.rowid"), &[json!(arg)])?;
    Ok(rows.into_iter().map(|mut r| {
        let cur = r["currency"].as_str().unwrap_or("INR").to_string();
        let a = r["amount_minor"].as_i64().unwrap_or(0);
        if let Some(o) = r.as_object_mut() { o.insert("amount".into(), dec(a, &cur)); }
        r
    }).collect())
}

pub fn payments(conn: &Connection, project_id: &str) -> Result<Vec<Value>> {
    currency_of(conn, project_id)?;
    payments_where(conn, "y.project_id = ?", project_id)
}

fn payment_row(conn: &Connection, id: &str) -> Result<Value> {
    payments_where(conn, "y.id = ?", id)?.pop().ok_or_else(|| bad("no such payment"))
}

type PaymentFields = (Option<String>, String, i64, Option<String>, String, Option<String>, Option<String>, Option<String>);

fn payment_fields(conn: &Connection, project_id: &str, v: &Value) -> Result<PaymentFields> {
    let cur = currency_of(conn, project_id)?;
    let section = section_in(conn, project_id, v)?;
    let d = date(v, "date")?.ok_or_else(|| bad("give the date"))?;
    let amt = amount(v, "amount", &cur)?.filter(|a| *a > 0).ok_or_else(|| bad("give the amount paid"))?;
    let mode = v["mode"].as_str().unwrap_or("upi");
    if !MODES.contains(&mode) { return Err(bad("unknown way of paying")); }
    Ok((section, d, amt, text(v, "vendor", 100)?, mode.into(), text(v, "account_name", 100)?, text(v, "reference", 60)?, text(v, "notes", 1000)?))
}

/// The transaction a payment is linked to must be money that went out, in the project's currency,
/// for the same amount, and not already linked to another payment.
fn check_link(conn: &Connection, project_id: &str, entry_id: &str, amount: i64, except: Option<&str>) -> Result<()> {
    let cur = currency_of(conn, project_id)?;
    let row: Option<(String, i64, String)> = conn.query_row("SELECT type, amount_minor, currency FROM transactions WHERE id = ?", [entry_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let (kind, minor, c) = row.ok_or_else(|| bad("no such transaction"))?;
    if kind == "income" { return Err(bad("link a payment to money that went out, not income")); }
    if c != cur || minor != amount { return Err(bad("the transaction's amount isn't the same as the payment's")); }
    let taken: Option<String> = conn.query_row("SELECT id FROM project_payments WHERE entry_id = ? AND id IS NOT ?", params![entry_id, except], |r| r.get(0)).optional()?;
    if taken.is_some() { return Err(bad("that transaction is already linked to a payment")); }
    Ok(())
}

/// Records a payment. With `record` ({account_id, category_id}) the transaction is added too
/// (tagged with the project's name); with `entry_id` it's linked to one already there.
pub fn add_payment(conn: &Connection, project_id: &str, v: &Value) -> Result<Value> {
    let (section, d, amt, vendor, mode, account_name, reference, notes) = payment_fields(conn, project_id, v)?;
    let id = new_id();
    atomic(conn, |tx| {
        let entry = match (&v["record"], v["entry_id"].as_str().filter(|s| !s.is_empty())) {
            (r @ Value::Object(_), _) => {
                let (name, cur): (String, String) = tx.query_row("SELECT name, currency FROM projects WHERE id = ?", [project_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
                let account = r["account_id"].as_str().ok_or_else(|| bad("choose the account it was paid from"))?;
                let acc_cur: String = tx.query_row("SELECT currency FROM accounts WHERE id = ?", [account], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such account"))?;
                if acc_cur != cur { return Err(bad("that account is in a different currency from the project")); }
                let tag: String = name.chars().filter(|c| *c != ',').take(40).collect();
                let t = transactions::create(tx, &json!({
                    "type": "expense", "account_id": account, "category_id": r["category_id"], "date": d,
                    "amount": money::to_string(amt, &cur), "merchant": vendor.clone().or(account_name.clone()),
                    "description": notes.clone().or(reference.clone()), "tags": tag,
                }))?;
                t["id"].as_str().map(str::to_string)
            }
            (_, Some(e)) => { check_link(tx, project_id, e, amt, None)?; Some(e.to_string()) }
            _ => None,
        };
        tx.execute("INSERT INTO project_payments (id, project_id, section_id, date, amount_minor, vendor, mode, account_name, reference, entry_id, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![id, project_id, section, d, amt, vendor, mode, account_name, reference, entry, notes])?;
        Ok(())
    })?;
    payment_row(conn, &id)
}

/// Changes a payment's details. `entry_id` links it (or, empty, unlinks it); the linked
/// transaction itself isn't changed.
pub fn update_payment(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    let (project_id, linked): (String, Option<String>) = conn.query_row("SELECT project_id, entry_id FROM project_payments WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?.ok_or_else(|| bad("no such payment"))?;
    let (section, d, amt, vendor, mode, account_name, reference, notes) = payment_fields(conn, &project_id, v)?;
    let entry = match v.get("entry_id") {
        None => linked,
        Some(Value::String(e)) if !e.is_empty() => { check_link(conn, &project_id, e, amt, Some(id))?; Some(e.clone()) }
        Some(_) => None,
    };
    if let Some(e) = &entry {
        let minor: Option<i64> = conn.query_row("SELECT amount_minor FROM transactions WHERE id = ?", [e], |r| r.get(0)).optional()?;
        if minor.is_some_and(|m| m != amt) { return Err(bad("the transaction's amount isn't the same as the payment's")); }
    }
    conn.execute("UPDATE project_payments SET section_id = ?, date = ?, amount_minor = ?, vendor = ?, mode = ?, account_name = ?, reference = ?, entry_id = ?, notes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![section, d, amt, vendor, mode, account_name, reference, entry, notes, id])?;
    payment_row(conn, id)
}

/// Deletes a payment. A linked transaction stays (delete it from Transactions if it shouldn't).
pub fn delete_payment(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM project_payments WHERE id = ?", [id])? == 0 { return Err(bad("no such payment")); }
    Ok(())
}

/// For each payment not linked to a transaction: transactions that look like it (money out, the
/// same amount, within a week), best first. A match on the name the bank showed (or the person
/// paid) in the transaction's text ranks highest. Transactions already linked aren't offered.
pub fn link_suggestions(conn: &Connection, project_id: &str) -> Result<Vec<Value>> {
    let cur = currency_of(conn, project_id)?;
    let mut out = vec![];
    for p in payments(conn, project_id)?.into_iter().filter(|p| p["entry_id"].is_null()) {
        let d = NaiveDate::parse_from_str(p["date"].as_str().unwrap_or_default(), "%Y-%m-%d").map_err(|_| bad("give the date"))?;
        let (from, to) = ((d - chrono::Duration::days(7)).to_string(), (d + chrono::Duration::days(7)).to_string());
        let cands = query(conn,
            "SELECT t.id, t.date, t.merchant, t.description, t.amount_minor, a.name AS account
             FROM transactions t LEFT JOIN accounts a ON a.id = t.account_id
             WHERE t.type <> 'income' AND t.currency = ? AND t.amount_minor = ? AND t.date BETWEEN ? AND ? AND coalesce(t.hidden, 0) = 0
               AND t.id NOT IN (SELECT entry_id FROM project_payments WHERE entry_id IS NOT NULL)",
            &[json!(cur), p["amount_minor"].clone(), json!(from), json!(to)])?;
        let names: Vec<String> = [&p["account_name"], &p["vendor"], &p["reference"]].iter().filter_map(|x| x.as_str()).map(fold).collect();
        let mut scored: Vec<(i64, Value)> = cands.into_iter().map(|c| {
            let hay = fold(&format!("{} {}", c["merchant"].as_str().unwrap_or(""), c["description"].as_str().unwrap_or("")));
            let named = names.iter().any(|n| !n.is_empty() && (hay.contains(n.as_str()) || n.split(' ').filter(|w| w.len() >= 3).any(|w| hay.contains(w))));
            let days = NaiveDate::parse_from_str(c["date"].as_str().unwrap_or(""), "%Y-%m-%d").map(|x| (x - d).num_days().abs()).unwrap_or(7);
            (if named { 100 } else { 0 } - days, json!({ "id": c["id"], "date": c["date"], "merchant": c["merchant"], "description": c["description"], "account": c["account"], "named": named }))
        }).collect();
        scored.sort_by_key(|b| std::cmp::Reverse(b.0));
        if !scored.is_empty() {
            out.push(json!({ "payment_id": p["id"], "candidates": scored.into_iter().take(3).map(|x| x.1).collect::<Vec<_>>() }));
        }
    }
    Ok(out)
}

/// Names used before, for suggestions while typing: people paid, names banks showed, units.
pub fn names(conn: &Connection) -> Result<Value> {
    let col = |sql: &str| -> Result<Vec<String>> {
        let mut st = conn.prepare(sql)?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    };
    Ok(json!({
        "vendors": col("SELECT vendor FROM (SELECT vendor, date FROM project_items UNION ALL SELECT vendor, date FROM project_payments) WHERE vendor IS NOT NULL GROUP BY lower(vendor) ORDER BY max(date) DESC LIMIT 200")?,
        "account_names": col("SELECT account_name FROM project_payments WHERE account_name IS NOT NULL GROUP BY lower(account_name) ORDER BY max(date) DESC LIMIT 200")?,
        "units": col("SELECT unit FROM project_items WHERE unit IS NOT NULL GROUP BY lower(unit) ORDER BY count(*) DESC LIMIT 50")?,
    }))
}

// ---------------------------------------------------------------- bills and receipts

/// Most files a project keeps.
pub const MAX_FILES_PER_PROJECT: i64 = 500;

fn file_link(conn: &Connection, project_id: &str, v: &Value) -> Result<(Option<String>, Option<String>)> {
    let item = v["item_id"].as_str().filter(|s| !s.is_empty());
    let payment = v["payment_id"].as_str().filter(|s| !s.is_empty());
    if item.is_some() && payment.is_some() { return Err(bad("a file goes with one bill item or one payment")); }
    if let Some(i) = item {
        let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM project_items WHERE id = ? AND project_id = ?)", params![i, project_id], |r| r.get(0))?;
        if !ok { return Err(bad("no such item")); }
    }
    if let Some(y) = payment {
        let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM project_payments WHERE id = ? AND project_id = ?)", params![y, project_id], |r| r.get(0))?;
        if !ok { return Err(bad("no such payment")); }
    }
    Ok((item.map(str::to_string), payment.map(str::to_string)))
}

/// Encrypts and keeps a bill, invoice or receipt (a PDF or a photo) with the project, optionally
/// for one bill item or payment (`item_id` / `payment_id` in `link`).
pub fn add_file(conn: &Connection, dk: &crate::crypto::Key, dir: &std::path::Path, project_id: &str, name: &str, bytes: &[u8], link: &Value) -> Result<Value> {
    currency_of(conn, project_id)?;
    if bytes.len() > crate::documents::MAX_FILE { return Err(bad("a file can be up to 25 MB")); }
    let mime = crate::documents::sniff(bytes).ok_or_else(|| bad("Vaulty keeps PDFs and photos (PNG, JPEG or WebP)"))?;
    let n: i64 = conn.query_row("SELECT count(*) FROM project_files WHERE project_id = ?", [project_id], |r| r.get(0))?;
    if n >= MAX_FILES_PER_PROJECT { return Err(bad("a project can keep up to 500 files")); }
    let (item, payment) = file_link(conn, project_id, link)?;
    let name: String = std::path::Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or("bill").chars().take(120).collect();
    let id = new_id();
    crate::documents::write_encrypted(dk, dir, &id, bytes)?;
    conn.execute("INSERT INTO project_files (id, project_id, item_id, payment_id, name, mime, size, sha256) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, project_id, item, payment, name, mime, bytes.len() as i64, crate::crypto::sha256_hex(bytes)])?;
    file_row(conn, &id)
}

fn file_row(conn: &Connection, id: &str) -> Result<Value> {
    files_where(conn, "f.id = ?", id)?.pop().ok_or_else(|| bad("no such file"))
}

fn files_where(conn: &Connection, filter: &str, arg: &str) -> Result<Vec<Value>> {
    query(conn, &format!(
        "SELECT f.id, f.project_id, f.item_id, f.payment_id, f.name, f.mime, f.size, f.created_at, i.item AS item, i.date AS item_date,
                y.vendor AS payment_vendor, y.date AS payment_date, y.amount_minor AS payment_amount_minor
         FROM project_files f LEFT JOIN project_items i ON i.id = f.item_id LEFT JOIN project_payments y ON y.id = f.payment_id
         WHERE {filter} ORDER BY f.created_at DESC, f.rowid DESC"), &[json!(arg)])
}

/// A project's files, newest first, with what each is for.
pub fn files(conn: &Connection, project_id: &str) -> Result<Vec<Value>> {
    let cur = currency_of(conn, project_id)?;
    Ok(files_where(conn, "f.project_id = ?", project_id)?.into_iter().map(|mut f| {
        let a = f["payment_amount_minor"].as_i64();
        if let Some(o) = f.as_object_mut() { o.insert("payment_amount".into(), json!(a.map(|a| money::to_decimal(a, &cur)))); }
        f
    }).collect())
}

/// Points a file at a bill item or payment (or, with neither, at the project alone).
pub fn link_file(conn: &Connection, id: &str, link: &Value) -> Result<Value> {
    let project_id: String = conn.query_row("SELECT project_id FROM project_files WHERE id = ?", [id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such file"))?;
    let (item, payment) = file_link(conn, &project_id, link)?;
    conn.execute("UPDATE project_files SET item_id = ?, payment_id = ? WHERE id = ?", params![item, payment, id])?;
    file_row(conn, id)
}

/// A file's name, type and contents, checked against the fingerprint taken when it was added.
pub fn read_file(conn: &Connection, dk: &crate::crypto::Key, dir: &std::path::Path, id: &str) -> Result<(String, String, Vec<u8>)> {
    let (name, mime, sha): (String, String, String) = conn.query_row("SELECT name, mime, sha256 FROM project_files WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?.ok_or_else(|| bad("no such file"))?;
    let bytes = crate::documents::read_encrypted(dk, dir, id)?;
    if crate::crypto::sha256_hex(&bytes) != sha { return Err(bad("this file is damaged")); }
    Ok((name, mime, bytes))
}

/// Removes a file from the project. The encrypted file stays for 30 days, so undo can bring it back.
pub fn remove_file(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM project_files WHERE id = ?", [id])? == 0 { return Err(bad("no such file")); }
    Ok(())
}
