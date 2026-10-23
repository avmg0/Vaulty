//! "In case of emergency": everything a family would need to find and claim a member's money,
//! on one page. Shown to the member's nominee (read-only), and to the member as a preview.
//!
//! Lists what exists and where (bank, card network and last 4 digits, institution), with balances.
//! Card numbers aren't stored by Vaulty, so they never appear.
use crate::error::Result;
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct Holding {
    pub name: String,
    /// The account or investment type (checking, cash, credit, loan, stock, mutual_fund…).
    pub kind: String,
    pub institution: Option<String>,
    pub currency: String,
    pub amount: f64,
    pub notes: Option<String>,
    /// Who is registered to receive it.
    pub nominee: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PolicyLine {
    pub name: String,
    pub kind: String,
    pub insurer: Option<String>,
    pub policy_no: Option<String>,
    pub currency: String,
    /// Sum assured or cover.
    pub cover: Option<f64>,
    pub covers: Option<String>,
    pub nominee: Option<String>,
    pub contact: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CardLine {
    pub name: String,
    pub kind: String,
    pub issuer: Option<String>,
    pub network: Option<String>,
    pub last4: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PersonLine {
    pub name: String,
    pub phone: Option<String>,
    pub currency: String,
    /// Positive: they owe the member. Negative: the member owes them.
    pub amount: f64,
}

#[derive(Debug, Serialize)]
pub struct Regular {
    pub name: String,
    /// expense, income or transfer
    pub kind: String,
    pub amount: f64,
    pub currency: String,
    pub frequency: String,
    pub every: i64,
    pub next_date: Option<String>,
    pub account: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DocumentLine {
    pub title: String,
    pub kind: String,
    pub holder: Option<String>,
    pub number: Option<String>,
    pub issuer: Option<String>,
    pub expires_on: Option<String>,
    /// Where the paper original is kept.
    pub kept_at: Option<String>,
    /// How many files (scans) Vaulty holds for it.
    pub files: usize,
}

#[derive(Debug, Serialize)]
pub struct Total { pub currency: String, pub owned: f64, pub owed: f64, pub net: f64 }

#[derive(Debug, Serialize)]
pub struct Estate {
    pub as_of: String,
    /// Bank, cash and other accounts the member owns.
    pub accounts: Vec<Holding>,
    /// Credit cards and loans: what's owed on them.
    pub debts: Vec<Holding>,
    pub investments: Vec<Holding>,
    /// Active insurance policies: what to claim, from whom, and who the nominee is.
    pub policies: Vec<PolicyLine>,
    pub cards: Vec<CardLine>,
    pub people: Vec<PersonLine>,
    /// Bills and income that come round regularly (to claim, keep paying, or stop).
    pub regular: Vec<Regular>,
    /// Papers: policies, IDs, property documents, wills, with where the originals are.
    pub documents: Vec<DocumentLine>,
    /// Per currency: owned (accounts and investments), owed (debts and money owed to people).
    pub totals: Vec<Total>,
}

fn s(v: &Value, k: &str) -> Option<String> { v[k].as_str().map(str::trim).filter(|x| !x.is_empty()).map(str::to_string) }

pub fn summary(conn: &Connection, today: NaiveDate) -> Result<Estate> {
    let main: String = conn.query_row("SELECT currency FROM user_settings WHERE id = 'default'", [], |r| r.get::<_, Option<String>>(0))?.unwrap_or_else(|| "INR".into());
    let (mut accounts, mut debts) = (vec![], vec![]);
    for a in crate::accounts::get_all(conn)? {
        let h = Holding {
            name: s(&a, "name").unwrap_or_default(), kind: s(&a, "type").unwrap_or_default(), institution: s(&a, "institution"),
            currency: s(&a, "currency").unwrap_or_else(|| main.clone()),
            amount: if a["ledger_kind"] == "liability" { -a["current_balance"].as_f64().unwrap_or(0.0) } else { a["current_balance"].as_f64().unwrap_or(0.0) },
            notes: s(&a, "notes"), nominee: s(&a, "nominee"),
        };
        if a["ledger_kind"] == "liability" { if h.amount.abs() > 0.004 { debts.push(h) } } else { accounts.push(h) }
    }
    let investments: Vec<Holding> = crate::investments::list(conn, today)?.into_iter()
        .filter(|i| !i["closed"].as_bool().unwrap_or(false))
        .map(|i| Holding {
            name: s(&i, "name").unwrap_or_default(), kind: s(&i, "kind").unwrap_or_default(),
            institution: Some([s(&i, "provider"), s(&i, "folio").map(|f| format!("folio {f}")), s(&i, "symbol")].into_iter().flatten().collect::<Vec<_>>().join(", ")).filter(|x| !x.is_empty()),
            currency: s(&i, "currency").unwrap_or_else(|| main.clone()), amount: i["value"].as_f64().unwrap_or(0.0), notes: s(&i, "notes"), nominee: s(&i, "nominee"),
        }).collect();
    let policies = crate::insurance::list(conn, today)?.into_iter().filter(|p| p["status"] == "active").map(|p| PolicyLine {
        name: s(&p, "name").unwrap_or_default(), kind: s(&p, "kind").unwrap_or_default(), insurer: s(&p, "insurer"), policy_no: s(&p, "policy_no"),
        currency: s(&p, "currency").unwrap_or_else(|| main.clone()), cover: p["cover"].as_f64(), covers: s(&p, "covers"), nominee: s(&p, "nominee"), contact: s(&p, "contact"),
    }).collect();
    let cards = crate::cards::list(conn)?.into_iter().map(|c| CardLine {
        name: s(&c, "name").unwrap_or_default(), kind: s(&c, "kind").unwrap_or_default(),
        issuer: s(&c, "issuer_name").or(s(&c, "institution")), network: s(&c, "network"), last4: s(&c, "last4"),
    }).collect();
    let mut people = vec![];
    for p in crate::people::list(conn)? {
        for b in p["balances"].as_array().into_iter().flatten() {
            people.push(PersonLine { name: s(&p, "name").unwrap_or_default(), phone: s(&p, "phone"), currency: s(b, "currency").unwrap_or_default(), amount: b["amount"].as_f64().unwrap_or(0.0) });
        }
    }
    let regular = crate::recurring::list(conn, today)?.into_iter().filter(|r| r["is_active"] == 1).map(|r| Regular {
        name: s(&r, "name").unwrap_or_default(), kind: s(&r, "type").unwrap_or_default(), amount: r["amount"].as_f64().unwrap_or(0.0),
        currency: s(&r, "currency").unwrap_or_else(|| main.clone()), frequency: s(&r, "frequency").unwrap_or_default(),
        every: r["every"].as_i64().unwrap_or(1), next_date: s(&r, "next_due_date"), account: s(&r, "account_name"),
    }).collect();

    let documents = crate::documents::list(conn, today)?.into_iter().map(|d| DocumentLine {
        title: s(&d, "title").unwrap_or_default(), kind: s(&d, "kind").unwrap_or_default(), holder: s(&d, "holder"), number: s(&d, "number"),
        issuer: s(&d, "issuer"), expires_on: s(&d, "expires_on"), kept_at: s(&d, "kept_at"), files: d["files"].as_array().map_or(0, Vec::len),
    }).collect();

    let mut by: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for h in accounts.iter().chain(&investments) { by.entry(h.currency.clone()).or_default().0 += h.amount; }
    for h in &debts { by.entry(h.currency.clone()).or_default().1 += h.amount; }
    for p in &people {
        let e = by.entry(p.currency.clone()).or_default();
        if p.amount > 0.0 { e.0 += p.amount } else { e.1 -= p.amount }
    }
    let round = |x: f64| (x * 100.0).round() / 100.0;
    let totals = by.into_iter().map(|(currency, (owned, owed))| Total { net: round(owned - owed), owned: round(owned), owed: round(owed), currency }).collect();
    Ok(Estate { as_of: today.to_string(), accounts, debts, investments, policies, cards, people, regular, documents, totals })
}
