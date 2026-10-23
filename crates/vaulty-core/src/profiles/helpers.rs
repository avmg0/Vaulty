//! Shared recovery: any 2 of 3 helpers can give a member back their profile.
//!
//! - A random 128-bit secret is split into 3 shares (Shamir, 2 needed). The member's data key is
//!   wrapped under a key derived from the secret; the secret itself is never stored.
//! - A helper is an adult family member on this device (their share is sealed to their public
//!   key, so they approve by signing in) or a printed card (the share is only on the card).
//! - Recovering: two shares, then a new password. 2FA stays on, so its code is needed too. The
//!   recovery key, biometric unlock and the helpers all stay as they were.
//! - Nobody can do it alone, the admin included: one share says nothing about the secret.
use super::*;
use crate::shamir;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const SECRET_LEN: usize = 16;
/// The nominee's card is the fourth point on the helpers' line.
pub(super) const NOMINEE_SLOT: u8 = 4;
const SHARES: u8 = 3;
const NEEDED: u8 = 2;
/// Crockford base32: no I, L, O or U; I and L read as 1, O as 0.
const B32: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Who will help, as the member chose.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum HelperChoice {
    Member { id: String },
    Card { label: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct HelperSlot {
    pub slot: u8,
    /// "member" or "card"
    pub kind: String,
    pub helper_id: Option<String>,
    /// The member's name, or who holds the card.
    pub name: String,
    /// False when the member was removed or their profile started over.
    pub usable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Helpers {
    pub set_up: bool,
    pub created_at: Option<String>,
    pub slots: Vec<HelperSlot>,
    /// Shares that can still be used; recovery needs 2.
    pub usable: usize,
}

/// A card to print, shown once.
#[derive(Debug, Serialize)]
pub struct HelperCard {
    pub slot: u8,
    /// Who keeps it (for a nominee card, the nominee's name).
    pub label: String,
    pub owner: String,
    pub code: String,
    /// A nominee card rather than a helper card.
    pub nominee: bool,
}

/// One helper's share, collected during recovery. Kept in memory only.
#[derive(Clone)]
pub struct HelperShare {
    pub(super) slot: u8,
    pub(super) helper_id: Option<String>,
    pub(super) share: shamir::Share,
}
impl HelperShare {
    pub fn slot(&self) -> u8 { self.slot }
}

#[derive(Debug, Serialize)]
pub struct HelperNotice { pub owner_name: String, pub at: i64 }

pub(super) fn helper_key(secret: &[u8]) -> Key {
    let mut h = Sha256::new();
    h.update(b"vaulty-helpers-v1");
    h.update(secret);
    Zeroizing::new(h.finalize().into())
}

pub(super) fn checksum(body: &[u8]) -> [u8; 2] {
    let d = Sha256::digest(body);
    [d[0], d[1]]
}

fn b32_encode(bytes: &[u8]) -> String {
    let (mut out, mut acc, mut bits) = (String::new(), 0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((acc >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 { out.push(B32[((acc << (5 - bits)) & 31) as usize] as char); }
    out
}

pub(super) fn b32_decode(s: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (vec![], 0u32, 0u32);
    for c in s.chars().filter(|c| c.is_ascii_alphanumeric()) {
        let c = match c.to_ascii_uppercase() { 'O' => '0', 'I' | 'L' => '1', c => c };
        let v = B32.iter().position(|&b| b as char == c)? as u32;
        acc = (acc << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// set id (4) · slot (1) · share (16) · checksum (2) = 23 bytes = 37 characters.
pub(super) fn card_code(set_id: &[u8], share: &shamir::Share) -> String {
    let mut body = set_id.to_vec();
    body.push(share.x);
    body.extend_from_slice(&share.bytes);
    let sum = checksum(&body);
    body.extend_from_slice(&sum);
    let code = Zeroizing::new(b32_encode(&body));
    grouped(&code, 5)
}

impl Registry {
    /// The member's helpers (also shown on the sign-in screen when recovering).
    pub fn helpers(&self, user_id: &str) -> Result<Helpers> {
        let created_at: Option<String> = self.conn.query_row("SELECT created_at FROM helper_sets WHERE user_id = ?", [user_id], |r| r.get(0)).optional()?;
        let slots: Vec<HelperSlot> = self.conn.prepare(
            "SELECT s.slot, s.kind, s.helper_id, COALESCE(u.name, s.label, ''), s.kind = 'card' OR (u.id IS NOT NULL AND s.sealed IS NOT NULL)
               FROM helper_shares s LEFT JOIN users u ON u.id = s.helper_id WHERE s.user_id = ? ORDER BY s.slot")?
            .query_map([user_id], |r| Ok(HelperSlot { slot: r.get(0)?, kind: r.get(1)?, helper_id: r.get(2)?, name: r.get(3)?, usable: r.get(4)? }))?
            .collect::<rusqlite::Result<_>>()?;
        let usable = slots.iter().filter(|s| s.usable).count();
        Ok(Helpers { set_up: created_at.is_some(), created_at, slots, usable })
    }

    /// Family members who can be helpers: adults with their own profile set up.
    pub fn helper_candidates(&self, user_id: &str) -> Result<Vec<UserSummary>> {
        Ok(self.list()?.into_iter().filter(|u| u.id != user_id && u.active && u.role != Role::Child).collect())
    }

    /// Chooses (or changes) the three helpers. Earlier shares and cards stop working.
    /// Returns the cards to print, shown only now.
    pub fn set_helpers(&self, user_id: &str, password: &str, choices: &[HelperChoice]) -> Result<Vec<HelperCard>> {
        let dk = self.data_key_from_password(user_id, password)?;
        let owner = self.get(user_id)?;
        if owner.role == Role::Child { return Err(Error::Invalid("children are helped back in by a parent".into())); }
        if choices.len() != SHARES as usize { return Err(Error::Invalid("choose three helpers".into())); }
        let candidates = self.helper_candidates(user_id)?;
        let mut members: Vec<&str> = vec![];
        for c in choices {
            match c {
                HelperChoice::Member { id } => {
                    if !candidates.iter().any(|u| &u.id == id) { return Err(Error::Invalid("a helper must be an adult family member with their own profile".into())); }
                    if members.contains(&id.as_str()) { return Err(Error::Invalid("choose a different person for each helper".into())); }
                    members.push(id);
                }
                HelperChoice::Card { label } => {
                    let n = label.trim().chars().count();
                    if n == 0 || n > 60 { return Err(Error::Invalid("say who will keep each card (up to 60 characters)".into())); }
                }
            }
        }
        // A line through the secret: each share is a point on it, and any two points give the line.
        let secret = Zeroizing::new(crypto::random_bytes::<SECRET_LEN>());
        let slope = Zeroizing::new(crypto::random_bytes::<SECRET_LEN>());
        let shares = (1..=SHARES).map(|x| shamir::share_at(secret.as_ref(), slope.as_ref(), x)).collect::<Result<Vec<_>>>()?;
        let set_id = crypto::random_bytes::<4>();
        let line = Zeroizing::new([secret.as_ref(), slope.as_ref()].concat());
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM helper_shares WHERE user_id = ?", [user_id])?;
        tx.execute("INSERT OR REPLACE INTO helper_sets (user_id, set_id, wrapped, line_sealed) VALUES (?, ?, ?, ?)",
            params![user_id, crypto::to_hex(&set_id), crypto::wrap(&helper_key(secret.as_ref()), &dk)?, crypto::seal(&dk, &line)?])?;
        let mut cards = vec![];
        for (c, share) in choices.iter().zip(&shares) {
            match c {
                HelperChoice::Member { id } => {
                    let mut plain = Zeroizing::new(vec![share.x]);
                    plain.extend_from_slice(&share.bytes);
                    tx.execute("INSERT INTO helper_shares (user_id, slot, kind, helper_id, sealed) VALUES (?, ?, 'member', ?, ?)",
                        params![user_id, share.x, id, crypto::seal_bytes_to(&self.public_key(id)?, &plain)?])?;
                }
                HelperChoice::Card { label } => {
                    let label = label.trim().to_string();
                    tx.execute("INSERT INTO helper_shares (user_id, slot, kind, label) VALUES (?, ?, 'card', ?)", params![user_id, share.x, label])?;
                    cards.push(HelperCard { slot: share.x, code: card_code(&set_id, share), label, owner: owner.name.clone(), nominee: false });
                }
            }
        }
        // A nominee's card is a point on the same line, so it's issued again with the new set.
        let nominee: Option<String> = tx.query_row("SELECT name FROM nominees WHERE user_id = ?", [user_id], |r| r.get(0)).optional()?;
        if let Some(name) = nominee {
            let share = shamir::share_at(secret.as_ref(), slope.as_ref(), NOMINEE_SLOT)?;
            cards.push(HelperCard { slot: NOMINEE_SLOT, code: card_code(&set_id, &share), label: name, owner: owner.name.clone(), nominee: true });
        }
        tx.commit()?;
        Ok(cards)
    }

    /// Also removes the nominee, whose card is part of the same set.
    pub fn remove_helpers(&self, user_id: &str, password: &str) -> Result<()> {
        self.verify_password(user_id, password)?;
        self.conn.execute("DELETE FROM nominee_requests WHERE user_id = ?", [user_id])?;
        self.conn.execute("DELETE FROM nominees WHERE user_id = ?", [user_id])?;
        self.conn.execute("DELETE FROM helper_shares WHERE user_id = ?", [user_id])?;
        self.conn.execute("DELETE FROM helper_sets WHERE user_id = ?", [user_id])?;
        Ok(())
    }

    /// A family member approves by entering their own password. Wrong guesses count against
    /// their profile, as when signing in.
    pub fn helper_share_from_member(&self, owner_id: &str, helper_id: &str, helper_password: &str) -> Result<HelperShare> {
        let row: Option<(u8, Option<String>)> = self.conn.query_row(
            "SELECT slot, sealed FROM helper_shares WHERE user_id = ? AND kind = 'member' AND helper_id = ?",
            [owner_id, helper_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let Some((slot, Some(sealed))) = row else { return Err(Error::Invalid("they aren't one of this profile's helpers".into())) };
        let dk = self.data_key_from_password(helper_id, helper_password)?;
        let helper = self.unlocked(helper_id, dk)?;
        let plain = crypto::open_sealed_bytes(&helper.secret_key, &sealed)?;
        let Some((&x, bytes)) = plain.split_first() else { return Err(Error::Auth("this helper's share is damaged".into())) };
        Ok(HelperShare { slot, helper_id: Some(helper_id.into()), share: shamir::Share { x, bytes: Zeroizing::new(bytes.to_vec()) } })
    }

    /// A printed helper card, typed in. A typo is caught by the card's checksum.
    pub fn helper_share_from_card(&self, owner_id: &str, code: &str) -> Result<HelperShare> {
        self.check_throttle(owner_id)?;
        let raw = Zeroizing::new(b32_decode(code).filter(|b| b.len() == 4 + 1 + SECRET_LEN + 2)
            .ok_or_else(|| Error::Invalid("a helper card code has 37 letters and numbers; check it and try again".into()))?);
        let (body, sum) = raw.split_at(raw.len() - 2);
        if checksum(body) != sum { return Err(Error::Invalid("that code has a typo; check it against the card".into())); }
        let set_id: Option<String> = self.conn.query_row("SELECT set_id FROM helper_sets WHERE user_id = ?", [owner_id], |r| r.get(0)).optional()?;
        if set_id.as_deref() != Some(crypto::to_hex(&body[..4]).as_str()) {
            self.register_failure(owner_id)?;
            return Err(Error::Auth("this card is from an older set of helpers, or for another profile".into()));
        }
        let x = body[4];
        let is_card: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM helper_shares WHERE user_id = ? AND slot = ? AND kind = 'card')", params![owner_id, x], |r| r.get(0))?;
        if !is_card { return Err(Error::Auth("this card is from an older set of helpers, or for another profile".into())); }
        Ok(HelperShare { slot: x, helper_id: None, share: shamir::Share { x, bytes: Zeroizing::new(body[5..].to_vec()) } })
    }

    /// Two helpers' shares give the member their profile back with a new password. With 2FA on,
    /// `second_factor` (an authenticator or backup code) is needed too.
    pub fn recover_with_helpers(&self, owner_id: &str, shares: &[HelperShare], new_password: &str, second_factor: Option<&str>) -> Result<Unlocked> {
        self.check_throttle(owner_id)?;
        let owner = self.get(owner_id)?;
        if !owner.active { return Err(Error::Auth("this profile hasn't been set up yet".into())); }
        let wrapped: String = self.conn.query_row("SELECT wrapped FROM helper_sets WHERE user_id = ?", [owner_id], |r| r.get(0)).optional()?
            .ok_or_else(|| Error::Invalid("shared recovery isn't set up for this profile".into()))?;
        let mut distinct: Vec<&HelperShare> = vec![];
        for s in shares { if !distinct.iter().any(|d| d.slot == s.slot) { distinct.push(s); } }
        if distinct.len() < NEEDED as usize { return Err(Error::Invalid("two different helpers are needed".into())); }
        let parts: Vec<shamir::Share> = distinct.iter().map(|s| s.share.clone()).collect();
        let dk = match shamir::combine(&parts).and_then(|secret| crypto::unwrap(&helper_key(&secret), &wrapped)) {
            Ok(dk) => dk,
            Err(_) => {
                self.register_failure(owner_id)?;
                return Err(Error::Auth("those helpers' shares don't open this profile; the set may have changed".into()));
            }
        };
        check_password_rules(new_password, &owner.name)?;
        let unlocked = self.unlocked(owner_id, dk)?;
        // 2FA stays on: its code is checked before anything changes.
        let (totp_on, sealed): (bool, Option<String>) = self.conn.query_row(
            "SELECT totp_enabled, totp_secret_sealed FROM users WHERE id = ?", [owner_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let unlocked = match (totp_on, sealed) {
            (true, Some(sealed)) => {
                let code = second_factor.map(str::trim).filter(|c| !c.is_empty())
                    .ok_or_else(|| Error::Auth("enter the code from your authenticator app, or a backup code".into()))?;
                let secret = String::from_utf8(crypto::open(&unlocked.data_key, &sealed)?).map_err(|_| Error::Auth("corrupt 2FA secret".into()))?;
                let pending = SecondFactor { user_id: owner_id.into(), secret_base32: secret.into(), unlocked };
                if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) { self.complete_with_totp(&pending, code)? } else { self.complete_with_backup_code(&pending, code)? }
            }
            _ => { self.record_success(owner_id)?; unlocked }
        };
        let tx = self.conn.unchecked_transaction()?;
        self.wrap_with_secret(owner_id, "password", new_password, &unlocked.data_key)?;
        self.conn.execute("UPDATE users SET password_hash = ? WHERE id = ?", params![crypto::hash_secret(new_password)?, owner_id])?;
        for id in distinct.iter().filter_map(|s| s.helper_id.as_deref()) {
            self.conn.execute("INSERT INTO helper_notices (helper_id, owner_name, at) VALUES (?, ?, ?)", params![id, owner.name, now()])?;
        }
        tx.commit()?;
        Ok(unlocked)
    }

    /// "Your help was used", once, after the helper signs in.
    pub fn take_helper_notices(&self, helper_id: &str) -> Result<Vec<HelperNotice>> {
        let out = self.conn.prepare("SELECT owner_name, at FROM helper_notices WHERE helper_id = ? ORDER BY at")?
            .query_map([helper_id], |r| Ok(HelperNotice { owner_name: r.get(0)?, at: r.get(1)? }))?
            .collect::<rusqlite::Result<_>>()?;
        self.conn.execute("DELETE FROM helper_notices WHERE helper_id = ?", [helper_id])?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_round_trip() {
        let bytes: Vec<u8> = (0..23).map(|i| i * 11).collect();
        let code = b32_encode(&bytes);
        assert_eq!(code.len(), 37);
        assert_eq!(b32_decode(&code).unwrap(), bytes);
        let sloppy = code.to_lowercase().replace('0', "o").replace('1', "l");
        assert_eq!(b32_decode(&sloppy).unwrap(), bytes, "look-alikes are forgiven");
    }
}
