use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::profiles::{HelperChoice, PasswordOutcome, Registry, Role, Unlocked};
use vaulty_core::{accounts, cards, db, estate, people, recurring, transactions};

struct Family { r: Registry, john: Unlocked, jane: String, jimmy: String }

fn family() -> Family {
    let r = Registry::open_in_memory().unwrap();
    let john = r.create_first_admin("John Doe", "#2563eb", "parent-password").unwrap();
    let add = |name: &str, pw: &str| {
        let inv = r.add_user(&john, name, "#000", Role::Member).unwrap();
        r.claim_profile(&inv.user.id, &inv.setup_code, pw).unwrap().user_id
    };
    let jane = add("Jane Doe", "jane-password!");
    let jimmy = add("Jimmy Doe", "jimmy-password!");
    Family { r, john, jane, jimmy }
}

fn jane_key(r: &Registry, jane: &str) -> Vec<u8> {
    let PasswordOutcome::SignedIn(u) = r.sign_in_with_password(jane, "jane-password!").unwrap() else { panic!() };
    u.data_key.to_vec()
}

#[test]
fn nominee_card_plus_one_helper_after_the_wait() {
    let Family { r, john, jane, jimmy } = family();
    assert!(r.set_nominee(&jane, "jane-password!", "Priya Doe", 7, None).is_err(), "helpers first");
    let helper_cards = r.set_helpers(&jane, "jane-password!", &[
        HelperChoice::Member { id: john.user_id.clone() }, HelperChoice::Member { id: jimmy.clone() }, HelperChoice::Card { label: "Rahul".into() },
    ]).unwrap();
    assert!(r.set_nominee(&jane, "jane-password!", "Priya Doe", 5, None).is_err(), "3, 7, 14 or 30 days");
    let card = r.set_nominee(&jane, "jane-password!", "Priya Doe", 7, Some("Papers are in the steel almirah.")).unwrap();
    assert!(card.nominee);
    assert_eq!(r.set_nominee(&jane, "jane-password!", "Priya Doe", 7, Some("Papers are in the steel almirah.")).unwrap().code, card.code, "issued again, the same card");

    // The nominee card isn't a helper card, and a helper card isn't a nominee card.
    assert!(r.helper_share_from_card(&jane, &card.code).is_err());
    assert!(r.nominee_start(&jane, &helper_cards[0].code).is_err());

    let status = r.nominee_start(&jane, &card.code).unwrap();
    let req = status.request.unwrap();
    assert_eq!(req.ready_at - req.started_at, 7 * 86_400);
    assert!(!req.ready);
    let from_john = r.helper_share_from_member(&jane, &john.user_id, "parent-password").unwrap();
    let err = r.open_as_nominee(&jane, &card.code, &from_john).err().unwrap().to_string();
    assert!(err.contains("waiting period"), "{err}");

    // Jane signs in, sees the request and cancels it: nothing opens.
    let PasswordOutcome::SignedIn(me) = r.sign_in_with_password(&jane, "jane-password!").unwrap() else { panic!() };
    assert_eq!(r.nominee_letter(&me).unwrap().as_deref(), Some("Papers are in the steel almirah."));
    r.nominee_answer(&me, false).unwrap();
    assert!(r.nominee(&jane).unwrap().request.is_none());
    assert!(r.open_as_nominee(&jane, &card.code, &from_john).is_err());

    // Started again and let through (as if the wait were over): read-only access with the letter.
    r.nominee_start(&jane, &card.code).unwrap();
    r.nominee_answer(&me, true).unwrap();
    let view = r.open_as_nominee(&jane, &card.code, &from_john).unwrap();
    assert_eq!(view.data_key.to_vec(), jane_key(&r, &jane));
    assert_eq!((view.owner_name.as_str(), view.letter.as_deref()), ("Jane Doe", Some("Papers are in the steel almirah.")));
    assert_eq!(r.take_helper_notices(&john.user_id).unwrap().len(), 1, "the helper is told");
    // With a printed helper card instead of John.
    let from_rahul = r.helper_share_from_card(&jane, &helper_cards[0].code).unwrap();
    assert!(r.open_as_nominee(&jane, &card.code, &from_rahul).is_ok());

    // New helpers: a new nominee card comes with them and the old one stops working.
    let again = r.set_helpers(&jane, "jane-password!", &[
        HelperChoice::Member { id: john.user_id.clone() }, HelperChoice::Card { label: "Rahul".into() }, HelperChoice::Card { label: "Meera".into() },
    ]).unwrap();
    let new_card = again.iter().find(|c| c.nominee).expect("nominee card reissued");
    assert_ne!(new_card.code, card.code);
    assert!(r.open_as_nominee(&jane, &card.code, &r.helper_share_from_card(&jane, &again[0].code).unwrap()).is_err());
    assert!(r.open_as_nominee(&jane, &new_card.code, &r.helper_share_from_card(&jane, &again[0].code).unwrap()).is_ok());

    // Turning helpers off removes the nominee too.
    r.remove_helpers(&jane, "jane-password!").unwrap();
    assert!(!r.nominee(&jane).unwrap().set_up);
}

#[test]
fn estate_summary_lists_what_a_family_needs() {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Savings", "type": "checking", "currency": "INR", "initial_balance": "250000", "notes": "Nominee: Priya" })).unwrap();
    accounts::create(&c, &json!({ "name": "Home loan", "type": "loan", "currency": "INR", "initial_balance": "1800000" })).unwrap();
    cards::create(&c, &json!({ "kind": "credit", "name": "Rewards", "currency": "INR", "owed": "12400", "network": "visa", "last4": "4242" })).unwrap();
    let rahul = people::create(&c, &json!({ "name": "Rahul Doe", "phone": "98xxxxxx10" })).unwrap();
    transactions::create(&c, &json!({ "type": "transfer", "account_id": bank["id"], "person_id": rahul["id"], "flow": "out", "amount": "5000", "date": "2026-10-01" })).unwrap();
    recurring::create(&c, &json!({ "name": "LIC premium", "type": "expense", "amount": "24000", "account_id": bank["id"], "frequency": "yearly", "start_date": "2027-03-01" }), today).unwrap();

    let e = estate::summary(&c, today).unwrap();
    let savings = e.accounts.iter().find(|a| a.name == "Savings").unwrap();
    assert_eq!((savings.amount, savings.notes.as_deref()), (245000.0, Some("Nominee: Priya")));
    let debts: Vec<(&str, f64)> = e.debts.iter().map(|d| (d.name.as_str(), d.amount)).collect();
    assert!(debts.contains(&("Home loan", 1800000.0)) && debts.contains(&("Rewards", 12400.0)), "{debts:?}");
    let card = e.cards.iter().find(|c| c.name == "Rewards").unwrap();
    assert_eq!((card.network.as_deref(), card.last4.as_deref()), (Some("visa"), Some("4242")));
    assert_eq!((e.people[0].name.as_str(), e.people[0].amount), ("Rahul Doe", 5000.0));
    assert_eq!(e.regular[0].name, "LIC premium");
    let inr = e.totals.iter().find(|t| t.currency == "INR").unwrap();
    assert_eq!((inr.owned, inr.owed, inr.net), (250000.0, 1812400.0, -1562400.0));
}
