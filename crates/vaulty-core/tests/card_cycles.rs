use chrono::NaiveDate;
use serde_json::{json, Value};
use vaulty_core::{accounts, calendar, card_cycles, cards, db, reminders, transactions};

fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

fn setup() -> (rusqlite::Connection, Value, Value) {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "200000", "opening_date": "2026-01-01" })).unwrap();
    let card = cards::create(&c, &json!({ "kind": "credit", "name": "Rewards", "currency": "INR", "statement_day": 15, "due_day": 5, "credit_limit": "200000" })).unwrap();
    (c, bank, card)
}

fn spend(c: &rusqlite::Connection, card: &Value, amount: &str, date: &str) {
    transactions::create(c, &json!({ "account_id": card["account_id"], "type": "expense", "amount": amount, "date": date, "merchant": "Shop" })).unwrap();
}
fn pay(c: &rusqlite::Connection, bank: &Value, card: &Value, amount: &str, date: &str) {
    transactions::create(c, &json!({ "type": "transfer", "account_id": bank["id"], "to_account_id": card["account_id"], "amount": amount, "date": date })).unwrap();
}

#[test]
fn statement_minimum_and_part_payment() {
    let (c, bank, card) = setup();
    let id = card["id"].as_str().unwrap();
    spend(&c, &card, "1000", "2026-08-25");
    pay(&c, &bank, &card, "1000", "2026-10-01");
    spend(&c, &card, "10000", "2026-09-20");
    pay(&c, &bank, &card, "3000", "2026-10-20");
    spend(&c, &card, "2000", "2026-10-18");

    let cy = card_cycles::for_card(&c, id, d("2026-10-23")).unwrap().unwrap();
    let cur = cy.current.as_ref().unwrap();
    assert_eq!((cur.statement_date.as_str(), cur.due_date.as_str()), ("2026-10-15", "2026-11-05"));
    assert_eq!((cur.total, cur.min, cur.paid, cur.left, cur.min_left), (10000.0, 500.0, 3000.0, 7000.0, 0.0));
    assert_eq!(cur.status, "min_paid");
    assert_eq!(cy.unbilled, 2000.0);
    assert_eq!((cy.next_statement.as_str(), cy.buy_today_due.as_str(), cy.interest_free_days), ("2026-11-15", "2026-12-05", 43));
    let sept = &cy.history[0];
    assert_eq!((sept.statement_date.as_str(), sept.total, sept.status.as_str()), ("2026-09-15", 1000.0, "paid"));
    assert!(cy.history.iter().all(|h| h.statement_date.as_str() >= "2026-08-15"), "nothing before the card was used");

    // The bank's own figures replace the estimate.
    card_cycles::enter(&c, id, &json!({ "statement_date": "2026-10-15", "total": "10250", "min": "600" })).unwrap();
    assert!(card_cycles::enter(&c, id, &json!({ "statement_date": "2026-10-15", "total": "100", "min": "600" })).is_err());
    let cur = card_cycles::for_card(&c, id, d("2026-10-23")).unwrap().unwrap().current.unwrap();
    assert_eq!((cur.total, cur.min, cur.left, cur.entered), (10250.0, 600.0, 7250.0, true));
    card_cycles::forget(&c, id, "2026-10-15").unwrap();

    // Reminded 2 days before the due date with what's left and the minimum (paid already: 0).
    let due = reminders::take_due(&c, d("2026-11-03")).unwrap();
    let r = due.iter().find(|r| r.kind == "card").unwrap();
    assert_eq!((r.date.as_str(), r.amount, r.previous), ("2026-11-05", Some(7000.0), Some(0.0)));
    // And on the calendar.
    let items = calendar::range(&c, "2026-11-01", "2026-11-30", "IN", d("2026-10-23")).unwrap();
    assert_eq!(items.iter().find(|i| i.kind == "card_due").unwrap().amount, Some(7000.0));
}

#[test]
fn minimum_rules_and_overdue() {
    let (c, _bank, card) = setup();
    let id = card["id"].as_str().unwrap();
    spend(&c, &card, "3000", "2026-09-20");
    let cur = card_cycles::for_card(&c, id, d("2026-10-23")).unwrap().unwrap().current.unwrap();
    assert_eq!((cur.min, cur.status.as_str()), (200.0, "due"), "₹200 floor beats 5%");

    // After the due date with nothing paid: overdue.
    let cur = card_cycles::for_card(&c, id, d("2026-11-07")).unwrap().unwrap().current.unwrap();
    assert_eq!((cur.status.as_str(), cur.left), ("overdue", 3000.0));

    // A card's own rule: 2.5%, no floor; rounded up to the rupee.
    cards::update(&c, id, &json!({ "min_due_percent": "2.5", "min_due_floor": "0" })).unwrap();
    spend(&c, &card, "1001", "2026-10-01");
    let cur = card_cycles::for_card(&c, id, d("2026-10-23")).unwrap().unwrap().current.unwrap();
    assert_eq!((cur.total, cur.min), (4001.0, 101.0));
    assert!(cards::update(&c, id, &json!({ "min_due_percent": "120" })).is_err());
    assert!(cards::update(&c, id, &json!({ "card_year_start": "13-40" })).is_err());
    cards::update(&c, id, &json!({ "card_year_start": "03-15", "annual_fee": "2500" })).unwrap();
    assert_eq!(cards::get(&c, id).unwrap()["annual_fee"], 2500.0);
}
