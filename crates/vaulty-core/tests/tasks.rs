use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, calendar, db, history, people, recurring, reminders, tasks};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn to_dos_link_to_bills_and_tick_off_when_paid() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-22");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    recurring::create(&c, &json!({ "name": "Car insurance", "type": "expense", "amount": "14500", "account_id": bank["id"], "frequency": "yearly", "start_date": "2026-11-02" }), today).unwrap();
    let rid = recurring::list(&c, today).unwrap()[0]["id"].as_str().unwrap().to_string();
    let john = people::create(&c, &json!({ "name": "John Doe" })).unwrap();

    let compare = tasks::create(&c, &json!({ "title": "Compare insurance quotes", "due_date": "2026-10-28", "priority": "high", "linked_type": "recurring", "linked_id": rid }), today).unwrap();
    assert_eq!(compare["linked_name"], "Car insurance");
    let ask = tasks::create(&c, &json!({ "title": "Ask John about the concert money", "linked_type": "person", "linked_id": john["id"] }), today).unwrap();
    tasks::create(&c, &json!({ "title": "File ITR", "due_date": "2026-10-20" }), today).unwrap();
    assert!(tasks::create(&c, &json!({ "title": "" }), today).is_err());
    assert!(tasks::create(&c, &json!({ "title": "x", "priority": "urgent" }), today).is_err());
    assert!(tasks::create(&c, &json!({ "title": "x", "linked_type": "recurring", "linked_id": "nope" }), today).is_err(), "must exist");
    assert!(tasks::create(&c, &json!({ "title": "x", "linked_type": "account", "linked_id": bank["id"] }), today).is_err());

    // Overdue first, then by date; undated last.
    let open = tasks::list(&c, "open", today).unwrap();
    assert_eq!(open.iter().map(|t| t["title"].as_str().unwrap()).collect::<Vec<_>>(), ["File ITR", "Compare insurance quotes", "Ask John about the concert money"]);
    assert_eq!(open[0]["overdue"], true);

    // On the calendar, and reminded about when due or overdue.
    let items = calendar::range(&c, "2026-10-01", "2026-10-31", "IN", today).unwrap();
    assert!(items.iter().any(|i| i.kind == "task" && i.title == "Compare insurance quotes" && i.date == "2026-10-28"));
    let due = reminders::take_due(&c, today).unwrap();
    assert!(due.iter().any(|r| r.kind == "task" && r.name == "File ITR" && r.days == -2));
    assert!(!due.iter().any(|r| r.kind == "task" && r.name == "Compare insurance quotes"), "not due yet");

    // Paying the bill ticks off its to-do, in the same undoable step.
    history::record(&c, "recurring.pay", |c| recurring::mark_paid(c, &rid, None, None, today)).unwrap();
    assert_eq!(tasks::get(&c, compare["id"].as_str().unwrap(), today).unwrap()["status"], "done");
    history::undo_last(&c).unwrap();
    assert_eq!(tasks::get(&c, compare["id"].as_str().unwrap(), today).unwrap()["status"], "pending");

    // Done and back; when the linked item goes, the to-do keeps its name and says so.
    let id = ask["id"].as_str().unwrap();
    assert_eq!(tasks::set_done(&c, id, true, today).unwrap()["status"], "done");
    assert_eq!(tasks::list(&c, "done", today).unwrap().len(), 1);
    tasks::set_done(&c, id, false, today).unwrap();
    people::delete(&c, john["id"].as_str().unwrap()).unwrap();
    let t = tasks::get(&c, id, today).unwrap();
    assert_eq!((t["linked_name"].as_str(), t["link_missing"].as_bool()), (Some("John Doe"), Some(true)));
    tasks::delete(&c, id).unwrap();
    assert!(tasks::get(&c, id, today).is_err());
}
