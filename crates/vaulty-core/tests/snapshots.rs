use serde_json::json;
use vaulty_core::{accounts, db, history, snapshots, Key};

fn key() -> Key { Key::new([9u8; 32]) }

#[test]
fn snapshot_is_encrypted_and_restores() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("m.db");
    let snaps = dir.path().join("snapshots");
    let c = db::open(&data, &key()).unwrap();
    accounts::create(&c, &json!({ "name": "Before", "type": "cash", "currency": "INR" })).unwrap();
    let s = snapshots::take(&c, &key(), &snaps, "manual").unwrap();
    assert!(!db::is_plain_sqlite(&snaps.join(&s.name)));
    assert!(snapshots::verify(&snaps.join(&s.name), &Key::new([1u8; 32])).is_err(), "other keys can't open it");

    // A mistake undo can't fix: everything replaced by a bad import.
    history::record(&c, "import", |c| { c.execute("DELETE FROM accounts", [])?; Ok(()) }).ok();
    accounts::create(&c, &json!({ "name": "After", "type": "cash", "currency": "INR" })).unwrap();
    drop(c);

    snapshots::restore(&snaps, &s.name, &data, &key()).unwrap();
    let c = db::open(&data, &key()).unwrap();
    let names: Vec<String> = accounts::get_all(&c).unwrap().iter().map(|a| a["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["Before"]);
}

#[test]
fn rejects_bad_names_and_lists_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let snaps = dir.path().join("snapshots");
    let c = db::open_in_memory().unwrap();
    let a = snapshots::take(&c, &key(), &snaps, "daily").unwrap();
    let b = snapshots::take(&c, &key(), &snaps, "before-import").unwrap();
    let list = snapshots::list(&snaps).unwrap();
    assert_eq!(list.len(), 2);
    assert!(list[0].name >= list[1].name);
    assert!(list.iter().any(|s| s.name == a.name) && list.iter().any(|s| s.name == b.name));
    assert!(snapshots::path_of(&snaps, "../m.db").is_err());
    assert!(snapshots::path_of(&snaps, "20261016T000000Z-daily.db").is_err(), "must exist");
    assert!(snapshots::take(&c, &key(), &snaps, "../evil").is_err());
    assert!(snapshots::age_of_latest(&snaps).unwrap().unwrap() < 60);
}

#[test]
fn pruning_keeps_a_bounded_set() {
    let dir = tempfile::tempdir().unwrap();
    let snaps = dir.path().join("snapshots");
    let c = db::open_in_memory().unwrap();
    // Fake 40 days of old daily snapshots by copying one under older names.
    let s = snapshots::take(&c, &key(), &snaps, "daily").unwrap();
    for d in 1..=40 {
        let at = chrono::Utc::now() - chrono::Duration::days(d);
        std::fs::copy(snaps.join(&s.name), snaps.join(format!("{}-daily.db", at.format("%Y%m%dT%H%M%SZ")))).unwrap();
    }
    for _ in 0..8 { snapshots::take(&c, &key(), &snaps, "before-import").unwrap(); }
    snapshots::take(&c, &key(), &snaps, "manual").unwrap(); // triggers pruning
    let list = snapshots::list(&snaps).unwrap();
    let befores = list.iter().filter(|s| s.reason.starts_with("before-")).count();
    let regular = list.len() - befores;
    assert_eq!(befores, 5);
    assert!(regular <= 7 + 4, "kept {regular}");
    assert!(regular >= 7);
}

#[test]
fn snapshot_before_pending_migrations() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("m.db");
    let snaps = dir.path().join("snapshots");
    {
        let c = db::open(&data, &key()).unwrap();
        c.execute("DELETE FROM _migrations WHERE name = '0008_market'", []).unwrap();
        c.execute_batch("DROP TABLE market_prefs; DROP TABLE market_quotes;").unwrap();
    }
    db::open_with_snapshots(&data, &key(), Some(&snaps)).unwrap();
    let list = snapshots::list(&snaps).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, "before-update");
}

/// On fast machines several snapshots land in the same second and get names a few seconds ahead.
/// A new one must still sort as the newest, or pruning deletes it the moment it's made.
#[test]
fn a_new_snapshot_is_never_pruned_straight_away() {
    let dir = tempfile::tempdir().unwrap();
    let snaps = dir.path().join("snapshots");
    let c = db::open_in_memory().unwrap();
    let first = snapshots::take(&c, &key(), &snaps, "before-import").unwrap();
    // Five "before" snapshots already named a few seconds ahead of the clock.
    for s in 1..=5 {
        let at = chrono::Utc::now() + chrono::Duration::seconds(s);
        std::fs::copy(snaps.join(&first.name), snaps.join(format!("{}-before-import.db", at.format("%Y%m%dT%H%M%SZ")))).unwrap();
    }
    // ...and the current second's name free again, as after an earlier prune.
    std::fs::remove_file(snaps.join(&first.name)).unwrap();
    let new = snapshots::take(&c, &key(), &snaps, "before-import").unwrap();
    let list = snapshots::list(&snaps).unwrap();
    assert!(list.iter().any(|s| s.name == new.name), "the snapshot just taken was pruned");
    assert_eq!(list.first().map(|s| s.name.as_str()), Some(new.name.as_str()), "and it's the newest");
}
