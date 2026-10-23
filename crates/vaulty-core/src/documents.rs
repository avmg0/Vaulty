//! Documents: passports, ID cards, insurance policies, property papers, wills, warranties.
//!
//! The details (title, number, expiry, where the original is kept) are rows in the member's
//! encrypted database. Each attached file (a PDF or photo) is encrypted on its own, under a key
//! derived from the member's data key, in the member's documents folder: `<file id>.vdoc` =
//! nonce (24 bytes) followed by XChaCha20-Poly1305 ciphertext, with the file id as associated data
//! (so files can't be swapped). Keeping files outside the database keeps safety copies small.
//!
//! Deleting a document puts it in the trash (undo brings it back); files of documents deleted more
//! than 30 days ago, and files nothing refers to any more, are removed by `purge`.
use crate::{crypto::{self, Key}, error::{Error, Result}, rows::*};
use base64::Engine;
use chacha20poly1305::{aead::{Aead, KeyInit, Payload}, XChaCha20Poly1305, XNonce};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const KINDS: &[&str] = &["id", "passport", "licence", "insurance", "property", "vehicle", "will", "certificate", "warranty", "medical", "tax", "other"];
/// Largest file accepted.
pub const MAX_FILE: usize = 25 * 1024 * 1024;
pub const MAX_FILES_PER_DOCUMENT: i64 = 10;
/// Files carried inside an encrypted backup, at most (more and the backup keeps the details only).
pub const MAX_BACKUP_FILES: usize = 60 * 1024 * 1024;
pub const TRASH_DAYS: i64 = 30;

fn file_key(dk: &Key) -> Key {
    let mut h = Sha256::new();
    h.update(b"vaulty-documents-v1");
    h.update(dk.as_ref());
    zeroize::Zeroizing::new(h.finalize().into())
}

fn path_of(dir: &Path, file_id: &str) -> Result<PathBuf> {
    if file_id.is_empty() || !file_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return Err(Error::Invalid("no such file".into())); }
    Ok(dir.join(format!("{file_id}.vdoc")))
}

/// What a file is, from its first bytes. Only PDFs and photos are kept.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF-") { Some("application/pdf") }
    else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("image/png") }
    else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { Some("image/jpeg") }
    else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" { Some("image/webp") }
    else { None }
}

fn write_encrypted(dk: &Key, dir: &Path, file_id: &str, plain: &[u8]) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let nonce = crypto::random_bytes::<24>();
    let ct = XChaCha20Poly1305::new(file_key(dk).as_ref().into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain, aad: file_id.as_bytes() })
        .map_err(|_| Error::Auth("encryption failed".into()))?;
    let path = path_of(dir, file_id)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, [nonce.as_slice(), &ct].concat())?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn read_encrypted(dk: &Key, dir: &Path, file_id: &str) -> Result<Vec<u8>> {
    let raw = std::fs::read(path_of(dir, file_id)?).map_err(|_| Error::Invalid("this file is missing from the documents folder".into()))?;
    if raw.len() < 24 { return Err(Error::Invalid("this file is damaged".into())); }
    let (nonce, ct) = raw.split_at(24);
    XChaCha20Poly1305::new(file_key(dk).as_ref().into())
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: file_id.as_bytes() })
        .map_err(|_| Error::Invalid("this file is damaged".into()))
}

const COLS: &str = "id, title, kind, holder, number, issuer, issued_on, expires_on, remind_days, kept_at, notes, created_at, updated_at";

fn decorate(conn: &Connection, mut d: Value, today: NaiveDate) -> Result<Value> {
    let files = query(conn, "SELECT id, name, mime, size, created_at FROM vault_document_files WHERE document_id = ? ORDER BY created_at, rowid", &[d["id"].clone()])?;
    let days = d["expires_on"].as_str().and_then(|e| NaiveDate::parse_from_str(e, "%Y-%m-%d").ok()).map(|e| (e - today).num_days());
    if let Some(o) = d.as_object_mut() {
        o.insert("files".into(), json!(files));
        o.insert("days_left".into(), json!(days));
        let soon = days.is_some_and(|n| n >= 0 && n <= o["remind_days"].as_i64().unwrap_or(30));
        o.insert("status".into(), json!(match days { Some(n) if n < 0 => "expired", Some(_) if soon => "expiring", Some(_) => "valid", None => "no_expiry" }));
    }
    Ok(d)
}

/// Documents not in the trash: those expiring soonest first, then by title.
pub fn list(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    query(conn, &format!("SELECT {COLS} FROM vault_documents WHERE deleted_at IS NULL ORDER BY expires_on IS NULL, expires_on, title COLLATE NOCASE"), &[])?
        .into_iter().map(|d| decorate(conn, d, today)).collect()
}

pub fn get(conn: &Connection, id: &str, today: NaiveDate) -> Result<Value> {
    let d = query_one(conn, &format!("SELECT {COLS} FROM vault_documents WHERE id = ? AND deleted_at IS NULL"), &[json!(id)])?
        .ok_or_else(|| Error::Invalid("no such document".into()))?;
    decorate(conn, d, today)
}

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

type Fields = (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64, Option<String>, Option<String>);

fn fields(v: &Value) -> Result<Fields> {
    let title = text(v, "title", 120)?.ok_or_else(|| Error::Invalid("give the document a name".into()))?;
    let kind = v["kind"].as_str().unwrap_or("other");
    if !KINDS.contains(&kind) { return Err(Error::Invalid("unknown kind of document".into())); }
    let remind = v["remind_days"].as_i64().unwrap_or(30);
    if !(0..=365).contains(&remind) { return Err(Error::Invalid("remind 0 to 365 days before it expires".into())); }
    let (issued, expires) = (date(v, "issued_on")?, date(v, "expires_on")?);
    if let (Some(i), Some(e)) = (&issued, &expires) { if e < i { return Err(Error::Invalid("it can't expire before it was issued".into())); } }
    Ok((title, kind.to_string(), text(v, "holder", 80)?, text(v, "number", 60)?, text(v, "issuer", 80)?, issued, expires, remind, text(v, "kept_at", 200)?, text(v, "notes", 2000)?))
}

pub fn create(conn: &Connection, v: &Value, today: NaiveDate) -> Result<Value> {
    let (title, kind, holder, number, issuer, issued, expires, remind, kept, notes) = fields(v)?;
    let id = new_id();
    conn.execute(
        "INSERT INTO vault_documents (id, title, kind, holder, number, issuer, issued_on, expires_on, remind_days, kept_at, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, title, kind, holder, number, issuer, issued, expires, remind, kept, notes])?;
    get(conn, &id, today)
}

pub fn update(conn: &Connection, id: &str, v: &Value, today: NaiveDate) -> Result<Value> {
    get(conn, id, today)?;
    let (title, kind, holder, number, issuer, issued, expires, remind, kept, notes) = fields(v)?;
    conn.execute(
        "UPDATE vault_documents SET title = ?, kind = ?, holder = ?, number = ?, issuer = ?, issued_on = ?, expires_on = ?, remind_days = ?,
                kept_at = ?, notes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![title, kind, holder, number, issuer, issued, expires, remind, kept, notes, id])?;
    get(conn, id, today)
}

/// To the trash. Undo brings it back with its files.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("UPDATE vault_documents SET deleted_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL", [id])? == 0 {
        return Err(Error::Invalid("no such document".into()));
    }
    Ok(())
}

/// Encrypts and attaches a file (a PDF or a photo).
pub fn add_file(conn: &Connection, dk: &Key, dir: &Path, document_id: &str, name: &str, bytes: &[u8]) -> Result<Value> {
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM vault_documents WHERE id = ? AND deleted_at IS NULL)", [document_id], |r| r.get(0))?;
    if !exists { return Err(Error::Invalid("no such document".into())); }
    if bytes.len() > MAX_FILE { return Err(Error::Invalid("a file can be up to 25 MB".into())); }
    let mime = sniff(bytes).ok_or_else(|| Error::Invalid("Vaulty keeps PDFs and photos (PNG, JPEG or WebP)".into()))?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM vault_document_files WHERE document_id = ?", [document_id], |r| r.get(0))?;
    if count >= MAX_FILES_PER_DOCUMENT { return Err(Error::Invalid("a document can have up to 10 files".into())); }
    let name: String = Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or("document").chars().take(120).collect();
    let id = new_id();
    write_encrypted(dk, dir, &id, bytes)?;
    conn.execute("INSERT INTO vault_document_files (id, document_id, name, mime, size, sha256) VALUES (?, ?, ?, ?, ?, ?)",
        params![id, document_id, name, mime, bytes.len() as i64, crypto::sha256_hex(bytes)])?;
    Ok(json!({ "id": id, "name": name, "mime": mime, "size": bytes.len() }))
}

/// A file's name, type and contents (checked against the fingerprint taken when it was added).
pub fn read_file(conn: &Connection, dk: &Key, dir: &Path, file_id: &str) -> Result<(String, String, Vec<u8>)> {
    let (name, mime, sha): (String, String, String) = conn.query_row(
        "SELECT f.name, f.mime, f.sha256 FROM vault_document_files f JOIN vault_documents d ON d.id = f.document_id WHERE f.id = ? AND d.deleted_at IS NULL",
        [file_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?.ok_or_else(|| Error::Invalid("no such file".into()))?;
    let bytes = read_encrypted(dk, dir, file_id)?;
    if crypto::sha256_hex(&bytes) != sha { return Err(Error::Invalid("this file is damaged".into())); }
    Ok((name, mime, bytes))
}

/// Detaches a file. The encrypted file stays until `purge`, so undo can bring it back.
pub fn remove_file(conn: &Connection, file_id: &str) -> Result<()> {
    if conn.execute("DELETE FROM vault_document_files WHERE id = ?", [file_id])? == 0 { return Err(Error::Invalid("no such file".into())); }
    Ok(())
}

/// Empties the trash after 30 days: documents deleted before then, and encrypted files nothing
/// refers to that are older than that. Returns how many files were removed.
pub fn purge(conn: &Connection, dir: &Path) -> Result<usize> {
    conn.execute(&format!("DELETE FROM vault_documents WHERE deleted_at IS NOT NULL AND deleted_at < datetime('now', '-{TRASH_DAYS} days')"), [])?;
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(0) };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(TRASH_DAYS as u64 * 86_400);
    let mut removed = 0;
    for e in entries.flatten() {
        let path = e.path();
        let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".vdoc")) else { continue };
        let used: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM vault_document_files WHERE id = ?)", [id], |r| r.get(0))?;
        let old = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < cutoff);
        if !used && old && std::fs::remove_file(&path).is_ok() { removed += 1; }
    }
    Ok(removed)
}

/// Documents expiring between two dates (for the calendar).
pub fn expiring_between(conn: &Connection, from: &str, to: &str) -> Result<Vec<Value>> {
    query(conn, "SELECT id, title, kind, holder, expires_on FROM vault_documents WHERE deleted_at IS NULL AND expires_on BETWEEN ? AND ? ORDER BY expires_on", &[json!(from), json!(to)])
}

/// Every attached file, base64, for an encrypted backup. None when they'd make it too large.
pub fn files_for_backup(conn: &Connection, dk: &Key, dir: &Path) -> Result<Option<Value>> {
    let total: i64 = conn.query_row("SELECT COALESCE(SUM(f.size), 0) FROM vault_document_files f JOIN vault_documents d ON d.id = f.document_id WHERE d.deleted_at IS NULL", [], |r| r.get(0))?;
    if total as usize > MAX_BACKUP_FILES { return Ok(None); }
    let mut out = serde_json::Map::new();
    let ids: Vec<String> = conn.prepare("SELECT f.id FROM vault_document_files f JOIN vault_documents d ON d.id = f.document_id WHERE d.deleted_at IS NULL")?
        .query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    for id in ids {
        if let Ok((_, _, bytes)) = read_file(conn, dk, dir, &id) { out.insert(id, json!(base64::engine::general_purpose::STANDARD.encode(bytes))); }
    }
    Ok(Some(Value::Object(out)))
}

/// After restoring a backup: writes the files it carried that aren't here yet. Each must match
/// the fingerprint in its restored row. Returns how many were written.
pub fn restore_files(conn: &Connection, dk: &Key, dir: &Path, files: &Value) -> Result<usize> {
    let Some(map) = files.as_object() else { return Ok(0) };
    let mut n = 0;
    for (id, data) in map {
        let Some(sha) = conn.query_row("SELECT sha256 FROM vault_document_files WHERE id = ?", [id], |r| r.get::<_, String>(0)).optional()? else { continue };
        if path_of(dir, id)?.exists() { continue; }
        let Some(bytes) = data.as_str().and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok()) else { continue };
        if bytes.len() > MAX_FILE || crypto::sha256_hex(&bytes) != sha { continue; }
        write_encrypted(dk, dir, id, &bytes)?;
        n += 1;
    }
    Ok(n)
}
