use chrono::NaiveDate;
use proptest::prelude::*;
use serde_json::json;
use vaulty_core::{accounts, calendar, cards, db, recurring};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

#[test]
fn bank_days_in_india_and_elsewhere() {
    // October 2026: Sat 10th is the 2nd Saturday, Sat 3rd the 1st.
    assert!(calendar::bank_closed(d("2026-10-10"), "IN", &[]));
    assert!(!calendar::bank_closed(d("2026-10-03"), "IN", &[]));
    assert!(calendar::bank_closed(d("2026-10-03"), "US", &[]));
    assert!(calendar::bank_closed(d("2026-10-04"), "IN", &[]), "Sunday");
    assert!(calendar::bank_closed(d("2026-10-02"), "IN", &[d("2026-10-02")]), "holiday");
    // Due Sun 11 Oct: Sat 10 is the 2nd Saturday, so pay by Fri 9.
    assert_eq!(calendar::last_working_day_before(d("2026-10-11"), "IN", &[]), d("2026-10-09"));
}

#[test]
fn bills_card_dues_events_and_holidays_in_one_view() {
    let c = db::open_in_memory().unwrap();
    let today = d("2026-10-19");
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    recurring::create(&c, &json!({ "name": "House rent", "type": "expense", "amount": "22000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-10-05" }), today).unwrap();
    recurring::create(&c, &json!({ "name": "Salary", "type": "income", "amount": "85000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-11-01" }), today).unwrap();
    cards::create(&c, &json!({ "kind": "credit", "name": "Rewards", "currency": "INR", "owed": "12400", "statement_day": 2, "due_day": 22 })).unwrap();
    calendar::create_event(&c, &json!({ "title": "Mum's birthday", "date": "1970-11-14", "repeat": "yearly", "icon": "🎂" })).unwrap();
    calendar::create_event(&c, &json!({ "title": "Diwali", "date": "2026-11-08", "type": "holiday" })).unwrap();

    let items = calendar::range(&c, "2026-10-01", "2026-11-30", "IN", today).unwrap();
    let find = |kind: &str, date: &str| items.iter().find(|i| i.kind == kind && i.date == date).unwrap_or_else(|| panic!("no {kind} on {date}: {items:#?}"));

    assert_eq!(find("holiday", "2026-10-02").title, "Gandhi Jayanti");
    assert_eq!(find("holiday", "2026-11-08").title, "Diwali");
    assert_eq!(find("event", "2026-11-14").title, "Mum's birthday", "yearly events repeat");
    assert_eq!(find("bill", "2026-11-05").amount, Some(22000.0));
    assert_eq!(find("income", "2026-11-01").title, "Salary");
    // The next card due date carries what's owed now; later ones don't guess.
    assert_eq!(find("card_due", "2026-10-22").amount, Some(12400.0));
    assert_eq!(find("card_due", "2026-11-22").amount, None);
    assert!(items.iter().any(|i| i.kind == "statement" && i.date == "2026-11-02"));
    // Rent due Sun 8 Nov (also Diwali): pay by Fri 6 Nov (Sat 7th is a 1st Saturday, open).
    assert_eq!(find("bill", "2026-11-05").pay_by, None);
    // Card due Sun 22 Nov: Sat 21 is not 2nd/4th... it's the 3rd Saturday, so open.
    assert_eq!(find("card_due", "2026-11-22").pay_by.as_deref(), Some("2026-11-21"));

    // Rent for October (5th) wasn't paid: overdue.
    assert!(find("bill", "2026-10-05").overdue);
    let rent = recurring::list(&c, today).unwrap().into_iter().find(|r| r["name"] == "House rent").unwrap();
    recurring::mark_paid(&c, rent["id"].as_str().unwrap(), None, None, today).unwrap();
    let items = calendar::range(&c, "2026-10-01", "2026-10-31", "IN", today).unwrap();
    assert!(!items.iter().any(|i| i.kind == "bill" && i.date == "2026-10-05"), "paid, so no longer due");
    assert!(items.iter().any(|i| i.kind == "paid" && i.title == "House rent"));

    assert!(calendar::range(&c, "2026-01-01", "2027-06-01", "IN", today).is_err(), "at most about a year");
    assert!(calendar::create_event(&c, &json!({ "title": "", "date": "2026-10-20" })).is_err());
    assert!(calendar::create_event(&c, &json!({ "title": "x", "date": "2026-10-20", "time": "25:00" })).is_err());
    assert!(calendar::create_event(&c, &json!({ "title": "x", "date": "2026-10-20", "repeat": "hourly" })).is_err());
}

#[test]
fn ics_round_trip_and_import() {
    let c = db::open_in_memory().unwrap();
    calendar::create_event(&c, &json!({ "title": "Meeting, with \"notes\"; a long name to test how lines fold in files", "date": "2026-10-20", "time": "14:30" })).unwrap();
    calendar::create_event(&c, &json!({ "title": "Anniversary", "date": "2010-02-14", "repeat": "yearly" })).unwrap();
    let items = calendar::range(&c, "2026-01-01", "2026-12-31", "IN", d("2026-10-19")).unwrap();
    let ics = calendar::to_ics(&items, false);
    assert!(ics.lines().all(|l| l.trim_end_matches('\r').len() <= 75), "folded");
    assert!(ics.contains("DTSTART:20261020T143000"));

    let back = calendar::parse_ics(ics.as_bytes()).unwrap();
    let meeting = back.iter().find(|e| e.date == "2026-10-20").unwrap();
    assert!(meeting.title.starts_with("Meeting, with \"notes\"; a long name"), "{}", meeting.title);
    assert_eq!(meeting.time.as_deref(), Some("14:30"));

    // A holiday list as a bank or government would publish it.
    let holidays = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:holi-2027@example\r\nDTSTART;VALUE=DATE:20270322\r\nSUMMARY:Holi\r\nEND:VEVENT\r\n\
                    BEGIN:VEVENT\r\nUID:ny@example\r\nDTSTART;VALUE=DATE:20270101\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:New Year\\, again\r\nEND:VEVENT\r\n\
                    BEGIN:VEVENT\r\nDTSTART:20270105T090000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let parsed = calendar::parse_ics(holidays.as_bytes()).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[1].title, "New Year, again");
    assert_eq!(parsed[1].repeat.as_deref(), Some("yearly"));
    assert_eq!(parsed[2].repeat, None, "weekly rules import the first date only");
    assert_eq!(calendar::import_events(&c, &parsed, true).unwrap(), 3);
    assert_eq!(calendar::import_events(&c, &parsed, true).unwrap(), 0, "importing again adds nothing");
    let jan = calendar::range(&c, "2028-01-01", "2028-01-31", "IN", d("2026-10-19")).unwrap();
    assert!(jan.iter().any(|i| i.kind == "holiday" && i.title == "New Year, again"));

    assert!(calendar::parse_ics(b"not a calendar").is_err());
    assert!(calendar::parse_ics(&vec![b'x'; calendar::MAX_ICS_BYTES + 1]).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, failure_persistence: None, ..ProptestConfig::default() })]

    /// Any bytes: an error is fine, a panic is not, and nothing imported is out of shape.
    #[test]
    fn ics_reader_survives_any_input(body in prop::collection::vec(prop_oneof![
        Just("BEGIN:VEVENT".to_string()), Just("END:VEVENT".to_string()), "DTSTART(;VALUE=DATE)?:[0-9TZ]{0,16}",
        "SUMMARY:.{0,100}", "RRULE:FREQ=(YEARLY|MONTHLY|WEEKLY)(;INTERVAL=[0-9])?", " .{0,20}", ".{0,40}"], 0..40)) {
        let text = format!("BEGIN:VCALENDAR\r\n{}\r\nEND:VCALENDAR", body.join("\r\n"));
        if let Ok(events) = calendar::parse_ics(text.as_bytes()) {
            for e in events {
                prop_assert!(NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").is_ok());
                prop_assert!(!e.title.is_empty() && e.title.chars().count() <= 80);
            }
        }
    }
}
