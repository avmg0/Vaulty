mod biometric;

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
    profiles::{Invite, PasswordOutcome, Registry, Role, SecondFactor, TotpEnrollment, UserSummary},
    rows, settings, transactions, Error, Result,
};

/// How long a "password OK, now enter your code" step stays valid.
const SECOND_FACTOR_TTL: Duration = Duration::from_secs(5 * 60);

struct Session {
    user: UserSummary,
    conn: Connection,
}

struct AppState {
    data_dir: PathBuf,
    registry: Mutex<Registry>,
    household: Mutex<Household>,
    session: Mutex<Option<Session>>,
    pending: Mutex<Option<(SecondFactor, Instant)>>,
}

fn poisoned<T>(_: T) -> Error {
    Error::Invalid("internal state lock poisoned".into())
}

impl AppState {
    fn user_db_path(&self, user_id: &str) -> PathBuf {
        self.data_dir.join("users").join(format!("{user_id}.db"))
    }

    fn registry<T>(&self, f: impl FnOnce(&Registry) -> Result<T>) -> Result<T> {
        f(&*self.registry.lock().map_err(poisoned)?)
    }

    fn household<T>(&self, f: impl FnOnce(&Household) -> Result<T>) -> Result<T> {
        f(&*self.household.lock().map_err(poisoned)?)
    }

    /// Runs `f` against the signed-in user's own database, then mirrors any change to
    /// the family sharing store. Lock order is always session → household.
    fn data_and_share<T>(&self, f: impl FnOnce(&Connection, &str, &Household) -> Result<T>) -> Result<T> {
        let guard = self.session.lock().map_err(poisoned)?;
        let s = guard.as_ref().ok_or_else(|| Error::Auth("locked".into()))?;
        let h = self.household.lock().map_err(poisoned)?;
        f(&s.conn, &s.user.id, &h)
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

    fn start_session(&self, user_id: &str) -> Result<UserSummary> {
        let user = self.registry(|r| r.get(user_id))?;
        let path = self.user_db_path(user_id);
        std::fs::create_dir_all(path.parent().expect("users dir"))?;
        let conn = db::open(&path)?;
        *self.pending.lock().map_err(poisoned)? = None;
        *self.session.lock().map_err(poisoned)? = Some(Session { user: user.clone(), conn });
        Ok(user)
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
    SignedIn { user: UserSummary },
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
fn setup_first_admin(s: S, name: String, color: String, password: String) -> Result<UserSummary> {
    let user = s.registry(|r| r.create_first_admin(&name, &color, &password))?;
    s.start_session(&user.id)
}

#[tauri::command]
fn claim_profile(s: S, user_id: String, setup_code: String, password: String) -> Result<UserSummary> {
    s.registry(|r| r.claim_profile(&user_id, &setup_code, &password))?;
    s.start_session(&user_id)
}

#[tauri::command]
fn sign_in(s: S, user_id: String, password: String) -> Result<SignIn> {
    match s.registry(|r| r.sign_in_with_password(&user_id, &password))? {
        PasswordOutcome::SignedIn => Ok(SignIn::SignedIn { user: s.start_session(&user_id)? }),
        PasswordOutcome::NeedsSecondFactor(sf) => {
            s.put_back_pending(sf);
            Ok(SignIn::NeedsSecondFactor)
        }
    }
}

#[tauri::command]
fn sign_in_totp(s: S, code: String) -> Result<UserSummary> {
    let sf = s.take_pending()?;
    match s.registry(|r| r.complete_with_totp(&sf, &code)) {
        Ok(()) => s.start_session(&sf.user_id),
        Err(e) => {
            s.put_back_pending(sf);
            Err(e)
        }
    }
}

#[tauri::command]
fn sign_in_backup_code(s: S, code: String) -> Result<UserSummary> {
    let sf = s.take_pending()?;
    match s.registry(|r| r.complete_with_backup_code(&sf, &code)) {
        Ok(()) => s.start_session(&sf.user_id),
        Err(e) => {
            s.put_back_pending(sf);
            Err(e)
        }
    }
}

#[tauri::command]
async fn sign_in_biometric(s: S<'_>, user_id: String) -> Result<UserSummary> {
    biometric::prompt("Unlock your Vaulty profile").await.map_err(Error::Auth)?;
    let token = biometric::read_token(&user_id).map_err(Error::Auth)?;
    s.registry(|r| r.sign_in_with_biometric_token(&user_id, &token))?;
    s.start_session(&user_id)
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
    let me = s.current_user()?;
    let role = Role::parse(&role)?;
    s.registry(|r| r.add_user(&me.id, &name, &color, role))
}

#[tauri::command]
fn users_reset(s: S, user_id: String) -> Result<Invite> {
    let me = s.current_user()?;
    let inv = s.registry(|r| r.reset_user(&me.id, &user_id))?;
    biometric::delete_token(&user_id);
    Ok(inv)
}

#[tauri::command]
fn users_remove(s: S, user_id: String) -> Result<()> {
    let me = s.current_user()?;
    s.registry(|r| r.remove_user(&me.id, &user_id))?;
    s.household(|h| h.forget_member(&user_id))?;
    biometric::delete_token(&user_id);
    let path = s.user_db_path(&user_id);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
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
#[tauri::command] fn accounts_create(s: S, account: Value) -> Result<Value> { s.data(|c| accounts::create(c, &account)) }
#[tauri::command] fn accounts_update(s: S, id: String, account: Value) -> Result<Value> { s.data(|c| accounts::update(c, &id, &account)) }
#[tauri::command] fn accounts_delete(s: S, id: String) -> Result<()> { s.data(|c| accounts::delete(c, &id)) }

#[tauri::command] fn categories_get_all(s: S) -> Result<Vec<Value>> { s.data(categories::get_all) }
#[tauri::command] fn categories_create(s: S, category: Value) -> Result<Value> { s.data(|c| categories::create(c, &category)) }
#[tauri::command] fn categories_update(s: S, id: String, category: Value) -> Result<Value> { s.data(|c| categories::update(c, &id, &category)) }
#[tauri::command] fn categories_delete(s: S, id: String) -> Result<()> { s.data(|c| categories::delete(c, &id)) }

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
    s.data_and_share(|c, me, h| {
        let row = transactions::create(c, &with_default_visibility(h, me, transaction)?)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_update(s: S, id: String, transaction: Value) -> Result<Value> {
    s.data_and_share(|c, me, h| {
        let row = transactions::update(c, &id, &transaction)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_set_visibility(s: S, id: String, visibility: String) -> Result<Value> {
    s.data_and_share(|c, me, h| {
        let row = transactions::set_visibility(c, &id, &visibility)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_delete(s: S, id: String) -> Result<()> {
    s.data_and_share(|c, me, h| {
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
    Ok(json!({ "prefs": prefs, "members": members }))
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
#[tauri::command] fn settings_update(s: S, settings: Value) -> Result<Value> { s.data(|c| settings::update(c, &settings)) }

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
    let counts = s.data_and_share(|c, me, h| {
        let counts = export::from_json(c, &data)?;
        h.republish_all(me, &transactions::shareable(c)?)?;
        Ok(counts)
    })?;
    Ok(serde_json::to_value(counts)?)
}
#[tauri::command]
fn db_size(s: S) -> Result<String> {
    let me = s.current_user()?;
    Ok(db::file_size_label(&s.user_db_path(&me.id)))
}

/// Escape hatch for screens not yet moved to typed commands. Always scoped to the
/// signed-in user's own database; parameters are bound and multi-statement SQL is rejected.
#[tauri::command] fn db_all(s: S, sql: String, params: Option<Vec<Value>>) -> Result<Vec<Value>> { s.data(|c| rows::query(c, &sql, &params.unwrap_or_default())) }
#[tauri::command] fn db_get(s: S, sql: String, params: Option<Vec<Value>>) -> Result<Option<Value>> { s.data(|c| rows::query_one(c, &sql, &params.unwrap_or_default())) }
#[tauri::command] fn db_run(s: S, sql: String, params: Option<Vec<Value>>) -> Result<rows::RunResult> { s.data(|c| rows::run(c, &sql, &params.unwrap_or_default())) }

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("users"))?;
            let registry = Registry::open(&data_dir.join("registry.db"))?;
            let household = Household::open(&data_dir.join("household.db"))?;
            app.manage(AppState {
                data_dir,
                registry: Mutex::new(registry),
                household: Mutex::new(household),
                session: Mutex::new(None),
                pending: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            users_list, session_current, setup_first_admin, claim_profile,
            sign_in, sign_in_totp, sign_in_backup_code, sign_in_biometric, sign_out,
            users_add, users_reset, users_remove, profile_update,
            security_status, password_change, totp_begin, totp_confirm, totp_disable,
            backup_codes_regenerate, biometric_enable, biometric_disable,
            accounts_get_all, accounts_create, accounts_update, accounts_delete,
            categories_get_all, categories_create, categories_update, categories_delete,
            transactions_get_all, transactions_create, transactions_update, transactions_delete,
            transactions_set_visibility, sharing_get, sharing_update, family_feed,
            settings_get, settings_update,
            export_json, export_csv, import_json, db_size, db_all, db_get, db_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vaulty");
}
