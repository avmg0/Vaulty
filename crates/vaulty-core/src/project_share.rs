//! Sharing a project as a file: everything in it (sections, people, bill items, payments, and the
//! bills themselves if wanted), to send to someone else who uses Vaulty. The app encrypts the file
//! with a password (as backups are). Opening it adds the project, or, when it's the same project
//! (sent back after changes), brings it up to date.
//!
//! Who is "you" differs between the two of you: the file says which person sent it, and the one
//! opening it says which person they are. Links to your own transactions are never sent.
use crate::{crypto::Key, documents, error::{Error, Result}, money, projects, rows::*};
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;

pub const FORMAT: &str = "vaulty-project";
pub const VERSION: i64 = 1;
/// Bills carried in a file, at most (more and the file leaves them out).
pub const MAX_FILES: usize = 60 * 1024 * 1024;

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }

/// The project as a shareable value. Gives the project a share id first, if it has none, so a
/// file sent back can find it.
pub fn export(conn: &Connection, dk: &Key, dir: &Path, project_id: &str, with_files: bool) -> Result<Value> {
    let p = query_one(conn, "SELECT id, name, kind, status, currency, budget_minor, start_date, end_date, notes, columns, hidden, labels, share_id FROM projects WHERE id = ?", &[json!(project_id)])?
        .ok_or_else(|| bad("no such project"))?;
    let share_id = match p["share_id"].as_str() {
        Some(s) => s.to_string(),
        None => { let s = new_id(); conn.execute("UPDATE projects SET share_id = ? WHERE id = ?", params![s, project_id])?; s }
    };
    let me: Option<String> = conn.query_row("SELECT id FROM project_members WHERE project_id = ? AND is_me = 1", [project_id], |r| r.get(0)).optional()?;
    let sections = query(conn, "SELECT id, name, budget_minor, position FROM project_sections WHERE project_id = ? ORDER BY position", &[json!(project_id)])?;
    let members = query(conn, "SELECT id, name, share, position FROM project_members WHERE project_id = ? ORDER BY position", &[json!(project_id)])?;
    let items = query(conn, "SELECT id, section_id, date, item, kind, qty_e4, unit, rate_minor, amount_minor, tax_minor, vendor, bill_no, notes, custom FROM project_items WHERE project_id = ? ORDER BY date, rowid", &[json!(project_id)])?;
    // Payments you made are marked as made by your person, so the other side sees who paid.
    let payments = query(conn, "SELECT id, section_id, date, amount_minor, vendor, mode, account_name, reference, notes, coalesce(paid_by, ?) AS paid_by FROM project_payments WHERE project_id = ? ORDER BY date, rowid",
        &[json!(me), json!(project_id)])?;
    let mut files = vec![];
    let mut left_out = 0;
    if with_files {
        let list = query(conn, "SELECT id, item_id, payment_id, name, mime, size, sha256 FROM project_files WHERE project_id = ?", &[json!(project_id)])?;
        let mut total = 0usize;
        for mut f in list {
            let size = f["size"].as_i64().unwrap_or(0) as usize;
            if total + size > MAX_FILES { left_out += 1; continue; }
            let id = f["id"].as_str().unwrap_or_default().to_string();
            match projects::read_file(conn, dk, dir, &id) {
                Ok((_, _, bytes)) => {
                    total += size;
                    if let Some(o) = f.as_object_mut() { o.insert("data".into(), json!(base64::engine::general_purpose::STANDARD.encode(bytes))); }
                    files.push(f);
                }
                Err(_) => left_out += 1,
            }
        }
    }
    Ok(json!({
        "format": FORMAT, "version": VERSION, "share_id": share_id, "sent_by": me, "exported_at": now(),
        "project": { "name": p["name"], "kind": p["kind"], "status": p["status"], "currency": p["currency"], "budget_minor": p["budget_minor"],
                     "start_date": p["start_date"], "end_date": p["end_date"], "notes": p["notes"], "columns": p["columns"], "hidden": p["hidden"], "labels": p["labels"] },
        "sections": sections, "members": members, "items": items, "payments": payments, "files": files, "files_left_out": left_out,
    }))
}

fn check_format(data: &Value) -> Result<()> {
    if data["format"] != FORMAT { return Err(bad("this isn't a shared Vaulty project")); }
    if data["version"].as_i64().unwrap_or(0) > VERSION { return Err(bad("this project was shared from a newer Vaulty; update Vaulty to open it")); }
    if data["share_id"].as_str().is_none_or(|s| s.is_empty() || s.len() > 64) { return Err(bad("this shared project is damaged")); }
    Ok(())
}

/// What a shared project holds, before opening it: its name, the people in it (to say which
/// one you are), and whether it updates a project you already have.
pub fn peek(conn: &Connection, data: &Value) -> Result<Value> {
    check_format(data)?;
    let existing: Option<(String, String)> = conn.query_row("SELECT id, name FROM projects WHERE share_id = ?", [data["share_id"].as_str()], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let n = |k: &str| data[k].as_array().map_or(0, Vec::len);
    let cur = data["project"]["currency"].as_str().unwrap_or("INR");
    let billed: i64 = data["items"].as_array().into_iter().flatten().map(|i| i["amount_minor"].as_i64().unwrap_or(0) + i["tax_minor"].as_i64().unwrap_or(0)).sum();
    let paid: i64 = data["payments"].as_array().into_iter().flatten().map(|y| y["amount_minor"].as_i64().unwrap_or(0)).sum();
    Ok(json!({
        "name": data["project"]["name"], "currency": cur, "sent_by": data["sent_by"], "exported_at": data["exported_at"],
        "members": data["members"].as_array().into_iter().flatten().map(|m| json!({ "id": m["id"], "name": m["name"] })).collect::<Vec<_>>(),
        "items": n("items"), "payments": n("payments"), "files": n("files"), "files_left_out": data["files_left_out"],
        "billed": money::to_decimal(billed, cur), "paid": money::to_decimal(paid, cur),
        "updates": existing.map(|(id, name)| json!({ "id": id, "name": name })),
    }))
}

fn s<'a>(v: &'a Value, k: &str, max: usize) -> Result<Option<&'a str>> {
    match v[k].as_str() {
        Some(t) if t.chars().count() > max => Err(bad("this shared project is damaged")),
        Some("") => Ok(None),
        t => Ok(t),
    }
}
fn id_of(v: &Value, k: &str) -> Result<Option<String>> {
    match v[k].as_str() {
        None | Some("") => Ok(None),
        Some(t) if t.len() <= 64 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => Ok(Some(t.to_string())),
        _ => Err(bad("this shared project is damaged")),
    }
}
fn need_id(v: &Value, k: &str) -> Result<String> { id_of(v, k)?.ok_or_else(|| bad("this shared project is damaged")) }
fn date_of(v: &Value, k: &str) -> Result<Option<String>> {
    match v[k].as_str() {
        None | Some("") => Ok(None),
        Some(d) => chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map(|d| Some(d.to_string())).map_err(|_| bad("this shared project is damaged")),
    }
}
fn minor(v: &Value, k: &str) -> Option<i64> { v[k].as_i64().filter(|m| (0..=money::MAX_MINOR).contains(m)) }

/// Opens a shared project: adds it, or brings the one it came from up to date (your links to
/// transactions are kept). `me` is the id of the person in it who is you (if it has people).
pub fn import(conn: &Connection, dk: &Key, dir: &Path, data: &Value, me: Option<&str>) -> Result<Value> {
    check_format(data)?;
    let share_id = data["share_id"].as_str().unwrap_or_default();
    let pj = &data["project"];
    let name = s(pj, "name", 100)?.ok_or_else(|| bad("this shared project is damaged"))?;
    let kind = pj["kind"].as_str().filter(|k| projects::KINDS.contains(k)).unwrap_or("group");
    let status = pj["status"].as_str().filter(|k| projects::STATUSES.contains(k)).unwrap_or("ongoing");
    let currency = pj["currency"].as_str().filter(|c| c.len() == 3 && c.chars().all(|x| x.is_ascii_alphabetic())).unwrap_or("INR").to_uppercase();
    // The project's own columns are checked as if typed in.
    let parse = |k: &str, empty: Value| pj[k].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).unwrap_or(empty);
    let (columns, hidden, labels) = projects::checked_columns(&parse("columns", json!([])), &parse("hidden", json!([])), &parse("labels", json!({})))
        .map_err(|_| bad("this shared project is damaged"))?;
    let members: Vec<Value> = data["members"].as_array().cloned().unwrap_or_default();
    if let Some(m) = me { if !members.iter().any(|x| x["id"] == m) { return Err(bad("choose which person you are")); } }
    if !members.is_empty() && me.is_none() { return Err(bad("choose which person you are")); }

    atomic(conn, |tx| {
        let existing: Option<String> = tx.query_row("SELECT id FROM projects WHERE share_id = ?", [share_id], |r| r.get(0)).optional()?;
        let updated = existing.is_some();
        // Your links from payments to your transactions, kept across the update.
        let mut links: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let id = match existing {
            Some(id) => {
                for r in query(tx, "SELECT id, entry_id FROM project_payments WHERE project_id = ? AND entry_id IS NOT NULL", &[json!(id)])? {
                    links.insert(r["id"].as_str().unwrap_or_default().into(), r["entry_id"].as_str().unwrap_or_default().into());
                }
                for t in ["project_files", "project_payments", "project_items", "project_members", "project_sections"] {
                    tx.execute(&format!("DELETE FROM {t} WHERE project_id = ?"), [&id])?;
                }
                tx.execute("UPDATE projects SET name = ?, kind = ?, status = ?, budget_minor = ?, start_date = ?, end_date = ?, notes = ?, columns = ?, hidden = ?, labels = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    params![name, kind, status, minor(pj, "budget_minor"), date_of(pj, "start_date")?, date_of(pj, "end_date")?, s(pj, "notes", 2000)?, columns, hidden, labels, id])?;
                id
            }
            None => {
                let id = new_id();
                tx.execute("INSERT INTO projects (id, name, kind, status, currency, budget_minor, start_date, end_date, notes, columns, hidden, labels, share_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![id, name, kind, status, currency, minor(pj, "budget_minor"), date_of(pj, "start_date")?, date_of(pj, "end_date")?, s(pj, "notes", 2000)?, columns, hidden, labels, share_id])?;
                id
            }
        };
        let taken = |tx: &Connection, t: &str, rid: &str| -> Result<bool> {
            Ok(tx.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {t} WHERE id = ?)"), [rid], |r| r.get(0))?)
        };
        for x in data["sections"].as_array().into_iter().flatten() {
            let sid = need_id(x, "id")?;
            if taken(tx, "project_sections", &sid)? { return Err(bad("this shared project is damaged")); }
            tx.execute("INSERT INTO project_sections (id, project_id, name, budget_minor, position) VALUES (?, ?, ?, ?, ?)",
                params![sid, id, s(x, "name", 60)?.unwrap_or("—"), minor(x, "budget_minor"), x["position"].as_i64().unwrap_or(0)])?;
        }
        for x in &members {
            let mid = need_id(x, "id")?;
            let share = x["share"].as_f64().filter(|v| v.is_finite() && (0.0..=1000.0).contains(v)).unwrap_or(1.0);
            tx.execute("INSERT INTO project_members (id, project_id, name, share, is_me, position) VALUES (?, ?, ?, ?, ?, ?)",
                params![mid, id, s(x, "name", 60)?.unwrap_or("—"), share, me == Some(mid.as_str()), x["position"].as_i64().unwrap_or(0)])?;
        }
        let cols: Vec<Value> = serde_json::from_str(&columns).unwrap_or_default();
        for x in data["items"].as_array().into_iter().flatten() {
            let custom = x["custom"].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).unwrap_or(json!({}));
            let custom = projects::check_custom(&cols, &custom).map_err(|_| bad("this shared project is damaged"))?;
            let kind = x["kind"].as_str().filter(|k| projects::ITEM_KINDS.contains(k)).unwrap_or("other");
            tx.execute(
                "INSERT INTO project_items (id, project_id, section_id, date, item, kind, qty_e4, unit, rate_minor, amount_minor, tax_minor, vendor, bill_no, notes, custom)
                 VALUES (?, ?, (SELECT id FROM project_sections WHERE id = ? AND project_id = ?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![need_id(x, "id")?, id, id_of(x, "section_id")?, id, date_of(x, "date")?.ok_or_else(|| bad("this shared project is damaged"))?,
                        s(x, "item", 200)?.unwrap_or("—"), kind, x["qty_e4"].as_i64().filter(|q| *q > 0), s(x, "unit", 20)?, minor(x, "rate_minor"),
                        minor(x, "amount_minor").unwrap_or(0), minor(x, "tax_minor").unwrap_or(0), s(x, "vendor", 100)?, s(x, "bill_no", 60)?, s(x, "notes", 1000)?, custom])?;
        }
        for x in data["payments"].as_array().into_iter().flatten() {
            let pid = need_id(x, "id")?;
            let mode = x["mode"].as_str().filter(|k| projects::MODES.contains(k)).unwrap_or("other");
            // Paid by the person who is you: yours.
            let paid_by = id_of(x, "paid_by")?.filter(|p| Some(p.as_str()) != me);
            let amount = minor(x, "amount_minor").filter(|a| *a > 0).ok_or_else(|| bad("this shared project is damaged"))?;
            let entry = links.get(&pid).filter(|_| paid_by.is_none()).cloned();
            tx.execute(
                "INSERT INTO project_payments (id, project_id, section_id, date, amount_minor, vendor, mode, account_name, reference, notes, paid_by, entry_id)
                 VALUES (?, ?, (SELECT id FROM project_sections WHERE id = ? AND project_id = ?), ?, ?, ?, ?, ?, ?, ?, (SELECT id FROM project_members WHERE id = ? AND project_id = ?),
                         (SELECT id FROM entries WHERE id = ?))",
                params![pid, id, id_of(x, "section_id")?, id, date_of(x, "date")?.ok_or_else(|| bad("this shared project is damaged"))?, amount,
                        s(x, "vendor", 100)?, mode, s(x, "account_name", 100)?, s(x, "reference", 60)?, s(x, "notes", 1000)?, paid_by, id, entry])?;
        }
        let mut files = 0;
        for x in data["files"].as_array().into_iter().flatten() {
            let fid = need_id(x, "id")?;
            let Some(bytes) = x["data"].as_str().and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()) else { continue };
            let Some(mime) = documents::sniff(&bytes) else { continue };
            if bytes.len() > documents::MAX_FILE { continue; }
            documents::write_encrypted(dk, dir, &fid, &bytes)?;
            let name: String = s(x, "name", 120)?.unwrap_or("bill").chars().filter(|c| !matches!(c, '/' | '\\')).collect();
            tx.execute(
                "INSERT INTO project_files (id, project_id, item_id, payment_id, name, mime, size, sha256) VALUES (?, ?,
                   (SELECT id FROM project_items WHERE id = ? AND project_id = ?), (SELECT id FROM project_payments WHERE id = ? AND project_id = ?), ?, ?, ?, ?)",
                params![fid, id, id_of(x, "item_id")?, id, id_of(x, "payment_id")?, id, name, mime, bytes.len() as i64, crate::crypto::sha256_hex(&bytes)])?;
            files += 1;
        }
        Ok(json!({ "id": id, "updated": updated, "files": files }))
    })
}
