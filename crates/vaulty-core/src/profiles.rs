//! Family members on this device: profiles, passwords, authenticator-app 2FA,
//! backup codes and biometric unlock tokens.
//!
//! The registry is separate from each user's finance database, so one member
//! can never open another member's data by being signed in.
use crate::crypto;
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
}

/// Returned to the admin once, to hand to the new member.
#[derive(Debug, Serialize)]
pub struct Invite {
    pub user: UserSummary,
    pub setup_code: String,
}

/// Password accepted, but an authenticator code is still required.
/// The shell keeps this in memory only, never on disk.
pub struct SecondFactor {
    pub user_id: String,
    secret_base32: String,
}

pub enum PasswordOutcome {
    SignedIn,
    NeedsSecondFactor(SecondFactor),
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
            "SELECT id, name, color, role, password_hash IS NOT NULL, totp_enabled, biometric_enabled
             FROM users ORDER BY role = 'admin' DESC, created_at",
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

    /// First run only: the person installing Vaulty becomes the admin.
    pub fn create_first_admin(&self, name: &str, color: &str, password: &str) -> Result<UserSummary> {
        if !self.is_empty()? {
            return Err(Error::Auth("an admin already exists; ask them to add you".into()));
        }
        check_password_rules(password)?;
        let id = crate::rows::new_id();
        self.conn.execute(
            "INSERT INTO users (id, name, color, role, password_hash, kdf_salt) VALUES (?, ?, ?, 'admin', ?, ?)",
            params![id, check_name(name)?, color, crypto::hash_secret(password)?, crypto::to_hex(&crypto::random_bytes::<16>())],
        )?;
        self.get(&id)
    }

    /// Admin adds a family member. They choose their own password later, using the setup code.
    pub fn add_user(&self, admin_id: &str, name: &str, color: &str, role: Role) -> Result<Invite> {
        self.require_admin(admin_id)?;
        let id = crate::rows::new_id();
        let code = crypto::random_code(8, CODE_ALPHABET);
        self.conn.execute(
            "INSERT INTO users (id, name, color, role, invite_hash, invite_expires_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![id, check_name(name)?, color, role.as_str(), crypto::hash_secret(&code)?, now() + INVITE_TTL_SECS],
        )?;
        Ok(Invite { user: self.get(&id)?, setup_code: format!("{}-{}", &code[..4], &code[4..]) })
    }

    /// New member claims their profile with the setup code and picks a password.
    pub fn claim_profile(&self, user_id: &str, setup_code: &str, password: &str) -> Result<()> {
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
        if !crypto::verify_secret(&normalize_code(setup_code), &hash) {
            self.register_failure(user_id)?;
            return Err(Error::Auth("incorrect setup code".into()));
        }
        check_password_rules(password)?;
        self.conn.execute(
            "UPDATE users SET password_hash = ?, kdf_salt = ?, invite_hash = NULL, invite_expires_at = NULL,
                    failed_attempts = 0, locked_until = 0, last_login_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![crypto::hash_secret(password)?, crypto::to_hex(&crypto::random_bytes::<16>()), user_id],
        )?;
        Ok(())
    }

    /// Admin helps a member who forgot their password: clears password, 2FA and
    /// biometrics and issues a new setup code. (Data stays readable today; once
    /// database encryption lands this will require the member's recovery key.)
    pub fn reset_user(&self, admin_id: &str, user_id: &str) -> Result<Invite> {
        self.require_admin(admin_id)?;
        if admin_id == user_id {
            return Err(Error::Invalid("use Change password for your own profile".into()));
        }
        let code = crypto::random_code(8, CODE_ALPHABET);
        self.conn.execute(
            "UPDATE users SET password_hash = NULL, kdf_salt = NULL, totp_enabled = 0, totp_secret_sealed = NULL,
                    totp_last_step = 0, biometric_enabled = 0, biometric_token_hash = NULL, failed_attempts = 0,
                    locked_until = 0, invite_hash = ?, invite_expires_at = ? WHERE id = ?",
            params![crypto::hash_secret(&code)?, now() + INVITE_TTL_SECS, user_id],
        )?;
        self.conn.execute("DELETE FROM backup_codes WHERE user_id = ?", [user_id])?;
        Ok(Invite { user: self.get(user_id)?, setup_code: format!("{}-{}", &code[..4], &code[4..]) })
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
        self.conn.execute("DELETE FROM users WHERE id = ?", [user_id])?;
        Ok(())
    }

    pub fn update_profile(&self, user_id: &str, name: &str, color: &str) -> Result<UserSummary> {
        self.conn.execute("UPDATE users SET name = ?, color = ? WHERE id = ?", params![check_name(name)?, color, user_id])?;
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
        let (totp_on, sealed, salt): (bool, Option<String>, Option<String>) = self.conn.query_row(
            "SELECT totp_enabled, totp_secret_sealed, kdf_salt FROM users WHERE id = ?",
            [user_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        match (totp_on, sealed, salt) {
            (true, Some(sealed), Some(salt)) => {
                let key = crypto::derive_key(password, &salt)?;
                let secret = String::from_utf8(crypto::open(&key, &sealed)?).map_err(|_| Error::Auth("corrupt 2FA secret".into()))?;
                Ok(PasswordOutcome::NeedsSecondFactor(SecondFactor { user_id: user_id.into(), secret_base32: secret }))
            }
            _ => {
                self.record_success(user_id)?;
                Ok(PasswordOutcome::SignedIn)
            }
        }
    }

    /// Second step: a 6-digit code from the authenticator app.
    pub fn complete_with_totp(&self, pending: &SecondFactor, code: &str) -> Result<()> {
        self.check_throttle(&pending.user_id)?;
        let name = self.get(&pending.user_id)?.name;
        let totp = build_totp(&pending.secret_base32, &name)?;
        let last: i64 = self.conn.query_row("SELECT totp_last_step FROM users WHERE id = ?", [&pending.user_id], |r| r.get(0))?;
        match totp.check_current(code.trim()) {
            Some(step) if step as i64 > last => {
                self.conn.execute("UPDATE users SET totp_last_step = ? WHERE id = ?", params![step as i64, pending.user_id])?;
                self.record_success(&pending.user_id)
            }
            Some(_) => Err(Error::Auth("that code was already used; wait for the next one".into())),
            None => {
                self.register_failure(&pending.user_id)?;
                Err(Error::Auth("incorrect code".into()))
            }
        }
    }

    /// Second step alternative: a one-time backup code.
    pub fn complete_with_backup_code(&self, pending: &SecondFactor, code: &str) -> Result<()> {
        self.check_throttle(&pending.user_id)?;
        let used = self.conn.execute(
            "UPDATE backup_codes SET used_at = CURRENT_TIMESTAMP WHERE user_id = ? AND code_hash = ? AND used_at IS NULL",
            params![pending.user_id, crypto::sha256_hex(normalize_code(code).as_bytes())],
        )?;
        if used == 1 {
            self.record_success(&pending.user_id)
        } else {
            self.register_failure(&pending.user_id)?;
            Err(Error::Auth("invalid or already-used backup code".into()))
        }
    }

    /// Biometric sign-in: the shell has already shown the OS prompt and read the
    /// device token from the keychain. Biometrics replace both password and 2FA on this device.
    pub fn sign_in_with_biometric_token(&self, user_id: &str, token: &str) -> Result<()> {
        self.check_throttle(user_id)?;
        let (on, hash): (bool, Option<String>) = self.conn.query_row(
            "SELECT biometric_enabled, biometric_token_hash FROM users WHERE id = ?",
            [user_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if on && hash.as_deref() == Some(crypto::sha256_hex(token.as_bytes()).as_str()) {
            self.record_success(user_id)
        } else {
            self.register_failure(user_id)?;
            Err(Error::Auth("biometric unlock is not set up on this device".into()))
        }
    }

    // ---------------------------------------------------------------- security settings
    // Every change here re-checks the password, even inside an active session.

    pub fn change_password(&self, user_id: &str, current: &str, new: &str) -> Result<()> {
        self.verify_password(user_id, current)?;
        check_password_rules(new)?;
        let (sealed, salt): (Option<String>, Option<String>) =
            self.conn.query_row("SELECT totp_secret_sealed, kdf_salt FROM users WHERE id = ?", [user_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let new_salt = crypto::to_hex(&crypto::random_bytes::<16>());
        // Re-seal the 2FA secret under the new password.
        let resealed = match (sealed, salt) {
            (Some(s), Some(old_salt)) => {
                let plain = crypto::open(&crypto::derive_key(current, &old_salt)?, &s)?;
                Some(crypto::seal(&crypto::derive_key(new, &new_salt)?, &plain)?)
            }
            _ => None,
        };
        self.conn.execute(
            "UPDATE users SET password_hash = ?, kdf_salt = ?, totp_secret_sealed = ? WHERE id = ?",
            params![crypto::hash_secret(new)?, new_salt, resealed, user_id],
        )?;
        Ok(())
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
    pub fn confirm_totp(&self, user_id: &str, password: &str, secret_base32: &str, code: &str) -> Result<Vec<String>> {
        self.verify_password(user_id, password)?;
        let name = self.get(user_id)?.name;
        let step = build_totp(secret_base32, &name)?
            .check_current(code.trim())
            .ok_or_else(|| Error::Auth("that code didn't match; check the time on your phone and try again".into()))?;
        let salt: String = self.conn.query_row("SELECT kdf_salt FROM users WHERE id = ?", [user_id], |r| r.get(0))?;
        let sealed = crypto::seal(&crypto::derive_key(password, &salt)?, secret_base32.as_bytes())?;
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

    /// Returns a fresh device token; the shell stores it in the OS keychain.
    pub fn enable_biometric(&self, user_id: &str, password: &str) -> Result<String> {
        self.verify_password(user_id, password)?;
        let token = crypto::to_hex(&crypto::random_bytes::<32>());
        self.conn.execute(
            "UPDATE users SET biometric_enabled = 1, biometric_token_hash = ? WHERE id = ?",
            params![crypto::sha256_hex(token.as_bytes()), user_id],
        )?;
        Ok(token)
    }

    pub fn disable_biometric(&self, user_id: &str) -> Result<()> {
        self.conn.execute("UPDATE users SET biometric_enabled = 0, biometric_token_hash = NULL WHERE id = ?", [user_id])?;
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
            codes.push(format!("{}-{}", &c[..4], &c[4..]));
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

impl SecondFactor {
    #[doc(hidden)]
    pub fn secret_for_tests(&self) -> &str {
        &self.secret_base32
    }
}
