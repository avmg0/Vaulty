//! Family members on this device: profiles, passwords, authenticator-app 2FA, backup codes,
//! biometric unlock, recovery keys, and the keys that encrypt each member's data.
//!
//! The registry is separate from each user's finance database, so one member can never open
//! another member's data by being signed in.
//!
//! ## Keys
//! - Each member's database is encrypted (SQLCipher, AES-256) with a random **data key**.
//! - The data key is stored only *wrapped*: under the password, the recovery key, a biometric
//!   token kept in the OS keychain, a pending setup code, and — for Child profiles only — sealed
//!   to each parent's public key. Any one of these unlocks it; none of them is the key itself.
//! - Each member has an X25519 key pair. Its secret half is sealed under their data key; the
//!   public half lets others share keys with them (the family store key, a child's data key).
//! - An admin cannot open an adult member's data. Adults recover with their own recovery key.
use crate::crypto::{self, Key};
use crate::error::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use totp_rs::{Builder, Secret, Totp};

pub const MIN_PASSWORD_LEN: usize = 8;
const MAX_FREE_ATTEMPTS: i64 = 5;
const MAX_LOCKOUT_SECS: i64 = 15 * 60;
const INVITE_TTL_SECS: i64 = 7 * 24 * 3600;
const BACKUP_CODE_COUNT: usize = 10;
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// 24 symbols from a 31-letter alphabet: about 119 bits.
const RECOVERY_LEN: usize = 24;
const ISSUER: &str = "Vaulty";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Member,
    Child,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
            Role::Child => "child",
        }
    }
    pub fn parse(s: &str) -> Result<Role> {
        match s {
            "admin" => Ok(Role::Admin),
            "member" => Ok(Role::Member),
            "child" => Ok(Role::Child),
            _ => Err(Error::Invalid(format!("unknown role `{s}`"))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserSummary {
    pub id: String,
    pub name: String,
    pub color: String,
    pub role: Role,
    /// `false` until the member has claimed the profile and set their own password.
    pub active: bool,
    pub totp_enabled: bool,
    pub biometric_enabled: bool,
    /// Senior mode: larger text and buttons, a shorter menu.
    pub senior: bool,
}

/// Returned to the admin once, to hand to the new member.
#[derive(Debug, Serialize)]
pub struct Invite {
    pub user: UserSummary,
    pub setup_code: String,
}

/// Keys released by a successful sign-in. Kept in memory for the session only and wiped on drop.
#[derive(Clone)]
pub struct Unlocked {
    pub user_id: String,
    pub data_key: Key,
    secret_key: Key,
    /// Key of the family sharing store, once this member has been given it.
    pub household_key: Option<Key>,
    /// A recovery key created during this sign-in (first setup, claim, upgrade or recovery).
    /// Show it once; it is never retrievable again.
    pub new_recovery_key: Option<String>,
}

/// Password accepted, but an authenticator code is still required.
/// The shell keeps this in memory only, never on disk.
pub struct SecondFactor {
    pub user_id: String,
    secret_base32: zeroize::Zeroizing<String>,
    unlocked: Unlocked,
}

pub enum PasswordOutcome {
    SignedIn(Unlocked),
    NeedsSecondFactor(SecondFactor),
}

/// Result of an admin reset.
pub struct Reset {
    pub invite: Invite,
    /// `false` when the old data could not be kept and the caller must delete the data file.
    pub data_kept: bool,
}

#[derive(Debug, Serialize)]
pub struct TotpEnrollment {
    pub secret_base32: String,
    pub otpauth_url: String,
    pub qr_png_base64: String,
}

pub struct Registry {
    conn: Connection,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn check_password_rules(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(Error::Invalid(format!("password must be at least {MIN_PASSWORD_LEN} characters")));
    }
    Ok(())
}

fn check_name(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 40 {
        return Err(Error::Invalid("name must be 1–40 characters".into()));
    }
    Ok(n)
}

fn grouped(code: &str, size: usize) -> String {
    code.as_bytes().chunks(size).map(|c| std::str::from_utf8(c).unwrap_or_default()).collect::<Vec<_>>().join("-")
}

fn normalize_code(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase()
}

fn build_totp(secret_base32: &str, account: &str) -> Result<Totp> {
    let secret = Secret::try_from_base32(secret_base32).map_err(|e| Error::Invalid(format!("bad TOTP secret: {e:?}")))?;
    Builder::new()
        .with_secret(secret)
        .with_account_name(account)
        .with_issuer(Some(ISSUER))
        .build()
        .map_err(|e| Error::Invalid(format!("TOTP setup failed: {e:?}")))
}

impl Registry {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self { conn: crate::db::open_with(path, crate::db::REGISTRY_MIGRATIONS)? })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self { conn: crate::db::memory_with(crate::db::REGISTRY_MIGRATIONS)? })
    }

    // ---------------------------------------------------------------- profiles

    pub fn list(&self) -> Result<Vec<UserSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, role, password_hash IS NOT NULL, totp_enabled, biometric_enabled, senior
             FROM users ORDER BY role = 'admin' DESC, created_at, rowid",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(UserSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                role: Role::parse(&r.get::<_, String>(3)?).unwrap_or(Role::Member),
                active: r.get(4)?,
                totp_enabled: r.get(5)?,
                biometric_enabled: r.get(6)?,
                senior: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get(&self, user_id: &str) -> Result<UserSummary> {
        self.list()?
            .into_iter()
            .find(|u| u.id == user_id)
            .ok_or_else(|| Error::Invalid("no such user".into()))
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0))? == 0)
    }

    /// First run only: the person installing Vaulty becomes the admin. Creates their data key,
    /// the family store key and a recovery key (returned in `new_recovery_key`).
    pub fn create_first_admin(&self, name: &str, color: &str, password: &str) -> Result<Unlocked> {
        if !self.is_empty()? {
            return Err(Error::Auth("an admin already exists; ask them to add you".into()));
        }
        check_password_rules(password)?;
        let id = crate::rows::new_id();
        let dk = crypto::new_key();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO users (id, name, color, role, password_hash) VALUES (?, ?, ?, 'admin', ?)",
            params![id, check_name(name)?, color, crypto::hash_secret(password)?],
        )?;
        let public = self.new_keypair(&id, &dk)?;
        self.wrap_with_secret(&id, "password", password, &dk)?;
        let recovery = self.new_recovery(&id, &dk)?;
        let hk = crypto::new_key();
        self.conn.execute("INSERT INTO household_keys (user_id, sealed) VALUES (?, ?)", params![id, crypto::seal_to(&public, &hk)?])?;
        tx.commit()?;
        let mut u = self.unlocked(&id, dk)?;
        u.new_recovery_key = Some(recovery);
        Ok(u)
    }

    /// Admin adds a family member. They choose their own password later, using the setup code.
    /// The new member's data key is created now, wrapped under the setup code, so the admin can
    /// share the family store key with them (and, for a child, keep a parent's recovery copy).
    pub fn add_user(&self, admin: &Unlocked, name: &str, color: &str, role: Role) -> Result<Invite> {
        self.require_admin(&admin.user_id)?;
        let id = crate::rows::new_id();
        let code = crypto::random_code(8, CODE_ALPHABET);
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO users (id, name, color, role, invite_hash, invite_expires_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![id, check_name(name)?, color, role.as_str(), crypto::hash_secret(&code)?, now() + INVITE_TTL_SECS],
        )?;
        self.issue_fresh_keys(&id, &code, role, admin)?;
        tx.commit()?;
        Ok(Invite { user: self.get(&id)?, setup_code: grouped(&code, 4) })
    }

    /// New member claims their profile with the setup code and picks a password.
    /// Returns their keys, including a new recovery key to show once.
    pub fn claim_profile(&self, user_id: &str, setup_code: &str, password: &str) -> Result<Unlocked> {
        self.check_throttle(user_id)?;
        let (hash, expires, has_pw): (Option<String>, Option<i64>, bool) = self.conn.query_row(
            "SELECT invite_hash, invite_expires_at, password_hash IS NOT NULL FROM users WHERE id = ?",
            [user_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if has_pw {
            return Err(Error::Auth("this profile is already set up".into()));
        }
        let Some(hash) = hash else { return Err(Error::Auth("no setup code issued; ask an admin".into())) };
        if expires.unwrap_or(0) < now() {
            return Err(Error::Auth("setup code expired; ask an admin for a new one".into()));
        }
        let code = normalize_code(setup_code);
        if !crypto::verify_secret(&code, &hash) {
            self.register_failure(user_id)?;
            return Err(Error::Auth("incorrect setup code".into()));
        }
        check_password_rules(password)?;
        let tx = self.conn.unchecked_transaction()?;
        let dk = match self.unwrap_with_secret(user_id, "setup", &code)? {
            Some(dk) => dk,
            None => {
                // Invited before encryption existed: start with fresh keys.
                let dk = crypto::new_key();
                self.new_keypair(user_id, &dk)?;
                dk
            }
        };
        self.wrap_with_secret(user_id, "password", password, &dk)?;
        self.conn.execute("DELETE FROM user_keys WHERE user_id = ? AND kind = 'setup'", [user_id])?;
        let recovery = self.new_recovery(user_id, &dk)?;
        self.conn.execute(
            "UPDATE users SET password_hash = ?, kdf_salt = NULL, invite_hash = NULL, invite_expires_at = NULL,
                    failed_attempts = 0, locked_until = 0, last_login_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![crypto::hash_secret(password)?, user_id],
        )?;
        tx.commit()?;
        let mut u = self.unlocked(user_id, dk)?;
        u.new_recovery_key = Some(recovery);
        Ok(u)
    }

    /// Admin helps a Child who forgot their password. A parent holds a recovery copy of each
    /// child's data key, so the child's data is kept: they get a new setup code and choose a new
    /// password. 2FA and biometric unlock are turned off.
    ///
    /// Adults can't be reset this way: their data is readable only with their own password or
    /// recovery key. See `erase_and_reset`.
    pub fn reset_user(&self, admin: &Unlocked, user_id: &str) -> Result<Reset> {
        self.require_admin(&admin.user_id)?;
        if admin.user_id == user_id {
            return Err(Error::Invalid("use Change password for your own profile".into()));
        }
        if self.get(user_id)?.role != Role::Child {
            return Err(Error::Auth("adults recover with their own recovery key; to start their profile over, use Erase and reset".into()));
        }
        let sealed: Option<String> = self.conn.query_row(
            "SELECT wrapped FROM user_keys WHERE user_id = ? AND kind = 'guardian' AND holder = ?",
            [user_id, &admin.user_id], |r| r.get(0)).optional()?;
        let Some(sealed) = sealed else {
            return Err(Error::Auth("you don't hold a recovery copy for this profile yet; it's added the next time they sign in. Use Erase and reset to start over".into()));
        };
        let dk = crypto::open_sealed(&admin.secret_key, &sealed)?;
        let code = crypto::random_code(8, CODE_ALPHABET);
        let tx = self.conn.unchecked_transaction()?;
        self.conn.execute("DELETE FROM user_keys WHERE user_id = ? AND kind IN ('password', 'recovery', 'biometric', 'setup')", [user_id])?;
        self.wrap_with_secret(user_id, "setup", &code, &dk)?;
        self.clear_sign_in(user_id, &code)?;
        tx.commit()?;
        Ok(Reset { invite: Invite { user: self.get(user_id)?, setup_code: grouped(&code, 4) }, data_kept: true })
    }

    /// Starts a member's profile over with empty data and a new setup code. The caller must
    /// delete their data file (`data_kept` is false). For adults who lost both their password
    /// and recovery key, or a child whose recovery copy is missing.
    pub fn erase_and_reset(&self, admin: &Unlocked, user_id: &str) -> Result<Reset> {
        self.require_admin(&admin.user_id)?;
        if admin.user_id == user_id {
            return Err(Error::Invalid("you can't erase your own profile while signed in".into()));
        }
        let role = self.get(user_id)?.role;
        let code = crypto::random_code(8, CODE_ALPHABET);
        let tx = self.conn.unchecked_transaction()?;
        self.conn.execute("DELETE FROM user_keys WHERE user_id = ?", [user_id])?;
        // Copies sealed to their old key pair are useless now; children re-share on next sign-in.
        self.conn.execute("DELETE FROM user_keys WHERE kind = 'guardian' AND holder = ?", [user_id])?;
        self.conn.execute("DELETE FROM household_keys WHERE user_id = ?", [user_id])?;
        self.issue_fresh_keys(user_id, &code, role, admin)?;
        self.clear_sign_in(user_id, &code)?;
        tx.commit()?;
        Ok(Reset { invite: Invite { user: self.get(user_id)?, setup_code: grouped(&code, 4) }, data_kept: false })
    }

    /// Removes the profile. The caller deletes the member's data file and keychain entry.
    pub fn remove_user(&self, admin_id: &str, user_id: &str) -> Result<()> {
        self.require_admin(admin_id)?;
        let target = self.get(user_id)?;
        if target.role == Role::Admin {
            let admins: i64 = self.conn.query_row("SELECT COUNT(*) FROM users WHERE role = 'admin'", [], |r| r.get(0))?;
            if admins <= 1 {
                return Err(Error::Invalid("can't remove the only admin".into()));
            }
        }
        // Keys held *by* this member for others (guardian copies) go too.
        self.conn.execute("DELETE FROM user_keys WHERE kind = 'guardian' AND holder = ?", [user_id])?;
        self.conn.execute("DELETE FROM users WHERE id = ?", [user_id])?;
        Ok(())
    }

    pub fn update_profile(&self, user_id: &str, name: &str, color: &str) -> Result<UserSummary> {
        self.conn.execute("UPDATE users SET name = ?, color = ? WHERE id = ?", params![check_name(name)?, color, user_id])?;
        self.get(user_id)
    }

    /// Turns senior mode on or off. Anyone can change their own; the admin can set it up for others.
    pub fn set_senior(&self, by: &str, user_id: &str, on: bool) -> Result<UserSummary> {
        if by != user_id { self.require_admin(by)?; }
        let n = self.conn.execute("UPDATE users SET senior = ? WHERE id = ?", params![on, user_id])?;
        if n == 0 { return Err(Error::Invalid("no such user".into())); }
        self.get(user_id)
    }

    fn require_admin(&self, user_id: &str) -> Result<()> {
        if self.get(user_id)?.role != Role::Admin {
            return Err(Error::Auth("only an admin can do that".into()));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- sign-in

    pub fn sign_in_with_password(&self, user_id: &str, password: &str) -> Result<PasswordOutcome> {
        self.verify_password(user_id, password)?;
        let (dk, recovery) = match self.unwrap_with_secret(user_id, "password", password)? {
            Some(dk) => (dk, None),
            None => self.upgrade_legacy(user_id, password)?,
        };
        let (totp_on, sealed): (bool, Option<String>) = self.conn.query_row(
            "SELECT totp_enabled, totp_secret_sealed FROM users WHERE id = ?", [user_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut unlocked = self.unlocked(user_id, dk)?;
        unlocked.new_recovery_key = recovery;
        match (totp_on, sealed) {
            (true, Some(sealed)) => {
                let secret = String::from_utf8(crypto::open(&unlocked.data_key, &sealed)?).map_err(|_| Error::Auth("corrupt 2FA secret".into()))?;
                Ok(PasswordOutcome::NeedsSecondFactor(SecondFactor { user_id: user_id.into(), secret_base32: secret.into(), unlocked }))
            }
            _ => {
                self.record_success(user_id)?;
                Ok(PasswordOutcome::SignedIn(unlocked))
            }
        }
    }

    /// Second step: a 6-digit code from the authenticator app.
    pub fn complete_with_totp(&self, pending: &SecondFactor, code: &str) -> Result<Unlocked> {
        self.check_throttle(&pending.user_id)?;
        let name = self.get(&pending.user_id)?.name;
        let totp = build_totp(&pending.secret_base32, &name)?;
        let last: i64 = self.conn.query_row("SELECT totp_last_step FROM users WHERE id = ?", [&pending.user_id], |r| r.get(0))?;
        match totp.check_current(code.trim()) {
            Some(step) if step as i64 > last => {
                self.conn.execute("UPDATE users SET totp_last_step = ? WHERE id = ?", params![step as i64, pending.user_id])?;
                self.record_success(&pending.user_id)?;
                Ok(pending.unlocked.clone())
            }
            Some(_) => Err(Error::Auth("that code was already used; wait for the next one".into())),
            None => {
                self.register_failure(&pending.user_id)?;
                Err(Error::Auth("incorrect code".into()))
            }
        }
    }

    /// Second step alternative: a one-time backup code.
    pub fn complete_with_backup_code(&self, pending: &SecondFactor, code: &str) -> Result<Unlocked> {
        self.check_throttle(&pending.user_id)?;
        let used = self.conn.execute(
            "UPDATE backup_codes SET used_at = CURRENT_TIMESTAMP WHERE user_id = ? AND code_hash = ? AND used_at IS NULL",
            params![pending.user_id, crypto::sha256_hex(normalize_code(code).as_bytes())],
        )?;
        if used == 1 {
            self.record_success(&pending.user_id)?;
            Ok(pending.unlocked.clone())
        } else {
            self.register_failure(&pending.user_id)?;
            Err(Error::Auth("invalid or already-used backup code".into()))
        }
    }

    /// Biometric sign-in: the shell has already shown the OS prompt and read the device token
    /// from the keychain. The token unwraps the data key; biometrics replace password and 2FA.
    pub fn sign_in_with_biometric_token(&self, user_id: &str, token: &str) -> Result<Unlocked> {
        self.check_throttle(user_id)?;
        let fail = || -> Result<Unlocked> {
            self.register_failure(user_id)?;
            Err(Error::Auth("biometric unlock is not set up on this device".into()))
        };
        let on: bool = self.conn.query_row("SELECT biometric_enabled FROM users WHERE id = ?", [user_id], |r| r.get(0))?;
        let wrapped: Option<String> = self.conn.query_row(
            "SELECT wrapped FROM user_keys WHERE user_id = ? AND kind = 'biometric'", [user_id], |r| r.get(0)).optional()?;
        let (true, Some(wrapped)) = (on, wrapped) else { return fail() };
        let Some(token_key) = token_to_key(token) else { return fail() };
        match crypto::unwrap(&token_key, &wrapped) {
            Ok(dk) => {
                self.record_success(user_id)?;
                self.unlocked(user_id, dk)
            }
            Err(_) => fail(),
        }
    }

    /// Forgot password: the member's recovery key unlocks their data, then they choose a new
    /// password. The recovery key is replaced (returned in `new_recovery_key`); 2FA and
    /// biometric unlock stay as they were.
    pub fn sign_in_with_recovery_key(&self, user_id: &str, recovery_key: &str, new_password: &str) -> Result<Unlocked> {
        self.check_throttle(user_id)?;
        if !self.get(user_id)?.active {
            return Err(Error::Auth("this profile hasn't been set up yet".into()));
        }
        let dk = match self.unwrap_with_secret(user_id, "recovery", &normalize_code(recovery_key)) {
            Ok(Some(dk)) => dk,
            _ => {
                self.register_failure(user_id)?;
                return Err(Error::Auth("that recovery key doesn't match this profile".into()));
            }
        };
        check_password_rules(new_password)?;
        let tx = self.conn.unchecked_transaction()?;
        self.wrap_with_secret(user_id, "password", new_password, &dk)?;
        self.conn.execute("UPDATE users SET password_hash = ? WHERE id = ?", params![crypto::hash_secret(new_password)?, user_id])?;
        let recovery = self.new_recovery(user_id, &dk)?;
        tx.commit()?;
        self.record_success(user_id)?;
        let mut u = self.unlocked(user_id, dk)?;
        u.new_recovery_key = Some(recovery);
        Ok(u)
    }

    /// Run at the start of every session. Gives the family store key to members who don't have
    /// it yet, keeps parents' recovery copies of a child's data key up to date, and creates the
    /// family store key on first use.
    pub fn sync_keys(&self, me: &mut Unlocked) -> Result<()> {
        let role = self.get(&me.user_id)?.role;
        let tx = self.conn.unchecked_transaction()?;
        if me.household_key.is_none() {
            let anyone: i64 = self.conn.query_row("SELECT COUNT(*) FROM household_keys", [], |r| r.get(0))?;
            if anyone == 0 {
                let hk = crypto::new_key();
                let public = self.public_key(&me.user_id)?;
                self.conn.execute("INSERT INTO household_keys (user_id, sealed) VALUES (?, ?)", params![me.user_id, crypto::seal_to(&public, &hk)?])?;
                me.household_key = Some(hk);
            }
        }
        if let Some(hk) = &me.household_key {
            let missing: Vec<(String, String)> = self.conn.prepare(
                "SELECT id, public_key FROM users WHERE public_key IS NOT NULL AND id NOT IN (SELECT user_id FROM household_keys)")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
            for (id, public) in missing {
                self.conn.execute("INSERT INTO household_keys (user_id, sealed) VALUES (?, ?)", params![id, crypto::seal_to(&public, hk)?])?;
            }
        }
        if role == Role::Child {
            self.add_guardian_copies(&me.user_id, &me.data_key)?;
        } else {
            self.conn.execute("DELETE FROM user_keys WHERE user_id = ? AND kind = 'guardian'", [&me.user_id])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---------------------------------------------------------------- security settings
    // Every change here re-checks the password, even inside an active session.

    pub fn change_password(&self, user_id: &str, current: &str, new: &str) -> Result<()> {
        let dk = self.data_key_from_password(user_id, current)?;
        check_password_rules(new)?;
        let tx = self.conn.unchecked_transaction()?;
        self.wrap_with_secret(user_id, "password", new, &dk)?;
        self.conn.execute("UPDATE users SET password_hash = ? WHERE id = ?", params![crypto::hash_secret(new)?, user_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Replaces the recovery key (e.g. if the printed copy was lost). Returns it to show once.
    pub fn new_recovery_key(&self, user_id: &str, password: &str) -> Result<String> {
        let dk = self.data_key_from_password(user_id, password)?;
        self.new_recovery(user_id, &dk)
    }

    /// Step 1 of 2FA setup: returns a fresh secret + QR. Nothing is saved yet.
    pub fn begin_totp(&self, user_id: &str, password: &str) -> Result<TotpEnrollment> {
        self.verify_password(user_id, password)?;
        let name = self.get(user_id)?.name;
        let secret = Secret::generate().to_base32();
        let totp = build_totp(&secret, &name)?;
        Ok(TotpEnrollment {
            otpauth_url: totp.to_url().map_err(|e| Error::Invalid(format!("{e:?}")))?,
            qr_png_base64: totp.to_qr_base64().map_err(|e| Error::Invalid(format!("{e:?}")))?,
            secret_base32: secret,
        })
    }

    /// Step 2: the user proves the app is set up by entering a code. Returns backup codes (shown once).
    /// The 2FA secret is sealed under the data key, so it survives password changes and recovery.
    pub fn confirm_totp(&self, user_id: &str, password: &str, secret_base32: &str, code: &str) -> Result<Vec<String>> {
        let dk = self.data_key_from_password(user_id, password)?;
        let name = self.get(user_id)?.name;
        let step = build_totp(secret_base32, &name)?
            .check_current(code.trim())
            .ok_or_else(|| Error::Auth("that code didn't match; check the time on your phone and try again".into()))?;
        let sealed = crypto::seal(&dk, secret_base32.as_bytes())?;
        self.conn.execute(
            "UPDATE users SET totp_enabled = 1, totp_secret_sealed = ?, totp_last_step = ? WHERE id = ?",
            params![sealed, step as i64, user_id],
        )?;
        self.new_backup_codes(user_id)
    }

    pub fn disable_totp(&self, user_id: &str, password: &str) -> Result<()> {
        self.verify_password(user_id, password)?;
        self.conn.execute(
            "UPDATE users SET totp_enabled = 0, totp_secret_sealed = NULL, totp_last_step = 0 WHERE id = ?",
            [user_id],
        )?;
        self.conn.execute("DELETE FROM backup_codes WHERE user_id = ?", [user_id])?;
        Ok(())
    }

    pub fn regenerate_backup_codes(&self, user_id: &str, password: &str) -> Result<Vec<String>> {
        self.verify_password(user_id, password)?;
        if !self.get(user_id)?.totp_enabled {
            return Err(Error::Invalid("turn on two-factor authentication first".into()));
        }
        self.new_backup_codes(user_id)
    }

    pub fn backup_codes_remaining(&self, user_id: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM backup_codes WHERE user_id = ? AND used_at IS NULL",
            [user_id],
            |r| r.get(0),
        )?)
    }

    /// Returns a fresh device token for the OS keychain. The token wraps the data key, so it is
    /// the only thing (besides the password and recovery key) that can open this member's data.
    /// Re-checks a signed-in member's password (e.g. to open the Hidden section). Wrong guesses
    /// count towards the same lockout as signing in.
    pub fn confirm_password(&self, user_id: &str, password: &str) -> Result<()> {
        self.verify_password(user_id, password)
    }

    pub fn enable_biometric(&self, user_id: &str, password: &str) -> Result<String> {
        let dk = self.data_key_from_password(user_id, password)?;
        let token = crypto::new_key();
        self.conn.execute(
            "INSERT OR REPLACE INTO user_keys (user_id, kind, holder, wrapped) VALUES (?, 'biometric', '', ?)",
            params![user_id, crypto::wrap(&token, &dk)?],
        )?;
        self.conn.execute("UPDATE users SET biometric_enabled = 1, biometric_token_hash = NULL WHERE id = ?", [user_id])?;
        Ok(crypto::to_hex(token.as_ref()))
    }

    pub fn disable_biometric(&self, user_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM user_keys WHERE user_id = ? AND kind = 'biometric'", [user_id])?;
        self.conn.execute("UPDATE users SET biometric_enabled = 0, biometric_token_hash = NULL WHERE id = ?", [user_id])?;
        Ok(())
    }

    // ---------------------------------------------------------------- keys (internal)

    fn unlocked(&self, user_id: &str, data_key: Key) -> Result<Unlocked> {
        let sealed: Option<String> = self.conn.query_row("SELECT secret_key_sealed FROM users WHERE id = ?", [user_id], |r| r.get(0))?;
        let secret_key = match sealed {
            Some(s) => crypto::unwrap(&data_key, &s)?,
            None => return Err(Error::Auth("this profile's keys are missing".into())),
        };
        let hk_sealed: Option<String> = self.conn.query_row("SELECT sealed FROM household_keys WHERE user_id = ?", [user_id], |r| r.get(0)).optional()?;
        let household_key = hk_sealed.map(|s| crypto::open_sealed(&secret_key, &s)).transpose()?;
        Ok(Unlocked { user_id: user_id.into(), data_key, secret_key, household_key, new_recovery_key: None })
    }

    /// Creates the member's key pair; returns the public key.
    fn new_keypair(&self, user_id: &str, dk: &Key) -> Result<String> {
        let (public, secret) = crypto::new_keypair();
        self.conn.execute("UPDATE users SET public_key = ?, secret_key_sealed = ? WHERE id = ?",
            params![public, crypto::wrap(dk, &secret)?, user_id])?;
        Ok(public)
    }

    fn public_key(&self, user_id: &str) -> Result<String> {
        self.conn.query_row("SELECT public_key FROM users WHERE id = ?", [user_id], |r| r.get::<_, Option<String>>(0))?
            .ok_or_else(|| Error::Auth("this profile's keys are missing".into()))
    }

    /// New data key and key pair, wrapped under a setup code; shares the family store key and,
    /// for a child, gives every parent a recovery copy.
    fn issue_fresh_keys(&self, user_id: &str, code: &str, role: Role, admin: &Unlocked) -> Result<()> {
        let dk = crypto::new_key();
        let public = self.new_keypair(user_id, &dk)?;
        self.wrap_with_secret(user_id, "setup", code, &dk)?;
        if let Some(hk) = &admin.household_key {
            self.conn.execute("INSERT OR REPLACE INTO household_keys (user_id, sealed) VALUES (?, ?)", params![user_id, crypto::seal_to(&public, hk)?])?;
        }
        if role == Role::Child {
            self.add_guardian_copies(user_id, &dk)?;
        }
        Ok(())
    }

    fn add_guardian_copies(&self, child_id: &str, dk: &Key) -> Result<()> {
        let admins: Vec<(String, String)> = self.conn.prepare(
            "SELECT id, public_key FROM users WHERE role = 'admin' AND public_key IS NOT NULL AND id <> ?1
               AND id NOT IN (SELECT holder FROM user_keys WHERE user_id = ?1 AND kind = 'guardian')")?
            .query_map([child_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        for (admin_id, public) in admins {
            self.conn.execute("INSERT INTO user_keys (user_id, kind, holder, wrapped) VALUES (?, 'guardian', ?, ?)",
                params![child_id, admin_id, crypto::seal_to(&public, dk)?])?;
        }
        Ok(())
    }

    fn wrap_with_secret(&self, user_id: &str, kind: &str, secret: &str, dk: &Key) -> Result<()> {
        let salt = crypto::new_salt();
        let wrapped = crypto::wrap(&crypto::derive_key(secret, &salt)?, dk)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO user_keys (user_id, kind, holder, salt, wrapped) VALUES (?, ?, '', ?, ?)",
            params![user_id, kind, salt, wrapped],
        )?;
        Ok(())
    }

    /// `Ok(None)` when no such wrap exists; an error when it exists but the secret is wrong.
    fn unwrap_with_secret(&self, user_id: &str, kind: &str, secret: &str) -> Result<Option<Key>> {
        let row: Option<(Option<String>, String)> = self.conn.query_row(
            "SELECT salt, wrapped FROM user_keys WHERE user_id = ? AND kind = ? AND holder = ''",
            [user_id, kind], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let Some((Some(salt), wrapped)) = row else { return Ok(None) };
        Ok(Some(crypto::unwrap(&crypto::derive_key(secret, &salt)?, &wrapped)?))
    }

    fn data_key_from_password(&self, user_id: &str, password: &str) -> Result<Key> {
        self.verify_password(user_id, password)?;
        self.unwrap_with_secret(user_id, "password", password)?
            .ok_or_else(|| Error::Auth("sign out and in again to finish upgrading this profile".into()))
    }

    fn new_recovery(&self, user_id: &str, dk: &Key) -> Result<String> {
        let code = crypto::random_code(RECOVERY_LEN, CODE_ALPHABET);
        self.wrap_with_secret(user_id, "recovery", &code, dk)?;
        Ok(grouped(&code, 4))
    }

    /// Profiles created before encryption: create their keys on the first password sign-in.
    /// The caller encrypts the existing plain data file with the returned key.
    fn upgrade_legacy(&self, user_id: &str, password: &str) -> Result<(Key, Option<String>)> {
        let dk = crypto::new_key();
        let tx = self.conn.unchecked_transaction()?;
        self.new_keypair(user_id, &dk)?;
        self.wrap_with_secret(user_id, "password", password, &dk)?;
        let recovery = self.new_recovery(user_id, &dk)?;
        let (sealed, salt): (Option<String>, Option<String>) =
            self.conn.query_row("SELECT totp_secret_sealed, kdf_salt FROM users WHERE id = ?", [user_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if let (Some(s), Some(salt)) = (sealed, salt) {
            let plain = zeroize::Zeroizing::new(crypto::open(&crypto::derive_key_legacy(password, &salt)?, &s)?);
            self.conn.execute("UPDATE users SET totp_secret_sealed = ? WHERE id = ?", params![crypto::seal(&dk, &plain)?, user_id])?;
        }
        // Old biometric tokens didn't wrap a key; ask for biometric unlock to be turned on again.
        self.conn.execute("UPDATE users SET kdf_salt = NULL, biometric_enabled = 0, biometric_token_hash = NULL WHERE id = ?", [user_id])?;
        tx.commit()?;
        Ok((dk, Some(recovery)))
    }

    fn clear_sign_in(&self, user_id: &str, code: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE users SET password_hash = NULL, kdf_salt = NULL, totp_enabled = 0, totp_secret_sealed = NULL,
                    totp_last_step = 0, biometric_enabled = 0, biometric_token_hash = NULL, failed_attempts = 0,
                    locked_until = 0, invite_hash = ?, invite_expires_at = ? WHERE id = ?",
            params![crypto::hash_secret(code)?, now() + INVITE_TTL_SECS, user_id],
        )?;
        self.conn.execute("DELETE FROM backup_codes WHERE user_id = ?", [user_id])?;
        Ok(())
    }

    // ---------------------------------------------------------------- internals

    fn verify_password(&self, user_id: &str, password: &str) -> Result<()> {
        self.check_throttle(user_id)?;
        let hash: Option<String> = self
            .conn
            .query_row("SELECT password_hash FROM users WHERE id = ?", [user_id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::Auth("no such user".into()))?;
        let Some(hash) = hash else { return Err(Error::Auth("this profile hasn't been set up yet".into())) };
        if crypto::verify_secret(password, &hash) {
            // Hashes made before the stronger Argon2 settings are upgraded on the next success.
            if !crypto::is_current_hash(&hash) {
                self.conn.execute("UPDATE users SET password_hash = ? WHERE id = ?", params![crypto::hash_secret(password)?, user_id])?;
            }
            Ok(())
        } else {
            self.register_failure(user_id)?;
            Err(Error::Auth("incorrect password".into()))
        }
    }

    fn new_backup_codes(&self, user_id: &str) -> Result<Vec<String>> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM backup_codes WHERE user_id = ?", [user_id])?;
        let mut codes = Vec::with_capacity(BACKUP_CODE_COUNT);
        for _ in 0..BACKUP_CODE_COUNT {
            let c = crypto::random_code(8, CODE_ALPHABET);
            tx.execute(
                "INSERT INTO backup_codes (user_id, code_hash) VALUES (?, ?)",
                params![user_id, crypto::sha256_hex(c.as_bytes())],
            )?;
            codes.push(grouped(&c, 4));
        }
        tx.commit()?;
        Ok(codes)
    }

    fn check_throttle(&self, user_id: &str) -> Result<()> {
        let until: i64 = self
            .conn
            .query_row("SELECT locked_until FROM users WHERE id = ?", [user_id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::Auth("no such user".into()))?;
        let wait = until - now();
        if wait > 0 { Err(Error::Throttled(wait)) } else { Ok(()) }
    }

    /// After 5 misses, each further miss locks the profile for 30s, 60s, 120s… up to 15 minutes.
    fn register_failure(&self, user_id: &str) -> Result<()> {
        let n: i64 = self.conn.query_row(
            "UPDATE users SET failed_attempts = failed_attempts + 1 WHERE id = ? RETURNING failed_attempts",
            [user_id],
            |r| r.get(0),
        )?;
        if n >= MAX_FREE_ATTEMPTS {
            let secs = (30i64 << (n - MAX_FREE_ATTEMPTS).min(10)).min(MAX_LOCKOUT_SECS);
            self.conn.execute("UPDATE users SET locked_until = ? WHERE id = ?", params![now() + secs, user_id])?;
        }
        Ok(())
    }

    fn record_success(&self, user_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE users SET failed_attempts = 0, locked_until = 0, last_login_at = CURRENT_TIMESTAMP WHERE id = ?",
            [user_id],
        )?;
        Ok(())
    }

    #[doc(hidden)]
    pub fn conn_for_tests(&self) -> &Connection {
        &self.conn
    }
}

fn token_to_key(token: &str) -> Option<Key> {
    let bytes = zeroize::Zeroizing::new(crypto::from_hex(token.trim()).ok()?);
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(zeroize::Zeroizing::new(arr))
}

impl SecondFactor {
    #[doc(hidden)]
    pub fn secret_for_tests(&self) -> &str {
        &self.secret_base32
    }
}

impl Unlocked {
    #[doc(hidden)]
    pub fn secret_key_for_tests(&self) -> &Key {
        &self.secret_key
    }
}
