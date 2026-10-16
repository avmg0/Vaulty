use crate::error::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

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
];

/// Device-level family sharing store.
pub const HOUSEHOLD_MIGRATIONS: Migrations = &[
    ("0001_sharing", include_str!("../migrations/household/0001_sharing.sql")),
    ("0002_minor_units", include_str!("../migrations/household/0002_minor_units.sql")),
];

/// Device-level user registry.
pub const REGISTRY_MIGRATIONS: Migrations = &[
    ("0001_users", include_str!("../migrations/registry/0001_users.sql")),
];

/// Opens (creating if needed) a user's finance database.
pub fn open(path: &Path) -> Result<Connection> {
    open_with(path, DATA_MIGRATIONS)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn, DATA_MIGRATIONS)?;
    Ok(conn)
}

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
