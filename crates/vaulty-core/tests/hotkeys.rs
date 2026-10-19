use vaulty_core::{db, history, hotkeys};

#[test]
fn shortcuts_are_normalised_and_checked() {
    assert_eq!(hotkeys::normalize("ctrl+shift+k").unwrap(), "Mod+Shift+K");
    assert_eq!(hotkeys::normalize("Cmd+Alt+5").unwrap(), "Mod+Alt+5");
    assert_eq!(hotkeys::normalize("F5").unwrap(), "F5");
    assert_eq!(hotkeys::normalize("Mod+,").unwrap(), "Mod+,");
    assert!(hotkeys::normalize("K").is_err(), "a bare letter would fire while typing");
    assert!(hotkeys::normalize("Shift+K").is_err());
    assert!(hotkeys::normalize("Mod+C").is_err(), "copy belongs to the system");
    assert!(hotkeys::normalize("Mod+Q").is_err());
    assert!(hotkeys::normalize("Mod+Tab").is_err());
    assert!(hotkeys::normalize("Mod++").is_err());
    assert!(hotkeys::normalize("Hyper+K").is_err());
}

#[test]
fn members_change_their_own_shortcuts_with_undo() {
    let c = db::open_in_memory().unwrap();
    let get = |c: &rusqlite::Connection, a: &str| hotkeys::list(c).unwrap().into_iter().find(|h| h.action == a).unwrap();
    assert_eq!(get(&c, "quick_add").keys, "Mod+J");
    assert_eq!(get(&c, "go_budgets").keys, "", "some start without one");

    history::record(&c, "hotkeys.set", |c| hotkeys::set(c, "quick_add", Some("Mod+Shift+A"))).unwrap();
    let q = get(&c, "quick_add");
    assert_eq!((q.keys.as_str(), q.custom), ("Mod+Shift+A", true));

    // Taken by another action: refused, and the message names it.
    let err = hotkeys::set(&c, "go_budgets", Some("ctrl+k")).unwrap_err().to_string();
    assert!(err.contains("Mod+K") && err.contains("palette"), "{err}");
    // Freed once the other moves.
    hotkeys::set(&c, "go_budgets", Some("Mod+J")).unwrap();
    // Off, then back to the default.
    hotkeys::set(&c, "lock", Some("")).unwrap();
    assert_eq!(get(&c, "lock").keys, "");
    hotkeys::set(&c, "lock", None).unwrap();
    assert_eq!((get(&c, "lock").keys.as_str(), get(&c, "lock").custom), ("Mod+L", false));
    assert!(hotkeys::set(&c, "make_tea", Some("Mod+T")).is_err());

    history::undo_last(&c).unwrap();
    assert_eq!(get(&c, "quick_add").keys, "Mod+J", "undo works like any other change");
    assert!(history::verify(&c).unwrap().broken_at.is_none());

    hotkeys::set(&c, "palette", Some("Mod+Alt+P")).unwrap();
    assert!(hotkeys::reset_all(&c).unwrap().iter().all(|h| !h.custom));
}
