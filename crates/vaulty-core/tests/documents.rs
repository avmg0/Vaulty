use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{calendar, db, documents, estate, export, history, reminders, Key};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }
fn key() -> Key { Key::new([7u8; 32]) }
const PDF: &[u8] = b"%PDF-1.7\n1 0 obj << /Type /Catalog >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

#[test]
fn files_are_encrypted_and_checked() {
    let c = db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let today = d("2026-10-23");
    let doc = documents::create(&c, &json!({ "title": "Jane's passport", "kind": "passport", "holder": "Jane Doe", "number": "Z1234567",
        "issued_on": "2017-03-02", "expires_on": "2027-03-01", "kept_at": "Steel almirah, top shelf" }), today).unwrap();
    let id = doc["id"].as_str().unwrap();
    assert!(documents::create(&c, &json!({ "title": " " }), today).is_err());
    assert!(documents::create(&c, &json!({ "title": "x", "kind": "spaceship" }), today).is_err());
    assert!(documents::create(&c, &json!({ "title": "x", "issued_on": "2026-01-02", "expires_on": "2025-01-01" }), today).is_err());

    let f = documents::add_file(&c, &key(), dir.path(), id, "/Users/jane/Scans/passport.pdf", PDF).unwrap();
    assert_eq!((f["name"].as_str(), f["mime"].as_str()), (Some("passport.pdf"), Some("application/pdf")));
    documents::add_file(&c, &key(), dir.path(), id, "back.png", PNG).unwrap();
    assert!(documents::add_file(&c, &key(), dir.path(), id, "notes.txt", b"hello").is_err(), "only PDFs and photos");

    // On disk the file is encrypted, and only this member's key opens it.
    let fid = f["id"].as_str().unwrap();
    let raw = std::fs::read(dir.path().join(format!("{fid}.vdoc"))).unwrap();
    assert!(!raw.windows(5).any(|w| w == b"%PDF-"));
    assert_eq!(documents::read_file(&c, &key(), dir.path(), fid).unwrap().2, PDF);
    assert!(documents::read_file(&c, &Key::new([1u8; 32]), dir.path(), fid).is_err());
    // Swapping two files on disk is noticed.
    let other = documents::get(&c, id, today).unwrap()["files"][1]["id"].as_str().unwrap().to_string();
    std::fs::copy(dir.path().join(format!("{other}.vdoc")), dir.path().join(format!("{fid}.vdoc"))).unwrap();
    assert!(documents::read_file(&c, &key(), dir.path(), fid).is_err());
    assert!(documents::read_file(&c, &key(), dir.path(), "../../etc/passwd").is_err());

    let got = documents::get(&c, id, today).unwrap();
    assert_eq!((got["status"].as_str(), got["files"].as_array().unwrap().len()), (Some("valid"), 2));
}

#[test]
fn trash_undo_and_purge() {
    let c = db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let today = d("2026-10-23");
    let doc = documents::create(&c, &json!({ "title": "Car insurance policy", "kind": "insurance" }), today).unwrap();
    let id = doc["id"].as_str().unwrap().to_string();
    let f = documents::add_file(&c, &key(), dir.path(), &id, "policy.pdf", PDF).unwrap();
    history::record(&c, "document.delete", |c| documents::delete(c, &id)).unwrap();
    assert!(documents::list(&c, today).unwrap().is_empty());
    history::undo_last(&c).unwrap();
    assert_eq!(documents::list(&c, today).unwrap().len(), 1, "back from the trash");
    // A detached file stays on disk for undo; purge only removes old unreferenced files.
    history::record(&c, "document.file_remove", |c| documents::remove_file(c, f["id"].as_str().unwrap())).unwrap();
    assert_eq!(documents::purge(&c, dir.path()).unwrap(), 0, "too recent to purge");
    history::undo_last(&c).unwrap();
    assert!(documents::read_file(&c, &key(), dir.path(), f["id"].as_str().unwrap()).is_ok());
}

#[test]
fn expiry_on_the_calendar_and_reminded() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-23");
    documents::create(&c, &json!({ "title": "Jane's passport", "kind": "passport", "expires_on": "2026-11-10", "remind_days": 30 }), today).unwrap();
    documents::create(&c, &json!({ "title": "Driving licence", "kind": "licence", "expires_on": "2027-06-01" }), today).unwrap();
    documents::create(&c, &json!({ "title": "Old PUC certificate", "kind": "vehicle", "expires_on": "2026-10-20" }), today).unwrap();
    let items = calendar::range(&c, "2026-11-01", "2026-11-30", "IN", today).unwrap();
    assert!(items.iter().any(|i| i.kind == "expiry" && i.title == "Jane's passport" && i.date == "2026-11-10"));
    let due = reminders::take_due(&c, today).unwrap();
    let names: Vec<(&str, i64)> = due.iter().filter(|r| r.kind == "expiry").map(|r| (r.name.as_str(), r.days)).collect();
    assert_eq!(names, [("Old PUC certificate", -3), ("Jane's passport", 18)]);
    assert!(reminders::take_due(&c, today).unwrap().iter().all(|r| r.kind != "expiry"), "once");
    let list = documents::list(&c, today).unwrap();
    assert_eq!(list.iter().map(|d| d["status"].as_str().unwrap()).collect::<Vec<_>>(), ["expired", "expiring", "valid"]);
}

#[test]
fn backups_carry_the_files_and_the_nominee_sees_the_papers() {
    let c = db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let today = d("2026-10-23");
    let doc = documents::create(&c, &json!({ "title": "Will", "kind": "will", "kept_at": "With Advocate R. Doe, Pune" }), today).unwrap();
    documents::add_file(&c, &key(), dir.path(), doc["id"].as_str().unwrap(), "will.pdf", PDF).unwrap();
    let files = documents::files_for_backup(&c, &key(), dir.path()).unwrap().unwrap();
    let mut backup = export::to_json(&c).unwrap();
    backup["document_files"] = files.clone();

    // Restored into another profile with a different key and folder.
    let c2 = db::open_in_memory().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let k2 = Key::new([3u8; 32]);
    export::from_json(&c2, &backup).unwrap();
    assert_eq!(documents::restore_files(&c2, &k2, dir2.path(), &backup["document_files"]).unwrap(), 1);
    let fid = documents::list(&c2, today).unwrap()[0]["files"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(documents::read_file(&c2, &k2, dir2.path(), &fid).unwrap().2, PDF);
    assert_eq!(documents::restore_files(&c2, &k2, dir2.path(), &files).unwrap(), 0, "already there");

    let e = estate::summary(&c, today).unwrap();
    assert_eq!((e.documents[0].title.as_str(), e.documents[0].kept_at.as_deref(), e.documents[0].files), ("Will", Some("With Advocate R. Doe, Pune"), 1));
}
