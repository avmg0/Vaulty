mod biometric;
mod market;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};
use vaulty_core::{
    accounts, categories, db, export, recurring, statements,
    household::{FeedFilter, FeedView, Household, SharingPrefs},
    profiles::{Invite, PasswordOutcome, Registry, Role, SecondFactor, TotpEnrollment, Unlocked, UserSummary},
    settings, transactions, Error, Result,
};

/// How long a "password OK, now enter your code" step stays valid.
const SECOND_FACTOR_TTL: Duration = Duration::from_secs(5 * 60);
/// How long the Hidden section stays open after unlocking it.
const HIDDEN_TTL: Duration = Duration::from_secs(5 * 60);

/// Everything unlocked for the signed-in member. Dropping it (sign-out, lock, quit) closes the
/// databases and wipes the keys from memory.
struct Session {
    user: UserSummary,
    conn: Connection,
    keys: Unlocked,
    /// The encrypted family store. In-memory stand-in (nothing shared) until this member has
    /// been given the family key, which happens the next time another member signs in.
    household: Household,
    household_ready: bool,
    /// When the Hidden section locks again. `None` while it is locked.
    hidden_until: Option<Instant>,
}

impl Session {
    fn hidden_open(&self) -> bool {
        self.hidden_until.is_some_and(|t| Instant::now() < t)
    }
}

struct AppState {
    data_dir: PathBuf,
    /// Running with sample data in a throwaway folder (`--demo` or `VAULTY_DEMO=1`).
    demo: bool,
    registry: Mutex<Registry>,
    session: Mutex<Option<Session>>,
    pending: Mutex<Option<(SecondFactor, Instant)>>,
}

/// Returned by every sign-in route. `recovery_key` is set only when a new one was just created
/// and must be shown to the member once.
#[derive(Serialize)]
struct SignedIn {
    #[serde(flatten)]
    user: UserSummary,
    recovery_key: Option<String>,
}

fn poisoned<T>(_: T) -> Error {
    Error::Invalid("internal state lock poisoned".into())
}

impl AppState {
    fn user_db_path(&self, user_id: &str) -> PathBuf {
        self.data_dir.join("users").join(format!("{user_id}.db"))
    }

    fn snapshot_dir(&self, user_id: &str) -> PathBuf {
        self.data_dir.join("snapshots").join(user_id)
    }

    fn registry<T>(&self, f: impl FnOnce(&Registry) -> Result<T>) -> Result<T> {
        f(&*self.registry.lock().map_err(poisoned)?)
    }

    fn household<T>(&self, f: impl FnOnce(&Household) -> Result<T>) -> Result<T> {
        let guard = self.session.lock().map_err(poisoned)?;
        let s = guard.as_ref().ok_or_else(|| Error::Auth("locked".into()))?;
        f(&s.household)
    }

    /// Runs `f` against the signed-in user's own database, then mirrors any change to
    /// the family sharing store.
    fn data_and_share<T>(&self, f: impl FnOnce(&Connection, &str, &Household) -> Result<T>) -> Result<T> {
        let guard = self.session.lock().map_err(poisoned)?;
        let s = guard.as_ref().ok_or_else(|| Error::Auth("locked".into()))?;
        f(&s.conn, &s.user.id, &s.household)
    }

    /// Like `data`, recorded as one undoable action in the member's history.
    fn act<T>(&self, action: &str, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        self.data(|c| vaulty_core::history::record(c, action, f))
    }

    /// Like `data_and_share`, recorded as one undoable action.
    fn act_and_share<T>(&self, action: &str, f: impl FnOnce(&Connection, &str, &Household) -> Result<T>) -> Result<T> {
        self.data_and_share(|c, me, h| vaulty_core::history::record(c, action, |c| f(c, me, h)))
    }

    fn keys<T>(&self, f: impl FnOnce(&Unlocked) -> Result<T>) -> Result<T> {
        let guard = self.session.lock().map_err(poisoned)?;
        let s = guard.as_ref().ok_or_else(|| Error::Auth("locked".into()))?;
        f(&s.keys)
    }

    /// Runs `f` against the signed-in user's own database.
    fn data<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.session.lock().map_err(poisoned)?;
        let s = guard.as_ref().ok_or_else(|| Error::Auth("locked".into()))?;
        f(&s.conn)
    }

    fn hidden_open(&self) -> Result<bool> {
        let guard = self.session.lock().map_err(poisoned)?;
        Ok(guard.as_ref().is_some_and(Session::hidden_open))
    }

    /// Exports include hidden transactions in full, so they need the Hidden section unlocked.
    fn guard_export(&self) -> Result<()> {
        if !self.hidden_open()? && self.data(transactions::hidden_count)? > 0 {
            return Err(Error::Auth("Unlock the Hidden section to export: the file will include hidden transactions".into()));
        }
        Ok(())
    }

    /// Refuses to touch a hidden transaction while the Hidden section is locked.
    fn guard_hidden(&self, id: &str) -> Result<()> {
        if !self.hidden_open()? && self.data(|c| transactions::is_hidden(c, id))? {
            return Err(Error::Auth("Unlock the Hidden section to change this transaction".into()));
        }
        Ok(())
    }

    fn open_hidden(&self) -> Result<HiddenStatus> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = guard.as_mut().ok_or_else(|| Error::Auth("locked".into()))?;
        s.hidden_until = Some(Instant::now() + HIDDEN_TTL);
        HiddenStatus::of(s)
    }

    fn set_session_biometric(&self, on: bool) -> Result<()> {
        if let Some(sess) = self.session.lock().map_err(poisoned)?.as_mut() { sess.user.biometric_enabled = on; }
        Ok(())
    }

    fn current_user(&self) -> Result<UserSummary> {
        let guard = self.session.lock().map_err(poisoned)?;
        guard.as_ref().map(|s| s.user.clone()).ok_or_else(|| Error::Auth("locked".into()))
    }

    fn start_session(&self, mut keys: Unlocked) -> Result<SignedIn> {
        let user_id = keys.user_id.clone();
        self.registry(|r| r.sync_keys(&mut keys))?;
        let user = self.registry(|r| r.get(&user_id))?;
        let path = self.user_db_path(&user_id);
        std::fs::create_dir_all(path.parent().expect("users dir"))?;
        let snaps = self.snapshot_dir(&user_id);
        let conn = db::open_with_snapshots(&path, &keys.data_key, Some(&snaps))?;
        // Daily safety copy. A failure here must not block signing in.
        if vaulty_core::snapshots::age_of_latest(&snaps)?.is_none_or(|age| age > vaulty_core::snapshots::DAILY_INTERVAL_SECS) {
            let _ = vaulty_core::snapshots::take(&conn, &keys.data_key, &snaps, "daily");
        }
        // A problem with the shared family store must never lock anyone out of their own data.
        let (household, household_ready) = match keys.household_key.as_ref().map(|hk| Household::open(&self.data_dir.join("household.db"), hk)) {
            Some(Ok(h)) => (h, true),
            _ => (Household::open_in_memory()?, false),
        };
        // Auto-pay items that fell due while the app was closed, as one undoable action.
        let has_due: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM recurring_payments WHERE auto_pay = 1 AND is_active = 1 AND next_due_date <= ?)",
            [today().to_string()], |r| r.get(0)).unwrap_or(false);
        if has_due {
            if let Ok(made) = vaulty_core::history::record(&conn, "recurring.auto", |c| recurring::run_auto_pay(c, today())) {
                for t in &made { let _ = household.publish(&user_id, t); }
            }
        }
        let recovery_key = keys.new_recovery_key.take();
        *self.pending.lock().map_err(poisoned)? = None;
        *self.session.lock().map_err(poisoned)? = Some(Session { user: user.clone(), conn, keys, household, household_ready, hidden_until: None });
        Ok(SignedIn { user, recovery_key })
    }

    fn delete_user_files(&self, user_id: &str) {
        let path = self.user_db_path(user_id);
        for suffix in ["", "-wal", "-shm"] {
            db::shred(std::path::Path::new(&format!("{}{suffix}", path.display())));
        }
        vaulty_core::snapshots::delete_all(&self.snapshot_dir(user_id));
    }

    fn take_pending(&self) -> Result<SecondFactor> {
        let mut p = self.pending.lock().map_err(poisoned)?;
        match p.take() {
            Some((sf, at)) if at.elapsed() < SECOND_FACTOR_TTL => Ok(sf),
            _ => Err(Error::Auth("sign-in timed out; enter your password again".into())),
        }
    }

    fn put_back_pending(&self, sf: SecondFactor) {
        if let Ok(mut p) = self.pending.lock() {
            *p = Some((sf, Instant::now()));
        }
    }
}

type S<'a> = State<'a, AppState>;

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SignIn {
    SignedIn { user: SignedIn },
    NeedsSecondFactor,
}

// ============================================================ profiles & sign-in

#[tauri::command]
fn users_list(s: S) -> Result<Vec<UserSummary>> {
    s.registry(|r| r.list())
}

#[tauri::command]
fn session_current(s: S) -> Result<Option<UserSummary>> {
    Ok(s.session.lock().map_err(poisoned)?.as_ref().map(|x| x.user.clone()))
}

#[tauri::command]
fn setup_first_admin(s: S, name: String, color: String, password: String) -> Result<SignedIn> {
    let keys = s.registry(|r| r.create_first_admin(&name, &color, &password))?;
    s.start_session(keys)
}

#[tauri::command]
fn claim_profile(s: S, user_id: String, setup_code: String, password: String) -> Result<SignedIn> {
    let keys = s.registry(|r| r.claim_profile(&user_id, &setup_code, &password))?;
    s.start_session(keys)
}

/// Forgot password: the member's recovery key opens their data and sets a new password.
#[tauri::command]
fn sign_in_recovery(s: S, user_id: String, recovery_key: String, new_password: String) -> Result<SignedIn> {
    let keys = s.registry(|r| r.sign_in_with_recovery_key(&user_id, &recovery_key, &new_password))?;
    s.start_session(keys)
}

#[tauri::command]
fn sign_in(s: S, user_id: String, password: String) -> Result<SignIn> {
    match s.registry(|r| r.sign_in_with_password(&user_id, &password))? {
        PasswordOutcome::SignedIn(keys) => Ok(SignIn::SignedIn { user: s.start_session(keys)? }),
        PasswordOutcome::NeedsSecondFactor(sf) => {
            s.put_back_pending(sf);
            Ok(SignIn::NeedsSecondFactor)
        }
    }
}

#[tauri::command]
fn sign_in_totp(s: S, code: String) -> Result<SignedIn> {
    let sf = s.take_pending()?;
    match s.registry(|r| r.complete_with_totp(&sf, &code)) {
        Ok(keys) => s.start_session(keys),
        Err(e) => {
            s.put_back_pending(sf);
            Err(e)
        }
    }
}

#[tauri::command]
fn sign_in_backup_code(s: S, code: String) -> Result<SignedIn> {
    let sf = s.take_pending()?;
    match s.registry(|r| r.complete_with_backup_code(&sf, &code)) {
        Ok(keys) => s.start_session(keys),
        Err(e) => {
            s.put_back_pending(sf);
            Err(e)
        }
    }
}

#[tauri::command]
async fn sign_in_biometric(s: S<'_>, user_id: String) -> Result<SignedIn> {
    biometric::prompt("Unlock your Vaulty profile").await.map_err(Error::Auth)?;
    let token = zeroize::Zeroizing::new(biometric::read_token(&user_id).map_err(Error::Auth)?);
    let keys = s.registry(|r| r.sign_in_with_biometric_token(&user_id, &token))?;
    s.start_session(keys)
}

#[tauri::command]
fn sign_out(s: S) -> Result<()> {
    *s.pending.lock().map_err(poisoned)? = None;
    *s.session.lock().map_err(poisoned)? = None; // drops (closes) the user's DB connection
    Ok(())
}

// ============================================================ family management (admin)

#[tauri::command]
fn users_add(s: S, name: String, color: String, role: String) -> Result<Invite> {
    let role = Role::parse(&role)?;
    s.keys(|k| s.registry(|r| r.add_user(k, &name, &color, role)))
}

/// Child forgot their password: a parent's recovery copy keeps their data.
#[tauri::command]
fn users_reset(s: S, user_id: String) -> Result<Invite> {
    let reset = s.keys(|k| s.registry(|r| r.reset_user(k, &user_id)))?;
    biometric::delete_token(&user_id);
    Ok(reset.invite)
}

/// Starts a profile over: deletes the member's data for good and issues a new setup code.
/// The UI asks the admin to type the member's name first.
#[tauri::command]
fn users_erase(s: S, user_id: String, confirm_name: String) -> Result<Invite> {
    let target = s.registry(|r| r.get(&user_id))?;
    if confirm_name.trim() != target.name {
        return Err(Error::Invalid("type the member's name exactly to confirm".into()));
    }
    let reset = s.keys(|k| s.registry(|r| r.erase_and_reset(k, &user_id)))?;
    s.household(|h| h.forget_member(&user_id))?;
    biometric::delete_token(&user_id);
    s.delete_user_files(&user_id);
    Ok(reset.invite)
}

#[tauri::command]
fn users_remove(s: S, user_id: String, confirm_name: String) -> Result<()> {
    let me = s.current_user()?;
    if confirm_name.trim() != s.registry(|r| r.get(&user_id))?.name {
        return Err(Error::Invalid("type the member's name exactly to confirm".into()));
    }
    s.registry(|r| r.remove_user(&me.id, &user_id))?;
    s.household(|h| h.forget_member(&user_id))?;
    biometric::delete_token(&user_id);
    s.delete_user_files(&user_id);
    Ok(())
}

#[tauri::command]
fn profile_update(s: S, name: String, color: String) -> Result<UserSummary> {
    let me = s.current_user()?;
    let user = s.registry(|r| r.update_profile(&me.id, &name, &color))?;
    if let Some(sess) = s.session.lock().map_err(poisoned)?.as_mut() {
        sess.user = user.clone();
    }
    Ok(user)
}

// ============================================================ personal security settings

#[tauri::command]
fn security_status(s: S) -> Result<Value> {
    let me = s.current_user()?;
    let u = s.registry(|r| r.get(&me.id))?;
    let remaining = s.registry(|r| r.backup_codes_remaining(&me.id))?;
    Ok(json!({
        "totp_enabled": u.totp_enabled,
        "backup_codes_remaining": remaining,
        "biometric_enabled": u.biometric_enabled,
        "biometric_available": biometric::available(),
    }))
}

#[tauri::command]
fn password_change(s: S, current: String, new: String) -> Result<()> {
    let me = s.current_user()?;
    s.registry(|r| r.change_password(&me.id, &current, &new))
}

/// Replaces the recovery key; returned once for the member to print or write down.
#[tauri::command]
fn recovery_key_new(s: S, password: String) -> Result<String> {
    let me = s.current_user()?;
    s.registry(|r| r.new_recovery_key(&me.id, &password))
}

#[tauri::command]
fn totp_begin(s: S, password: String) -> Result<TotpEnrollment> {
    let me = s.current_user()?;
    s.registry(|r| r.begin_totp(&me.id, &password))
}

#[tauri::command]
fn totp_confirm(s: S, password: String, secret: String, code: String) -> Result<Vec<String>> {
    let me = s.current_user()?;
    s.registry(|r| r.confirm_totp(&me.id, &password, &secret, &code))
}

#[tauri::command]
fn totp_disable(s: S, password: String) -> Result<()> {
    let me = s.current_user()?;
    s.registry(|r| r.disable_totp(&me.id, &password))
}

#[tauri::command]
fn backup_codes_regenerate(s: S, password: String) -> Result<Vec<String>> {
    let me = s.current_user()?;
    s.registry(|r| r.regenerate_backup_codes(&me.id, &password))
}

#[tauri::command]
async fn biometric_enable(s: S<'_>, password: String) -> Result<()> {
    let me = s.current_user()?;
    // Prove the prompt works on this device before relying on it.
    biometric::prompt("Confirm to turn on biometric unlock").await.map_err(Error::Auth)?;
    let token = s.registry(|r| r.enable_biometric(&me.id, &password))?;
    if let Err(e) = biometric::store_token(&me.id, &token) {
        s.registry(|r| r.disable_biometric(&me.id))?;
        return Err(Error::Auth(e));
    }
    s.set_session_biometric(true)
}

#[tauri::command]
fn biometric_disable(s: S) -> Result<()> {
    let me = s.current_user()?;
    biometric::delete_token(&me.id);
    s.registry(|r| r.disable_biometric(&me.id))?;
    s.set_session_biometric(false)
}

// ============================================================ finance data (signed-in user's own DB)

#[tauri::command] fn accounts_get_all(s: S) -> Result<Vec<Value>> { s.data(accounts::get_all) }
#[tauri::command] fn accounts_create(s: S, account: Value) -> Result<Value> { s.act("account.create", |c| accounts::create(c, &account)) }
#[tauri::command] fn accounts_update(s: S, id: String, account: Value) -> Result<Value> { s.act("account.update", |c| accounts::update(c, &id, &account)) }
#[tauri::command] fn accounts_delete(s: S, id: String) -> Result<()> { s.act("account.delete", |c| accounts::delete(c, &id)) }

#[tauri::command] fn categories_get_all(s: S) -> Result<Vec<Value>> { s.data(categories::get_all) }
#[tauri::command] fn categories_create(s: S, category: Value) -> Result<Value> { s.act("category.create", |c| categories::create(c, &category)) }
#[tauri::command] fn categories_update(s: S, id: String, category: Value) -> Result<Value> { s.act("category.update", |c| categories::update(c, &id, &category)) }
#[tauri::command] fn categories_delete(s: S, id: String) -> Result<()> { s.act("category.delete", |c| categories::delete(c, &id)) }

// ============================================================ statement import

/// Reads a statement the member picked, refusing anything over the size limit before loading it.
fn read_statement(path: &str) -> Result<statements::Table> {
    let p = std::path::Path::new(path);
    if std::fs::metadata(p)?.len() as usize > statements::MAX_BYTES { return Err(Error::Invalid("that file is larger than 20 MB".into())); }
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    statements::read(&std::fs::read(p)?, &name)
}

/// Day-first unless the profile reads dates month-first.
fn default_order(c: &Connection) -> Result<statements::DateOrder> {
    let s = settings::get(c)?;
    let us = s["date_format"] == "mdy" || (s["date_format"] != "dmy" && s["locale"].as_str().is_some_and(|l| l.ends_with("-US") || l.ends_with("-PH")));
    Ok(if us { statements::DateOrder::Mdy } else { statements::DateOrder::Dmy })
}

#[tauri::command]
fn statement_inspect(s: S, path: String, account_id: String) -> Result<Value> {
    let t = read_statement(&path)?;
    s.data(|c| statements::inspect(c, &t, &account_id, default_order(c)?))
}

#[tauri::command]
fn statement_preview(s: S, path: String, account_id: String, mapping: statements::Mapping) -> Result<Vec<statements::ParsedRow>> {
    let t = read_statement(&path)?;
    s.data(|c| statements::preview(c, &t, &mapping, &account_id))
}

/// Imports the chosen rows: `skip` lists new rows to leave out, `include` duplicates to add anyway.
#[tauri::command]
fn statement_import(s: S, path: String, account_id: String, mapping: statements::Mapping, skip: Vec<usize>, include: Vec<usize>) -> Result<usize> {
    let t = read_statement(&path)?;
    let me = s.current_user()?;
    s.keys(|k| s.data(|c| vaulty_core::snapshots::take(c, &k.data_key, &s.snapshot_dir(&me.id), "before-import")))?;
    s.act("statement.import", |c| statements::import(c, &t, &mapping, &account_id, &skip, &include))
}

// ============================================================ recurring payments

fn today() -> chrono::NaiveDate { chrono::Local::now().date_naive() }
fn day(s: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| Error::Invalid("dates look like 2026-10-19".into()))
}

#[tauri::command] fn recurring_list(s: S) -> Result<Vec<Value>> { s.data(|c| recurring::list(c, today())) }
#[tauri::command] fn recurring_create(s: S, item: Value) -> Result<Value> { s.act("recurring.create", |c| recurring::create(c, &item, today())) }
#[tauri::command] fn recurring_update(s: S, id: String, item: Value) -> Result<Value> { s.act("recurring.update", |c| recurring::update(c, &id, &item, today())) }
#[tauri::command] fn recurring_set_active(s: S, id: String, active: bool) -> Result<Value> {
    s.act(if active { "recurring.resume" } else { "recurring.pause" }, |c| recurring::set_active(c, &id, active, today()))
}
#[tauri::command] fn recurring_delete(s: S, id: String) -> Result<()> { s.act("recurring.delete", |c| recurring::delete(c, &id)) }
#[tauri::command] fn recurring_skip(s: S, id: String) -> Result<Value> { s.act("recurring.skip", |c| recurring::skip(c, &id, today())) }
#[tauri::command] fn recurring_history(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| recurring::history(c, &id)) }
/// Occurrences between two dates, for "coming up" lists and the calendar.
#[tauri::command] fn recurring_upcoming(s: S, from: String, to: String) -> Result<Vec<Value>> {
    let (from, to) = (day(&from)?, day(&to)?);
    if (to - from).num_days() > 400 { return Err(Error::Invalid("pick a range of at most a year".into())); }
    s.data(|c| recurring::upcoming(c, from, to))
}
/// Records the due occurrence as paid: creates its transaction and moves to the next date.
#[tauri::command] fn recurring_pay(s: S, id: String, date: Option<String>, amount: Option<Value>) -> Result<Value> {
    s.act_and_share("recurring.pay", |c, me, h| {
        let row = recurring::mark_paid(c, &id, date.as_deref(), amount.as_ref(), today())?;
        h.publish(me, &row)?;
        Ok(row)
    })
}

/// Each category's budget and what was spent in `month` (YYYY-MM).
#[tauri::command] fn budgets_month(s: S, month: String) -> Result<Value> { s.data(|c| vaulty_core::budgets::month(c, &month)) }
/// Sets a category's monthly budget; null or 0 removes it.
#[tauri::command] fn budgets_set(s: S, category_id: String, amount: Value) -> Result<Option<Value>> {
    s.act("budget.set", |c| vaulty_core::budgets::set(c, &category_id, &amount))
}

/// Hidden transactions come back masked (amount, date and account only) unless the caller asks
/// with `reveal_hidden` and the Hidden section is unlocked.
#[tauri::command]
fn transactions_get_all(s: S, filters: Option<Value>) -> Result<Vec<Value>> {
    let wants = filters.as_ref().and_then(|f| f.get("reveal_hidden")).and_then(Value::as_bool).unwrap_or(false);
    let reveal = wants && s.hidden_open()?;
    s.data(|c| transactions::list(c, filters.as_ref(), reveal))
}

// ============================================================ hidden section

#[derive(Serialize)]
struct HiddenStatus {
    unlocked: bool,
    /// Seconds until it locks again.
    seconds_left: u64,
    count: i64,
    biometric: bool,
}

impl HiddenStatus {
    fn of(s: &Session) -> Result<Self> {
        let left = s.hidden_until.filter(|_| s.hidden_open()).map(|t| t.saturating_duration_since(Instant::now()).as_secs()).unwrap_or(0);
        Ok(HiddenStatus { unlocked: left > 0, seconds_left: left, count: transactions::hidden_count(&s.conn)?, biometric: s.user.biometric_enabled && biometric::available() })
    }
}

#[tauri::command]
fn hidden_status(s: S) -> Result<HiddenStatus> {
    let guard = s.session.lock().map_err(poisoned)?;
    HiddenStatus::of(guard.as_ref().ok_or_else(|| Error::Auth("locked".into()))?)
}

#[tauri::command]
fn hidden_unlock(s: S, password: String) -> Result<HiddenStatus> {
    let me = s.current_user()?;
    s.registry(|r| r.confirm_password(&me.id, &password))?;
    s.open_hidden()
}

#[tauri::command]
async fn hidden_unlock_biometric(s: S<'_>) -> Result<HiddenStatus> {
    let me = s.current_user()?;
    if !me.biometric_enabled { return Err(Error::Auth("biometric unlock isn't turned on for this profile".into())); }
    biometric::prompt("Open your hidden transactions").await.map_err(Error::Auth)?;
    let token = zeroize::Zeroizing::new(biometric::read_token(&me.id).map_err(Error::Auth)?);
    // Proves the token still unlocks this profile; the keys it yields are dropped (and wiped) at once.
    drop(s.registry(|r| r.sign_in_with_biometric_token(&me.id, &token))?);
    s.open_hidden()
}

#[tauri::command]
fn hidden_lock(s: S) -> Result<()> {
    if let Some(sess) = s.session.lock().map_err(poisoned)?.as_mut() { sess.hidden_until = None; }
    Ok(())
}

/// Hiding is always allowed; bringing a transaction back into view needs the section unlocked.
#[tauri::command]
fn transactions_set_hidden(s: S, id: String, hidden: bool) -> Result<Value> {
    if !hidden && !s.hidden_open()? { return Err(Error::Auth("Unlock the Hidden section first".into())); }
    let reveal = s.hidden_open()?;
    let row = s.act_and_share(if hidden { "transaction.hide" } else { "transaction.unhide" }, |c, me, h| {
        let row = transactions::set_hidden(c, &id, hidden)?;
        h.publish(me, &row)?;
        Ok(row)
    })?;
    Ok(if reveal { row } else { transactions::mask(row) })
}

/// Remembers the merchant's category for next time (not for hidden or split transactions).
fn learn_rule(c: &Connection, row: &Value) -> Result<()> {
    if row["hidden"] == 1 || row.get("splits").is_some() { return Ok(()); }
    if let (Some(m), Some(cat)) = (row["merchant"].as_str(), row["category_id"].as_str()) { vaulty_core::rules::learn(c, m, cat)?; }
    Ok(())
}

// ============================================================ rules and quick add

/// The category a rule gives this merchant, if any.
#[tauri::command] fn rules_suggest(s: S, merchant: String, kind: String) -> Result<Option<String>> { s.data(|c| vaulty_core::rules::suggest(c, &merchant, &kind)) }
#[tauri::command] fn rules_list(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::rules::list) }
#[tauri::command] fn rules_add(s: S, pattern: String, category_id: String) -> Result<Value> { s.act("rules.add", |c| vaulty_core::rules::add(c, &pattern, &category_id)) }
#[tauri::command] fn rules_delete(s: S, id: String) -> Result<()> { s.act("rules.delete", |c| vaulty_core::rules::delete(c, &id)) }
/// How many uncategorised transactions the rules would categorise.
#[tauri::command] fn rules_pending(s: S) -> Result<usize> { s.data(|c| Ok(vaulty_core::rules::pending(c)?.len())) }
#[tauri::command] fn rules_apply(s: S) -> Result<usize> { s.act("rules.apply", vaulty_core::rules::apply_all) }
/// Reads one line of plain language ("250 swiggy yesterday") as a transaction, without saving.
#[tauri::command]
fn quick_parse(s: S, text: String) -> Result<vaulty_core::quick::Parsed> {
    if text.chars().count() > 200 { return Err(Error::Invalid("keep it to one short line".into())); }
    s.data(|c| vaulty_core::quick::parse(c, &text, today(), &accounts::get_all(c)?))
}

/// New transactions take the member's default visibility unless the form picked one.
fn with_default_visibility(h: &Household, user_id: &str, mut t: Value) -> Result<Value> {
    if t.get("visibility").and_then(Value::as_str).is_none() {
        let v = h.prefs(user_id)?.default_visibility;
        if let Some(o) = t.as_object_mut() {
            o.insert("visibility".into(), json!(v.as_str()));
        }
    }
    Ok(t)
}

#[tauri::command]
fn transactions_create(s: S, transaction: Value) -> Result<Value> {
    s.act_and_share("transaction.create", |c, me, h| {
        let row = transactions::create(c, &with_default_visibility(h, me, transaction)?)?;
        learn_rule(c, &row)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_update(s: S, id: String, transaction: Value) -> Result<Value> {
    s.guard_hidden(&id)?;
    s.act_and_share("transaction.update", |c, me, h| {
        let row = transactions::update(c, &id, &transaction)?;
        learn_rule(c, &row)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_set_visibility(s: S, id: String, visibility: String) -> Result<Value> {
    s.guard_hidden(&id)?;
    s.act_and_share("transaction.visibility", |c, me, h| {
        let row = transactions::set_visibility(c, &id, &visibility)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_delete(s: S, id: String) -> Result<()> {
    s.guard_hidden(&id)?;
    s.act_and_share("transaction.delete", |c, me, h| {
        transactions::delete(c, &id)?;
        h.withdraw(me, &id)
    })
}

// ============================================================ family sharing

#[derive(Serialize)]
struct FamilyMember {
    #[serde(flatten)]
    user: UserSummary,
    sharing_enabled: bool,
}

#[tauri::command]
fn sharing_get(s: S) -> Result<Value> {
    let me = s.current_user()?;
    let prefs = s.household(|h| h.prefs(&me.id))?;
    let enabled = s.household(|h| h.enabled_members())?;
    let members: Vec<FamilyMember> = s
        .registry(|r| r.list())?
        .into_iter()
        .map(|u| FamilyMember { sharing_enabled: enabled.contains(&u.id), user: u })
        .collect();
    let available = s.session.lock().map_err(poisoned)?.as_ref().is_some_and(|x| x.household_ready);
    Ok(json!({ "prefs": prefs, "members": members, "available": available }))
}

#[tauri::command]
fn sharing_update(s: S, prefs: SharingPrefs) -> Result<SharingPrefs> {
    s.data_and_share(|c, me, h| {
        if h.set_prefs(me, &prefs)? {
            h.republish_all(me, &transactions::shareable(c)?)?;
        }
        h.prefs(me)
    })
}

#[tauri::command]
fn family_feed(s: S, view: FeedView, filter: Option<FeedFilter>) -> Result<Vec<Value>> {
    let me = s.current_user()?;
    s.household(|h| h.feed(&me.id, view, &filter.unwrap_or_default()))
}

#[tauri::command] fn settings_get(s: S) -> Result<Value> { s.data(settings::get) }
#[tauri::command] fn settings_update(s: S, settings: Value) -> Result<Value> { s.act("settings.update", |c| settings::update(c, &settings)) }

#[tauri::command]
fn export_json(s: S, path: String) -> Result<()> {
    s.guard_export()?;
    let v = s.data(export::to_json)?;
    std::fs::write(path, serde_json::to_vec_pretty(&v)?)?;
    Ok(())
}
#[tauri::command]
fn export_csv(s: S, path: String) -> Result<usize> {
    s.guard_export()?;
    let (text, n) = s.data(export::to_csv)?;
    std::fs::write(path, text)?;
    Ok(n)
}
/// Writes a password-protected backup (`.vaulty`, see docs/BACKUP_FORMAT.md).
#[tauri::command]
fn backup_export(s: S, path: String, password: String, hint: Option<String>) -> Result<()> {
    s.guard_export()?;
    let bytes = s.data(|c| vaulty_core::backup::encrypt(c, &password, hint.as_deref()))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// What a backup file is, before restoring: encrypted (with its hint and date) or plain JSON.
#[tauri::command]
fn backup_inspect(path: String) -> Result<Value> {
    let bytes = read_limited(&path, 200 * 1024 * 1024)?;
    Ok(match vaulty_core::backup::header(&bytes) {
        Some(h) => json!({ "encrypted": true, "hint": h["hint"], "created_at": h["created_at"] }),
        None => {
            let v: Value = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("that isn't a Vaulty backup".into()))?;
            json!({ "encrypted": false, "hint": null, "created_at": v["exported_at"] })
        }
    })
}

fn read_limited(path: &str, max: u64) -> Result<Vec<u8>> {
    if std::fs::metadata(path)?.len() > max { return Err(Error::Invalid("that file is too large to be a backup".into())); }
    Ok(std::fs::read(path)?)
}

/// Restores a backup into this profile (encrypted ones need their password). Existing items are
/// kept; a safety copy is taken first and the restore can be undone.
#[tauri::command]
fn backup_import(s: S, path: String, password: Option<String>) -> Result<Value> {
    let bytes = read_limited(&path, 200 * 1024 * 1024)?;
    let data = match vaulty_core::backup::header(&bytes) {
        Some(_) => vaulty_core::backup::decrypt(&bytes, password.as_deref().ok_or_else(|| Error::Auth("this backup needs its password".into()))?)?,
        None => serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("that isn't a Vaulty backup".into()))?,
    };
    restore_backup(&s, data)
}

#[tauri::command]
fn import_json(s: S, path: String) -> Result<Value> {
    restore_backup(&s, serde_json::from_slice(&read_limited(&path, 200 * 1024 * 1024)?)?)
}

fn restore_backup(s: &AppState, data: Value) -> Result<Value> {
    let me = s.current_user()?;
    s.keys(|k| s.data(|c| vaulty_core::snapshots::take(c, &k.data_key, &s.snapshot_dir(&me.id), "before-import")))?;
    let counts = s.act_and_share("import", |c, me, h| {
        let counts = export::from_json(c, &data)?;
        h.republish_all(me, &transactions::shareable(c)?)?;
        Ok(counts)
    })?;
    Ok(serde_json::to_value(counts)?)
}
#[tauri::command] fn cards_list(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::cards::list) }
/// Card commands never receive a full card number: only network, last 4 and the other details.
#[tauri::command] fn cards_create(s: S, card: Value) -> Result<Value> { s.act("card.create", |c| vaulty_core::cards::create(c, &card)) }
#[tauri::command] fn cards_update(s: S, id: String, card: Value) -> Result<Value> { s.act("card.update", |c| vaulty_core::cards::update(c, &id, &card)) }
#[tauri::command] fn cards_delete(s: S, id: String) -> Result<()> { s.act("card.delete", |c| vaulty_core::cards::delete(c, &id)) }

#[tauri::command] fn market_get(s: S) -> Result<Value> { s.data(vaulty_core::market::prefs_view) }
#[tauri::command] fn market_update(s: S, prefs: Value) -> Result<Value> { s.data(|c| vaulty_core::market::update_prefs(c, &prefs)) }
/// `which` is `stock` or `crypto`; an empty key removes it.
#[tauri::command] fn market_set_key(s: S, which: String, key: Option<String>) -> Result<Value> {
    s.data(|c| { vaulty_core::market::set_key(c, &which, key.as_deref())?; vaulty_core::market::prefs_view(c) })
}
#[tauri::command] fn market_quotes(s: S) -> Result<Vec<vaulty_core::market::Quote>> { s.data(vaulty_core::market::cached) }

/// Fetches fresh prices, caches them and returns the ticker. The profile stays unlocked for other
/// commands while the request is in flight.
#[tauri::command]
async fn market_refresh(s: S<'_>) -> Result<Value> {
    let job = s.data(|c| {
        let p = vaulty_core::market::prefs(c)?;
        if !p.enabled { return Err(Error::Invalid("the market ticker is turned off".into())); }
        let plan = vaulty_core::market::plan(&p);
        // Free stock plans allow 8 symbols a minute; long lists are covered over several refreshes.
        let stock_batch = vaulty_core::market::next_stocks(c, &plan.stocks, 8)?;
        Ok(market::Job {
            provider: p.stock_provider.clone(),
            plan,
            stock_batch,
            stock_key: vaulty_core::market::key(c, "stock")?,
            crypto_key: vaulty_core::market::key(c, "crypto")?,
        })
    })?;
    let (quotes, problems) = market::fetch(job).await?;
    let ticker = s.data(|c| { vaulty_core::market::save_quotes(c, &quotes)?; vaulty_core::market::cached(c) })?;
    Ok(json!({ "quotes": ticker, "problems": problems }))
}

// ============================================================ history, undo and trash

#[tauri::command] fn history_recent(s: S, limit: Option<i64>) -> Result<Vec<Value>> { s.data(|c| vaulty_core::history::recent(c, limit.unwrap_or(100))) }
#[tauri::command] fn history_trash(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::history::trash) }
/// Re-checks the tamper-evident chain and looks for data changed outside Vaulty.
#[tauri::command] fn history_verify(s: S) -> Result<vaulty_core::history::Verification> { s.data(vaulty_core::history::verify) }
#[tauri::command] fn history_item(s: S, table: String, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::history::for_item(c, &table, &id)) }

/// Undo one action (restore from trash, or undo from history). Shared items are re-published.
#[tauri::command]
fn history_undo(s: S, batch_id: i64) -> Result<()> {
    s.data_and_share(|c, me, h| {
        vaulty_core::history::undo(c, batch_id)?;
        h.republish_all(me, &transactions::shareable(c)?)?;
        Ok(())
    })
}

/// Ctrl+Z: undo the most recent action. Returns what was undone, or null if nothing.
#[tauri::command]
fn history_undo_last(s: S) -> Result<Option<Value>> {
    s.data_and_share(|c, me, h| {
        let done = vaulty_core::history::undo_last(c)?;
        if done.is_some() { h.republish_all(me, &transactions::shareable(c)?)?; }
        Ok(done)
    })
}

// ============================================================ snapshots

#[tauri::command]
fn snapshots_list(s: S) -> Result<Vec<vaulty_core::snapshots::Snapshot>> {
    let me = s.current_user()?;
    vaulty_core::snapshots::list(&s.snapshot_dir(&me.id))
}

#[tauri::command]
fn snapshots_create(s: S) -> Result<vaulty_core::snapshots::Snapshot> {
    let me = s.current_user()?;
    s.keys(|k| s.data(|c| vaulty_core::snapshots::take(c, &k.data_key, &s.snapshot_dir(&me.id), "manual")))
}

/// Replaces the member's data with a snapshot. The current data is snapshotted first, so a
/// restore can itself be reversed.
#[tauri::command]
fn snapshots_restore(s: S, name: String) -> Result<()> {
    let mut guard = s.session.lock().map_err(poisoned)?;
    let sess = guard.take().ok_or_else(|| Error::Auth("locked".into()))?;
    let Session { user, conn, keys, household, household_ready, .. } = sess;
    let dir = s.snapshot_dir(&user.id);
    let path = s.user_db_path(&user.id);
    let result = (|| {
        vaulty_core::snapshots::path_of(&dir, &name)?;
        vaulty_core::snapshots::take(&conn, &keys.data_key, &dir, "before-restore")?;
        drop(conn);
        vaulty_core::snapshots::restore(&dir, &name, &path, &keys.data_key)
    })();
    // Reopen whatever is on disk now (the snapshot, or the untouched original on failure).
    let conn = db::open(&path, &keys.data_key)?;
    if result.is_ok() {
        household.republish_all(&user.id, &transactions::shareable(&conn)?)?;
    }
    *guard = Some(Session { user, conn, keys, household, household_ready, hidden_until: None });
    result
}

/// Countries with tax rules, the rules themselves, and an estimate comparing every regime.
#[tauri::command] fn tax_countries() -> Result<Vec<String>> { vaulty_core::tax::countries() }
#[tauri::command] fn tax_describe(country: String) -> Result<Value> { vaulty_core::tax::describe(&country) }
#[tauri::command] fn tax_compare(country: String, year: Option<String>, input: Value) -> Result<Value> {
    vaulty_core::tax::compare(&country, year.as_deref(), &input)
}

/// Bank / card / broker directory search. `currency` ranks that country's institutions first.
#[tauri::command]
fn institutions_search(query: String, currency: Option<String>, limit: Option<usize>) -> Vec<vaulty_core::institutions::Institution> {
    let country = currency.as_deref().and_then(vaulty_core::institutions::country_for_currency);
    vaulty_core::institutions::search(&query, country, limit.unwrap_or(20)).into_iter().cloned().collect()
}

#[tauri::command]
fn db_size(s: S) -> Result<String> {
    let me = s.current_user()?;
    Ok(db::file_size_label(&s.user_db_path(&me.id)))
}

// ============================================================ demo mode

/// Password for every sample profile. Only ever used for the throwaway demo folder.
const DEMO_PASSWORD: &str = "vaulty-demo";

fn demo_requested() -> bool {
    std::env::args().any(|a| a == "--demo") || std::env::var("VAULTY_DEMO").is_ok_and(|v| v == "1")
}

/// Builds a fresh demo household in `dir` (wiping any earlier demo there): John Doe (admin,
/// salaried), Jane Doe (freelancer) and Jenny Doe (child), each with six months of sample data.
fn build_demo(dir: &std::path::Path) -> Result<Registry> {
    if dir.exists() { std::fs::remove_dir_all(dir)?; }
    std::fs::create_dir_all(dir.join("users"))?;
    let registry = Registry::open(&dir.join("registry.db"))?;
    let today = chrono::Local::now().date_naive();
    let seed = |keys: &Unlocked, persona| -> Result<()> {
        let conn = db::open(&dir.join("users").join(format!("{}.db", keys.user_id)), &keys.data_key)?;
        vaulty_core::demo::seed(&conn, persona, today)
    };
    let admin = registry.create_first_admin("John Doe", "#2563eb", DEMO_PASSWORD)?;
    seed(&admin, vaulty_core::demo::Persona::Earner)?;
    for (name, color, role, persona) in [("Jane Doe", "#7c3aed", Role::Member, vaulty_core::demo::Persona::Freelancer),
                                         ("Jenny Doe", "#e11d48", Role::Child, vaulty_core::demo::Persona::Child)] {
        let invite = registry.add_user(&admin, name, color, role)?;
        let member = registry.claim_profile(&invite.user.id, &invite.setup_code, DEMO_PASSWORD)?;
        seed(&member, persona)?;
    }
    Ok(registry)
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    demo: bool,
    demo_password: Option<&'static str>,
}

#[tauri::command]
fn app_info(s: S) -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION"), demo: s.demo, demo_password: s.demo.then_some(DEMO_PASSWORD) }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let demo = demo_requested();
            let (data_dir, registry) = if demo {
                let dir = std::env::temp_dir().join("vaulty-demo");
                let registry = build_demo(&dir)?;
                (dir, registry)
            } else {
                let dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(dir.join("users"))?;
                let registry = Registry::open(&dir.join("registry.db"))?;
                (dir, registry)
            };
            app.manage(AppState {
                demo,
                data_dir,
                registry: Mutex::new(registry),
                session: Mutex::new(None),
                pending: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info, users_list, session_current, setup_first_admin, claim_profile,
            hidden_status, hidden_unlock, hidden_unlock_biometric, hidden_lock, transactions_set_hidden,
            sign_in, sign_in_totp, sign_in_backup_code, sign_in_biometric, sign_out,
            users_add, users_reset, users_erase, users_remove, profile_update, sign_in_recovery, recovery_key_new,
            security_status, password_change, totp_begin, totp_confirm, totp_disable,
            backup_codes_regenerate, biometric_enable, biometric_disable,
            accounts_get_all, accounts_create, accounts_update, accounts_delete,
            categories_get_all, categories_create, categories_update, categories_delete, budgets_month, budgets_set,
            statement_inspect, statement_preview, statement_import, rules_suggest, rules_list, rules_add, rules_delete, rules_pending, rules_apply, quick_parse,
            recurring_list, recurring_create, recurring_update, recurring_set_active, recurring_delete, recurring_skip, recurring_history, recurring_upcoming, recurring_pay,
            transactions_get_all, transactions_create, transactions_update, transactions_delete,
            transactions_set_visibility, sharing_get, sharing_update, family_feed,
            settings_get, settings_update,
            export_json, export_csv, import_json, backup_export, backup_inspect, backup_import, db_size, institutions_search, tax_countries, tax_describe, tax_compare, snapshots_list, snapshots_create, snapshots_restore, history_recent, history_trash, history_item, history_verify, history_undo, history_undo_last, market_get, market_update, market_set_key, market_quotes, market_refresh, cards_list, cards_create, cards_update, cards_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vaulty");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_household_signs_in_with_sample_data() {
        let dir = std::env::temp_dir().join(format!("vaulty-demo-test-{}", std::process::id()));
        let registry = build_demo(&dir).unwrap();
        let users = registry.list().unwrap();
        assert_eq!(users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), ["John Doe", "Jane Doe", "Jenny Doe"]);
        for u in &users {
            let PasswordOutcome::SignedIn(keys) = registry.sign_in_with_password(&u.id, DEMO_PASSWORD).unwrap() else { panic!("no second factor in demo") };
            let conn = db::open(&dir.join("users").join(format!("{}.db", u.id)), &keys.data_key).unwrap();
            assert!(!transactions::list(&conn, None, false).unwrap().is_empty(), "{} has sample data", u.name);
        }
        // Building again starts from scratch. (Close the first one: Windows can't delete open files.)
        drop(registry);
        let again = build_demo(&dir).unwrap();
        assert_eq!(again.list().unwrap().len(), 3);
        drop(again);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
