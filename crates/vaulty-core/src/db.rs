use crate::crypto::{to_hex, Key};
use crate::error::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub type Migrations = &'static [(&'static str, &'static str)];

/// Per-user finance data. Append new entries; never edit shipped ones.
pub const DATA_MIGRATIONS: Migrations = &[
    ("0001_init", include_str!("../migrations/data/0001_init.sql")),
    ("0002_seed", include_str!("../migrations/data/0002_seed.sql")),
    ("0003_visibility", include_str!("../migrations/data/0003_visibility.sql")),
    ("0004_ledger", include_str!("../migrations/data/0004_ledger.sql")),
    ("0005_institutions", include_str!("../migrations/data/0005_institutions.sql")),
    ("0006_card_details", include_str!("../migrations/data/0006_card_details.sql")),
    ("0007_cards", include_str!("../migrations/data/0007_cards.sql")),
    ("0008_market", include_str!("../migrations/data/0008_market.sql")),
    ("0009_hidden", include_str!("../migrations/data/0009_hidden.sql")),
    ("0010_formats", include_str!("../migrations/data/0010_formats.sql")),
    ("0011_budgets", include_str!("../migrations/data/0011_budgets.sql")),
    ("0012_recurring", include_str!("../migrations/data/0012_recurring.sql")),
    ("0013_import_templates", include_str!("../migrations/data/0013_import_templates.sql")),
    ("0014_rules", include_str!("../migrations/data/0014_rules.sql")),
    ("0015_updates", include_str!("../migrations/data/0015_updates.sql")),
    ("0016_auto_lock", include_str!("../migrations/data/0016_auto_lock.sql")),
    ("0017_reminders", include_str!("../migrations/data/0017_reminders.sql")),
    ("0018_people", include_str!("../migrations/data/0018_people.sql")),
    ("0019_subscriptions", include_str!("../migrations/data/0019_subscriptions.sql")),
    ("0020_low_balance", include_str!("../migrations/data/0020_low_balance.sql")),
    ("0021_documents", include_str!("../migrations/data/0021_documents.sql")),
    ("0022_card_statements", include_str!("../migrations/data/0022_card_statements.sql")),
    ("0023_holdings", include_str!("../migrations/data/0023_holdings.sql")),
    ("0024_projects", include_str!("../migrations/data/0024_projects.sql")),
    ("0025_project_labels", include_str!("../migrations/data/0025_project_labels.sql")),
    ("0026_project_files", include_str!("../migrations/data/0026_project_files.sql")),
    ("0027_project_sharing", include_str!("../migrations/data/0027_project_sharing.sql")),
    ("0028_project_settle", include_str!("../migrations/data/0028_project_settle.sql")),
    ("0029_project_warranty", include_str!("../migrations/data/0029_project_warranty.sql")),
    ("0030_fund_prices", include_str!("../migrations/data/0030_fund_prices.sql")),
    ("0031_tax_plan", include_str!("../migrations/data/0031_tax_plan.sql")),
    ("0032_goals", include_str!("../migrations/data/0032_goals.sql")),
    ("0033_insurance", include_str!("../migrations/data/0033_insurance.sql")),
    ("0034_nominees", include_str!("../migrations/data/0034_nominees.sql")),
];

/// Device-level family sharing store.
pub const HOUSEHOLD_MIGRATIONS: Migrations = &[
    ("0001_sharing", include_str!("../migrations/household/0001_sharing.sql")),
    ("0002_minor_units", include_str!("../migrations/household/0002_minor_units.sql")),
];

/// Device-level user registry.
pub const REGISTRY_MIGRATIONS: Migrations = &[
    ("0001_users", include_str!("../migrations/registry/0001_users.sql")),
    ("0002_keys", include_str!("../migrations/registry/0002_keys.sql")),
    ("0003_senior", include_str!("../migrations/registry/0003_senior.sql")),
    ("0004_sign_in_failures", include_str!("../migrations/registry/0004_sign_in_failures.sql")),
    ("0005_locked_reminders", include_str!("../migrations/registry/0005_locked_reminders.sql")),
    ("0006_helpers", include_str!("../migrations/registry/0006_helpers.sql")),
    ("0007_nominee", include_str!("../migrations/registry/0007_nominee.sql")),
    ("0008_app_settings", include_str!("../migrations/registry/0008_app_settings.sql")),
];

/// Opens (creating if needed) a member's encrypted finance database.
pub fn open(path: &Path, key: &Key) -> Result<Connection> {
    open_with_snapshots(path, key, None)
}

/// As `open`, taking a `before-update` snapshot into `snapshots` when migrations are pending.
pub fn open_with_snapshots(path: &Path, key: &Key, snapshots: Option<&Path>) -> Result<Connection> {
    let conn = open_encrypted(path, key, &[])?;
    if let Some(dir) = snapshots {
        if pending(&conn, DATA_MIGRATIONS)? > 0 && pending(&conn, DATA_MIGRATIONS)? < DATA_MIGRATIONS.len() {
            crate::snapshots::take(&conn, key, dir, "before-update")?;
        }
    }
    with_history(&conn)?;
    Ok(conn)
}

/// Number of migrations not yet applied.
pub fn pending(conn: &Connection, migrations: Migrations) -> Result<usize> {
    let has: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = '_migrations')", [], |r| r.get(0))?;
    if !has { return Ok(migrations.len()); }
    let mut n = 0;
    for (name, _) in migrations {
        let done: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?)", [name], |r| r.get(0))?;
        if !done { n += 1; }
    }
    Ok(n)
}

/// Data migrations wrapped by the change-history triggers: drop them, migrate, reinstall from
/// the new columns (a migration that alters a tracked table would otherwise trip over them).
fn with_history(conn: &Connection) -> Result<()> {
    let has_log: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'change_context')", [], |r| r.get(0))?;
    if has_log { crate::history::drop_triggers(conn)?; }
    let updating = has_log && pending(conn, DATA_MIGRATIONS)? > 0;
    migrate(conn, DATA_MIGRATIONS)?;
    crate::history::install(conn)?;
    if updating { crate::history::record_app_update(conn)?; }
    Ok(())
}

/// Opens an SQLCipher database (AES-256), encrypting a legacy plain file first if needed.
/// The key is a random 256-bit value (never a password), passed raw so SQLCipher skips its own KDF.
pub fn open_encrypted(path: &Path, key: &Key, migrations: Migrations) -> Result<Connection> {
    if is_plain_sqlite(path) {
        encrypt_plain_file(path, key)?;
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)?;
    apply_key(&conn, key)?;
    // Sorts and temporary tables stay in memory: SQLite's temp files are not encrypted.
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    configure(&conn)?;
    migrate(&conn, migrations)?;
    Ok(conn)
}

fn key_pragma(key: &Key) -> zeroize::Zeroizing<String> {
    zeroize::Zeroizing::new(format!("x'{}'", to_hex(key.as_ref())))
}

/// Must run before anything else touches the file. Wrong keys fail here, not later.
fn apply_key(conn: &Connection, key: &Key) -> Result<()> {
    conn.pragma_update(None, "key", key_pragma(key).as_str())?;
    // Wipe SQLCipher's own copies of keys and pages from memory when they are freed.
    conn.pragma_update(None, "cipher_memory_security", "ON")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(|_| Error::Auth("this data can't be opened with the key provided".into()))?;
    Ok(())
}

/// True when `path` is an unencrypted SQLite file (its first 16 bytes are the SQLite header).
pub fn is_plain_sqlite(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"SQLite format 3\0"
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.display()))
}

/// Encrypts a legacy plain database in place: export to an encrypted copy, verify it, then swap.
/// The plain file is overwritten with zeros before removal (best effort; SSDs may keep old blocks).
pub fn encrypt_plain_file(path: &Path, key: &Key) -> Result<()> {
    let tmp = sibling(path, ".encrypting");
    let _ = std::fs::remove_file(&tmp);
    {
        let plain = Connection::open(path)?;
        plain.pragma_update(None, "journal_mode", "DELETE")?; // fold any WAL into the main file
        let tmp_s = tmp.to_str().ok_or_else(|| Error::Invalid("data folder path isn't valid UTF-8".into()))?;
        plain.execute("ATTACH DATABASE ?1 AS enc KEY ?2", rusqlite::params![tmp_s, key_pragma(key).as_str()])?;
        plain.query_row("SELECT sqlcipher_export('enc')", [], |_| Ok(()))?;
        let v: i64 = plain.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        plain.execute_batch(&format!("PRAGMA enc.user_version = {v}; DETACH DATABASE enc;"))?;
    }
    // Prove the copy opens with the key before touching the original.
    {
        let c = Connection::open(&tmp)?;
        apply_key(&c, key)?;
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)).ok().filter(|s| s == "ok")
            .ok_or_else(|| Error::Invalid("encrypted copy failed its integrity check; original left untouched".into()))?;
    }
    shred(path);
    for s in ["-wal", "-shm"] { shred(&sibling(path, s)); }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Overwrites a file with zeros, then deletes it.
pub fn shred(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(path) {
            use std::io::Write;
            let zeros = vec![0u8; 64 * 1024];
            let mut left = meta.len();
            while left > 0 {
                let n = left.min(zeros.len() as u64) as usize;
                if f.write_all(&zeros[..n]).is_err() { break; }
                left -= n as u64;
            }
            let _ = f.sync_all();
        }
        let _ = std::fs::remove_file(path);
    }
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    with_history(&conn)?;
    Ok(conn)
}

/// Unencrypted database: only the registry (names and sign-in data, no finances).
pub fn open_with(path: &Path, migrations: Migrations) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    configure(&conn)?;
    migrate(&conn, migrations)?;
    Ok(conn)
}

pub fn memory_with(migrations: Migrations) -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn, migrations)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

/// Applies any migrations not yet recorded in `_migrations`, each in its own transaction.
pub fn migrate(conn: &Connection, migrations: Migrations) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY, applied_at TEXT DEFAULT CURRENT_TIMESTAMP)",
    )?;
    for (name, sql) in migrations {
        let done: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?)",
            [name],
            |r| r.get(0),
        )?;
        if done {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute("INSERT INTO _migrations (name) VALUES (?)", [name])?;
        tx.commit()?;
    }
    Ok(())
}

pub fn file_size_label(path: &Path) -> String {
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}
