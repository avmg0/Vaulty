//! Sample family (placeholder names only): A = John Doe and B = Jane Doe (parents),
//! C = Jimmy Doe and D = Jenny Doe (children).
use serde_json::json;
use vaulty_core::household::{FeedFilter, FeedView, Household, SharingPrefs, Visibility};
use vaulty_core::{accounts, db, transactions};

struct Member {
    id: &'static str,
    db: rusqlite::Connection,
    account: String,
}

fn member(id: &'static str) -> Member {
    let db = db::open_in_memory().unwrap();
    let acc = accounts::create(&db, &json!({"name": format!("{id} wallet"), "type": "cash", "currency": "INR"})).unwrap();
    Member { id, db, account: acc["id"].as_str().unwrap().into() }
}

fn spend(h: &Household, m: &Member, merchant: &str, amount: f64, vis: &str) -> String {
    let row = transactions::create(
        &m.db,
        &json!({"account_id": m.account, "type": "expense", "amount": amount, "date": "2026-10-10",
                "merchant": merchant, "notes": "secret note", "visibility": vis}),
    )
    .unwrap();
    h.publish(m.id, &row).unwrap();
    row["id"].as_str().unwrap().into()
}

fn on(shared_with: &[&str]) -> SharingPrefs {
    SharingPrefs { enabled: true, default_visibility: Visibility::Private, shared_with: shared_with.iter().map(|s| s.to_string()).collect() }
}

fn merchants(h: &Household, viewer: &str, view: FeedView) -> Vec<String> {
    let mut v: Vec<String> = h
        .feed(viewer, view, &FeedFilter::default())
        .unwrap()
        .into_iter()
        .map(|r| r["merchant"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn nothing_is_shared_until_the_owner_opts_in() {
    let h = Household::open_in_memory().unwrap();
    let c = member("C");
    h.set_prefs("A", &on(&[])).unwrap();
    spend(&h, &c, "Comics", 200.0, "public");
    assert!(merchants(&h, "A", FeedView::All).is_empty(), "C hasn't turned sharing on");
}

#[test]
fn viewers_must_opt_in_too() {
    let h = Household::open_in_memory().unwrap();
    let c = member("C");
    h.set_prefs("C", &on(&["A"])).unwrap();
    spend(&h, &c, "Comics", 200.0, "shared");
    assert!(h.feed("A", FeedView::All, &FeedFilter::default()).is_err(), "A hasn't turned sharing on");
    h.set_prefs("A", &on(&[])).unwrap();
    assert_eq!(merchants(&h, "A", FeedView::All), ["Comics"]);
}

#[test]
fn private_shared_public_audiences() {
    let h = Household::open_in_memory().unwrap();
    let (c, d) = (member("C"), member("D"));
    for p in ["A", "B"] {
        h.set_prefs(p, &on(&[])).unwrap();
    }
    h.set_prefs("C", &on(&["A", "B"])).unwrap(); // C shares with both parents
    h.set_prefs("D", &on(&["A"])).unwrap(); // D shares only with A

    spend(&h, &c, "C diary", 50.0, "private");
    spend(&h, &c, "C school trip", 1500.0, "shared");
    spend(&h, &c, "C family dinner", 2400.0, "public");
    spend(&h, &d, "D games", 999.0, "shared");
    spend(&h, &d, "D groceries", 600.0, "public");

    assert_eq!(merchants(&h, "A", FeedView::All), ["C family dinner", "C school trip", "D games", "D groceries"]);
    assert_eq!(merchants(&h, "B", FeedView::All), ["C family dinner", "C school trip", "D groceries"]);
    assert_eq!(merchants(&h, "A", FeedView::Shared), ["C school trip", "D games"]);
    assert_eq!(merchants(&h, "B", FeedView::Public), ["C family dinner", "D groceries"]);
    // Siblings see each other's public items only, plus their own published items.
    assert_eq!(merchants(&h, "D", FeedView::All), ["C family dinner", "D games", "D groceries"]);
}

#[test]
fn notes_never_leave_the_owners_database() {
    let h = Household::open_in_memory().unwrap();
    let c = member("C");
    h.set_prefs("A", &on(&[])).unwrap();
    h.set_prefs("C", &on(&[])).unwrap();
    spend(&h, &c, "Cinema", 300.0, "public");
    let row = &h.feed("A", FeedView::All, &FeedFilter::default()).unwrap()[0];
    assert!(row.get("notes").is_none());
    assert_eq!(row["currency"], "INR");
    assert_eq!(row["owner_id"], "C");
}

#[test]
fn owner_can_withdraw_by_visibility_change_or_turning_off() {
    let h = Household::open_in_memory().unwrap();
    let c = member("C");
    h.set_prefs("A", &on(&[])).unwrap();
    h.set_prefs("C", &on(&["A"])).unwrap();
    let id = spend(&h, &c, "Shoes", 1200.0, "shared");
    spend(&h, &c, "Books", 400.0, "public");

    // Make one private again.
    let row = transactions::set_visibility(&c.db, &id, "private").unwrap();
    h.publish("C", &row).unwrap();
    assert_eq!(merchants(&h, "A", FeedView::All), ["Books"]);

    // Turn sharing off entirely: everything is withdrawn.
    h.set_prefs("C", &SharingPrefs { enabled: false, ..on(&["A"]) }).unwrap();
    assert!(merchants(&h, "A", FeedView::All).is_empty());

    // Turn it back on: republish restores what's still marked shared/public.
    h.set_prefs("C", &on(&["A"])).unwrap();
    h.republish_all("C", &transactions::shareable(&c.db).unwrap()).unwrap();
    assert_eq!(merchants(&h, "A", FeedView::All), ["Books"]);
}

#[test]
fn removing_someone_from_the_audience_hides_shared_items_immediately() {
    let h = Household::open_in_memory().unwrap();
    let c = member("C");
    h.set_prefs("B", &on(&[])).unwrap();
    h.set_prefs("C", &on(&["A", "B"])).unwrap();
    spend(&h, &c, "Concert", 2000.0, "shared");
    assert_eq!(merchants(&h, "B", FeedView::All), ["Concert"]);
    h.set_prefs("C", &on(&["A"])).unwrap();
    assert!(merchants(&h, "B", FeedView::All).is_empty());
}

#[test]
fn deleting_a_transaction_or_member_withdraws_it() {
    let h = Household::open_in_memory().unwrap();
    let c = member("C");
    h.set_prefs("A", &on(&[])).unwrap();
    h.set_prefs("C", &on(&[])).unwrap();
    let id = spend(&h, &c, "Snacks", 80.0, "public");
    transactions::delete(&c.db, &id).unwrap();
    h.withdraw("C", &id).unwrap();
    assert!(merchants(&h, "A", FeedView::All).is_empty());

    spend(&h, &c, "Stationery", 120.0, "public");
    h.forget_member("C").unwrap();
    assert!(merchants(&h, "A", FeedView::All).is_empty());
}

#[test]
fn cannot_share_with_self_and_bad_visibility_is_rejected() {
    let h = Household::open_in_memory().unwrap();
    assert!(h.set_prefs("C", &on(&["C"])).is_err());
    let c = member("C");
    assert!(transactions::create(&c.db, &json!({"account_id": c.account, "type": "expense", "amount": 1, "date": "2026-10-10", "visibility": "everyone"})).is_err());
}

#[test]
fn feed_filters_by_member_and_date() {
    let h = Household::open_in_memory().unwrap();
    let (c, d) = (member("C"), member("D"));
    h.set_prefs("A", &on(&[])).unwrap();
    h.set_prefs("C", &on(&[])).unwrap();
    h.set_prefs("D", &on(&[])).unwrap();
    spend(&h, &c, "C1", 10.0, "public");
    spend(&h, &d, "D1", 20.0, "public");
    let only_c = h.feed("A", FeedView::All, &FeedFilter { owner_id: Some("C".into()), ..Default::default() }).unwrap();
    assert_eq!(only_c.len(), 1);
    let none = h.feed("A", FeedView::All, &FeedFilter { start_date: Some("2026-11-01".into()), ..Default::default() }).unwrap();
    assert!(none.is_empty());
}
