//! Double-entry ledger (see migrations/data/0004_ledger.sql for the model).
//!
//! The rest of the crate talks about "transactions", "accounts" and "categories"; this module
//! turns those into entries whose postings always balance. Users never see debits and credits.
use crate::error::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

pub const PERSONAL_WORKSPACE: &str = "ws_personal";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "asset" => Self::Asset,
            "liability" => Self::Liability,
            "equity" => Self::Equity,
            "income" => Self::Income,
            "expense" => Self::Expense,
            _ => return Err(Error::Invalid(format!("unknown ledger kind `{s}`"))),
        })
    }

    /// Ledger kind for a user-facing account type.
    pub fn for_account_type(account_type: &str) -> Self {
        match account_type {
            "credit" | "loan" | "liability" => Self::Liability,
            _ => Self::Asset,
        }
    }
}

/// System accounts every workspace has.
#[derive(Debug, Clone, Copy)]
pub enum System {
    OpeningBalance,
    Adjustment,
    UncategorizedIncome,
    UncategorizedExpense,
}

impl System {
    fn key(self) -> &'static str {
        match self {
            Self::OpeningBalance => "opening_balance",
            Self::Adjustment => "adjustment",
            Self::UncategorizedIncome => "uncategorized_income",
            Self::UncategorizedExpense => "uncategorized_expense",
        }
    }
}

pub fn system_account(conn: &Connection, workspace: &str, which: System) -> Result<String> {
    conn.query_row(
        "SELECT id FROM ledger_accounts WHERE workspace_id = ? AND system_key = ?",
        params![workspace, which.key()],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| Error::Invalid(format!("workspace `{workspace}` is missing its {} account", which.key())))
}

/// Ledger account of a user-facing account: (ledger id, kind, currency).
pub fn for_account(conn: &Connection, account_id: &str) -> Result<(String, Kind, String)> {
    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT l.id, l.kind, a.currency FROM ledger_accounts l JOIN accounts a ON a.id = l.account_id WHERE a.id = ?",
            [account_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (id, kind, currency) = row.ok_or_else(|| Error::Invalid("no such account".into()))?;
    Ok((id, Kind::parse(&kind)?, currency.unwrap_or_else(|| "USD".into())))
}

/// Ledger account of a category: (ledger id, kind).
pub fn for_category(conn: &Connection, category_id: &str) -> Result<(String, Kind)> {
    let row: Option<(String, String)> = conn
        .query_row("SELECT id, kind FROM ledger_accounts WHERE category_id = ?", [category_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let (id, kind) = row.ok_or_else(|| Error::Invalid("no such category".into()))?;
    Ok((id, Kind::parse(&kind)?))
}

/// The ledger account of a person (what they owe you, or you them).
pub fn for_person(conn: &Connection, person_id: &str) -> Result<String> {
    conn.query_row("SELECT l.id FROM ledger_accounts l JOIN people p ON p.id = l.person_id WHERE l.person_id = ? AND p.is_active = 1", [person_id], |r| r.get(0))
        .optional()?.ok_or_else(|| Error::Invalid("no such person".into()))
}

pub fn create_account_ledger(conn: &Connection, account_id: &str, name: &str, kind: Kind) -> Result<String> {
    let id = format!("la_acc_{account_id}");
    conn.execute(
        "INSERT INTO ledger_accounts (id, workspace_id, kind, name, account_id) VALUES (?, ?, ?, ?, ?)",
        params![id, PERSONAL_WORKSPACE, kind_str(kind), name, account_id],
    )?;
    Ok(id)
}

pub fn create_category_ledger(conn: &Connection, category_id: &str, name: &str, kind: Kind) -> Result<String> {
    let id = format!("la_cat_{category_id}");
    conn.execute(
        "INSERT INTO ledger_accounts (id, workspace_id, kind, name, category_id) VALUES (?, ?, ?, ?, ?)",
        params![id, PERSONAL_WORKSPACE, kind_str(kind), name, category_id],
    )?;
    Ok(id)
}

pub fn kind_str(k: Kind) -> &'static str {
    match k {
        Kind::Asset => "asset",
        Kind::Liability => "liability",
        Kind::Equity => "equity",
        Kind::Income => "income",
        Kind::Expense => "expense",
    }
}

/// One side of an entry.
#[derive(Debug, Clone)]
pub struct Posting {
    pub ledger_account_id: String,
    pub amount_minor: i64,
    pub currency: String,
    pub memo: Option<String>,
}

/// Writes postings for an entry, refusing anything that doesn't balance per currency.
pub fn post(conn: &Connection, entry_id: &str, postings: &[Posting]) -> Result<()> {
    if postings.len() < 2 {
        return Err(Error::Invalid("an entry needs at least two postings".into()));
    }
    let mut totals: std::collections::BTreeMap<&str, i128> = Default::default();
    for p in postings {
        *totals.entry(p.currency.as_str()).or_default() += p.amount_minor as i128;
    }
    if let Some((cur, sum)) = totals.iter().find(|(_, s)| **s != 0) {
        return Err(Error::Invalid(format!("entry does not balance in {cur} (off by {sum})")));
    }
    for p in postings {
        conn.execute(
            "INSERT INTO postings (entry_id, ledger_account_id, amount_minor, currency, memo) VALUES (?, ?, ?, ?, ?)",
            params![entry_id, p.ledger_account_id, p.amount_minor, p.currency, p.memo],
        )?;
    }
    Ok(())
}

/// Entries whose postings don't sum to zero in some currency. Should always be empty.
pub fn unbalanced_entries(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT entry_id FROM postings GROUP BY entry_id, currency HAVING SUM(amount_minor) <> 0
         UNION
         SELECT e.id FROM entries e WHERE (SELECT COUNT(*) FROM postings p WHERE p.entry_id = e.id) < 2",
    )?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    Ok(ids)
}

/// Balance of a ledger account in minor units (debit-positive). `cleared_only` skips pending entries.
pub fn balance(conn: &Connection, ledger_account_id: &str, cleared_only: bool) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(p.amount_minor), 0) FROM postings p JOIN entries e ON e.id = p.entry_id
         WHERE p.ledger_account_id = ?1 AND (?2 = 0 OR e.is_cleared = 1)",
        params![ledger_account_id, cleared_only],
        |r| r.get(0),
    )?)
}

/// Totals per ledger kind and currency for a workspace. Assets + expenses = liabilities +
/// equity + income (as credits), i.e. every currency's grand total is zero.
#[derive(Debug, Serialize)]
pub struct TrialBalanceLine {
    pub kind: Kind,
    pub currency: String,
    pub total_minor: i64,
}

pub fn trial_balance(conn: &Connection, workspace: &str) -> Result<Vec<TrialBalanceLine>> {
    let mut stmt = conn.prepare(
        "SELECT l.kind, p.currency, SUM(p.amount_minor) FROM postings p
         JOIN ledger_accounts l ON l.id = p.ledger_account_id
         WHERE l.workspace_id = ? GROUP BY l.kind, p.currency ORDER BY l.kind, p.currency",
    )?;
    let rows = stmt.query_map([workspace], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (kind, currency, total_minor) = row?;
        out.push(TrialBalanceLine { kind: Kind::parse(&kind)?, currency, total_minor });
    }
    Ok(out)
}
