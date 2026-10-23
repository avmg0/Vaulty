//! Nominee access: if something happens to a member, the person they named can see a read-only
//! summary of their money (and a letter they left), as a bank nominee would claim it.
//!
//! - The nominee has a printed card: a fourth point on the member's helpers' line, so the card
//!   plus any one helper rebuild the key. The card alone opens nothing.
//! - The nominee first starts a request with the card. Access opens after a waiting period (3–30
//!   days, 7 by default). If the member signs in meanwhile they're told, and can cancel it.
//! - What opens is a summary, read once and shown; nothing can be changed. The member's 2FA isn't
//!   asked for (their phone may be gone with them); the helper and the wait are the safeguards.
use super::helpers::{b32_decode, card_code, checksum, helper_key, HelperCard, HelperShare, NOMINEE_SLOT};
use super::*;
use crate::shamir;
use zeroize::Zeroizing;

pub const WAIT_CHOICES: &[i64] = &[3, 7, 14, 30];
const LETTER_MAX: usize = 5000;
const DAY: i64 = 86_400;

#[derive(Debug, Clone, Serialize)]
pub struct NomineeRequest {
    pub started_at: i64,
    /// When access opens (unless the member cancels).
    pub ready_at: i64,
    /// The member signed in and let it go ahead.
    pub allowed: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NomineeStatus {
    pub set_up: bool,
    pub name: Option<String>,
    pub wait_days: i64,
    pub has_letter: bool,
    pub request: Option<NomineeRequest>,
}

/// What a nominee opens: the data key (the caller reads the summary, read-only) and the letter.
pub struct NomineeView {
    pub data_key: Key,
    pub owner_name: String,
    pub letter: Option<String>,
}

impl Registry {
    pub fn nominee(&self, owner_id: &str) -> Result<NomineeStatus> {
        let row: Option<(String, i64, bool)> = self.conn.query_row(
            "SELECT name, wait_days, letter_sealed IS NOT NULL FROM nominees WHERE user_id = ?", [owner_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        let Some((name, wait_days, has_letter)) = row else {
            return Ok(NomineeStatus { set_up: false, name: None, wait_days: 7, has_letter: false, request: None });
        };
        let request = self.conn.query_row("SELECT started_at, allowed FROM nominee_requests WHERE user_id = ?", [owner_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?))).optional()?
            .map(|(started_at, allowed)| {
                let ready_at = started_at + wait_days * DAY;
                NomineeRequest { started_at, ready_at, allowed, ready: allowed || now() >= ready_at }
            });
        Ok(NomineeStatus { set_up: true, name: Some(name), wait_days, has_letter, request })
    }

    /// Names (or renames) the nominee, sets the waiting period and the letter, and returns the
    /// nominee card. Needs family helpers, since the card works together with one of them.
    pub fn set_nominee(&self, owner_id: &str, password: &str, name: &str, wait_days: i64, letter: Option<&str>) -> Result<HelperCard> {
        let dk = self.data_key_from_password(owner_id, password)?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 { return Err(Error::Invalid("give your nominee's name (up to 60 characters)".into())); }
        if !WAIT_CHOICES.contains(&wait_days) { return Err(Error::Invalid("the waiting period is 3, 7, 14 or 30 days".into())); }
        let letter = letter.map(str::trim).filter(|l| !l.is_empty());
        if letter.is_some_and(|l| l.chars().count() > LETTER_MAX) { return Err(Error::Invalid("the letter can be up to 5,000 characters".into())); }
        let (set_id, line): (String, Option<String>) = self.conn.query_row(
            "SELECT set_id, line_sealed FROM helper_sets WHERE user_id = ?", [owner_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?
            .ok_or_else(|| Error::Invalid("choose your family helpers first: the nominee card works together with one of them".into()))?;
        let line = Zeroizing::new(crypto::open(&dk, &line.ok_or_else(|| Error::Invalid("choose your helpers again first, so a nominee card can be made".into()))?)?);
        let (secret, slope) = line.split_at(line.len() / 2);
        let share = shamir::share_at(secret, slope, NOMINEE_SLOT)?;
        let sealed = letter.map(|l| crypto::seal(&dk, l.as_bytes())).transpose()?;
        self.conn.execute(
            "INSERT INTO nominees (user_id, name, wait_days, letter_sealed) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(user_id) DO UPDATE SET name = ?2, wait_days = ?3, letter_sealed = ?4",
            params![owner_id, name, wait_days, sealed])?;
        Ok(HelperCard { slot: NOMINEE_SLOT, code: card_code(&crypto::from_hex(&set_id)?, &share), label: name.into(), owner: self.get(owner_id)?.name, nominee: true })
    }

    pub fn remove_nominee(&self, owner_id: &str, password: &str) -> Result<()> {
        self.verify_password(owner_id, password)?;
        self.conn.execute("DELETE FROM nominee_requests WHERE user_id = ?", [owner_id])?;
        self.conn.execute("DELETE FROM nominees WHERE user_id = ?", [owner_id])?;
        Ok(())
    }

    /// The member's letter, for them to read back (they're signed in).
    pub fn nominee_letter(&self, me: &Unlocked) -> Result<Option<String>> {
        let sealed: Option<String> = self.conn.query_row("SELECT letter_sealed FROM nominees WHERE user_id = ?", [&me.user_id], |r| r.get(0)).optional()?.flatten();
        sealed.map(|s| String::from_utf8(crypto::open(&me.data_key, &s)?).map_err(|_| Error::Invalid("the letter is damaged".into()))).transpose()
    }

    /// Reads a nominee card for this member. A typo is caught by the checksum.
    fn nominee_share(&self, owner_id: &str, code: &str) -> Result<shamir::Share> {
        self.check_throttle(owner_id)?;
        let raw = Zeroizing::new(b32_decode(code).filter(|b| b.len() == 23)
            .ok_or_else(|| Error::Invalid("a nominee card code has 37 letters and numbers; check it and try again".into()))?);
        let (body, sum) = raw.split_at(raw.len() - 2);
        if checksum(body) != sum { return Err(Error::Invalid("that code has a typo; check it against the card".into())); }
        let set_id: Option<String> = self.conn.query_row(
            "SELECT h.set_id FROM helper_sets h JOIN nominees n ON n.user_id = h.user_id WHERE h.user_id = ?", [owner_id], |r| r.get(0)).optional()?;
        if body[4] != NOMINEE_SLOT || set_id.as_deref() != Some(crypto::to_hex(&body[..4]).as_str()) {
            self.register_failure(owner_id)?;
            return Err(Error::Auth("this isn't a current nominee card for this profile".into()));
        }
        Ok(shamir::Share { x: NOMINEE_SLOT, bytes: Zeroizing::new(body[5..].to_vec()) })
    }

    /// The nominee starts the waiting period with their card. Starting again changes nothing.
    pub fn nominee_start(&self, owner_id: &str, code: &str) -> Result<NomineeStatus> {
        self.nominee_share(owner_id, code)?;
        self.conn.execute("INSERT OR IGNORE INTO nominee_requests (user_id, started_at) VALUES (?, ?)", params![owner_id, now()])?;
        self.nominee(owner_id)
    }

    /// The member, signed in, cancels a request (it wasn't expected) or lets it go ahead now.
    pub fn nominee_answer(&self, me: &Unlocked, allow: bool) -> Result<()> {
        if allow { self.conn.execute("UPDATE nominee_requests SET allowed = 1 WHERE user_id = ?", [&me.user_id])?; }
        else { self.conn.execute("DELETE FROM nominee_requests WHERE user_id = ?", [&me.user_id])?; }
        Ok(())
    }

    /// After the wait: the nominee card and one helper's approval open the member's data for reading.
    pub fn open_as_nominee(&self, owner_id: &str, code: &str, helper: &HelperShare) -> Result<NomineeView> {
        let status = self.nominee(owner_id)?;
        if !status.set_up { return Err(Error::Invalid("no nominee is set up for this profile".into())); }
        match &status.request {
            None => return Err(Error::Invalid("start the request with your nominee card first".into())),
            Some(r) if !r.ready => return Err(Error::Invalid("the waiting period isn't over yet".into())),
            _ => {}
        }
        let card = self.nominee_share(owner_id, code)?;
        let wrapped: String = self.conn.query_row("SELECT wrapped FROM helper_sets WHERE user_id = ?", [owner_id], |r| r.get(0))?;
        let dk = match shamir::combine(&[card, helper.share.clone()]).and_then(|s| crypto::unwrap(&helper_key(&s), &wrapped)) {
            Ok(dk) => dk,
            Err(_) => {
                self.register_failure(owner_id)?;
                return Err(Error::Auth("the card and the helper's approval don't open this profile".into()));
            }
        };
        let owner_name = self.get(owner_id)?.name;
        if let Some(id) = helper.helper_id.as_deref() {
            self.conn.execute("INSERT INTO helper_notices (helper_id, owner_name, at) VALUES (?, ?, ?)", params![id, owner_name, now()])?;
        }
        let sealed: Option<String> = self.conn.query_row("SELECT letter_sealed FROM nominees WHERE user_id = ?", [owner_id], |r| r.get(0))?;
        let letter = sealed.map(|s| crypto::open(&dk, &s)).transpose()?.map(|b| String::from_utf8_lossy(&b).into_owned());
        Ok(NomineeView { data_key: dk, owner_name, letter })
    }
}
