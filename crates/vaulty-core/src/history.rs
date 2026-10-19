//! Change history, undo and trash.
//!
//! Every insert, update and delete on the member's data is captured by SQLite triggers into
//! `change_log`, with the row before and after as JSON. Changes are grouped into a *batch* per
//! user action (one batch = what one Undo reverses). Triggers only record while a batch is open,
//! so migrations and internal bookkeeping are not logged.
//!
//! - **Undo** applies a batch's inverse as a new batch, so an undo can itself be undone.
//! - **Trash** is the list of recent batches that deleted something; restoring = undoing it.
//! - A batch can't be undone if a later change touched the same rows; the caller is told why.
//!
//! The log lives in the member's encrypted database. Batches older than 90 days are purged.

use crate::{error::{Error, Result}, rows::atomic};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

/// Tables whose changes are recorded. Excludes caches, provider keys (never copied into the log),
/// static data, BLOB-holding tables and the log itself.
const TRACKED: &[&str] = &[
    "accounts", "cards", "categories", "entries", "postings", "ledger_accounts", "workspaces",
    "calendar_events", "goals", "investments", "investment_events", "journal_entries",
    "recurring_payments", "tasks", "user_settings", "hotkeys", "budgets", "category_rules", "import_templates", "people",
];

pub const TRASH_DAYS: i64 = 30;
pub const KEEP_DAYS: i64 = 90;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS change_batches (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  at         TEXT NOT NULL,
  action     TEXT NOT NULL,              -- e.g. transaction.delete, account.update, undo
  undo_of    INTEGER,                    -- for an undo: the batch it reversed
  undone_by  INTEGER                     -- set when this batch has been undone
);
CREATE TABLE IF NOT EXISTS change_log (
  seq       INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id  INTEGER NOT NULL REFERENCES change_batches(id) ON DELETE CASCADE,
  tbl       TEXT NOT NULL,
  row_id    TEXT NOT NULL,
  op        TEXT NOT NULL CHECK (op IN ('insert', 'update', 'delete')),
  before    TEXT,
  after     TEXT
);
CREATE INDEX IF NOT EXISTS idx_change_log_row ON change_log(tbl, row_id);
CREATE INDEX IF NOT EXISTS idx_change_log_batch ON change_log(batch_id);
-- Chain state: the fingerprint of the last batch removed by the 90-day clean-up, so the chain
-- still verifies from there.
CREATE TABLE IF NOT EXISTS history_meta (k TEXT PRIMARY KEY, v TEXT NOT NULL);
-- The open batch, if any. One row; triggers read it.
CREATE TABLE IF NOT EXISTS change_context (only INTEGER PRIMARY KEY CHECK (only = 1), batch_id INTEGER);
INSERT OR IGNORE INTO change_context (only, batch_id) VALUES (1, NULL);
";

fn columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let cols = stmt.query_map([], |r| r.get::<_, String>(1))?.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cols)
}

fn ident(name: &str) -> String { format!("\"{}\"", name.replace('"', "\"\"")) }

/// Drops the logging triggers (before migrations, which may alter tracked tables).
pub fn drop_triggers(conn: &Connection) -> Result<()> {
    let names: Vec<String> = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'trigger' AND name LIKE '_hist_%'")?
        .query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    for n in names { conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {}", ident(&n)))?; }
    Ok(())
}

/// Creates the log tables and (re)creates triggers from the current columns. Run on every open.
pub fn install(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    // Tamper-evident chain (added later): each batch stores its predecessor's fingerprint and its own.
    let cols = columns(conn, "change_batches")?;
    if !cols.iter().any(|c| c == "hash") {
        conn.execute_batch("ALTER TABLE change_batches ADD COLUMN prev_hash TEXT; ALTER TABLE change_batches ADD COLUMN hash TEXT;")?;
    }
    drop_triggers(conn)?;
    // A batch left open by a crash must not capture unrelated changes.
    conn.execute("UPDATE change_context SET batch_id = NULL", [])?;
    for t in TRACKED {
        let cols = columns(conn, t)?;
        if cols.is_empty() || !cols.iter().any(|c| c == "id") { continue; }
        let obj = |p: &str, skip_ts: bool| cols.iter()
            .filter(|c| !(skip_ts && c.as_str() == "updated_at"))
            .map(|c| format!("'{}', {p}.{}", c.replace('\'', "''"), ident(c)))
            .collect::<Vec<_>>().join(", ");
        let batch = "(SELECT batch_id FROM change_context WHERE only = 1)";
        let tq = ident(t);
        conn.execute_batch(&format!("
            CREATE TRIGGER {ins} AFTER INSERT ON {tq} WHEN {batch} IS NOT NULL BEGIN
              INSERT INTO change_log (batch_id, tbl, row_id, op, after) VALUES ({batch}, '{t}', NEW.id, 'insert', json_object({new}));
            END;
            CREATE TRIGGER {upd} AFTER UPDATE ON {tq} WHEN {batch} IS NOT NULL AND json_object({old_c}) IS NOT json_object({new_c}) BEGIN
              INSERT INTO change_log (batch_id, tbl, row_id, op, before, after) VALUES ({batch}, '{t}', NEW.id, 'update', json_object({old}), json_object({new}));
            END;
            CREATE TRIGGER {del} AFTER DELETE ON {tq} WHEN {batch} IS NOT NULL BEGIN
              INSERT INTO change_log (batch_id, tbl, row_id, op, before) VALUES ({batch}, '{t}', OLD.id, 'delete', json_object({old}));
            END;",
            ins = ident(&format!("_hist_{t}_ins")), upd = ident(&format!("_hist_{t}_upd")), del = ident(&format!("_hist_{t}_del")),
            new = obj("NEW", false), old = obj("OLD", false), new_c = obj("NEW", true), old_c = obj("OLD", true),
        ))?;
    }
    purge(conn)?;
    Ok(())
}

fn purge(conn: &Connection) -> Result<()> {
    let cutoff: String = conn.query_row(&format!("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-{KEEP_DAYS} days')"), [], |r| r.get(0))?;
    purge_before(conn, &cutoff)
}

/// Removes batches older than `cutoff` (RFC 3339), keeping the newest removed fingerprint as the
/// chain's checkpoint so what remains still verifies.
pub fn purge_before(conn: &Connection, cutoff: &str) -> Result<()> {
    atomic(conn, |c| {
        let last: Option<(i64, String)> = c.query_row(
            "SELECT id, hash FROM change_batches WHERE at < ? AND hash IS NOT NULL ORDER BY id DESC LIMIT 1", [cutoff],
            |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        if let Some((id, hash)) = last {
            // Everything up to and including the checkpoint goes, so no unchained gap is left.
            c.execute("INSERT OR REPLACE INTO history_meta (k, v) VALUES ('anchor', ?)", [&hash])?;
            c.execute("DELETE FROM change_batches WHERE id <= ?", [id])?;
        }
        c.execute("DELETE FROM change_batches WHERE at < ? AND hash IS NULL", [cutoff])?;
        Ok(())
    })
}

// ------------------------------------------------------------------ tamper evidence

/// Where the chain starts when nothing has been cleaned up yet.
const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn field(buf: &mut Vec<u8>, v: &str) {
    // Length-prefixed, so no choice of values can make two different batches hash alike.
    buf.extend_from_slice(v.len().to_string().as_bytes());
    buf.push(b':');
    buf.extend_from_slice(v.as_bytes());
    buf.push(b'\n');
}

/// SHA-256 over the previous fingerprint and everything the batch recorded. `undone_by` is left
/// out on purpose: it changes later, and the undo itself is its own chained batch.
fn batch_hash(conn: &Connection, id: i64, prev: &str) -> Result<String> {
    let (at, action, undo_of): (String, String, Option<i64>) = conn.query_row(
        "SELECT at, action, undo_of FROM change_batches WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut buf = Vec::with_capacity(512);
    for v in ["vaulty-history-v1", prev, &id.to_string(), &at, &action, &undo_of.map(|u| u.to_string()).unwrap_or_default()] { field(&mut buf, v); }
    let mut stmt = conn.prepare("SELECT seq, tbl, row_id, op, before, after FROM change_log WHERE batch_id = ? ORDER BY seq")?;
    let rows = stmt.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?,
        r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?)))?;
    for row in rows {
        let (seq, tbl, rid, op, before, after) = row?;
        for v in [seq.to_string(), tbl, rid, op, before.unwrap_or_default(), after.unwrap_or_default()] { field(&mut buf, &v); }
    }
    Ok(crate::crypto::sha256_hex(&buf))
}

fn anchor(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT v FROM history_meta WHERE k = 'anchor'", [], |r| r.get(0)).optional()?.unwrap_or_else(|| GENESIS.to_string()))
}

/// Seals a closed batch onto the end of the chain.
fn seal(conn: &Connection, id: i64) -> Result<()> {
    let prev: Option<String> = conn.query_row(
        "SELECT hash FROM change_batches WHERE id < ? AND hash IS NOT NULL ORDER BY id DESC LIMIT 1", [id], |r| r.get(0)).optional()?;
    let prev = match prev { Some(p) => p, None => anchor(conn)? };
    let hash = batch_hash(conn, id, &prev)?;
    conn.execute("UPDATE change_batches SET prev_hash = ?, hash = ? WHERE id = ?", params![prev, hash, id])?;
    Ok(())
}

/// Result of checking the history.
#[derive(Debug, serde::Serialize)]
pub struct Verification {
    /// True when the chain is unbroken and nothing was changed outside Vaulty.
    pub ok: bool,
    /// Actions checked.
    pub checked: i64,
    /// Time of the oldest checked action.
    pub since: Option<String>,
    /// Fingerprint of the latest action; write it down or give it to an accountant to detect
    /// later rewriting of the whole chain.
    pub head: String,
    /// First action that doesn't match, if any.
    pub broken_at: Option<i64>,
    pub reason: Option<String>,
    /// Items whose current contents differ from their last recorded state (changed outside Vaulty).
    pub changed_outside: Vec<Value>,
    /// Older actions recorded before the chain existed (not covered).
    pub unchained: i64,
}

/// Re-checks every fingerprint in order, then compares each item with its last recorded state.
pub fn verify(conn: &Connection) -> Result<Verification> {
    let mut prev = anchor(conn)?;
    let rows: Vec<(i64, String, Option<String>, Option<String>)> = conn.prepare(
        "SELECT id, at, prev_hash, hash FROM change_batches ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
    let (mut checked, mut unchained, mut since) = (0i64, 0i64, None);
    let mut broken: Option<(i64, String)> = None;
    let mut seen_chain = false;
    for (id, at, p, h) in rows {
        let Some(h) = h else {
            if seen_chain { broken = Some((id, "an action lost its fingerprint".into())); break; }
            unchained += 1;
            continue;
        };
        seen_chain = true;
        if p.as_deref() != Some(prev.as_str()) {
            broken = Some((id, "an earlier action is missing or out of order".into()));
            break;
        }
        if batch_hash(conn, id, &prev)? != h {
            broken = Some((id, "this action's details were altered".into()));
            break;
        }
        since.get_or_insert(at);
        prev = h;
        checked += 1;
    }
    let changed_outside = if broken.is_none() { drift(conn)? } else { vec![] };
    Ok(Verification {
        ok: broken.is_none() && changed_outside.is_empty(),
        checked, since, head: prev, unchained,
        broken_at: broken.as_ref().map(|b| b.0), reason: broken.map(|b| b.1),
        changed_outside,
    })
}

/// After an app update changes stored data (migrations run without recording), write those
/// changes into the history as one "app.update" action, so verification doesn't flag them as
/// edits made outside Vaulty.
pub fn record_app_update(conn: &Connection) -> Result<usize> {
    let changed = drift(conn)?;
    if changed.is_empty() { return Ok(0); }
    record(conn, "app.update", |c| {
        let batch: i64 = c.query_row("SELECT batch_id FROM change_context WHERE only = 1", [], |r| r.get(0))?;
        for item in &changed {
            let (t, rid) = (item["table"].as_str().unwrap_or_default(), item["id"].as_str().unwrap_or_default());
            let cols = columns(c, t)?;
            let obj = cols.iter().map(|col| format!("'{}', {}", col.replace('\'', "''"), ident(col))).collect::<Vec<_>>().join(", ");
            let before: Option<String> = c.query_row(
                "SELECT after FROM change_log WHERE tbl = ? AND row_id = ? ORDER BY seq DESC LIMIT 1", [t, rid], |r| r.get(0)).optional()?.flatten();
            let after: Option<String> = c.query_row(&format!("SELECT json_object({obj}) FROM {} WHERE id = ?", ident(t)), [rid], |r| r.get(0)).optional()?;
            let op = if after.is_some() { "update" } else { "delete" };
            c.execute("INSERT INTO change_log (batch_id, tbl, row_id, op, before, after) VALUES (?, ?, ?, ?, ?, ?)",
                params![batch, t, rid, op, before, after])?;
        }
        Ok(changed.len())
    })
}

/// Items whose current row differs from the last state the history recorded for them.
fn drift(conn: &Connection) -> Result<Vec<Value>> {
    let mut out = vec![];
    for t in TRACKED {
        let cols = columns(conn, t)?;
        if !cols.iter().any(|c| c == "id") { continue; }
        // Same row shape as the triggers write, minus updated_at (maintained by its own trigger).
        let obj = cols.iter().filter(|c| c.as_str() != "updated_at")
            .map(|c| format!("'{}', {}", c.replace('\'', "''"), ident(c))).collect::<Vec<_>>().join(", ");
        let mut last = conn.prepare(
            "SELECT l.row_id, l.op, l.after FROM change_log l
              WHERE l.tbl = ?1 AND l.seq = (SELECT MAX(seq) FROM change_log m WHERE m.tbl = ?1 AND m.row_id = l.row_id)")?;
        let logged: Vec<(String, String, Option<String>)> = last.query_map([t], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        for (rid, op, after) in logged {
            let current: Option<String> = conn.query_row(&format!("SELECT json_object({obj}) FROM {} WHERE id = ?", ident(t)), [&rid], |r| r.get(0)).optional()?;
            // Compare only the fields that were recorded: a column added by a later app update
            // isn't an edit made outside Vaulty.
            let same = |cur: &str, rec: &str| -> bool {
                let (Ok(Value::Object(c)), Ok(Value::Object(r))) = (serde_json::from_str::<Value>(cur), serde_json::from_str::<Value>(rec)) else { return cur == rec };
                r.iter().filter(|(k, _)| k.as_str() != "updated_at").all(|(k, v)| c.get(k) == Some(v))
            };
            let differs = match (op.as_str(), current.as_deref(), after.as_deref()) {
                ("delete", Some(_), _) => true,
                ("delete", None, _) => false,
                (_, None, _) => true,
                (_, Some(cur), Some(a)) => !same(cur, a),
                _ => false,
            };
            if differs { out.push(json!({ "table": t, "id": rid })); }
        }
    }
    Ok(out)
}


/// Runs `f` as one undoable action. Nested calls join the outer batch.
pub fn record<T>(conn: &Connection, action: &str, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let open: Option<i64> = conn.query_row("SELECT batch_id FROM change_context WHERE only = 1", [], |r| r.get(0))?;
    if open.is_some() { return f(conn); }
    atomic(conn, |c| {
        c.execute("INSERT INTO change_batches (at, action) VALUES (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), ?)", [action])?;
        let id = c.last_insert_rowid();
        c.execute("UPDATE change_context SET batch_id = ? WHERE only = 1", [id])?;
        let out = f(c);
        c.execute("UPDATE change_context SET batch_id = NULL WHERE only = 1", [])?;
        let out = out?;
        let n: i64 = c.query_row("SELECT COUNT(*) FROM change_log WHERE batch_id = ?", [id], |r| r.get(0))?;
        if n == 0 { c.execute("DELETE FROM change_batches WHERE id = ?", [id])?; } else { seal(c, id)?; }
        Ok(out)
    })
}

struct Change { tbl: String, row_id: String, op: String, before: Option<Value>, after: Option<Value> }

fn changes(conn: &Connection, batch: i64) -> Result<Vec<Change>> {
    let mut stmt = conn.prepare("SELECT tbl, row_id, op, before, after FROM change_log WHERE batch_id = ? ORDER BY seq")?;
    let parse = |s: Option<String>| s.and_then(|s| serde_json::from_str(&s).ok());
    let rows = stmt.query_map([batch], |r| {
        Ok(Change { tbl: r.get(0)?, row_id: r.get(1)?, op: r.get(2)?, before: parse(r.get(3)?), after: parse(r.get(4)?) })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Why a batch can't be undone, if it can't.
fn blocker(conn: &Connection, batch: i64) -> Result<Option<String>> {
    let (undone, action): (Option<i64>, String) = conn.query_row("SELECT undone_by, action FROM change_batches WHERE id = ?", [batch], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?.ok_or_else(|| Error::Invalid("that change is no longer in the history".into()))?;
    if undone.is_some() { return Ok(Some(crate::error::msg("already undone").into())); }
    // Reversing a data-format update could leave data the app no longer understands.
    if action == "app.update" { return Ok(Some(crate::error::msg("app updates can't be undone; restore a safety copy instead").into())); }
    // Later changes to the same rows block it, except a change that was undone and the undo that
    // reversed it: together they cancel out (add, edit, undo, undo works like any undo stack).
    let later: i64 = conn.query_row(
        "SELECT COUNT(*) FROM change_log l JOIN change_batches b ON b.id = l.batch_id
          WHERE l.batch_id > ?1 AND b.undone_by IS NULL
            AND NOT (b.undo_of IS NOT NULL AND (SELECT o.undone_by FROM change_batches o WHERE o.id = b.undo_of) = b.id)
            AND EXISTS (SELECT 1 FROM change_log m WHERE m.batch_id = ?1 AND m.tbl = l.tbl AND m.row_id = l.row_id)",
        [batch], |r| r.get(0))?;
    Ok((later > 0).then(|| crate::error::msg("changed again since; undo the later change first").into()))
}

/// Reverses a batch. Returns the id of the new (undo) batch.
pub fn undo(conn: &Connection, batch: i64) -> Result<i64> {
    if let Some(why) = blocker(conn, batch)? { return Err(Error::Invalid(format!("can't undo: {why}"))); }
    let list = changes(conn, batch)?;
    // Rows come back in reverse order; foreign keys are checked at commit, not per statement.
    conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    let res = record(conn, "undo", |c| {
        for ch in list.iter().rev() {
            if !TRACKED.contains(&ch.tbl.as_str()) { return Err(Error::Invalid(format!("unknown table {}", ch.tbl))); }
            let cols = columns(c, &ch.tbl)?;
            let t = ident(&ch.tbl);
            match (ch.op.as_str(), &ch.before) {
                ("insert", _) => { c.execute(&format!("DELETE FROM {t} WHERE id = ?"), [&ch.row_id])?; }
                ("delete", Some(Value::Object(row))) => {
                    let (names, vals) = known(row, &cols);
                    let ph = vec!["?"; names.len()].join(", ");
                    c.execute(&format!("INSERT INTO {t} ({}) VALUES ({ph})", names.join(", ")), rusqlite::params_from_iter(vals))?;
                }
                ("update", Some(Value::Object(row))) => {
                    let (names, vals) = known(row, &cols);
                    let set = names.iter().map(|n| format!("{n} = ?")).collect::<Vec<_>>().join(", ");
                    let mut vals = vals;
                    vals.push(rusqlite::types::Value::Text(ch.row_id.clone()));
                    c.execute(&format!("UPDATE {t} SET {set} WHERE id = ?"), rusqlite::params_from_iter(vals))?;
                }
                _ => return Err(Error::Invalid("history entry is incomplete".into())),
            }
        }
        let id: i64 = c.query_row("SELECT batch_id FROM change_context WHERE only = 1", [], |r| r.get(0))?;
        c.execute("UPDATE change_batches SET undo_of = ? WHERE id = ?", params![batch, id])?;
        c.execute("UPDATE change_batches SET undone_by = ? WHERE id = ?", params![id, batch])?;
        // An undo that is itself undone re-opens the original.
        c.execute("UPDATE change_batches SET undone_by = NULL WHERE id = (SELECT undo_of FROM change_batches WHERE id = ?)", [batch])?;
        Ok(id)
    });
    let _ = conn.execute_batch("PRAGMA defer_foreign_keys = OFF");
    res
}

/// Undoes the most recent action that hasn't been undone (Ctrl+Z). `None` if there's nothing.
pub fn undo_last(conn: &Connection) -> Result<Option<Value>> {
    let last: Option<i64> = conn.query_row(
        "SELECT id FROM change_batches WHERE undone_by IS NULL AND undo_of IS NULL AND action <> 'app.update' ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).optional()?;
    let Some(batch) = last else { return Ok(None) };
    let summary = describe(conn, batch)?;
    undo(conn, batch)?;
    Ok(Some(summary))
}

/// Column names and values for the columns that still exist.
fn known(row: &Map<String, Value>, cols: &[String]) -> (Vec<String>, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value as V;
    row.iter().filter(|(k, _)| cols.contains(k)).map(|(k, v)| {
        let val = match v {
            Value::Null => V::Null,
            Value::Bool(b) => V::Integer(*b as i64),
            Value::Number(n) => n.as_i64().map(V::Integer).unwrap_or_else(|| V::Real(n.as_f64().unwrap_or(0.0))),
            Value::String(s) => V::Text(s.clone()),
            other => V::Text(other.to_string()),
        };
        (ident(k), val)
    }).unzip()
}

/// What a batch did, for the UI: action, time, what kind of item and its name.
fn describe(conn: &Connection, batch: i64) -> Result<Value> {
    let (at, action, undo_of, undone_by): (String, String, Option<i64>, Option<i64>) = conn.query_row(
        "SELECT at, action, undo_of, undone_by FROM change_batches WHERE id = ?", [batch], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let list = changes(conn, batch)?;
    // The "main" row: the first change on a table people think in terms of.
    let main = list.iter().find(|c| matches!(c.tbl.as_str(), "entries" | "accounts" | "cards" | "categories" | "budgets" | "goals" | "tasks" | "calendar_events" | "investments" | "recurring_payments" | "journal_entries"))
        .or_else(|| list.first());
    let (kind, title, op) = main.map(|c| {
        let row = c.after.as_ref().or(c.before.as_ref());
        let pick = |keys: &[&str]| keys.iter().find_map(|k| row.and_then(|r| r.get(*k)).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned));
        let title = match c.tbl.as_str() {
            "entries" if entry_hidden(conn, &c.row_id, row) => crate::transactions::MASK.to_string(),
            "entries" => pick(&["merchant", "description"]).unwrap_or_else(|| "Transaction".into()),
            "journal_entries" | "tasks" | "calendar_events" => pick(&["title"]).unwrap_or_default(),
            "user_settings" => String::new(),
            "budgets" => pick(&["category_id"]).and_then(|cat| conn.query_row("SELECT name FROM categories WHERE id = ?", [cat], |r| r.get(0)).ok()).unwrap_or_default(),
            _ => pick(&["name", "symbol"]).unwrap_or_default(),
        };
        (c.tbl.clone(), title, c.op.clone())
    }).unwrap_or_default();
    // Amount of a transaction: its positive posting.
    let amount = if kind == "entries" {
        list.iter().filter(|c| c.tbl == "postings").filter_map(|c| c.after.as_ref().or(c.before.as_ref()))
            .filter_map(|r| Some((r.get("amount_minor")?.as_i64()?, r.get("currency")?.as_str()?.to_owned())))
            .find(|(a, _)| *a > 0)
            .map(|(a, cur)| json!({ "amount": crate::money::to_decimal(a, &cur), "currency": cur }))
    } else { None };
    let blocked = blocker(conn, batch)?;
    Ok(json!({
        "id": batch, "at": at, "action": action, "kind": kind, "op": op, "title": title, "amount": amount,
        "changes": list.len(), "undo_of": undo_of, "undone": undone_by.is_some(),
        "can_undo": blocked.is_none(), "blocked_reason": blocked,
    }))
}

/// True when a transaction is hidden now, or was hidden in the recorded row, so history never
/// shows its details outside the Hidden section.
fn entry_hidden(conn: &Connection, id: &str, row: Option<&Value>) -> bool {
    row.and_then(|r| r.get("hidden")).and_then(Value::as_i64) == Some(1)
        || conn.query_row("SELECT hidden FROM entries WHERE id = ?", [id], |r| r.get::<_, i64>(0)).map(|h| h == 1).unwrap_or(false)
}

/// Recent actions, newest first.
pub fn recent(conn: &Connection, limit: i64) -> Result<Vec<Value>> {
    let ids: Vec<i64> = conn.prepare("SELECT id FROM change_batches ORDER BY id DESC LIMIT ?")?
        .query_map([limit.clamp(1, 500)], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    ids.into_iter().map(|id| describe(conn, id)).collect()
}

/// Deleted items from the last 30 days that can still be restored.
pub fn trash(conn: &Connection) -> Result<Vec<Value>> {
    let ids: Vec<i64> = conn.prepare(&format!(
        "SELECT DISTINCT b.id FROM change_batches b JOIN change_log l ON l.batch_id = b.id
          WHERE b.undone_by IS NULL AND b.undo_of IS NULL AND l.op = 'delete'
            AND b.at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-{TRASH_DAYS} days')
          ORDER BY b.id DESC"))?
        .query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    let mut out = vec![];
    for id in ids {
        let d = describe(conn, id)?;
        // Soft deletes (archiving) also count: they're updates, handled by `recent`.
        if d["op"] == "delete" || d["action"].as_str().is_some_and(|a| a.ends_with(".delete")) { out.push(d); }
    }
    Ok(out)
}

/// Every action that touched one item (e.g. a transaction and its postings), newest first.
pub fn for_item(conn: &Connection, table: &str, id: &str) -> Result<Vec<Value>> {
    if !TRACKED.contains(&table) { return Err(Error::Invalid("no history for that kind of item".into())); }
    let sql = if table == "entries" {
        "SELECT DISTINCT batch_id FROM change_log WHERE (tbl = 'entries' AND row_id = ?1)
            OR (tbl = 'postings' AND (json_extract(after, '$.entry_id') = ?1 OR json_extract(before, '$.entry_id') = ?1)) ORDER BY batch_id DESC"
    } else {
        "SELECT DISTINCT batch_id FROM change_log WHERE tbl = ?2 AND row_id = ?1 ORDER BY batch_id DESC"
    };
    let mut stmt = conn.prepare(sql)?;
    let get = |r: &rusqlite::Row| r.get::<_, i64>(0);
    let ids: Vec<i64> = if table == "entries" {
        stmt.query_map(params![id], get)?.collect::<std::result::Result<_, _>>()?
    } else {
        stmt.query_map(params![id, table], get)?.collect::<std::result::Result<_, _>>()?
    };
    ids.into_iter().map(|b| describe(conn, b)).collect()
}

/// Number of actions in the history (for tests and the UI).
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM change_batches", [], |r| r.get(0))?)
}
