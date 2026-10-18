//! Password-protected backup files (`.vaulty`).
//!
//! The file is JSON, so it can be read without Vaulty (see docs/BACKUP_FORMAT.md):
//!
//! ```json
//! { "format": "vaulty-backup", "version": 1, "created_at": "2026-10-19T08:00:00Z", "hint": "…",
//!   "kdf": { "name": "argon2id", "version": 19, "memory_kib": 65536, "iterations": 3, "parallelism": 1, "salt": "<hex>" },
//!   "cipher": "xchacha20-poly1305", "nonce": "<hex>", "data": "<hex>" }
//! ```
//!
//! `data` is the normal JSON backup (`export::to_json`), encrypted with a key derived from the
//! backup password. Every header field except `data` is bound as associated data, so changing
//! the settings, hint or date makes the file fail to open.

use crate::{crypto, error::{Error, Result}, export};
use rusqlite::Connection;
use serde_json::{json, Value};

pub const FORMAT: &str = "vaulty-backup";
pub const VERSION: i64 = 1;
pub const MIN_PASSWORD: usize = 10;

/// The header fields in a fixed order, as authenticated with the ciphertext.
fn aad(h: &Value) -> Vec<u8> {
    let k = &h["kdf"];
    format!("{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        h["format"].as_str().unwrap_or_default(), h["version"], h["created_at"].as_str().unwrap_or_default(), h["hint"].as_str().unwrap_or_default(),
        k["name"].as_str().unwrap_or_default(), k["version"], k["memory_kib"], k["iterations"], k["parallelism"], k["salt"].as_str().unwrap_or_default(),
        h["cipher"].as_str().unwrap_or_default(), h["nonce"].as_str().unwrap_or_default()).into_bytes()
}

/// An encrypted backup of everything in the profile.
pub fn encrypt(conn: &Connection, password: &str, hint: Option<&str>) -> Result<Vec<u8>> {
    if password.chars().count() < MIN_PASSWORD { return Err(Error::Invalid(format!("use a backup password of at least {MIN_PASSWORD} characters"))); }
    let hint = hint.map(str::trim).filter(|h| !h.is_empty());
    if hint.is_some_and(|h| h.chars().count() > 80) { return Err(Error::Invalid("keep the hint under 80 characters".into())); }
    if hint.is_some_and(|h| h.contains(password)) { return Err(Error::Invalid("the hint can't contain the password".into())); }
    let plain = zeroize::Zeroizing::new(serde_json::to_vec(&export::to_json(conn)?)?);
    let salt = crypto::new_salt();
    let key = crypto::derive_key(password, &salt)?;
    let nonce = crypto::random_bytes::<24>();
    let mut header = json!({
        "format": FORMAT, "version": VERSION,
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true), "hint": hint,
        "kdf": { "name": "argon2id", "version": 19, "memory_kib": 65536, "iterations": 3, "parallelism": 1, "salt": salt },
        "cipher": "xchacha20-poly1305", "nonce": crypto::to_hex(&nonce),
    });
    // Encrypt with the nonce already in the header so it's covered by the authentication too.
    let ct = {
        use chacha20poly1305::{aead::{Aead, Payload}, KeyInit, XChaCha20Poly1305, XNonce};
        XChaCha20Poly1305::new(key.as_ref().into())
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &plain, aad: &aad(&header) })
            .map_err(|_| Error::Auth("encryption failed".into()))?
    };
    header["data"] = json!(crypto::to_hex(&ct));
    Ok(serde_json::to_vec_pretty(&header)?)
}

/// The header of an encrypted backup, or None if `bytes` is something else (e.g. a plain JSON backup).
pub fn header(bytes: &[u8]) -> Option<Value> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    (v["format"] == FORMAT).then(|| { let mut h = v; if let Some(o) = h.as_object_mut() { o.remove("data"); } h })
}

/// Decrypts a backup file to the plain backup JSON.
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Value> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| Error::Invalid("that isn't a Vaulty backup file".into()))?;
    if v["format"] != FORMAT { return Err(Error::Invalid("that isn't an encrypted Vaulty backup".into())); }
    if v["version"].as_i64() != Some(VERSION) { return Err(Error::Invalid("this backup was made by a newer Vaulty; update first".into())); }
    let k = &v["kdf"];
    if k["name"] != "argon2id" || k["memory_kib"] != 65536 || k["iterations"] != 3 || k["parallelism"] != 1 || v["cipher"] != "xchacha20-poly1305" {
        return Err(Error::Invalid("unsupported backup settings".into()));
    }
    let key = crypto::derive_key(password, k["salt"].as_str().unwrap_or_default())?;
    let nonce = crypto::from_hex(v["nonce"].as_str().unwrap_or_default())?;
    let ct = crypto::from_hex(v["data"].as_str().unwrap_or_default())?;
    let plain = zeroize::Zeroizing::new(crypto::open_aad(&key, &nonce, &ct, &aad(&v))?);
    serde_json::from_slice(&plain).map_err(|_| Error::Invalid("the backup's contents are damaged".into()))
}
