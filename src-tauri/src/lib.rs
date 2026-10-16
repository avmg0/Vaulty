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
    accounts, categories, db, export,
    household::{FeedFilter, FeedView, Household, SharingPrefs},
    profiles::{Invite, PasswordOutcome, Registry, Role, SecondFactor, TotpEnrollment, Unlocked, UserSummary},
    settings, transactions, Error, Result,
};

/// How long a "password OK, now enter your code" step stays valid.
const SECOND_FACTOR_TTL: Duration = Duration::from_secs(5 * 60);

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
}

struct AppState {
    data_dir: PathBuf,
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
        let (household, household_ready) = match &keys.household_key {
            Some(hk) => (Household::open(&self.data_dir.join("household.db"), hk)?, true),
            None => (Household::open_in_memory()?, false),
        };
        let recovery_key = keys.new_recovery_key.take();
        *self.pending.lock().map_err(poisoned)? = None;
        *self.session.lock().map_err(poisoned)? = Some(Session { user: user.clone(), conn, keys, household, household_ready });
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
    Ok(())
}

#[tauri::command]
fn biometric_disable(s: S) -> Result<()> {
    let me = s.current_user()?;
    biometric::delete_token(&me.id);
    s.registry(|r| r.disable_biometric(&me.id))
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

#[tauri::command] fn transactions_get_all(s: S, filters: Option<Value>) -> Result<Vec<Value>> { s.data(|c| transactions::get_all(c, filters.as_ref())) }

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
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_update(s: S, id: String, transaction: Value) -> Result<Value> {
    s.act_and_share("transaction.update", |c, me, h| {
        let row = transactions::update(c, &id, &transaction)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_set_visibility(s: S, id: String, visibility: String) -> Result<Value> {
    s.act_and_share("transaction.visibility", |c, me, h| {
        let row = transactions::set_visibility(c, &id, &visibility)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_delete(s: S, id: String) -> Result<()> {
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
    let v = s.data(export::to_json)?;
    std::fs::write(path, serde_json::to_vec_pretty(&v)?)?;
    Ok(())
}
#[tauri::command]
fn export_csv(s: S, path: String) -> Result<usize> {
    let (text, n) = s.data(export::to_csv)?;
    std::fs::write(path, text)?;
    Ok(n)
}
#[tauri::command]
fn import_json(s: S, path: String) -> Result<Value> {
    let data: Value = serde_json::from_slice(&std::fs::read(path)?)?;
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
    let Session { user, conn, keys, household, household_ready } = sess;
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
    *guard = Some(Session { user, conn, keys, household, household_ready });
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("users"))?;
            let registry = Registry::open(&data_dir.join("registry.db"))?;
            app.manage(AppState {
                data_dir,
                registry: Mutex::new(registry),
                session: Mutex::new(None),
                pending: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            users_list, session_current, setup_first_admin, claim_profile,
            sign_in, sign_in_totp, sign_in_backup_code, sign_in_biometric, sign_out,
            users_add, users_reset, users_erase, users_remove, profile_update, sign_in_recovery, recovery_key_new,
            security_status, password_change, totp_begin, totp_confirm, totp_disable,
            backup_codes_regenerate, biometric_enable, biometric_disable,
            accounts_get_all, accounts_create, accounts_update, accounts_delete,
            categories_get_all, categories_create, categories_update, categories_delete,
            transactions_get_all, transactions_create, transactions_update, transactions_delete,
            transactions_set_visibility, sharing_get, sharing_update, family_feed,
            settings_get, settings_update,
            export_json, export_csv, import_json, db_size, institutions_search, tax_countries, tax_describe, tax_compare, snapshots_list, snapshots_create, snapshots_restore, history_recent, history_trash, history_item, history_undo, history_undo_last, market_get, market_update, market_set_key, market_quotes, market_refresh, cards_list, cards_create, cards_update, cards_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vaulty");
}
