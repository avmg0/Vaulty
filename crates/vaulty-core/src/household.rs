//! Family sharing.
//!
//! Every member's finances live in their own private database. Family sharing is opt-in
//! on **both** sides: a member must turn it on before anything of theirs is published,
//! and before they can see anything anyone else published.
//!
//! Each transaction has a visibility:
//! - `private`: never leaves the owner's database.
//! - `shared`:  visible to the owner plus the specific members they picked (e.g. parents).
//! - `public`:  visible to everyone in the family who has Family sharing turned on.
//!
//! Shared and public items are copied (date, amount, merchant, category, account name;
//! never notes, tags or receipts) into this device-level store. The owner can change an
//! item's visibility, change who they share with, or turn sharing off at any time, which
//! immediately withdraws what they published.
use crate::error::{Error, Result};
use crate::rows::query;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Private,
    Shared,
    Public,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Private => "private",
            Visibility::Shared => "shared",
            Visibility::Public => "public",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "private" => Ok(Self::Private),
            "shared" => Ok(Self::Shared),
            "public" => Ok(Self::Public),
            _ => Err(Error::Invalid(format!("unknown visibility `{s}`"))),
        }
    }
}

/// One member's sharing settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SharingPrefs {
    /// Consent: publish my shared/public items and let me see what others share.
    pub enabled: bool,
    /// Visibility pre-selected for new transactions.
    pub default_visibility: Visibility,
    /// Who sees my `shared` items.
    pub shared_with: Vec<String>,
}

impl Default for SharingPrefs {
    fn default() -> Self {
        Self { enabled: false, default_visibility: Visibility::Private, shared_with: Vec::new() }
    }
}

/// Which slice of the family view to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedView {
    /// Everything I'm allowed to see (others' public, others' shared-with-me, and my own published items).
    All,
    Shared,
    Public,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub owner_id: Option<String>,
    pub limit: Option<i64>,
}

pub struct Household {
    conn: Connection,
}

impl Household {
    /// Opens the encrypted family store with the household key (see `profiles::Unlocked`).
    pub fn open(path: &Path, key: &crate::Key) -> Result<Self> {
        Ok(Self { conn: crate::db::open_encrypted(path, key, crate::db::HOUSEHOLD_MIGRATIONS)? })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self { conn: crate::db::memory_with(crate::db::HOUSEHOLD_MIGRATIONS)? })
    }

    // ------------------------------------------------------------ settings

    pub fn prefs(&self, user_id: &str) -> Result<SharingPrefs> {
        let row: Option<(bool, String)> = self
            .conn
            .query_row("SELECT enabled, default_visibility FROM members WHERE user_id = ?", [user_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((enabled, default_visibility)) = row else { return Ok(SharingPrefs::default()) };
        let mut stmt = self.conn.prepare("SELECT viewer_id FROM share_targets WHERE owner_id = ? ORDER BY viewer_id")?;
        let shared_with = stmt.query_map([user_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(SharingPrefs { enabled, default_visibility: Visibility::parse(&default_visibility)?, shared_with })
    }

    /// Saves a member's own settings. Returns true when the caller must (re)publish the
    /// member's items, i.e. sharing is on. Turning sharing off withdraws everything at once.
    pub fn set_prefs(&self, user_id: &str, p: &SharingPrefs) -> Result<bool> {
        if p.shared_with.iter().any(|v| v == user_id) {
            return Err(Error::Invalid("you don't need to share with yourself".into()));
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO members (user_id, enabled, default_visibility, updated_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(user_id) DO UPDATE SET enabled = ?2, default_visibility = ?3, updated_at = CURRENT_TIMESTAMP",
            params![user_id, p.enabled, p.default_visibility.as_str()],
        )?;
        tx.execute("DELETE FROM share_targets WHERE owner_id = ?", [user_id])?;
        for viewer in &p.shared_with {
            tx.execute("INSERT OR IGNORE INTO share_targets (owner_id, viewer_id) VALUES (?, ?)", params![user_id, viewer])?;
        }
        if !p.enabled {
            tx.execute("DELETE FROM shared_items WHERE owner_id = ?", [user_id])?;
        }
        tx.commit()?;
        Ok(p.enabled)
    }

    /// user_id → whether they have Family sharing on. Members never configured are absent.
    pub fn enabled_members(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT user_id FROM members WHERE enabled = 1")?;
        let rows = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Called when a profile is deleted from the device.
    pub fn forget_member(&self, user_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM members WHERE user_id = ?", [user_id])?;
        self.conn.execute("DELETE FROM share_targets WHERE owner_id = ?1 OR viewer_id = ?1", [user_id])?;
        self.conn.execute("DELETE FROM shared_items WHERE owner_id = ?", [user_id])?;
        Ok(())
    }

    // ------------------------------------------------------------ publishing

    /// Publish (or withdraw) one transaction after it was created or edited.
    /// `row` is the owner's transaction as returned by `transactions::get_one` (exact `amount_minor` + `currency`).
    pub fn publish(&self, owner_id: &str, row: &Value) -> Result<()> {
        let id = row["id"].as_str().ok_or_else(|| Error::Invalid("transaction without id".into()))?;
        let vis = Visibility::parse(row["visibility"].as_str().unwrap_or("private"))?;
        if vis == Visibility::Private || !self.prefs(owner_id)?.enabled {
            return self.withdraw(owner_id, id);
        }
        self.conn.execute(
            "INSERT INTO shared_items (owner_id, item_id, visibility, date, type, amount_minor, amount_scale, currency, merchant,
                                       description, category_name, category_icon, category_color, account_name, published_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, CURRENT_TIMESTAMP)
             ON CONFLICT(owner_id, item_id) DO UPDATE SET
               visibility = ?3, date = ?4, type = ?5, amount_minor = ?6, amount_scale = ?7, currency = ?8, merchant = ?9,
               description = ?10, category_name = ?11, category_icon = ?12, category_color = ?13, account_name = ?14,
               published_at = CURRENT_TIMESTAMP",
            params![
                owner_id,
                id,
                vis.as_str(),
                row["date"].as_str().unwrap_or_default(),
                row["type"].as_str().unwrap_or_default(),
                row["amount_minor"].as_i64().unwrap_or(0),
                crate::money::scale(row["currency"].as_str().unwrap_or("USD")),
                row["currency"].as_str(),
                row["merchant"].as_str(),
                row["description"].as_str(),
                row["category_name"].as_str(),
                row["category_icon"].as_str(),
                row["category_color"].as_str(),
                row["account_name"].as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn withdraw(&self, owner_id: &str, item_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM shared_items WHERE owner_id = ? AND item_id = ?", params![owner_id, item_id])?;
        Ok(())
    }

    /// Replace everything an owner has published (after turning sharing on, or a bulk change).
    pub fn republish_all(&self, owner_id: &str, rows: &[Value]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM shared_items WHERE owner_id = ?", [owner_id])?;
        tx.commit()?;
        if !self.prefs(owner_id)?.enabled {
            return Ok(0);
        }
        let mut n = 0;
        for r in rows {
            self.publish(owner_id, r)?;
            n += 1;
        }
        Ok(n)
    }

    // ------------------------------------------------------------ reading

    /// The family view for `viewer_id`. Each row carries `owner_id` and `visibility`.
    /// Both the viewer and each owner must have Family sharing turned on.
    pub fn feed(&self, viewer_id: &str, view: FeedView, f: &FeedFilter) -> Result<Vec<Value>> {
        if !self.prefs(viewer_id)?.enabled {
            return Err(Error::Auth("turn on Family sharing in Settings to see what your family shares".into()));
        }
        let mut sql = String::from(
            "SELECT s.owner_id, s.item_id AS id, s.visibility, s.date, s.type, s.amount_minor,
                    s.amount_minor * 1.0 / s.amount_scale AS amount, s.currency, s.merchant,
                    s.description, s.category_name, s.category_icon, s.category_color, s.account_name,
                    (s.owner_id = ?1) AS is_mine
             FROM shared_items s
             JOIN members m ON m.user_id = s.owner_id AND m.enabled = 1
             WHERE (
                    s.owner_id = ?1
                 OR s.visibility = 'public'
                 OR (s.visibility = 'shared' AND EXISTS (
                        SELECT 1 FROM share_targets t WHERE t.owner_id = s.owner_id AND t.viewer_id = ?1))
             )",
        );
        let mut p: Vec<Value> = vec![json!(viewer_id)];
        match view {
            FeedView::All => {}
            FeedView::Shared => sql.push_str(" AND s.visibility = 'shared'"),
            FeedView::Public => sql.push_str(" AND s.visibility = 'public'"),
        }
        if let Some(d) = &f.start_date {
            sql.push_str(" AND s.date >= ?");
            p.push(json!(d));
        }
        if let Some(d) = &f.end_date {
            sql.push_str(" AND s.date <= ?");
            p.push(json!(d));
        }
        if let Some(o) = &f.owner_id {
            sql.push_str(" AND s.owner_id = ?");
            p.push(json!(o));
        }
        sql.push_str(" ORDER BY s.date DESC, s.published_at DESC");
        if let Some(l) = f.limit {
            sql.push_str(" LIMIT ?");
            p.push(json!(l));
        }
        // `?1` (the viewer) is referenced several times; later plain `?`s bind to params 2, 3, …
        query(&self.conn, &sql, &p)
    }

    #[doc(hidden)]
    pub fn conn_for_tests(&self) -> &Connection {
        &self.conn
    }
}
