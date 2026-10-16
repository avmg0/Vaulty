use vaulty_core::profiles::{PasswordOutcome, Registry, Role, Unlocked};
use vaulty_core::Error;

fn setup() -> (Registry, String) {
    let (r, a) = setup_keys();
    (r, a.user_id)
}

fn setup_keys() -> (Registry, Unlocked) {
    let r = Registry::open_in_memory().unwrap();
    let admin = r.create_first_admin("John Doe", "#2563eb", "parent-password").unwrap();
    (r, admin)
}

fn now_code(secret: &str, name: &str) -> String {
    totp_rs::Builder::new()
        .with_secret(totp_rs::Secret::try_from_base32(secret).unwrap())
        .with_account_name(name)
        .with_issuer(Some("Vaulty"))
        .build()
        .unwrap()
        .generate_current()
        .to_string()
}

#[test]
fn first_admin_only_once() {
    let (r, _) = setup();
    assert!(r.create_first_admin("John Smith", "#000", "another-password").is_err());
    assert_eq!(r.list().unwrap().len(), 1);
}

#[test]
fn admin_adds_member_who_sets_own_password() {
    let (r, admin) = setup_keys();
    let invite = r.add_user(&admin, "Jenny Doe", "#e11d48", Role::Child).unwrap();
    assert!(!invite.user.active);
    assert!(matches!(r.sign_in_with_password(&invite.user.id, "whatever-pw"), Err(Error::Auth(_))));
    assert!(r.claim_profile(&invite.user.id, "WRONG-CODE", "jenny-password").is_err());
    r.claim_profile(&invite.user.id, &invite.setup_code.to_lowercase(), "jenny-password").unwrap();
    assert!(r.get(&invite.user.id).unwrap().active);
    assert!(matches!(r.sign_in_with_password(&invite.user.id, "jenny-password").unwrap(), PasswordOutcome::SignedIn(_)));
    // Can't be claimed twice.
    assert!(r.claim_profile(&invite.user.id, &invite.setup_code, "hijack-password").is_err());
}

#[test]
fn only_admins_manage_users() {
    let (r, keys) = setup_keys();
    let admin = keys.user_id.clone();
    let inv = r.add_user(&keys, "Jimmy Doe", "#000", Role::Member).unwrap();
    let member = r.claim_profile(&inv.user.id, &inv.setup_code, "member-password").unwrap();
    assert!(r.add_user(&member, "Jane Smith", "#000", Role::Member).is_err());
    assert!(r.remove_user(&inv.user.id, &admin).is_err());
    assert!(r.remove_user(&admin, &admin).is_err(), "only admin can't be removed");
    r.remove_user(&admin, &inv.user.id).unwrap();
    assert_eq!(r.list().unwrap().len(), 1);
}

#[test]
fn wrong_passwords_lock_the_profile() {
    let (r, admin) = setup();
    for _ in 0..5 {
        assert!(matches!(r.sign_in_with_password(&admin, "nope-nope"), Err(Error::Auth(_))));
    }
    assert!(matches!(r.sign_in_with_password(&admin, "parent-password"), Err(Error::Throttled(s)) if s > 0 && s <= 30));
}

#[test]
fn totp_enrollment_sign_in_and_replay_protection() {
    let (r, admin) = setup();
    let e = r.begin_totp(&admin, "parent-password").unwrap();
    assert!(e.otpauth_url.starts_with("otpauth://totp/Vaulty:John%20Doe"), "{}", e.otpauth_url);
    assert!(!e.qr_png_base64.is_empty());
    let codes = r.confirm_totp(&admin, "parent-password", &e.secret_base32, &now_code(&e.secret_base32, "John Doe")).unwrap();
    assert_eq!(codes.len(), 10);
    assert!(r.get(&admin).unwrap().totp_enabled);

    let PasswordOutcome::NeedsSecondFactor(pending) = r.sign_in_with_password(&admin, "parent-password").unwrap() else {
        panic!("2FA should be required");
    };
    assert_eq!(pending.secret_for_tests(), e.secret_base32, "secret is unsealed with the data key");
    // The code used during enrolment can't be reused within the same 30s step.
    let step = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() / 30;
    let before = step();
    let reuse = r.complete_with_totp(&pending, &now_code(&e.secret_base32, "John Doe"));
    if step() == before { assert!(reuse.is_err()); }
    // Backup code works exactly once.
    r.complete_with_backup_code(&pending, &codes[0].to_lowercase()).unwrap();
    assert!(r.complete_with_backup_code(&pending, &codes[0]).is_err());
    assert_eq!(r.backup_codes_remaining(&admin).unwrap(), 9);
}

#[test]
fn totp_secret_survives_password_change() {
    let (r, admin) = setup();
    let e = r.begin_totp(&admin, "parent-password").unwrap();
    r.confirm_totp(&admin, "parent-password", &e.secret_base32, &now_code(&e.secret_base32, "John Doe")).unwrap();
    r.change_password(&admin, "parent-password", "new-parent-password").unwrap();
    assert!(r.sign_in_with_password(&admin, "parent-password").is_err());
    let PasswordOutcome::NeedsSecondFactor(p) = r.sign_in_with_password(&admin, "new-parent-password").unwrap() else { panic!() };
    assert_eq!(p.secret_for_tests(), e.secret_base32);
    // Sealed secret is not stored in plain text.
    let sealed: String = r.conn_for_tests().query_row("SELECT totp_secret_sealed FROM users", [], |x| x.get(0)).unwrap();
    assert!(!sealed.contains(&e.secret_base32));
}

#[test]
fn biometric_token_flow() {
    let (r, admin) = setup();
    assert!(r.sign_in_with_biometric_token(&admin, "anything").is_err());
    assert!(r.enable_biometric(&admin, "wrong-password").is_err());
    let token = r.enable_biometric(&admin, "parent-password").unwrap();
    r.sign_in_with_biometric_token(&admin, &token).unwrap();
    assert!(r.sign_in_with_biometric_token(&admin, "forged").is_err());
    r.disable_biometric(&admin).unwrap();
    assert!(r.sign_in_with_biometric_token(&admin, &token).is_err());
}

#[test]
fn adults_can_only_be_erased_and_reset() {
    let (r, admin) = setup_keys();
    let inv = r.add_user(&admin, "Jane Doe", "#16a34a", Role::Admin).unwrap();
    let jane = r.claim_profile(&inv.user.id, &inv.setup_code, "jane-password!").unwrap();
    assert!(r.reset_user(&admin, &inv.user.id).is_err(), "an admin can't take over an adult's data");
    let reset = r.erase_and_reset(&admin, &inv.user.id).unwrap();
    assert!(!reset.data_kept);
    assert!(!reset.invite.user.active);
    assert!(r.sign_in_with_password(&inv.user.id, "jane-password!").is_err());
    let fresh = r.claim_profile(&inv.user.id, &reset.invite.setup_code, "jane-new-password").unwrap();
    assert_ne!(*fresh.data_key, *jane.data_key, "erased profile starts with a new key");
    // Second admin can now remove the first.
    r.remove_user(&inv.user.id, &admin.user_id).unwrap();
}

#[test]
fn password_hashes_use_strong_argon2_settings() {
    let (r, admin) = setup();
    let hash: String = r.conn_for_tests().query_row("SELECT password_hash FROM users WHERE id = ?", [&admin], |x| x.get(0)).unwrap();
    assert!(hash.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"), "{hash}");
}
