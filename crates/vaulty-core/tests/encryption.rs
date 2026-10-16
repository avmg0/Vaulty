//! Encryption at rest: data files are unreadable without the member's data key, and every
//! unlock route (password, recovery key, biometric token, parent for a child) yields that key.
use serde_json::json;
use vaulty_core::{accounts, db, household::Household, profiles::{PasswordOutcome, Registry, Role, Unlocked}};

fn signed_in(r: &Registry, id: &str, pw: &str) -> Unlocked {
    match r.sign_in_with_password(id, pw).unwrap() { PasswordOutcome::SignedIn(u) => u, _ => panic!("expected sign-in") }
}

fn file_contains(path: &std::path::Path, needle: &str) -> bool {
    let bytes = std::fs::read(path).unwrap();
    bytes.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[test]
fn data_files_are_unreadable_without_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("member.db");
    let r = Registry::open_in_memory().unwrap();
    let me = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    {
        let c = db::open(&path, &me.data_key).unwrap();
        accounts::create(&c, &json!({ "name": "Secret Savings Account", "type": "savings", "currency": "INR", "initial_balance": "100" })).unwrap();
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    }
    assert!(!db::is_plain_sqlite(&path), "file must not carry the plain SQLite header");
    assert!(!file_contains(&path, "Secret Savings"), "account name must not appear in the file");
    // Plain SQLite can't read it.
    let plain = rusqlite::Connection::open(&path).unwrap();
    assert!(plain.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_err());
    // Another member's key can't open it.
    let inv = r.add_user(&me, "Jane Doe", "#000", Role::Member).unwrap();
    let jane = r.claim_profile(&inv.user.id, &inv.setup_code, "jane-password").unwrap();
    assert!(db::open(&path, &jane.data_key).is_err());
    // The owner's key can, after signing in again.
    let again = signed_in(&r, &me.user_id, "parent-password");
    let c = db::open(&path, &again.data_key).unwrap();
    assert_eq!(accounts::get_all(&c).unwrap()[0]["name"], "Secret Savings Account");
}

#[test]
fn legacy_plain_files_are_encrypted_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    {
        let c = db::open_with(&path, db::DATA_MIGRATIONS).unwrap();
        accounts::create(&c, &json!({ "name": "Old Wallet", "type": "cash", "currency": "INR", "initial_balance": "5" })).unwrap();
    }
    assert!(db::is_plain_sqlite(&path));
    let r = Registry::open_in_memory().unwrap();
    let me = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    let c = db::open(&path, &me.data_key).unwrap();
    assert_eq!(accounts::get_all(&c).unwrap()[0]["name"], "Old Wallet");
    drop(c);
    assert!(!db::is_plain_sqlite(&path));
    assert!(!file_contains(&path, "Old Wallet"));
    assert!(!dir.path().join("old.db.encrypting").exists());
}

#[test]
fn password_change_and_recovery_keep_the_same_data_key() {
    let r = Registry::open_in_memory().unwrap();
    let me = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    let recovery = me.new_recovery_key.clone().expect("first setup shows a recovery key");
    assert_eq!(recovery.len(), 29, "24 symbols in groups of four: {recovery}");

    r.change_password(&me.user_id, "parent-password", "changed-password").unwrap();
    assert_eq!(*signed_in(&r, &me.user_id, "changed-password").data_key, *me.data_key);

    // Forgot password: the recovery key opens the same data and sets a new password.
    assert!(r.sign_in_with_recovery_key(&me.user_id, "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF", "whatever-pass").is_err());
    let rec = r.sign_in_with_recovery_key(&me.user_id, &recovery.to_lowercase().replace('-', " "), "brand-new-password").unwrap();
    assert_eq!(*rec.data_key, *me.data_key);
    assert!(r.sign_in_with_password(&me.user_id, "changed-password").is_err());
    assert_eq!(*signed_in(&r, &me.user_id, "brand-new-password").data_key, *me.data_key);
    // The used recovery key is replaced.
    let next = rec.new_recovery_key.expect("a new recovery key replaces the used one");
    assert_ne!(next, recovery);
    assert!(r.sign_in_with_recovery_key(&me.user_id, &recovery, "another-password").is_err());
}

#[test]
fn biometric_token_unwraps_the_data_key() {
    let r = Registry::open_in_memory().unwrap();
    let me = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    let token = r.enable_biometric(&me.user_id, "parent-password").unwrap();
    assert_eq!(*r.sign_in_with_biometric_token(&me.user_id, &token).unwrap().data_key, *me.data_key);
    let forged = "00".repeat(32);
    assert!(r.sign_in_with_biometric_token(&me.user_id, &forged).is_err());
    // Still valid after a password change: the token wraps the data key, not the password.
    r.change_password(&me.user_id, "parent-password", "changed-password").unwrap();
    assert!(r.sign_in_with_biometric_token(&me.user_id, &token).is_ok());
    r.disable_biometric(&me.user_id).unwrap();
    assert!(r.sign_in_with_biometric_token(&me.user_id, &token).is_err());
}

#[test]
fn parents_can_recover_a_child_but_not_an_adult() {
    let dir = tempfile::tempdir().unwrap();
    let r = Registry::open_in_memory().unwrap();
    let parent = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    let inv = r.add_user(&parent, "Jenny Doe", "#000", Role::Child).unwrap();
    let child = r.claim_profile(&inv.user.id, &inv.setup_code, "jenny-password").unwrap();
    let path = dir.path().join("jenny.db");
    {
        let c = db::open(&path, &child.data_key).unwrap();
        accounts::create(&c, &json!({ "name": "Piggy Bank", "type": "cash", "currency": "INR", "initial_balance": "250" })).unwrap();
    }
    // Jenny forgets her password; a parent resets it and her data is kept.
    let reset = r.reset_user(&parent, &inv.user.id).unwrap();
    assert!(reset.data_kept);
    assert!(r.sign_in_with_password(&inv.user.id, "jenny-password").is_err());
    let back = r.claim_profile(&inv.user.id, &reset.invite.setup_code, "jenny-new-password").unwrap();
    let c = db::open(&path, &back.data_key).unwrap();
    assert_eq!(accounts::get_all(&c).unwrap()[0]["name"], "Piggy Bank");

    // An adult member can't be reset by an admin.
    let inv = r.add_user(&parent, "Jane Doe", "#000", Role::Member).unwrap();
    r.claim_profile(&inv.user.id, &inv.setup_code, "jane-password").unwrap();
    assert!(r.reset_user(&parent, &inv.user.id).is_err());
    // And the admin holds no copy of an adult's key.
    let copies: i64 = r.conn_for_tests().query_row(
        "SELECT COUNT(*) FROM user_keys WHERE user_id = ? AND kind = 'guardian'", [&inv.user.id], |x| x.get(0)).unwrap();
    assert_eq!(copies, 0);
}

#[test]
fn a_second_parent_gets_a_recovery_copy_when_the_child_signs_in() {
    let r = Registry::open_in_memory().unwrap();
    let mum = r.create_first_admin("Jane Doe", "#000", "parent-password").unwrap();
    let inv = r.add_user(&mum, "Jenny Doe", "#000", Role::Child).unwrap();
    r.claim_profile(&inv.user.id, &inv.setup_code, "jenny-password").unwrap();
    let dad_inv = r.add_user(&mum, "John Doe", "#000", Role::Admin).unwrap();
    let dad = r.claim_profile(&dad_inv.user.id, &dad_inv.setup_code, "dad-password").unwrap();
    assert!(r.reset_user(&dad, &inv.user.id).is_err(), "no copy until Jenny signs in");
    let mut jenny = signed_in(&r, &inv.user.id, "jenny-password");
    r.sync_keys(&mut jenny).unwrap();
    assert!(r.reset_user(&dad, &inv.user.id).unwrap().data_kept);
}

#[test]
fn family_store_key_is_shared_and_the_store_is_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("household.db");
    let r = Registry::open_in_memory().unwrap();
    let mut admin = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    r.sync_keys(&mut admin).unwrap();
    let hk = admin.household_key.clone().expect("first admin creates the family key");
    let inv = r.add_user(&admin, "Jane Doe", "#000", Role::Member).unwrap();
    let jane = r.claim_profile(&inv.user.id, &inv.setup_code, "jane-password").unwrap();
    assert_eq!(*jane.household_key.clone().unwrap(), *hk);
    {
        let h = Household::open(&path, &hk).unwrap();
        h.set_prefs(&admin.user_id, &vaulty_core::household::SharingPrefs { enabled: true, default_visibility: vaulty_core::household::Visibility::Private, shared_with: vec![] }).unwrap();
    }
    assert!(!db::is_plain_sqlite(&path));
    assert!(Household::open(&path, &jane.data_key).is_err(), "a personal key can't open the family store");
    assert!(Household::open(&path, jane.household_key.as_ref().unwrap()).is_ok());
}

#[test]
fn profiles_from_before_encryption_upgrade_on_sign_in() {
    let r = Registry::open_in_memory().unwrap();
    let me = r.create_first_admin("John Doe", "#000", "parent-password").unwrap();
    // Simulate an older install: no wrapped keys or key pair.
    r.conn_for_tests().execute_batch("DELETE FROM user_keys; DELETE FROM household_keys; UPDATE users SET public_key = NULL, secret_key_sealed = NULL;").unwrap();
    let up = signed_in(&r, &me.user_id, "parent-password");
    assert!(up.new_recovery_key.is_some(), "upgrade shows a recovery key");
    let again = signed_in(&r, &me.user_id, "parent-password");
    assert_eq!(*again.data_key, *up.data_key);
    assert!(again.new_recovery_key.is_none());
}
