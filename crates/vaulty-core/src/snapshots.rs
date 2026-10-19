//! Automatic local snapshots of a member's database, for recovering from bad imports, mistakes
//! beyond undo's reach, or a damaged file.
//!
//! Snapshots are full copies made with SQLCipher's export, keyed with the same data key, so they
//! are as encrypted as the original. They live next to the data (`snapshots/<member>/`) and
//! protect against mistakes, not against losing the device: that's what exported backups are for.
//!
//! Kept: the newest snapshot of each of the last 7 days, the newest of each of the 4 weeks before
//! that, and the 5 most recent taken before risky operations (import, update, restore).

use crate::crypto::Key;
use crate::error::{Error, Result};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const KEEP_DAYS: usize = 7;
const KEEP_WEEKS: usize = 4;
const KEEP_BEFORE: usize = 5;
pub const DAILY_INTERVAL_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub name: String,
    /// RFC 3339, UTC.
    pub at: String,
    /// `daily`, `manual`, or `before-import` / `before-update` / `before-restore`.
    pub reason: String,
    pub size: u64,
}

fn valid_reason(r: &str) -> bool {
    matches!(r, "daily" | "manual" | "before-import" | "before-update" | "before-restore")
}

/// File names look like `20261016T143005Z-daily.db` (no characters Windows forbids).
fn parse_name(name: &str) -> Option<(chrono::DateTime<chrono::Utc>, String)> {
    let stem = name.strip_suffix(".db")?;
    let (ts, reason) = stem.split_once('-')?;
    let at = chrono::NaiveDateTime::parse_from_str(ts, "%Y%m%dT%H%M%SZ").ok()?.and_utc();
    valid_reason(reason).then(|| (at, reason.to_string()))
}

/// Copies `conn`'s database into a new encrypted snapshot.
pub fn take(conn: &Connection, key: &Key, dir: &Path, reason: &str) -> Result<Snapshot> {
    if !valid_reason(reason) { return Err(Error::Invalid(format!("unknown snapshot reason `{reason}`"))); }
    std::fs::create_dir_all(dir)?;
    // Names sort by time, and pruning keeps the newest. If several are taken within a second, the
    // clock is bumped past the newest existing one, so a new snapshot never sorts as older than an
    // earlier one (it would then be pruned straight away).
    let mut now = chrono::Utc::now();
    if let Some(latest) = list(dir)?.iter().filter_map(|s| parse_name(&s.name).map(|(at, _)| at)).max() {
        if latest >= now { now = latest + chrono::Duration::seconds(1); }
    }
    let mut path;
    loop {
        path = dir.join(format!("{}-{reason}.db", now.format("%Y%m%dT%H%M%SZ")));
        if !path.exists() { break; }
        now += chrono::Duration::seconds(1);
    }
    let tmp = path.with_extension("partial");
    {
        let tmp_s = tmp.to_str().ok_or_else(|| Error::Invalid("data folder path isn't valid UTF-8".into()))?;
        let key_sql = zeroize::Zeroizing::new(format!("x'{}'", crate::crypto::to_hex(key.as_ref())));
        conn.execute("ATTACH DATABASE ?1 AS snap KEY ?2", rusqlite::params![tmp_s, key_sql.as_str()])?;
        let copied = conn.query_row("SELECT sqlcipher_export('snap')", [], |_| Ok(()))
            .and_then(|_| conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)))
            .and_then(|v| conn.execute_batch(&format!("PRAGMA snap.user_version = {v}")));
        conn.execute_batch("DETACH DATABASE snap")?;
        copied?;
    }
    verify(&tmp, key)?;
    std::fs::rename(&tmp, &path)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let size = std::fs::metadata(&path)?.len();
    prune(dir, &name)?;
    Ok(Snapshot { at: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true), reason: reason.into(), size, name })
}

/// Opens a snapshot with the key and runs SQLite's integrity check.
pub fn verify(path: &Path, key: &Key) -> Result<()> {
    if crate::db::is_plain_sqlite(path) { return Err(Error::Invalid("snapshot is not encrypted".into())); }
    let c = Connection::open(path)?;
    c.pragma_update(None, "key", format!("x'{}'", crate::crypto::to_hex(key.as_ref())))?;
    let ok: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|_| Error::Invalid("snapshot can't be opened with this profile's key".into()))?;
    if ok != "ok" { return Err(Error::Invalid("snapshot is damaged".into())); }
    Ok(())
}

/// Snapshots in `dir`, newest first.
pub fn list(dir: &Path) -> Result<Vec<Snapshot>> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Ok(vec![]) };
    let mut out: Vec<Snapshot> = rd.filter_map(|e| e.ok()).filter_map(|e| {
        let name = e.file_name().to_str()?.to_string();
        let (at, reason) = parse_name(&name)?;
        Some(Snapshot { name, at: at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true), reason, size: e.metadata().ok()?.len() })
    }).collect();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Seconds since the newest daily or manual snapshot (`None` if there isn't one).
pub fn age_of_latest(dir: &Path) -> Result<Option<i64>> {
    Ok(list(dir)?.into_iter().find(|s| s.reason == "daily" || s.reason == "manual").and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(&s.at).ok().map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds())
    }))
}

/// Keeps a bounded set of snapshots, always including `just_taken`.
fn prune(dir: &Path, just_taken: &str) -> Result<()> {
    let all = list(dir)?;
    let mut keep: HashSet<String> = HashSet::from([just_taken.to_string()]);
    let (mut days, mut weeks, mut before) = (vec![], vec![], 0);
    for s in &all {
        let Some((at, _)) = parse_name(&s.name) else { continue };
        if s.reason.starts_with("before-") {
            if before < KEEP_BEFORE { keep.insert(s.name.clone()); before += 1; }
            continue;
        }
        let day = at.date_naive();
        let week = at.format("%G-W%V").to_string();
        if days.len() < KEEP_DAYS && !days.contains(&day) {
            days.push(day);
            keep.insert(s.name.clone());
        } else if !days.contains(&day) && weeks.len() < KEEP_WEEKS && !weeks.contains(&week) {
            weeks.push(week);
            keep.insert(s.name.clone());
        }
    }
    for s in all.iter().filter(|s| !keep.contains(&s.name)) {
        crate::db::shred(&dir.join(&s.name));
    }
    Ok(())
}

/// Path of a snapshot by name, refusing anything that isn't a snapshot file in `dir`.
pub fn path_of(dir: &Path, name: &str) -> Result<PathBuf> {
    if name.contains(['/', '\\']) || parse_name(name).is_none() {
        return Err(Error::Invalid("no such snapshot".into()));
    }
    let p = dir.join(name);
    if !p.is_file() { return Err(Error::Invalid("no such snapshot".into())); }
    Ok(p)
}

/// Replaces the data file at `target` with a snapshot. The caller must have closed every
/// connection to `target` and should have taken a `before-restore` snapshot first.
pub fn restore(dir: &Path, name: &str, target: &Path, key: &Key) -> Result<()> {
    let src = path_of(dir, name)?;
    verify(&src, key)?;
    let tmp = PathBuf::from(format!("{}.restoring", target.display()));
    std::fs::copy(&src, &tmp)?;
    verify(&tmp, key)?;
    for s in ["-wal", "-shm"] { crate::db::shred(Path::new(&format!("{}{s}", target.display()))); }
    std::fs::rename(&tmp, target)?;
    Ok(())
}

/// Deletes every snapshot for a member (when their profile is erased or removed).
pub fn delete_all(dir: &Path) {
    if let Ok(list) = list(dir) {
        for s in list { crate::db::shred(&dir.join(s.name)); }
    }
    let _ = std::fs::remove_dir(dir);
}
