mod biometric;
#[cfg(desktop)]
mod desktop;
mod ocr;
mod market;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};
use tauri::{Manager, State};
use vaulty_core::{
    accounts, limits, categories, db, export, recurring, statements,
    household::{FeedFilter, FeedView, Household, SharingPrefs},
    profiles::{Invite, PasswordOutcome, Registry, Role, SecondFactor, TotpEnrollment, Unlocked, UserSummary},
    settings, transactions, Error, Result,
};

/// How long a "password OK, now enter your code" step stays valid.
const SECOND_FACTOR_TTL: Duration = Duration::from_secs(5 * 60);
/// How long helpers' approvals are kept while a member recovers with their help.
const HELPING_TTL: Duration = Duration::from_secs(10 * 60);
/// How long the Hidden section stays open after unlocking it.
const HIDDEN_TTL: Duration = Duration::from_secs(5 * 60);
/// The window locks itself when the auto-lock time is up; the app enforces the same limit this
/// much later, in case the window is frozen or misbehaving.
const LOCK_GRACE: Duration = Duration::from_secs(60);

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
    /// When the Hidden section locks again. `None` while it is locked. Wall-clock time, so time
    /// spent asleep counts (the monotonic clock pauses during sleep on macOS).
    hidden_until: Option<SystemTime>,
    /// Last time the person used the app (reported by the window), and how long it may sit idle.
    last_active: SystemTime,
    auto_lock: Option<Duration>,
    /// The business book switched to, if any. Pages read and write it in place of the personal
    /// book; family sharing never sees it.
    book: Option<Book>,
}

struct Book {
    id: String,
    conn: Connection,
    key: vaulty_core::Key,
    /// Stand-in sharing store: business transactions are never shared with the family.
    quiet: Household,
}

impl Session {
    fn new(user: UserSummary, conn: Connection, keys: Unlocked, household: Household, household_ready: bool) -> Self {
        let auto_lock = vaulty_core::settings::auto_lock(&conn).unwrap_or(Some(Duration::from_secs(5 * 60)));
        Session { user, conn, keys, household, household_ready, hidden_until: None, last_active: SystemTime::now(), auto_lock, book: None }
    }

    fn hidden_open(&self) -> bool {
        self.hidden_until.is_some_and(|t| SystemTime::now() < t)
    }

    /// Unused for longer than the auto-lock time (plus grace). A clock set backwards counts as use.
    fn idle_expired(&self) -> bool {
        self.auto_lock.is_some_and(|limit| SystemTime::now().duration_since(self.last_active).unwrap_or_default() > limit + LOCK_GRACE)
    }
}

/// The signed-in session, unless it has sat idle past its auto-lock time; then it's dropped
/// (closing the database and wiping the keys) and the caller gets "locked".
fn live(guard: &mut Option<Session>) -> Result<&mut Session> {
    if guard.as_ref().is_some_and(Session::idle_expired) { *guard = None; }
    guard.as_mut().ok_or_else(|| Error::Auth("locked".into()))
}

struct AppState {
    data_dir: PathBuf,
    /// Running with sample data in a throwaway folder (`--demo` or `VAULTY_DEMO=1`).
    demo: bool,
    registry: Mutex<Registry>,
    session: Mutex<Option<Session>>,
    pending: Mutex<Option<(SecondFactor, Instant)>>,
    /// Helpers' shares collected so far for a member recovering their profile (memory only).
    helping: Mutex<Option<Helping>>,
    /// Files the person chose in a file dialog this run. File commands only touch these, so a
    /// script injected into the interface couldn't read or overwrite any other file.
    picked: Mutex<Vec<String>>,
    /// Notifications shown in the last minute (a misbehaving window can't flood the desktop).
    shown: Mutex<Vec<Instant>>,
}

struct Helping { owner: String, shares: Vec<vaulty_core::profiles::HelperShare>, started: Instant }

/// Returned by every sign-in route. `recovery_key` is set only when a new one was just created
/// and must be shown to the member once.
#[derive(Serialize)]
struct SignedIn {
    #[serde(flatten)]
    user: UserSummary,
    recovery_key: Option<String>,
    /// Wrong passwords or codes for this profile since it was last signed in to.
    failed_attempts: Option<vaulty_core::profiles::FailureNotice>,
    /// Members this person helped back into their profile since they last signed in.
    helped: Vec<vaulty_core::profiles::HelperNotice>,
    /// Their nominee asked for access and it hasn't been allowed: shown so they can cancel it.
    nominee_request: Option<vaulty_core::profiles::NomineeRequest>,
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
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        f(&s.household)
    }

    /// Runs `f` against the signed-in user's own database, then mirrors any change to
    /// the family sharing store.
    fn data_and_share<T>(&self, f: impl FnOnce(&Connection, &str, &Household) -> Result<T>) -> Result<T> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        match &s.book {
            Some(b) => f(&b.conn, &s.user.id, &b.quiet),
            None => f(&s.conn, &s.user.id, &s.household),
        }
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
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        f(&s.keys)
    }

    /// Runs `f` against the book in use: the member's personal book, or the business book
    /// they switched to.
    fn data<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        f(s.book.as_ref().map_or(&s.conn, |b| &b.conn))
    }

    /// Runs `f` against the member's personal book, whichever book is in use.
    fn personal<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        f(&s.conn)
    }

    /// The business book in use, with the personal book and the member's keys.
    fn business<T>(&self, f: impl FnOnce(&Connection, &Book, &Unlocked) -> Result<T>) -> Result<T> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        let b = s.book.as_ref().ok_or_else(|| Error::Invalid("switch to a business first".into()))?;
        f(&s.conn, b, &s.keys)
    }

    /// Refuses while a business book is in use (for things only the personal book has).
    fn personal_only(&self) -> Result<()> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        if live(&mut guard)?.book.is_some() { return Err(Error::Invalid("switch back to your personal book to do this".into())); }
        Ok(())
    }

    fn business_db_path(&self, user_id: &str, id: &str) -> PathBuf {
        self.data_dir.join("users").join(format!("{user_id}-biz-{id}.db"))
    }

    /// Where the book in use keeps its safety copies and its attached files.
    fn book_dirs(&self, s: &Session) -> (PathBuf, PathBuf) {
        let (snaps, files) = (self.snapshot_dir(&s.user.id), self.documents_dir(&s.user.id));
        match &s.book { Some(b) => (snaps.join(format!("biz-{}", b.id)), files.join(format!("biz-{}", b.id))), None => (snaps, files) }
    }
    fn files_dir(&self) -> Result<PathBuf> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        Ok(self.book_dirs(live(&mut guard)?).1)
    }
    fn snaps_dir(&self) -> Result<PathBuf> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        Ok(self.book_dirs(live(&mut guard)?).0)
    }

    /// A safety copy of the book in use.
    fn take_snapshot(&self, reason: &str) -> Result<vaulty_core::snapshots::Snapshot> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        let dir = self.book_dirs(s).0;
        match &s.book {
            Some(b) => vaulty_core::snapshots::take(&b.conn, &b.key, &dir, reason),
            None => vaulty_core::snapshots::take(&s.conn, &s.keys.data_key, &dir, reason),
        }
    }

    fn hidden_open(&self) -> Result<bool> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        Ok(live(&mut guard).is_ok_and(|s| s.hidden_open()))
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
        let s = live(&mut guard)?;
        s.hidden_until = Some(SystemTime::now() + HIDDEN_TTL);
        HiddenStatus::of(s)
    }

    fn set_session_biometric(&self, on: bool) -> Result<()> {
        if let Some(sess) = self.session.lock().map_err(poisoned)?.as_mut() { sess.user.biometric_enabled = on; }
        Ok(())
    }

    fn current_user(&self) -> Result<UserSummary> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        live(&mut guard).map(|s| s.user.clone())
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
        // Empty the documents trash (best effort).
        let _ = vaulty_core::documents::purge(&conn, &self.documents_dir(&user_id));
        let recovery_key = keys.new_recovery_key.take();
        let failed_attempts = self.registry(|r| r.take_failure_notice(&user_id))?;
        let helped = self.registry(|r| r.take_helper_notices(&user_id))?;
        let nominee_request = self.registry(|r| r.nominee(&user_id))?.request.filter(|q| !q.allowed);
        *self.pending.lock().map_err(poisoned)? = None;
        *self.helping.lock().map_err(poisoned)? = None;
        *self.session.lock().map_err(poisoned)? = Some(Session::new(user.clone(), conn, keys, household, household_ready));
        Ok(SignedIn { user, recovery_key, failed_attempts, helped, nominee_request })
    }

    fn documents_dir(&self, user_id: &str) -> PathBuf {
        self.data_dir.join("documents").join(user_id)
    }

    /// Runs `f` with the book in use and the key its attached files are encrypted with (one lock).
    fn data_with_keys<T>(&self, f: impl FnOnce(&Connection, &vaulty_core::Key) -> Result<T>) -> Result<T> {
        let mut guard = self.session.lock().map_err(poisoned)?;
        let s = live(&mut guard)?;
        match &s.book { Some(b) => f(&b.conn, &b.key), None => f(&s.conn, &s.keys.data_key) }
    }

    fn delete_user_files(&self, user_id: &str) {
        let _ = std::fs::remove_dir_all(self.documents_dir(user_id));
        let path = self.user_db_path(user_id);
        for suffix in ["", "-wal", "-shm"] {
            db::shred(std::path::Path::new(&format!("{}{suffix}", path.display())));
        }
        vaulty_core::snapshots::delete_all(&self.snapshot_dir(user_id));
        // The member's business books.
        if let Ok(entries) = std::fs::read_dir(self.data_dir.join("users")) {
            for e in entries.flatten() {
                if e.file_name().to_str().is_some_and(|n| n.starts_with(&format!("{user_id}-biz-"))) { db::shred(&e.path()); }
            }
        }
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
    let mut guard = s.session.lock().map_err(poisoned)?;
    Ok(live(&mut guard).ok().map(|x| x.user.clone()))
}

/// The window reports that the person is using the app (throttled on its side).
#[tauri::command]
fn session_touch(s: S) -> Result<()> {
    let mut guard = s.session.lock().map_err(poisoned)?;
    live(&mut guard)?.last_active = SystemTime::now();
    Ok(())
}

/// Locks an idle session even if the window never asks for anything again, and tells the window.
/// Also tells the window about payments due today for members who asked to be reminded while
/// locked (from 9 in the morning, local time).
fn watch_idle(app: tauri::AppHandle) {
    use tauri::Emitter;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        let state = app.state::<AppState>();
        let now = chrono::Local::now();
        if now.format("%H").to_string().parse::<u32>().unwrap_or(0) >= 9 {
            if let Ok(due) = state.registry(|r| r.take_locked_reminders(now.date_naive())) {
                for u in due { let _ = app.emit("vaulty://locked-reminder", json!({ "name": u.name })); }
            }
        }
        let Ok(mut guard) = state.session.lock() else { continue };
        if guard.as_ref().is_some_and(Session::idle_expired) {
            *guard = None;
            drop(guard);
            let _ = app.emit("vaulty://locked", ());
        }
    });
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

// ------------------------------------------------------------ shared recovery (helpers)

/// Who can help this member back in (names only), on the sign-in screen.
#[tauri::command]
fn helpers_for(s: S, user_id: String) -> Result<vaulty_core::profiles::Helpers> {
    s.registry(|r| r.helpers(&user_id))
}

impl AppState {
    /// Adds an approval, starting over if it's for someone else or has gone stale. Returns the
    /// helper slots approved so far.
    fn add_help(&self, owner: &str, share: vaulty_core::profiles::HelperShare) -> Result<Vec<u8>> {
        let mut h = self.helping.lock().map_err(poisoned)?;
        if h.as_ref().is_none_or(|x| x.owner != owner || x.started.elapsed() > HELPING_TTL) {
            *h = Some(Helping { owner: owner.into(), shares: vec![], started: Instant::now() });
        }
        let help = h.as_mut().expect("just set");
        help.shares.retain(|x| x.slot() != share.slot());
        help.shares.push(share);
        Ok(help.shares.iter().map(|x| x.slot()).collect())
    }
}

/// A family member helper approves with their own password.
#[tauri::command]
fn helpers_approve_member(s: S, user_id: String, helper_id: String, password: String) -> Result<Vec<u8>> {
    let share = s.registry(|r| r.helper_share_from_member(&user_id, &helper_id, &password))?;
    s.add_help(&user_id, share)
}

/// A printed helper card's code.
#[tauri::command]
fn helpers_approve_card(s: S, user_id: String, code: String) -> Result<Vec<u8>> {
    limits::text("code", &code, 100)?;
    let share = s.registry(|r| r.helper_share_from_card(&user_id, &code))?;
    s.add_help(&user_id, share)
}

#[tauri::command]
fn helpers_cancel(s: S) -> Result<()> {
    *s.helping.lock().map_err(poisoned)? = None;
    Ok(())
}

/// With two helpers' approvals: a new password (and the 2FA code, if 2FA is on), then signed in.
#[tauri::command]
fn sign_in_helpers(s: S, user_id: String, new_password: String, code: Option<String>) -> Result<SignedIn> {
    let shares = {
        let h = s.helping.lock().map_err(poisoned)?;
        match h.as_ref() {
            Some(x) if x.owner == user_id && x.started.elapsed() <= HELPING_TTL => x.shares.clone(),
            _ => return Err(Error::Auth("the helpers' approvals timed out; ask them again".into())),
        }
    };
    let keys = s.registry(|r| r.recover_with_helpers(&user_id, &shares, &new_password, code.as_deref()))?;
    s.start_session(keys)
}

// ============================================================ card statements and benefits

#[tauri::command]
fn card_cycles_get(s: S, card_id: String) -> Result<Option<vaulty_core::card_cycles::CardCycles>> { s.data(|c| vaulty_core::card_cycles::for_card(c, &card_id, today())) }
#[tauri::command]
fn card_statement_enter(s: S, card_id: String, statement: Value) -> Result<()> { limits::check(&statement)?; s.act("card.statement", |c| vaulty_core::card_cycles::enter(c, &card_id, &statement)) }
#[tauri::command]
fn card_statement_forget(s: S, card_id: String, statement_date: String) -> Result<()> { s.act("card.statement", |c| vaulty_core::card_cycles::forget(c, &card_id, &statement_date)) }
#[tauri::command]
fn card_benefits_list(s: S, card_id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::card_benefits::list(c, &card_id, today())) }
#[tauri::command]
fn card_benefits_create(s: S, card_id: String, benefit: Value) -> Result<Value> { limits::check(&benefit)?; s.act("card.benefit_create", |c| vaulty_core::card_benefits::create(c, &card_id, &benefit, today())) }
#[tauri::command]
fn card_benefits_update(s: S, id: String, benefit: Value) -> Result<()> { limits::check(&benefit)?; s.act("card.benefit_update", |c| vaulty_core::card_benefits::update(c, &id, &benefit, today())) }
#[tauri::command]
fn card_benefits_delete(s: S, id: String) -> Result<()> { s.act("card.benefit_delete", |c| vaulty_core::card_benefits::delete(c, &id)) }
#[tauri::command]
fn card_benefits_use(s: S, id: String, by: i64) -> Result<i64> { s.act("card.perk", |c| vaulty_core::card_benefits::use_perk(c, &id, by, today())) }
/// Credit cards ranked for a purchase today.
#[tauri::command]
fn cards_best(s: S, amount: Option<Value>, category_id: Option<String>, place: Option<String>) -> Result<Vec<vaulty_core::card_benefits::Pick>> {
    if let Some(p) = &place { limits::text("place", p, 100)?; }
    s.data(|c| vaulty_core::card_benefits::best_for(c, today(), amount.as_ref(), category_id.as_deref(), place.as_deref()))
}

// ============================================================ documents

// ------------------------------------------------------------ businesses (separate books)

/// The member's businesses (from the personal book).
#[tauri::command] fn businesses_list(s: S) -> Result<Vec<Value>> { s.personal(vaulty_core::business::list) }

/// The book in use: `null` for personal, or the business's id, name and details.
#[tauri::command] fn book_current(s: S) -> Result<Option<Value>> {
    let mut guard = s.session.lock().map_err(poisoned)?;
    let sess = live(&mut guard)?;
    match &sess.book { Some(b) => vaulty_core::business::profile(&b.conn), None => Ok(None) }
}

/// Opens a business book in place of the personal one (`id`), or goes back to personal (`null`).
#[tauri::command] fn book_switch(s: S, id: Option<String>) -> Result<Option<Value>> {
    let mut guard = s.session.lock().map_err(poisoned)?;
    let sess = live(&mut guard)?;
    sess.book = None;
    let Some(id) = id else { return Ok(None) };
    let user = sess.user.id.clone();
    let path = s.business_db_path(&user, &id);
    let key = vaulty_core::business::key_of(&sess.conn, &sess.keys.data_key, &id)?;
    let conn = vaulty_core::business::open(&sess.conn, &sess.keys.data_key, &path, &id)?;
    let snaps = s.snapshot_dir(&user).join(format!("biz-{id}"));
    // Daily safety copy and trash clean-up for the business book too (best effort).
    if vaulty_core::snapshots::age_of_latest(&snaps).ok().flatten().is_none_or(|age| age > vaulty_core::snapshots::DAILY_INTERVAL_SECS) {
        let _ = vaulty_core::snapshots::take(&conn, &key, &snaps, "daily");
    }
    let _ = vaulty_core::documents::purge(&conn, &s.documents_dir(&user).join(format!("biz-{id}")));
    let profile = vaulty_core::business::profile(&conn)?;
    sess.book = Some(Book { id, conn, key, quiet: Household::open_in_memory()? });
    Ok(profile)
}

/// Starts a business: its own book, a capital account in the personal book, and switches to it.
#[tauri::command] fn business_create(s: S, business: Value) -> Result<Option<Value>> {
    limits::check(&business)?;
    let id = {
        let mut guard = s.session.lock().map_err(poisoned)?;
        let sess = live(&mut guard)?;
        if sess.book.is_some() { return Err(Error::Invalid("switch back to your personal book to start a business".into())); }
        let user = sess.user.id.clone();
        std::fs::create_dir_all(s.data_dir.join("users"))?;
        let (id, conn) = vaulty_core::history::record(&sess.conn, "business.create", |p| vaulty_core::business::create(p, &sess.keys.data_key, |id| s.business_db_path(&user, id), &business))?;
        drop(conn);
        id
    };
    book_switch(s, Some(id))
}

/// Changes the business in use: its name and registration details.
#[tauri::command] fn business_update(s: S, business: Value) -> Result<Option<Value>> {
    limits::check(&business)?;
    s.business(|p, b, _| { vaulty_core::business::update(p, &b.conn, &b.id, &business)?; vaulty_core::business::profile(&b.conn) })
}

/// Money between the owner and the business in use, on both books.
#[tauri::command] fn business_owner_money(s: S, money: Value) -> Result<Value> {
    limits::check(&money)?;
    s.business(|p, b, _| vaulty_core::history::record(&b.conn, "business.owner_money", |bc| vaulty_core::history::record(p, "business.owner_money", |pc| vaulty_core::business::owner_money(pc, bc, &b.id, &money))))
}
/// The personal book's accounts, for choosing where owner money came from or went (from inside a business).
#[tauri::command] fn personal_accounts(s: S) -> Result<Vec<Value>> { s.personal(vaulty_core::accounts::get_all) }
#[tauri::command] fn business_owner_totals(s: S) -> Result<Value> { s.data(vaulty_core::business::owner_totals) }
#[tauri::command] fn business_archive(s: S, id: String, archived: bool) -> Result<()> {
    s.personal_only()?;
    s.personal(|c| vaulty_core::business::set_archived(c, &id, archived))
}

/// Goals with progress and plan.
#[tauri::command] fn goals_list(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::goals::list(c, today())) }
#[tauri::command] fn goals_summary(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::goals::summary(c, today())) }
#[tauri::command] fn goals_create(s: S, goal: Value) -> Result<Value> { limits::check(&goal)?; s.act("goal.create", |c| vaulty_core::goals::create(c, &goal)) }
#[tauri::command] fn goals_update(s: S, id: String, goal: Value) -> Result<()> { limits::check(&goal)?; s.act("goal.update", |c| vaulty_core::goals::update(c, &id, &goal)) }
#[tauri::command] fn goals_set_done(s: S, id: String, done: bool) -> Result<()> { s.act(if done { "goal.done" } else { "goal.reopen" }, |c| vaulty_core::goals::set_done(c, &id, done, today())) }
#[tauri::command] fn goals_delete(s: S, id: String) -> Result<()> { s.act("goal.delete", |c| vaulty_core::goals::delete(c, &id)) }
/// Insurance policies with premiums, due dates and claims.
#[tauri::command] fn insurance_list(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::insurance::list(c, today())) }
#[tauri::command] fn insurance_summary(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::insurance::summary(c, today())) }
#[tauri::command] fn insurance_create(s: S, policy: Value) -> Result<Value> { limits::check(&policy)?; s.act("policy.create", |c| vaulty_core::insurance::create(c, &policy)) }
#[tauri::command] fn insurance_update(s: S, id: String, policy: Value) -> Result<()> { limits::check(&policy)?; s.act("policy.update", |c| vaulty_core::insurance::update(c, &id, &policy)) }
#[tauri::command] fn insurance_delete(s: S, id: String) -> Result<()> { s.act("policy.delete", |c| vaulty_core::insurance::delete(c, &id)) }
#[tauri::command] fn insurance_pay(s: S, id: String, payment: Value) -> Result<Value> { limits::check(&payment)?; s.act("policy.pay", |c| vaulty_core::insurance::pay(c, &id, &payment)) }
#[tauri::command] fn insurance_payment_delete(s: S, id: String) -> Result<()> { s.act("policy.payment_delete", |c| vaulty_core::insurance::delete_payment(c, &id)) }
#[tauri::command] fn insurance_claim_add(s: S, policy_id: String, claim: Value) -> Result<Value> { limits::check(&claim)?; s.act("policy.claim_add", |c| vaulty_core::insurance::add_claim(c, &policy_id, &claim)) }
#[tauri::command] fn insurance_claim_update(s: S, id: String, claim: Value) -> Result<()> { limits::check(&claim)?; s.act("policy.claim_update", |c| vaulty_core::insurance::update_claim(c, &id, &claim)) }
#[tauri::command] fn insurance_claim_delete(s: S, id: String) -> Result<()> { s.act("policy.claim_delete", |c| vaulty_core::insurance::delete_claim(c, &id)) }
/// Net worth at each month end and today, per currency.
#[tauri::command] fn networth_timeline(s: S, months: u32) -> Result<Vec<Value>> { s.data(|c| vaulty_core::networth::timeline(c, months, today())) }
/// Nominees on accounts, investments and policies, missing ones first.
#[tauri::command] fn nominees_list(s: S) -> Result<Value> { s.data(|c| vaulty_core::nominees::overview(c, today())) }
#[tauri::command] fn nominees_set(s: S, kind: String, id: String, nominee: Option<String>) -> Result<()> {
    if let Some(n) = &nominee { limits::text("nominee", n, 200)?; }
    s.act("nominee.set", |c| vaulty_core::nominees::set(c, &kind, &id, nominee.as_deref()))
}
/// Unusual spending in the last 30 days (double charges, unusual amounts, large first payments).
#[tauri::command] fn anomalies_list(s: S) -> Result<Vec<vaulty_core::anomalies::Anomaly>> { s.data(|c| vaulty_core::anomalies::find(c, today())) }
#[tauri::command] fn anomalies_dismiss(s: S, key: String) -> Result<()> { s.data(|c| vaulty_core::anomalies::dismiss(c, &key)) }
#[tauri::command] fn anomalies_reset(s: S) -> Result<usize> { s.data(vaulty_core::anomalies::reset) }
/// The money journal.
#[tauri::command] fn journal_list(s: S, search: Option<String>, tag: Option<String>) -> Result<Value> { s.data(|c| vaulty_core::journal::list(c, search.as_deref(), tag.as_deref(), today())) }
#[tauri::command] fn journal_month_facts(s: S, month: String) -> Result<Value> { s.data(|c| vaulty_core::journal::month_facts(c, &month)) }
#[tauri::command] fn journal_create(s: S, entry: Value) -> Result<Value> { limits::check(&entry)?; s.act("journal.create", |c| vaulty_core::journal::create(c, &entry)) }
#[tauri::command] fn journal_update(s: S, id: String, entry: Value) -> Result<()> { limits::check(&entry)?; s.act("journal.update", |c| vaulty_core::journal::update(c, &id, &entry)) }
#[tauri::command] fn journal_delete(s: S, id: String) -> Result<()> { s.act("journal.delete", |c| vaulty_core::journal::delete(c, &id)) }
/// A money calculator (SIP, EMI, FD …); nothing is read or kept.
#[tauri::command] fn calc_run(kind: String, input: Value) -> Result<Value> { limits::check(&input)?; vaulty_core::calc::run(&kind, &input) }
#[tauri::command]
fn documents_list(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::documents::list(c, today())) }
#[tauri::command]
fn documents_create(s: S, document: Value) -> Result<Value> { limits::check(&document)?; s.act("document.create", |c| vaulty_core::documents::create(c, &document, today())) }
#[tauri::command]
fn documents_update(s: S, id: String, document: Value) -> Result<Value> { limits::check(&document)?; s.act("document.update", |c| vaulty_core::documents::update(c, &id, &document, today())) }
#[tauri::command]
fn documents_delete(s: S, id: String) -> Result<()> { s.act("document.delete", |c| vaulty_core::documents::delete(c, &id)) }
#[tauri::command]
fn documents_remove_file(s: S, file_id: String) -> Result<()> { s.act("document.file_remove", |c| vaulty_core::documents::remove_file(c, &file_id)) }

/// Attaches a file the person chose (a PDF or photo), encrypted.
#[tauri::command]
fn documents_add_file(s: S, document_id: String, path: String) -> Result<Value> {
    s.chosen(&path)?;
    let bytes = read_limited(&path, vaulty_core::documents::MAX_FILE as u64)?;
    let dir = s.files_dir()?;
    s.data_with_keys(|c, k| vaulty_core::history::record(c, "document.file_add", |c| vaulty_core::documents::add_file(c, k, &dir, &document_id, &path, &bytes)))
}

/// A photo, for previewing in the window (base64). PDFs are saved and opened outside Vaulty.
#[tauri::command]
fn documents_preview(s: S, file_id: String) -> Result<Value> {
    use base64::Engine;
    let dir = s.files_dir()?;
    let (name, mime, bytes) = s.data_with_keys(|c, k| vaulty_core::documents::read_file(c, k, &dir, &file_id))?;
    if !mime.starts_with("image/") { return Err(Error::Invalid("only photos can be shown here; save a copy to open a PDF".into())); }
    Ok(json!({ "name": name, "mime": mime, "data": base64::engine::general_purpose::STANDARD.encode(bytes) }))
}

/// Saves a decrypted copy where the person chose.
#[tauri::command]
fn documents_save_file(s: S, file_id: String, path: String) -> Result<()> {
    s.chosen(&path)?;
    let dir = s.files_dir()?;
    let (_, _, bytes) = s.data_with_keys(|c, k| vaulty_core::documents::read_file(c, k, &dir, &file_id))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

// ============================================================ projects

#[tauri::command]
fn projects_list(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::projects::list) }
#[tauri::command]
fn projects_get(s: S, id: String) -> Result<Value> { s.data(|c| vaulty_core::projects::get(c, &id)) }
#[tauri::command]
fn projects_items(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::projects::items(c, &id)) }
#[tauri::command]
fn projects_payments(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::projects::payments(c, &id)) }
#[tauri::command]
fn projects_link_suggestions(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::projects::link_suggestions(c, &id)) }
#[tauri::command]
fn projects_names(s: S) -> Result<Value> { s.data(vaulty_core::projects::names) }
#[tauri::command]
fn projects_create(s: S, project: Value) -> Result<Value> { limits::check(&project)?; s.act("project.create", |c| vaulty_core::projects::create(c, &project)) }
#[tauri::command]
fn projects_update(s: S, id: String, project: Value) -> Result<Value> { limits::check(&project)?; s.act("project.update", |c| vaulty_core::projects::update(c, &id, &project)) }
#[tauri::command]
fn projects_delete(s: S, id: String) -> Result<()> { s.act("project.delete", |c| vaulty_core::projects::delete(c, &id)) }
#[tauri::command]
fn projects_section_add(s: S, project_id: String, section: Value) -> Result<Value> { limits::check(&section)?; s.act("project.section_add", |c| vaulty_core::projects::add_section(c, &project_id, &section)) }
#[tauri::command]
fn projects_section_update(s: S, id: String, section: Value) -> Result<()> { limits::check(&section)?; s.act("project.section_update", |c| vaulty_core::projects::update_section(c, &id, &section)) }
#[tauri::command]
fn projects_section_delete(s: S, id: String) -> Result<()> { s.act("project.section_delete", |c| vaulty_core::projects::delete_section(c, &id)) }
#[tauri::command]
fn projects_section_order(s: S, project_id: String, ids: Vec<String>) -> Result<()> {
    if ids.len() > 100 { return Err(Error::Invalid("too many sections".into())); }
    s.act("project.section_order", |c| vaulty_core::projects::order_sections(c, &project_id, &ids))
}
#[tauri::command]
fn projects_item_add(s: S, project_id: String, item: Value) -> Result<Value> { limits::check(&item)?; s.act("project.item_add", |c| vaulty_core::projects::add_item(c, &project_id, &item)) }
#[tauri::command]
fn projects_item_update(s: S, id: String, item: Value) -> Result<Value> { limits::check(&item)?; s.act("project.item_update", |c| vaulty_core::projects::update_item(c, &id, &item)) }
#[tauri::command]
fn projects_item_delete(s: S, id: String) -> Result<()> { s.act("project.item_delete", |c| vaulty_core::projects::delete_item(c, &id)) }
#[tauri::command]
fn projects_payment_add(s: S, project_id: String, payment: Value) -> Result<Value> { limits::check(&payment)?; s.act("project.payment_add", |c| vaulty_core::projects::add_payment(c, &project_id, &payment)) }
#[tauri::command]
fn projects_payment_update(s: S, id: String, payment: Value) -> Result<Value> { limits::check(&payment)?; s.act("project.payment_update", |c| vaulty_core::projects::update_payment(c, &id, &payment)) }
#[tauri::command]
fn projects_payment_delete(s: S, id: String) -> Result<()> { s.act("project.payment_delete", |c| vaulty_core::projects::delete_payment(c, &id)) }

#[tauri::command]
fn projects_files(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::projects::files(c, &id)) }
#[tauri::command]
fn projects_file_link(s: S, id: String, link: Value) -> Result<Value> { limits::check(&link)?; s.act("project.file_link", |c| vaulty_core::projects::link_file(c, &id, &link)) }
#[tauri::command]
fn projects_file_remove(s: S, id: String) -> Result<()> { s.act("project.file_remove", |c| vaulty_core::projects::remove_file(c, &id)) }

/// Keeps a bill or receipt the person chose (a PDF or photo) with the project, encrypted.
#[tauri::command]
fn projects_file_add(s: S, project_id: String, path: String, link: Value) -> Result<Value> {
    limits::check(&link)?;
    s.chosen(&path)?;
    let bytes = read_limited(&path, vaulty_core::documents::MAX_FILE as u64)?;
    let dir = s.files_dir()?;
    s.data_with_keys(|c, k| vaulty_core::history::record(c, "project.file_add", |c| vaulty_core::projects::add_file(c, k, &dir, &project_id, &path, &bytes, &link)))
}

/// A photo of a bill, for showing in the window (base64). PDFs are saved and opened outside Vaulty.
#[tauri::command]
fn projects_file_preview(s: S, id: String) -> Result<Value> {
    use base64::Engine;
    let dir = s.files_dir()?;
    let (name, mime, bytes) = s.data_with_keys(|c, k| vaulty_core::projects::read_file(c, k, &dir, &id))?;
    if !mime.starts_with("image/") { return Err(Error::Invalid("only photos can be shown here; save a copy to open a PDF".into())); }
    Ok(json!({ "name": name, "mime": mime, "data": base64::engine::general_purpose::STANDARD.encode(bytes) }))
}

/// Saves a decrypted copy of a bill where the person chose.
#[tauri::command]
fn projects_file_save(s: S, id: String, path: String) -> Result<()> {
    s.chosen(&path)?;
    let dir = s.files_dir()?;
    let (_, _, bytes) = s.data_with_keys(|c, k| vaulty_core::projects::read_file(c, k, &dir, &id))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Reads a kept PDF bill into draft items, to check before adding.
#[tauri::command]
fn projects_read_bill(s: S, id: String) -> Result<vaulty_core::invoice::Draft> {
    let dir = s.files_dir()?;
    s.data_with_keys(|c, k| vaulty_core::projects::read_bill(c, k, &dir, &id, ocr::recognize))
}
/// Which text recognition this computer has for photos of bills ("" when none).
#[tauri::command]
fn ocr_engine() -> &'static str { ocr::engine() }
#[tauri::command]
fn projects_items_add(s: S, project_id: String, batch: Value) -> Result<Vec<Value>> { limits::check(&batch)?; s.act("project.items_add", |c| vaulty_core::projects::add_items(c, &project_id, &batch)) }

#[tauri::command]
fn projects_members(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::projects::members(c, &id)) }
#[tauri::command]
fn projects_members_set(s: S, id: String, members: Value) -> Result<Vec<Value>> { limits::check(&members)?; s.act("project.members", |c| vaulty_core::projects::set_members(c, &id, &members)) }
#[tauri::command]
fn projects_split(s: S, id: String) -> Result<Value> { s.data(|c| vaulty_core::projects::split(c, &id)) }

#[tauri::command]
fn projects_settle_preview(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::projects::settle_preview(c, &id)) }
#[tauri::command]
fn projects_settle_record(s: S, id: String, settle: Value) -> Result<usize> { limits::check(&settle)?; s.act("project.settle", |c| vaulty_core::projects::record_settlement(c, &id, &settle, today())) }

/// Saves the project as a password-protected file to send to someone who uses Vaulty.
#[tauri::command]
fn projects_share_export(s: S, id: String, path: String, password: String, with_files: bool) -> Result<Value> {
    limits::text("password", &password, limits::PASSWORD)?;
    if password.chars().count() < 8 { return Err(Error::Invalid("use a password of at least 8 characters".into())); }
    s.chosen(&path)?;
    let dir = s.files_dir()?;
    let data = s.data_with_keys(|c, k| vaulty_core::history::record(c, "project.share", |c| vaulty_core::project_share::export(c, k, &dir, &id, with_files)))?;
    std::fs::write(path, vaulty_core::backup::encrypt_value(&data, &password, None)?)?;
    Ok(json!({ "files": data["files"].as_array().map_or(0, Vec::len), "files_left_out": data["files_left_out"] }))
}

fn read_share(s: &S, path: &str, password: &str) -> Result<Value> {
    limits::text("password", password, limits::PASSWORD)?;
    s.chosen(path)?;
    let bytes = read_limited(path, 200 * 1024 * 1024)?;
    if vaulty_core::backup::header(&bytes).is_none() { return Err(Error::Invalid("this isn't a shared Vaulty project".into())); }
    vaulty_core::backup::decrypt(&bytes, password)
}

/// What a shared project file holds, before opening it.
#[tauri::command]
fn projects_share_peek(s: S, path: String, password: String) -> Result<Value> {
    let data = read_share(&s, &path, &password)?;
    s.data(|c| vaulty_core::project_share::peek(c, &data))
}

/// Opens a shared project file: adds the project, or updates the one it came from.
#[tauri::command]
fn projects_share_import(s: S, path: String, password: String, me: Option<String>) -> Result<Value> {
    let data = read_share(&s, &path, &password)?;
    let dir = s.files_dir()?;
    s.data_with_keys(|c, k| vaulty_core::history::record(c, "project.share_open", |c| vaulty_core::project_share::import(c, k, &dir, &data, me.as_deref())))
}

/// The project as an Excel workbook, saved where the person chose.
#[tauri::command]
fn projects_export_xlsx(s: S, id: String, path: String, labels: Value) -> Result<()> {
    limits::check(&labels)?;
    s.chosen(&path)?;
    let bytes = s.data(|c| vaulty_core::project_export::to_xlsx(c, &id, &labels))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

// ============================================================ investments

#[tauri::command]
fn investments_list(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::investments::list(c, today())) }
#[tauri::command]
fn investments_summary(s: S) -> Result<Vec<Value>> { s.data(|c| vaulty_core::investments::summary(c, today())) }
#[tauri::command]
fn investments_transactions(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::investments::transactions(c, &id)) }
#[tauri::command]
fn investments_create(s: S, holding: Value) -> Result<Value> { limits::check(&holding)?; s.act("investment.create", |c| vaulty_core::investments::create(c, &holding, today())) }
#[tauri::command]
fn investments_update(s: S, id: String, holding: Value) -> Result<Value> { limits::check(&holding)?; s.act("investment.update", |c| vaulty_core::investments::update(c, &id, &holding, today())) }
#[tauri::command]
fn investments_set_price(s: S, id: String, price: Value) -> Result<Value> { s.act("investment.price", |c| vaulty_core::investments::set_price(c, &id, &price, today(), today())) }
#[tauri::command]
fn investments_delete(s: S, id: String) -> Result<()> { s.act("investment.delete", |c| vaulty_core::investments::delete(c, &id)) }

fn read_cas(s: &S, path: &str, password: Option<String>) -> Result<vaulty_core::cas::Cas> {
    s.chosen(path)?;
    let bytes = read_limited(path, vaulty_core::pdf_text::MAX_BYTES as u64)?;
    let password = password.map(zeroize::Zeroizing::new);
    vaulty_core::cas::read(&bytes, password.as_deref().map(String::as_str))
}

/// What reading a fund statement (CAMS or KFintech) would add, before anything is saved.
#[tauri::command]
fn cas_preview(s: S, path: String, password: Option<String>) -> Result<Value> {
    let cas = read_cas(&s, &path, password)?;
    s.data(|c| vaulty_core::investments::preview(c, &cas))
}

/// Reads the statement in.
#[tauri::command]
fn cas_import(s: S, path: String, password: Option<String>) -> Result<Value> {
    let cas = read_cas(&s, &path, password)?;
    s.act("investment.statement", |c| vaulty_core::investments::import_cas(c, &cas))
}

// ------------------------------------------------------------ nominee

/// On the sign-in screen: whether this member has a nominee, and any request in progress.
#[tauri::command]
fn nominee_for(s: S, user_id: String) -> Result<vaulty_core::profiles::NomineeStatus> {
    s.registry(|r| r.nominee(&user_id))
}

/// The nominee starts the waiting period with their card.
#[tauri::command]
fn nominee_start(s: S, user_id: String, code: String) -> Result<vaulty_core::profiles::NomineeStatus> {
    limits::text("code", &code, 100)?;
    s.registry(|r| r.nominee_start(&user_id, &code))
}

/// What the nominee sees. Read once and returned; nothing stays open.
#[derive(Serialize)]
struct NomineeOpened { owner_name: String, letter: Option<String>, estate: vaulty_core::estate::Estate }

/// After the wait: the nominee card plus one helper's approval (collected as when recovering).
#[tauri::command]
fn sign_in_nominee(s: S, user_id: String, code: String) -> Result<NomineeOpened> {
    limits::text("code", &code, 100)?;
    let helper = {
        let h = s.helping.lock().map_err(poisoned)?;
        match h.as_ref() {
            Some(x) if x.owner == user_id && x.started.elapsed() <= HELPING_TTL && !x.shares.is_empty() => x.shares[0].clone(),
            _ => return Err(Error::Auth("ask one of their helpers to approve first".into())),
        }
    };
    let view = s.registry(|r| r.open_as_nominee(&user_id, &code, &helper))?;
    let conn = db::open(&s.user_db_path(&user_id), &view.data_key)?;
    conn.execute_batch("PRAGMA query_only = 1;")?;
    let estate = vaulty_core::estate::summary(&conn, today())?;
    *s.helping.lock().map_err(poisoned)? = None;
    Ok(NomineeOpened { owner_name: view.owner_name, letter: view.letter, estate })
}

/// The signed-in member's nominee, with their letter.
#[tauri::command]
fn nominee_mine(s: S) -> Result<Value> {
    let me = s.current_user()?;
    let status = s.registry(|r| r.nominee(&me.id))?;
    let letter = s.keys(|k| s.registry(|r| r.nominee_letter(k)))?;
    Ok(json!({ "nominee": status, "letter": letter }))
}

#[tauri::command]
fn nominee_set(s: S, password: String, name: String, wait_days: i64, letter: Option<String>) -> Result<vaulty_core::profiles::HelperCard> {
    limits::text("name", &name, 100)?;
    if let Some(l) = &letter { limits::text("letter", l, 10_000)?; }
    let me = s.current_user()?;
    s.registry(|r| r.set_nominee(&me.id, &password, &name, wait_days, letter.as_deref()))
}

#[tauri::command]
fn nominee_remove(s: S, password: String) -> Result<()> {
    let me = s.current_user()?;
    s.registry(|r| r.remove_nominee(&me.id, &password))
}

/// The member answers a nominee's request: cancel it (`allow` false) or let it go ahead now.
#[tauri::command]
fn nominee_answer(s: S, allow: bool) -> Result<()> {
    s.keys(|k| s.registry(|r| r.nominee_answer(k, allow)))
}

/// The member's own "In case of emergency" summary, as their nominee would see it.
#[tauri::command]
fn estate_preview(s: S) -> Result<vaulty_core::estate::Estate> {
    s.personal(|c| vaulty_core::estate::summary(c, today()))
}

/// The signed-in member's own helpers.
#[tauri::command]
fn helpers_mine(s: S) -> Result<Value> {
    let me = s.current_user()?;
    let helpers = s.registry(|r| r.helpers(&me.id))?;
    let candidates = s.registry(|r| r.helper_candidates(&me.id))?;
    Ok(json!({ "helpers": helpers, "candidates": candidates }))
}

/// Chooses three helpers; returns the cards to print (shown once).
#[tauri::command]
fn helpers_set(s: S, password: String, choices: Value) -> Result<Vec<vaulty_core::profiles::HelperCard>> {
    limits::check(&choices)?;
    let choices: Vec<vaulty_core::profiles::HelperChoice> = serde_json::from_value(choices).map_err(|_| Error::Invalid("choose three helpers".into()))?;
    let me = s.current_user()?;
    s.registry(|r| r.set_helpers(&me.id, &password, &choices))
}

#[tauri::command]
fn helpers_remove(s: S, password: String) -> Result<()> {
    let me = s.current_user()?;
    s.registry(|r| r.remove_helpers(&me.id, &password))
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
    *s.helping.lock().map_err(poisoned)? = None;
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
fn users_set_senior(s: S, user_id: String, on: bool) -> Result<UserSummary> {
    let me = s.current_user()?;
    let user = s.registry(|r| r.set_senior(&me.id, &user_id, on))?;
    if user.id == me.id {
        if let Some(sess) = s.session.lock().map_err(poisoned)?.as_mut() { sess.user = user.clone(); }
    }
    Ok(user)
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
#[tauri::command] fn accounts_create(s: S, account: Value) -> Result<Value> { limits::check(&account)?; s.act("account.create", |c| accounts::create(c, &account)) }
#[tauri::command] fn accounts_update(s: S, id: String, account: Value) -> Result<Value> { limits::check(&account)?; s.act("account.update", |c| accounts::update(c, &id, &account)) }
#[tauri::command] fn accounts_delete(s: S, id: String) -> Result<()> { s.act("account.delete", |c| accounts::delete(c, &id)) }

#[tauri::command] fn categories_get_all(s: S) -> Result<Vec<Value>> { s.data(categories::get_all) }
#[tauri::command] fn categories_create(s: S, category: Value) -> Result<Value> { limits::check(&category)?; s.act("category.create", |c| categories::create(c, &category)) }
#[tauri::command] fn categories_update(s: S, id: String, category: Value) -> Result<Value> { limits::check(&category)?; s.act("category.update", |c| categories::update(c, &id, &category)) }
#[tauri::command] fn categories_delete(s: S, id: String) -> Result<()> { s.act("category.delete", |c| categories::delete(c, &id)) }

// ============================================================ statement import

// ============================================================ choosing files

/// What a file dialog is for. The kind decides the file types offered and whether it opens or saves.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Pick { Statement, Backup, SaveBackup, SaveCsv, Calendar, SaveCalendar, SaveReport, Document, SaveDocument, FundStatement, SaveProject, ProjectShare, SaveProjectShare, TaxForm }

impl AppState {
    /// Refuses a path the person didn't choose in a file dialog this run.
    fn chosen(&self, path: &str) -> Result<()> {
        if self.picked.lock().map_err(|_| Error::Invalid("busy".into()))?.iter().any(|p| p == path) { Ok(()) }
        else { Err(Error::Invalid("choose the file again".into())) }
    }
}

/// Opens the system file dialog and returns what the person chose (nothing if they cancelled).
/// `label` names the file types in the dialog, in the interface's language.
#[tauri::command]
async fn pick_file(app: tauri::AppHandle, s: S<'_>, kind: Pick, label: String, name: Option<String>) -> Result<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    limits::text("label", &label, 100)?;
    // A suggested file name (for saving a copy of a document): the last path part only.
    let suggested: String = name.as_deref().and_then(|n| std::path::Path::new(n).file_name()).and_then(|n| n.to_str()).unwrap_or("document").chars().take(120).collect();
    let stamp = chrono::Local::now().format("%Y-%m-%d").to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let d = app.dialog().file();
    match kind {
        Pick::Statement => d.add_filter(label, &["csv", "txt", "xlsx", "xls", "xlsm", "ods", "pdf"]).pick_file(move |p| { let _ = tx.send(p); }),
        Pick::Backup => d.add_filter(label, &["vaulty", "json"]).pick_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveBackup => d.add_filter(label, &["vaulty"]).set_file_name(format!("vaulty-backup-{stamp}.vaulty")).save_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveCsv => d.add_filter(label, &["csv"]).set_file_name(format!("vaulty-transactions-{stamp}.csv")).save_file(move |p| { let _ = tx.send(p); }),
        Pick::Calendar => d.add_filter(label, &["ics"]).pick_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveCalendar => d.add_filter(label, &["ics"]).set_file_name(format!("vaulty-calendar-{stamp}.ics")).save_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveReport => d.add_filter(label, &["xlsx"]).set_file_name(format!("vaulty-report-{stamp}.xlsx")).save_file(move |p| { let _ = tx.send(p); }),
        Pick::Document => d.add_filter(label, &["pdf", "png", "jpg", "jpeg", "webp"]).pick_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveDocument => d.set_file_name(suggested).save_file(move |p| { let _ = tx.send(p); }),
        Pick::FundStatement | Pick::TaxForm => d.add_filter(label, &["pdf"]).pick_file(move |p| { let _ = tx.send(p); }),
        Pick::ProjectShare => d.add_filter(label, &["vaultyproject"]).pick_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveProjectShare => d.add_filter(label, &["vaultyproject"]).set_file_name(format!("{}.vaultyproject", suggested.trim_end_matches(".vaultyproject"))).save_file(move |p| { let _ = tx.send(p); }),
        Pick::SaveProject => d.add_filter(label, &["xlsx"]).set_file_name(format!("{}-{stamp}.xlsx", suggested.trim_end_matches(".xlsx"))).save_file(move |p| { let _ = tx.send(p); }),
    }
    let Some(chosen) = rx.await.ok().flatten() else { return Ok(None) };
    let path = chosen.into_path().map_err(|_| Error::Invalid("that location can't be used".into()))?.to_string_lossy().into_owned();
    let mut picked = s.picked.lock().map_err(|_| Error::Invalid("busy".into()))?;
    if picked.len() >= 50 { picked.remove(0); }
    picked.push(path.clone());
    Ok(Some(path))
}

/// Reads a statement the member picked, refusing anything over the size limit before loading it.
fn read_statement(path: &str, password: Option<&str>) -> Result<statements::Table> {
    let p = std::path::Path::new(path);
    if std::fs::metadata(p)?.len() as usize > statements::MAX_BYTES { return Err(Error::Invalid("that file is larger than 20 MB".into())); }
    if let Some(pw) = password { limits::text("password", pw, limits::PASSWORD)?; }
    let bytes = std::fs::read(p)?;
    if bytes.starts_with(b"%PDF") { return statements::read_pdf(&bytes, password.filter(|p| !p.is_empty())); }
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    statements::read(&bytes, &name)
}

/// Day-first unless the profile reads dates month-first.
fn default_order(c: &Connection) -> Result<statements::DateOrder> {
    let s = settings::get(c)?;
    let us = s["date_format"] == "mdy" || (s["date_format"] != "dmy" && s["locale"].as_str().is_some_and(|l| l.ends_with("-US") || l.ends_with("-PH")));
    Ok(if us { statements::DateOrder::Mdy } else { statements::DateOrder::Dmy })
}

#[tauri::command]
fn statement_inspect(s: S, path: String, account_id: String, password: Option<String>) -> Result<Value> {
    s.chosen(&path)?;
    let t = read_statement(&path, password.as_deref())?;
    s.data(|c| statements::inspect(c, &t, &account_id, default_order(c)?))
}

#[tauri::command]
fn statement_preview(s: S, path: String, account_id: String, mapping: statements::Mapping, password: Option<String>) -> Result<Vec<statements::ParsedRow>> {
    s.chosen(&path)?;
    let t = read_statement(&path, password.as_deref())?;
    s.data(|c| statements::preview(c, &t, &mapping, &account_id))
}

/// Imports the chosen rows: `skip` lists new rows to leave out, `include` duplicates to add anyway.
#[tauri::command]
fn statement_import(s: S, path: String, account_id: String, mapping: statements::Mapping, skip: Vec<usize>, include: Vec<usize>, password: Option<String>) -> Result<usize> {
    s.chosen(&path)?;
    let t = read_statement(&path, password.as_deref())?;
    s.take_snapshot("before-import")?;
    s.act("statement.import", |c| statements::import(c, &t, &mapping, &account_id, &skip, &include))
}

// ============================================================ recurring payments

fn today() -> chrono::NaiveDate { chrono::Local::now().date_naive() }
fn day(s: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| Error::Invalid("dates look like 2026-10-19".into()))
}

#[tauri::command] fn recurring_list(s: S) -> Result<Vec<Value>> { s.data(|c| recurring::list(c, today())) }
#[tauri::command] fn recurring_create(s: S, item: Value) -> Result<Value> { limits::check(&item)?; s.act("recurring.create", |c| recurring::create(c, &item, today())) }
#[tauri::command] fn recurring_update(s: S, id: String, item: Value) -> Result<Value> { limits::check(&item)?; s.act("recurring.update", |c| recurring::update(c, &id, &item, today())) }
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
#[tauri::command] fn recurring_pay(s: S, id: String, date: Option<String>, amount: Option<Value>) -> Result<Value> { if let Some(a) = &amount { limits::check(a)? }
    s.act_and_share("recurring.pay", |c, me, h| {
        let row = recurring::mark_paid(c, &id, date.as_deref(), amount.as_ref(), today())?;
        h.publish(me, &row)?;
        Ok(row)
    })
}

/// Each category's budget and what was spent in `month` (YYYY-MM).
#[tauri::command] fn budgets_month(s: S, month: String) -> Result<Value> { s.data(|c| vaulty_core::budgets::month(c, &month)) }
/// Sets a category's monthly budget; null or 0 removes it.
#[tauri::command] fn budgets_set(s: S, category_id: String, amount: Value) -> Result<Option<Value>> { limits::check(&amount)?;
    s.act("budget.set", |c| vaulty_core::budgets::set(c, &category_id, &amount))
}

/// Hidden transactions come back masked (amount, date and account only) unless the caller asks
/// with `reveal_hidden` and the Hidden section is unlocked.
#[tauri::command]
fn transactions_get_all(s: S, filters: Option<Value>) -> Result<Vec<Value>> { if let Some(f) = &filters { limits::check(f)? }
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
        let left = s.hidden_until.filter(|_| s.hidden_open()).and_then(|t| t.duration_since(SystemTime::now()).ok()).map(|d| d.as_secs()).unwrap_or(0);
        Ok(HiddenStatus { unlocked: left > 0, seconds_left: left, count: transactions::hidden_count(&s.conn)?, biometric: s.user.biometric_enabled && biometric::available() })
    }
}

#[tauri::command]
fn hidden_status(s: S) -> Result<HiddenStatus> {
    let mut guard = s.session.lock().map_err(poisoned)?;
    HiddenStatus::of(live(&mut guard)?)
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

/// Applies one change to many transactions as a single undoable action (see `transactions::bulk`).
#[tauri::command]
fn transactions_bulk(s: S, ids: Vec<String>, action: Value) -> Result<usize> { limits::check(&action)?;
    for id in &ids { s.guard_hidden(id)?; }
    s.act_and_share("transaction.bulk", |c, me, h| {
        let n = transactions::bulk(c, &ids, &action)?;
        h.republish_all(me, &transactions::shareable(c)?)?;
        Ok(n)
    })
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
fn transactions_create(s: S, transaction: Value) -> Result<Value> { limits::check(&transaction)?;
    s.act_and_share("transaction.create", |c, me, h| {
        let row = transactions::create(c, &with_default_visibility(h, me, transaction)?)?;
        learn_rule(c, &row)?;
        h.publish(me, &row)?;
        Ok(row)
    })
}
#[tauri::command]
fn transactions_update(s: S, id: String, transaction: Value) -> Result<Value> { limits::check(&transaction)?;
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
#[tauri::command]
fn settings_update(s: S, settings: Value) -> Result<Value> {
    limits::check(&settings)?;
    let out = s.act("settings.update", |c| settings::update(c, &settings))?;
    if settings.get("auto_lock_minutes").is_some() {
        let mut guard = s.session.lock().map_err(poisoned)?;
        let sess = live(&mut guard)?;
        sess.auto_lock = settings::auto_lock(&sess.conn)?;
        sess.last_active = SystemTime::now();
    }
    Ok(out)
}

#[tauri::command]
fn export_json(s: S, path: String) -> Result<()> {
    s.chosen(&path)?;
    s.guard_export()?;
    let v = s.data(export::to_json)?;
    std::fs::write(path, serde_json::to_vec_pretty(&v)?)?;
    Ok(())
}
#[tauri::command]
fn export_csv(s: S, path: String) -> Result<usize> {
    s.chosen(&path)?;
    s.guard_export()?;
    let (text, n) = s.data(export::to_csv)?;
    std::fs::write(path, text)?;
    Ok(n)
}
/// Writes a password-protected backup (`.vaulty`, see docs/BACKUP_FORMAT.md).
#[tauri::command]
fn backup_export(s: S, path: String, password: String, hint: Option<String>) -> Result<Value> {
    s.chosen(&path)?;
    s.guard_export()?;
    let dir = s.files_dir()?;
    let (mut data, files) = s.data_with_keys(|c, k| Ok((export::to_json(c)?, vaulty_core::documents::files_for_backup(c, k, &dir)?)))?;
    let has_files = s.data(|c| Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM vault_document_files)", [], |r| r.get::<_, bool>(0))?))?;
    let included = files.is_some();
    if let Some(f) = files { data["document_files"] = f; }
    std::fs::write(path, vaulty_core::backup::encrypt_value(&data, &password, hint.as_deref())?)?;
    // Too many document files to carry: the backup has their details, and the window says so.
    Ok(json!({ "document_files_left_out": has_files && !included }))
}

/// What a backup file is, before restoring: encrypted (with its hint and date) or plain JSON.
#[tauri::command]
fn backup_inspect(s: S, path: String) -> Result<Value> {
    s.chosen(&path)?;
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
    if std::fs::metadata(path)?.len() > max { return Err(Error::Invalid(format!("that file is larger than {} MB", max / (1024 * 1024)))); }
    Ok(std::fs::read(path)?)
}

/// Restores a backup into this profile (encrypted ones need their password). Existing items are
/// kept; a safety copy is taken first and the restore can be undone.
#[tauri::command]
fn backup_import(s: S, path: String, password: Option<String>) -> Result<Value> {
    s.chosen(&path)?;
    let bytes = read_limited(&path, 200 * 1024 * 1024)?;
    let data = match vaulty_core::backup::header(&bytes) {
        Some(_) => vaulty_core::backup::decrypt(&bytes, password.as_deref().ok_or_else(|| Error::Auth("this backup needs its password".into()))?)?,
        None => serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("that isn't a Vaulty backup".into()))?,
    };
    restore_backup(&s, data)
}

#[tauri::command]
fn import_json(s: S, path: String) -> Result<Value> {
    s.chosen(&path)?;
    restore_backup(&s, serde_json::from_slice(&read_limited(&path, 200 * 1024 * 1024)?)?)
}

fn restore_backup(s: &AppState, data: Value) -> Result<Value> {
    s.take_snapshot("before-import")?;
    let counts = s.act_and_share("import", |c, me, h| {
        let counts = export::from_json(c, &data)?;
        h.republish_all(me, &transactions::shareable(c)?)?;
        Ok(counts)
    })?;
    // Document files carried by an encrypted backup, re-encrypted with this member's key.
    let dir = s.files_dir()?;
    let files = s.data_with_keys(|c, k| vaulty_core::documents::restore_files(c, k, &dir, &data["document_files"]))?;
    let mut out = serde_json::to_value(counts)?;
    out["document_files"] = json!(files);
    Ok(out)
}
#[tauri::command] fn cards_list(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::cards::list) }
/// Card commands never receive a full card number: only network, last 4 and the other details.
#[tauri::command] fn cards_create(s: S, card: Value) -> Result<Value> { limits::check(&card)?; s.act("card.create", |c| vaulty_core::cards::create(c, &card)) }
#[tauri::command] fn cards_update(s: S, id: String, card: Value) -> Result<Value> { limits::check(&card)?; s.act("card.update", |c| vaulty_core::cards::update(c, &id, &card)) }
#[tauri::command] fn cards_delete(s: S, id: String) -> Result<()> { s.act("card.delete", |c| vaulty_core::cards::delete(c, &id)) }

#[tauri::command] fn hotkeys_list(s: S) -> Result<Vec<vaulty_core::hotkeys::Hotkey>> { s.data(vaulty_core::hotkeys::list) }
/// `keys`: the new combination, `""` to turn it off, or nothing to put back the default.
#[tauri::command] fn hotkeys_set(s: S, action: String, keys: Option<String>) -> Result<Vec<vaulty_core::hotkeys::Hotkey>> {
    limits::text("keys", keys.as_deref().unwrap_or_default(), 40)?;
    s.act("hotkeys.set", |c| vaulty_core::hotkeys::set(c, &action, keys.as_deref()))
}
#[tauri::command] fn hotkeys_reset(s: S) -> Result<Vec<vaulty_core::hotkeys::Hotkey>> { s.act("hotkeys.reset", vaulty_core::hotkeys::reset_all) }

// ============================================================ calendar

/// The country part of the profile's locale ("en-IN" → "IN"), for holidays and bank days.
fn country(c: &Connection) -> Result<String> {
    Ok(settings::get(c)?["locale"].as_str().and_then(|l| l.rsplit('-').next()).filter(|r| r.len() == 2).unwrap_or("IN").to_string())
}

#[tauri::command]
fn calendar_range(s: S, from: String, to: String) -> Result<Vec<vaulty_core::calendar::Item>> {
    s.data(|c| vaulty_core::calendar::range(c, &from, &to, &country(c)?, today()))
}
#[tauri::command] fn calendar_events(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::calendar::events) }
#[tauri::command] fn calendar_event_create(s: S, event: Value) -> Result<Value> { limits::check(&event)?; s.act("event.create", |c| vaulty_core::calendar::create_event(c, &event)) }
#[tauri::command] fn calendar_event_update(s: S, id: String, event: Value) -> Result<Value> { limits::check(&event)?; s.act("event.update", |c| vaulty_core::calendar::update_event(c, &id, &event)) }
#[tauri::command] fn calendar_event_delete(s: S, id: String) -> Result<()> { s.act("event.delete", |c| vaulty_core::calendar::delete_event(c, &id)) }

/// Writes the dates from `from` to `to` as an `.ics` file the person chose.
#[tauri::command]
fn calendar_export(s: S, path: String, from: String, to: String, include_amounts: bool) -> Result<usize> {
    s.chosen(&path)?;
    let items = s.data(|c| vaulty_core::calendar::range(c, &from, &to, &country(c)?, today()))?;
    let n = items.iter().filter(|i| i.kind != "paid").count();
    std::fs::write(path, vaulty_core::calendar::to_ics(&items, include_amounts))?;
    Ok(n)
}

fn read_ics(path: &str) -> Result<Vec<vaulty_core::calendar::Imported>> {
    vaulty_core::calendar::parse_ics(&read_limited(path, vaulty_core::calendar::MAX_ICS_BYTES as u64)?)
}

/// What an `.ics` file holds, before importing it.
#[tauri::command]
fn calendar_import_preview(s: S, path: String) -> Result<Vec<vaulty_core::calendar::Imported>> {
    s.chosen(&path)?;
    read_ics(&path)
}

#[tauri::command]
fn calendar_import(s: S, path: String, as_holidays: bool) -> Result<usize> {
    s.chosen(&path)?;
    let events = read_ics(&path)?;
    s.act("calendar.import", |c| vaulty_core::calendar::import_events(c, &events, as_holidays))
}

// ============================================================ reminders

/// Reminders due now that haven't been shown (the window words them and calls `notify_show`).
/// Also refreshes the dates kept for reminders while locked.
#[tauri::command]
fn reminders_take(s: S) -> Result<Vec<vaulty_core::reminders::Due>> {
    let me = s.current_user()?;
    let dates = s.data(|c| vaulty_core::reminders::locked_schedule(c, today()))?;
    s.registry(|r| r.set_locked_reminders(&me.id, &dates))?;
    s.data(|c| vaulty_core::reminders::take_due(c, today()))
}

/// "granted", "denied" or "prompt" (not asked yet).
#[tauri::command]
fn notify_permission(app: tauri::AppHandle, ask: bool) -> Result<String> {
    use tauri_plugin_notification::{NotificationExt, PermissionState};
    let n = app.notification();
    let mut state = n.permission_state().map_err(|e| Error::Invalid(e.to_string()))?;
    if ask && !matches!(state, PermissionState::Granted | PermissionState::Denied) {
        state = n.request_permission().map_err(|e| Error::Invalid(e.to_string()))?;
    }
    Ok(match state { PermissionState::Granted => "granted", PermissionState::Denied => "denied", _ => "prompt" }.into())
}

/// Shows one system notification. Short text only, at most 10 a minute.
#[tauri::command]
fn notify_show(app: tauri::AppHandle, s: S, title: String, body: String) -> Result<()> {
    use tauri_plugin_notification::NotificationExt;
    limits::text("title", &title, 120)?;
    limits::text("body", &body, 300)?;
    {
        let mut shown = s.shown.lock().map_err(poisoned)?;
        shown.retain(|t| t.elapsed() < Duration::from_secs(60));
        if shown.len() >= 10 { return Ok(()); }
        shown.push(Instant::now());
    }
    app.notification().builder().title(title).body(body).show().map_err(|e| Error::Invalid(format!("couldn't show a notification ({e})")))
}

// ============================================================ lend and borrow

#[tauri::command] fn people_list(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::people::list) }
#[tauri::command] fn people_totals(s: S) -> Result<Vec<Value>> { s.data(vaulty_core::people::totals) }
#[tauri::command] fn people_history(s: S, id: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::people::history(c, &id)) }
#[tauri::command] fn people_create(s: S, person: Value) -> Result<Value> { limits::check(&person)?; s.act("person.create", |c| vaulty_core::people::create(c, &person)) }
#[tauri::command] fn people_update(s: S, id: String, person: Value) -> Result<Value> { limits::check(&person)?; s.act("person.update", |c| vaulty_core::people::update(c, &id, &person)) }
#[tauri::command] fn people_delete(s: S, id: String) -> Result<()> { s.act("person.delete", |c| vaulty_core::people::delete(c, &id)) }

// ============================================================ to-dos

#[tauri::command] fn tasks_list(s: S, status: String) -> Result<Vec<Value>> { s.data(|c| vaulty_core::tasks::list(c, &status, today())) }
#[tauri::command] fn tasks_create(s: S, task: Value) -> Result<Value> { limits::check(&task)?; s.act("task.create", |c| vaulty_core::tasks::create(c, &task, today())) }
#[tauri::command] fn tasks_update(s: S, id: String, task: Value) -> Result<Value> { limits::check(&task)?; s.act("task.update", |c| vaulty_core::tasks::update(c, &id, &task, today())) }
#[tauri::command] fn tasks_set_done(s: S, id: String, done: bool) -> Result<Value> { s.act(if done { "task.done" } else { "task.reopen" }, |c| vaulty_core::tasks::set_done(c, &id, done, today())) }
#[tauri::command] fn tasks_delete(s: S, id: String) -> Result<()> { s.act("task.delete", |c| vaulty_core::tasks::delete(c, &id)) }

// ============================================================ subscriptions

#[tauri::command] fn subscriptions_detect(s: S) -> Result<Vec<vaulty_core::subscriptions::Candidate>> { s.data(|c| vaulty_core::subscriptions::detect(c, today())) }
#[tauri::command] fn subscriptions_dismiss(s: S, key: String) -> Result<()> { limits::text("key", &key, 100)?; s.data(|c| vaulty_core::subscriptions::dismiss(c, &key)) }
#[tauri::command] fn subscriptions_rises(s: S) -> Result<Vec<vaulty_core::subscriptions::RecurringRise>> { s.data(vaulty_core::subscriptions::recurring_rises) }

// ============================================================ reports

#[tauri::command]
fn reports_overview(s: S, from: String, to: String) -> Result<vaulty_core::reports::Overview> { s.data(|c| vaulty_core::reports::overview(c, &from, &to)) }
#[tauri::command]
fn reports_forecast(s: S, days: i64) -> Result<vaulty_core::reports::Forecast> { s.data(|c| vaulty_core::reports::forecast(c, today(), days)) }
#[tauri::command]
fn reports_safe_to_spend(s: S) -> Result<vaulty_core::reports::SafeToSpend> { s.data(|c| vaulty_core::reports::safe_to_spend(c, today())) }

/// Writes the report for `from`–`to` (and the forecast from today) as an Excel file the person chose.
/// `labels`: the sheet and column names in the interface's language.
#[tauri::command]
fn reports_export_xlsx(s: S, path: String, from: String, to: String, labels: Value) -> Result<()> {
    limits::check(&labels)?;
    s.chosen(&path)?;
    let bytes = s.data(|c| vaulty_core::report_export::to_xlsx(c, &from, &to, today(), &labels))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Opens the system print dialog for the window (where "Save as PDF" is offered).
#[tauri::command]
fn print_window(window: tauri::WebviewWindow) -> Result<()> {
    window.print().map_err(|_| Error::Invalid("printing isn't available".into()))
}

#[tauri::command]
fn reports_low_balances(s: S) -> Result<Vec<vaulty_core::reports::LowBalance>> { s.data(|c| vaulty_core::reports::low_balances(c, today())) }

#[tauri::command] fn market_get(s: S) -> Result<Value> { s.data(vaulty_core::market::prefs_view) }
#[tauri::command] fn market_update(s: S, prefs: Value) -> Result<Value> { limits::check(&prefs)?; s.data(|c| vaulty_core::market::update_prefs(c, &prefs)) }
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

/// Fund prices from AMFI: fetched once a day when turned on (or now, with `now`), and set on
/// every fund held. Returns what changed, or `skipped` when there was nothing to do.
#[tauri::command]
async fn fund_prices_update(s: S<'_>, now: bool) -> Result<Value> {
    let go = s.data(|c| Ok(vaulty_core::investments::has_funds(c)? && (now || vaulty_core::investments::fund_prices_due(c, today())?)))?;
    if !go { return Ok(json!({ "skipped": true })); }
    let text = market::fetch_amfi().await?;
    s.act("investment.prices", |c| vaulty_core::investments::apply_fund_prices(c, &text, today()))
}
#[tauri::command]
fn fund_prices_status(s: S) -> Result<Value> { s.data(vaulty_core::investments::fund_prices_status) }

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
    vaulty_core::snapshots::list(&s.snaps_dir()?)
}

#[tauri::command]
fn snapshots_create(s: S) -> Result<vaulty_core::snapshots::Snapshot> {
    s.take_snapshot("manual")
}

/// Replaces the member's data with a snapshot. The current data is snapshotted first, so a
/// restore can itself be reversed.
#[tauri::command]
fn snapshots_restore(s: S, name: String) -> Result<()> {
    s.personal_only()?;
    let mut guard = s.session.lock().map_err(poisoned)?;
    live(&mut guard)?;
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
    *guard = Some(Session::new(user, conn, keys, household, household_ready));
    result
}

/// Countries with tax rules, the rules themselves, and an estimate comparing every regime.
#[tauri::command] fn tax_countries() -> Result<Vec<String>> { vaulty_core::tax::countries() }
/// The kept tax plan for a year: figures, records, payments, estimate, what's left, instalments.
#[tauri::command] fn tax_plan_get(s: S, country: String, year: String) -> Result<Value> { s.data(|c| vaulty_core::tax_plan::plan(c, &country, &year, today())) }
/// Saves the planner's figures as they're typed (not an undoable step each time).
#[tauri::command] fn tax_plan_save(s: S, country: String, year: String, plan: Value) -> Result<()> { limits::check(&plan)?; s.data(|c| vaulty_core::tax_plan::save(c, &country, &year, &plan)) }
#[tauri::command] fn tax_payment_add(s: S, country: String, year: String, payment: Value) -> Result<Value> { limits::check(&payment)?; s.act("tax.payment_add", |c| vaulty_core::tax_plan::add_payment(c, &country, &year, &payment)) }
#[tauri::command] fn tax_payment_delete(s: S, id: String) -> Result<()> { s.act("tax.payment_delete", |c| vaulty_core::tax_plan::delete_payment(c, &id)) }
#[tauri::command] fn tax_links_set(s: S, country: String, year: String, links: Value) -> Result<Vec<Value>> { limits::check(&links)?; s.act("tax.links", |c| vaulty_core::tax_plan::set_links(c, &country, &year, &links)) }
/// What a Form 16 says, offered to the planner (nothing is kept from the file).
#[tauri::command] fn form16_read(s: S, path: String, password: Option<String>) -> Result<Value> {
    s.chosen(&path)?;
    let bytes = read_limited(&path, vaulty_core::pdf_text::MAX_BYTES as u64)?;
    let password = password.map(zeroize::Zeroizing::new);
    vaulty_core::form16::offer(&vaulty_core::form16::read_pdf(&bytes, password.as_deref().map(String::as_str))?)
}
/// Fills the plan from a Form 16: the chosen figures, the regime and TDS by quarter, as one step.
#[tauri::command] fn form16_apply(s: S, country: String, year: String, data: Value) -> Result<Value> { limits::check(&data)?; s.act("tax.form16", |c| vaulty_core::tax_plan::apply_form16(c, &country, &year, &data)) }
#[tauri::command] fn tax_describe(country: String) -> Result<Value> { vaulty_core::tax::describe(&country) }
#[tauri::command] fn tax_compare(country: String, year: Option<String>, input: Value) -> Result<Value> { limits::check(&input)?;
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

// ============================================================ updates

/// The update found by the last check, kept until it's installed.
#[derive(Default)]
struct PendingUpdate(tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>);

#[derive(Serialize)]
struct UpdateInfo { version: String, current: String, notes: Option<String>, date: Option<String> }

/// Release builds get the public half of the update signing key; local and CI builds don't, and
/// can't update themselves.
fn updates_configured(app: &tauri::AppHandle) -> bool {
    app.config().plugins.0.get("updater").and_then(|u| u.get("pubkey")).and_then(Value::as_str).is_some_and(|k| !k.trim().is_empty())
}

async fn find_update(app: &tauri::AppHandle) -> Result<Option<UpdateInfo>> {
    use tauri_plugin_updater::UpdaterExt;
    if !updates_configured(app) {
        return Err(Error::Invalid("this copy of Vaulty can't update itself; download new versions from the releases page".into()));
    }
    let offline = |e: tauri_plugin_updater::Error| Error::Invalid(format!("couldn't check for updates ({e})"));
    let found = app.updater().map_err(offline)?.check().await.map_err(offline)?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(), current: u.current_version.clone(), notes: u.body.clone(),
        date: u.date.map(|d| d.date().to_string()),
    });
    *app.state::<PendingUpdate>().0.lock().await = found;
    Ok(info)
}

/// "Check now" in Settings; works whether or not automatic checks are on.
#[tauri::command]
async fn update_check(app: tauri::AppHandle, s: S<'_>) -> Result<Option<UpdateInfo>> {
    s.current_user()?;
    let found = find_update(&app).await?;
    s.data(|c| settings::mark_update_checked(c, chrono::Utc::now()))?;
    Ok(found)
}

/// After signing in: checks only when automatic checks are on and a week has passed. Being
/// offline is normal, so failures are quiet.
#[tauri::command]
async fn update_auto(app: tauri::AppHandle, s: S<'_>) -> Result<Option<UpdateInfo>> {
    if !updates_configured(&app) || !s.data(|c| settings::update_due(c, chrono::Utc::now()))? { return Ok(None); }
    let found = find_update(&app).await.unwrap_or(None);
    s.data(|c| settings::mark_update_checked(c, chrono::Utc::now()))?;
    Ok(found)
}

/// Downloads the update found by the last check, verifies its signature against the key built
/// into this copy, installs it and restarts. Only the family admin can do this.
#[tauri::command]
async fn update_install(app: tauri::AppHandle, s: S<'_>) -> Result<()> {
    if s.current_user()?.role != Role::Admin { return Err(Error::Invalid("ask the family admin to install updates".into())); }
    let update = app.state::<PendingUpdate>().0.lock().await.take().ok_or_else(|| Error::Invalid("check for updates first".into()))?;
    update.download_and_install(|_, _| {}, || {}).await
        .map_err(|e| Error::Invalid(format!("the update couldn't be installed ({e}); nothing was changed")))?;
    app.restart()
}

// ============================================================ tray, shortcut, keep running (this device)

/// Device settings for the Settings page: tray icon, quick-add shortcut, keep running when closed.
#[tauri::command]
fn app_settings_get(s: S) -> Result<Value> {
    let get = |k: &str| s.registry(|r| r.app_setting(k));
    #[cfg(desktop)]
    let choices = desktop::SHORTCUTS;
    #[cfg(not(desktop))]
    let choices: &[&str] = &[];
    Ok(json!({
        "tray": get("tray")?.as_deref() != Some("off"),
        "shortcut": get("shortcut")?.unwrap_or_default(),
        "keep_running": get("keep_running")?.as_deref() == Some("on"),
        "shortcut_choices": choices,
        "mac": cfg!(target_os = "macos"),
    }))
}

/// Changes a device setting and applies it straight away. Needs someone signed in.
#[tauri::command]
fn app_settings_set(app: tauri::AppHandle, s: S, key: String, value: Value) -> Result<()> {
    s.current_user()?;
    let text = match (&key[..], &value) {
        ("tray" | "keep_running", Value::Bool(on)) => if *on { "on" } else { "off" }.to_string(),
        ("shortcut", Value::String(k)) => k.clone(),
        _ => return Err(Error::Invalid("unknown setting".into())),
    };
    #[cfg(desktop)]
    match &key[..] {
        "tray" => desktop::set_tray(&app, text == "on")?,
        "shortcut" => desktop::set_shortcut(&app, &text)?,
        _ => {}
    }
    #[cfg(not(desktop))]
    let _ = &app;
    s.registry(|r| r.set_app_setting(&key, &text))
}

/// The tray menu's words in the window's language.
#[tauri::command]
fn tray_labels(app: tauri::AppHandle, labels: Vec<String>) -> Result<()> {
    if labels.len() != 4 || labels.iter().any(|l| l.trim().is_empty() || l.chars().count() > 40) { return Err(Error::Invalid("bad labels".into())); }
    #[cfg(desktop)]
    {
        if let Ok(mut l) = app.state::<desktop::TrayLabels>().0.lock() {
            *l = [labels[0].clone(), labels[1].clone(), labels[2].clone(), labels[3].clone()];
        }
        let on = app.state::<AppState>().registry(|r| r.app_setting("tray"))?.as_deref() != Some("off");
        if on { desktop::set_tray(&app, true)?; }
    }
    #[cfg(not(desktop))]
    let _ = (&app, labels);
    Ok(())
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder
        .plugin(desktop::shortcut_plugin())
        .manage(desktop::TrayLabels::default())
        // With "keep running" on, closing the window hides it; the tray brings it back.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if desktop::keep_running(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        });
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PendingUpdate::default())
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
                helping: Mutex::new(None),
                picked: Mutex::new(Vec::new()),
                shown: Mutex::new(Vec::new()),
            });
            watch_idle(app.handle().clone());
            #[cfg(desktop)]
            desktop::apply_saved(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_settings_get, app_settings_set, tray_labels, pick_file, tasks_list, tasks_create, tasks_update, tasks_set_done, tasks_delete, subscriptions_detect, subscriptions_dismiss, subscriptions_rises, people_list, people_totals, people_history, people_create, people_update, people_delete, reports_overview, reports_forecast, reports_safe_to_spend, card_cycles_get, card_statement_enter, card_statement_forget, card_benefits_list, card_benefits_create, card_benefits_update, card_benefits_delete, card_benefits_use, cards_best, projects_list, projects_get, projects_items, projects_payments, projects_link_suggestions, projects_names, projects_create, projects_update, projects_delete, projects_section_add, projects_section_update, projects_section_delete, projects_section_order, projects_item_add, projects_item_update, projects_item_delete, projects_payment_add, projects_payment_update, projects_payment_delete, projects_export_xlsx, projects_files, projects_file_link, projects_file_remove, projects_file_add, projects_file_preview, projects_file_save, projects_read_bill, projects_items_add, ocr_engine, projects_members, projects_members_set, projects_split, projects_settle_preview, projects_settle_record, projects_share_export, projects_share_peek, projects_share_import, investments_list, investments_summary, investments_transactions, investments_create, investments_update, investments_set_price, investments_delete, cas_preview, cas_import, documents_list, documents_create, documents_update, documents_delete, documents_remove_file, documents_add_file, documents_preview, documents_save_file, nominee_for, nominee_start, sign_in_nominee, nominee_mine, nominee_set, nominee_remove, nominee_answer, estate_preview, helpers_for, helpers_approve_member, helpers_approve_card, helpers_cancel, sign_in_helpers, helpers_mine, helpers_set, helpers_remove, reports_low_balances, reports_export_xlsx, print_window, reminders_take, notify_permission, notify_show, calendar_range, calendar_events, calendar_event_create, calendar_event_update, calendar_event_delete,
            calendar_export, calendar_import_preview, calendar_import, users_set_senior, session_touch, hotkeys_list, hotkeys_set, hotkeys_reset, update_check, update_auto, update_install,
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
            transactions_set_visibility, transactions_bulk, sharing_get, sharing_update, family_feed,
            settings_get, settings_update,
            export_json, export_csv, import_json, backup_export, backup_inspect, backup_import, db_size, institutions_search, tax_countries, tax_describe, tax_compare, tax_plan_get, tax_plan_save, tax_payment_add, tax_payment_delete, tax_links_set, form16_read, form16_apply, businesses_list, book_current, book_switch, business_create, business_update, business_owner_money, business_owner_totals, business_archive, personal_accounts, goals_list, goals_summary, goals_create, goals_update, goals_set_done, goals_delete, calc_run, networth_timeline, nominees_list, nominees_set, anomalies_list, anomalies_dismiss, anomalies_reset, journal_list, journal_month_facts, journal_create, journal_update, journal_delete, insurance_list, insurance_summary, insurance_create, insurance_update, insurance_delete, insurance_pay, insurance_payment_delete, insurance_claim_add, insurance_claim_update, insurance_claim_delete, snapshots_list, snapshots_create, snapshots_restore, history_recent, history_trash, history_item, history_verify, history_undo, history_undo_last, market_get, market_update, market_set_key, market_quotes, market_refresh, fund_prices_update, fund_prices_status, cards_list, cards_create, cards_update, cards_delete,
        ])
        .build(tauri::generate_context!())
        .expect("error while running Vaulty")
        .run(|_app, _event| {
            // macOS: clicking the Dock icon brings back a hidden window.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event { desktop::show_window(_app); }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let registry = Registry::open_in_memory().unwrap();
        let keys = registry.create_first_admin("John Doe", "#2563eb", "parent-password").unwrap();
        let user = registry.get(&keys.user_id).unwrap();
        Session::new(user, db::open_in_memory().unwrap(), keys, Household::open_in_memory().unwrap(), false)
    }

    #[test]
    fn idle_sessions_lock_in_the_app_not_just_the_window() {
        let mut slot = Some(session());
        let s = slot.as_mut().unwrap();
        assert_eq!(s.auto_lock, Some(Duration::from_secs(5 * 60)), "5 minutes unless changed");
        assert!(live(&mut slot).is_ok());

        // Just past the limit: the window should have locked; the app waits out the grace period.
        slot.as_mut().unwrap().last_active = SystemTime::now() - Duration::from_secs(5 * 60 + 30);
        assert!(live(&mut slot).is_ok());
        // Past the grace period too: the session is dropped and its keys wiped.
        slot.as_mut().unwrap().last_active = SystemTime::now() - Duration::from_secs(5 * 60 + 61);
        assert!(matches!(live(&mut slot), Err(Error::Auth(_))));
        assert!(slot.is_none());

        // A clock moved backwards counts as use, not as a long absence.
        let mut slot = Some(session());
        slot.as_mut().unwrap().last_active = SystemTime::now() + Duration::from_secs(3600);
        assert!(live(&mut slot).is_ok());

        // "Never" turns it off.
        let mut slot = Some(session());
        settings::update(&slot.as_ref().unwrap().conn, &json!({ "auto_lock_minutes": 0 })).unwrap();
        let s = slot.as_mut().unwrap();
        s.auto_lock = settings::auto_lock(&s.conn).unwrap();
        s.last_active = SystemTime::now() - Duration::from_secs(86_400);
        assert!(live(&mut slot).is_ok());
        assert!(settings::update(&slot.as_ref().unwrap().conn, &json!({ "auto_lock_minutes": 7 })).is_err());
    }

    #[test]
    fn hidden_section_time_counts_while_asleep() {
        let mut s = session();
        s.hidden_until = Some(SystemTime::now() - Duration::from_secs(1));
        assert!(!s.hidden_open());
        s.hidden_until = Some(SystemTime::now() + HIDDEN_TTL);
        assert!(s.hidden_open());
    }

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
