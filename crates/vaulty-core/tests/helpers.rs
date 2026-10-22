use vaulty_core::profiles::{HelperChoice, PasswordOutcome, Registry, Role, Unlocked};

struct Family { r: Registry, john: Unlocked, jane: String, jimmy: String, jenny: String }

fn family() -> Family {
    let r = Registry::open_in_memory().unwrap();
    let john = r.create_first_admin("John Doe", "#2563eb", "parent-password").unwrap();
    let add = |name: &str, role, pw: &str| {
        let inv = r.add_user(&john, name, "#000", role).unwrap();
        r.claim_profile(&inv.user.id, &inv.setup_code, pw).unwrap().user_id
    };
    let jane = add("Jane Doe", Role::Member, "jane-password!");
    let jimmy = add("Jimmy Doe", Role::Member, "jimmy-password!");
    let jenny = add("Jenny Doe", Role::Child, "jenny-password!");
    Family { r, john, jane, jimmy, jenny }
}

fn member(id: &str) -> HelperChoice { HelperChoice::Member { id: id.into() } }
fn card(label: &str) -> HelperChoice { HelperChoice::Card { label: label.into() } }

fn signs_in(r: &Registry, id: &str, pw: &str) -> bool {
    matches!(r.sign_in_with_password(id, pw), Ok(PasswordOutcome::SignedIn(_) | PasswordOutcome::NeedsSecondFactor(_)))
}

#[test]
fn choosing_helpers() {
    let Family { r, john, jane, jimmy, jenny } = family();
    let pw = "jane-password!";
    assert!(r.set_helpers(&jane, "wrong-password", &[member(&john.user_id), member(&jimmy), card("Rahul")]).is_err());
    assert!(r.set_helpers(&jane, pw, &[member(&john.user_id), member(&jimmy)]).is_err(), "three needed");
    assert!(r.set_helpers(&jane, pw, &[member(&john.user_id), member(&jenny), card("Rahul")]).is_err(), "no children");
    assert!(r.set_helpers(&jane, pw, &[member(&john.user_id), member(&jane), card("Rahul")]).is_err(), "not herself");
    assert!(r.set_helpers(&jane, pw, &[member(&john.user_id), member(&john.user_id), card("Rahul")]).is_err(), "different people");
    assert!(r.set_helpers(&jane, pw, &[member(&john.user_id), member(&jimmy), card("  ")]).is_err(), "a card needs a holder");
    assert!(r.set_helpers(&jenny, "jenny-password!", &[member(&john.user_id), member(&jimmy), card("Rahul")]).is_err(), "children are helped by a parent");

    let cards = r.set_helpers(&jane, pw, &[member(&john.user_id), member(&jimmy), card("Rahul Doe (brother)")]).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!((cards[0].owner.as_str(), cards[0].label.as_str()), ("Jane Doe", "Rahul Doe (brother)"));
    assert_eq!(cards[0].code.replace('-', "").len(), 37);
    let h = r.helpers(&jane).unwrap();
    assert!(h.set_up);
    assert_eq!(h.usable, 3);
    assert_eq!(h.slots.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["John Doe", "Jimmy Doe", "Rahul Doe (brother)"]);
    assert_eq!(r.helper_candidates(&jane).unwrap().iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), ["John Doe", "Jimmy Doe"]);
}

#[test]
fn two_helpers_give_the_profile_back() {
    let Family { r, john, jane, jimmy, .. } = family();
    let cards = r.set_helpers(&jane, "jane-password!", &[member(&john.user_id), member(&jimmy), card("Rahul")]).unwrap();
    let code = &cards[0].code;

    // One helper alone, even the admin, isn't enough.
    let from_john = r.helper_share_from_member(&jane, &john.user_id, "parent-password").unwrap();
    assert!(r.recover_with_helpers(&jane, std::slice::from_ref(&from_john), "jane-new-password", None).is_err());
    assert!(r.recover_with_helpers(&jane, &[from_john.clone(), from_john.clone()], "jane-new-password", None).is_err(), "the same helper twice");
    // Helpers prove who they are with their own password.
    assert!(r.helper_share_from_member(&jane, &jimmy, "not-jimmys-password").is_err());
    assert!(r.helper_share_from_member(&jane, &jane, "jane-password!").is_err(), "not a helper");
    // A typo on the card is caught before it counts as a wrong guess.
    let mut typo: Vec<char> = code.chars().collect();
    typo[3] = if typo[3] == 'A' { 'B' } else { 'A' };
    let err = r.helper_share_from_card(&jane, &typo.into_iter().collect::<String>()).err().unwrap().to_string();
    assert!(err.contains("typo"), "{err}");

    let from_card = r.helper_share_from_card(&jane, &code.to_lowercase().replace('-', " ")).unwrap();
    assert!(r.recover_with_helpers(&jane, &[from_john.clone(), from_card.clone()], "password", None).is_err(), "new password rules apply");
    let back = r.recover_with_helpers(&jane, &[from_john, from_card], "jane-new-password", None).unwrap();
    assert_eq!(back.user_id, jane);
    assert!(signs_in(&r, &jane, "jane-new-password"));
    assert!(!signs_in(&r, &jane, "jane-password!"));
    // John is told his help was used; the card's holder isn't on the device.
    let notices = r.take_helper_notices(&john.user_id).unwrap();
    assert_eq!(notices.iter().map(|n| n.owner_name.as_str()).collect::<Vec<_>>(), ["Jane Doe"]);
    assert!(r.take_helper_notices(&john.user_id).unwrap().is_empty(), "once");

    // Any two: Jimmy and John this time.
    let a = r.helper_share_from_member(&jane, &jimmy, "jimmy-password!").unwrap();
    let b = r.helper_share_from_member(&jane, &john.user_id, "parent-password").unwrap();
    r.recover_with_helpers(&jane, &[a, b], "jane-third-password", None).unwrap();
}

#[test]
fn changing_helpers_retires_old_cards() {
    let Family { r, john, jane, jimmy, .. } = family();
    let old = r.set_helpers(&jane, "jane-password!", &[member(&john.user_id), member(&jimmy), card("Rahul")]).unwrap();
    let new = r.set_helpers(&jane, "jane-password!", &[member(&john.user_id), card("Rahul"), card("Priya")]).unwrap();
    assert_eq!(new.len(), 2);
    assert!(r.helper_share_from_card(&jane, &old[0].code).is_err(), "older set");
    assert!(r.helper_share_from_member(&jane, &jimmy, "jimmy-password!").is_err(), "no longer a helper");
    let a = r.helper_share_from_card(&jane, &new[0].code).unwrap();
    let b = r.helper_share_from_card(&jane, &new[1].code).unwrap();
    r.recover_with_helpers(&jane, &[a, b], "jane-new-password", None).unwrap();
    r.remove_helpers(&jane, "jane-new-password").unwrap();
    assert!(!r.helpers(&jane).unwrap().set_up);
    assert!(r.helper_share_from_card(&jane, &new[0].code).is_err());
}

#[test]
fn two_factor_stays_on() {
    let Family { r, john, jane, jimmy, .. } = family();
    let e = r.begin_totp(&jane, "jane-password!").unwrap();
    let now = totp_rs::Builder::new().with_secret(totp_rs::Secret::try_from_base32(&e.secret_base32).unwrap())
        .with_account_name("Jane Doe").with_issuer(Some("Vaulty")).build().unwrap().generate_current().to_string();
    let backup = r.confirm_totp(&jane, "jane-password!", &e.secret_base32, &now).unwrap();
    r.set_helpers(&jane, "jane-password!", &[member(&john.user_id), member(&jimmy), card("Rahul")]).unwrap();
    let shares = [r.helper_share_from_member(&jane, &john.user_id, "parent-password").unwrap(), r.helper_share_from_member(&jane, &jimmy, "jimmy-password!").unwrap()];
    assert!(r.recover_with_helpers(&jane, &shares, "jane-new-password", None).is_err(), "code needed");
    assert!(r.recover_with_helpers(&jane, &shares, "jane-new-password", Some("000000")).is_err());
    assert!(signs_in(&r, &jane, "jane-password!"), "nothing changed after a wrong code");
    r.recover_with_helpers(&jane, &shares, "jane-new-password", Some(&backup[0])).unwrap();
    assert!(r.get(&jane).unwrap().totp_enabled);
    assert!(matches!(r.sign_in_with_password(&jane, "jane-new-password").unwrap(), PasswordOutcome::NeedsSecondFactor(_)));
}

#[test]
fn a_helper_starting_over_can_no_longer_help() {
    let Family { r, john, jane, jimmy, .. } = family();
    r.set_helpers(&jane, "jane-password!", &[member(&john.user_id), member(&jimmy), card("Rahul")]).unwrap();
    r.erase_and_reset(&john, &jimmy).unwrap();
    let h = r.helpers(&jane).unwrap();
    assert_eq!(h.usable, 2);
    assert!(!h.slots.iter().find(|s| s.name == "Jimmy Doe").unwrap().usable);
    // Removing a member leaves the slot unusable too.
    r.remove_user(&john.user_id, &jimmy).unwrap();
    assert_eq!(r.helpers(&jane).unwrap().usable, 2);
    // And a member who starts over loses their own helpers (their old data key is gone).
    r.erase_and_reset(&john, &jane).unwrap();
    assert!(!r.helpers(&jane).unwrap().set_up);
}
