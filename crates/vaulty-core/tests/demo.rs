use chrono::NaiveDate;
use vaulty_core::{accounts, db, demo, history, ledger, transactions};

#[test]
fn sample_data_is_balanced_repeatable_and_one_undoable_action() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
    let make = || {
        let c = db::open_in_memory().unwrap();
        demo::seed(&c, demo::Persona::Earner, today).unwrap();
        c
    };
    let c = make();
    let rows = transactions::list(&c, None, true).unwrap();
    assert!(rows.len() > 100, "six months of history ({} rows)", rows.len());
    assert!(rows.iter().all(|r| r["date"].as_str().unwrap() <= "2026-10-18"), "nothing in the future");
    assert_eq!(transactions::hidden_count(&c).unwrap(), 2);
    assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
    assert!(history::verify(&c).unwrap().ok);
    assert_eq!(history::recent(&c, 5).unwrap().len(), 1, "seeding is one action");

    // Same seed, same data.
    let balances = |c: &rusqlite::Connection| accounts::get_all(c).unwrap().iter().map(|a| a["current_balance"].as_f64().unwrap()).collect::<Vec<_>>();
    assert_eq!(balances(&c), balances(&make()));

    for p in [demo::Persona::Freelancer, demo::Persona::Child] {
        let c = db::open_in_memory().unwrap();
        demo::seed(&c, p, today).unwrap();
        assert!(!transactions::list(&c, None, true).unwrap().is_empty());
        assert!(accounts::get_all(&c).unwrap().iter().all(|a| a["current_balance"].as_f64().unwrap() >= 0.0), "{p:?} never goes negative");
    }
}
