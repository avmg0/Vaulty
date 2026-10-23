//! What credit cards give back, and which card to use today.
//!
//! - **Rewards:** a percent back on a category (with its sub-categories), at a place, or on
//!   everything else; optionally capped per statement. Points cards are entered as the percent
//!   their points are worth.
//! - **Milestones:** spend a target in the card year (e.g. to have the annual fee waived).
//! - **Perks:** so many per card year (e.g. airport lounge visits), counted as they're used.
//!
//! The card year starts on the card's `card_year_start` (MM-DD), or 1 January.
//!
//! **Best card for today**, for a purchase (amount, category, place): among credit cards with
//! enough limit left, the one giving the most back, then the one with the most days until the
//! purchase must be paid without interest.
use crate::{card_cycles, error::{Error, Result}, money, rows::*};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

pub const KINDS: &[&str] = &["reward", "milestone", "perk"];

/// The start of the card year that `today` falls in.
pub fn card_year_start(mm_dd: Option<&str>, today: NaiveDate) -> NaiveDate {
    let (m, d) = mm_dd.and_then(|s| s.split_once('-')).and_then(|(m, d)| Some((m.parse::<u32>().ok()?, d.parse::<u32>().ok()?))).unwrap_or((1, 1));
    let on = |y: i32| (1..=d).rev().find_map(|d| NaiveDate::from_ymd_opt(y, m, d)).unwrap_or(today);
    let this = on(today.year());
    if this <= today { this } else { on(today.year() - 1) }
}

struct CardInfo { account_id: String, currency: String, year_start: NaiveDate, cycle_start: NaiveDate }

fn card_info(conn: &Connection, card_id: &str, today: NaiveDate) -> Result<CardInfo> {
    let row = query_one(conn, "SELECT c.kind, c.account_id, c.card_year_start, a.currency FROM cards c LEFT JOIN accounts a ON a.id = c.account_id WHERE c.id = ?", &[json!(card_id)])?
        .ok_or_else(|| Error::Invalid("no such card".into()))?;
    if row["kind"] != "credit" { return Err(Error::Invalid("benefits are tracked for credit cards".into())); }
    let account_id = row["account_id"].as_str().unwrap_or_default().to_string();
    // The current statement period: since the latest statement, or this calendar month.
    let cycle_start = card_cycles::for_card(conn, card_id, today)?
        .and_then(|c| c.current.and_then(|s| NaiveDate::parse_from_str(&s.statement_date, "%Y-%m-%d").ok()).map(|d| d.succ_opt().unwrap_or(d)))
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today));
    Ok(CardInfo { account_id, currency: row["currency"].as_str().unwrap_or("INR").into(), year_start: card_year_start(row["card_year_start"].as_str(), today), cycle_start })
}

/// Spending on a card since `from`, optionally only what a reward covers.
fn spent(conn: &Connection, account_id: &str, from: NaiveDate, today: NaiveDate, b: Option<&Value>) -> Result<i64> {
    let mut sql = "SELECT COALESCE(SUM(t.amount_minor), 0) FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
                    WHERE t.account_id = ?1 AND t.type = 'expense' AND t.date BETWEEN ?2 AND ?3".to_string();
    let mut args = vec![json!(account_id), json!(from.to_string()), json!(today.to_string())];
    if let Some(b) = b {
        if let Some(cat) = b["category_id"].as_str() { sql.push_str(" AND (t.category_id = ?4 OR c.parent_id = ?4)"); args.push(json!(cat)); }
        else if let Some(m) = b["merchant"].as_str() { sql.push_str(" AND LOWER(COALESCE(t.merchant, t.description, '')) LIKE '%' || LOWER(?4) || '%'"); args.push(json!(m)); }
    }
    let n: i64 = conn.query_row(&sql, rusqlite::params_from_iter(args.iter().map(|v| match v { Value::String(s) => rusqlite::types::Value::Text(s.clone()), _ => rusqlite::types::Value::Null })), |r| r.get(0))?;
    Ok(n.abs())
}

fn earned(rate: f64, spend: i64, cap: Option<i64>) -> i64 {
    let e = (spend as f64 * rate / 100.0).round() as i64;
    cap.map_or(e, |c| e.min(c))
}

/// A card's benefits, with progress: rewards earned this statement, milestone spend this card
/// year, perks used this card year.
pub fn list(conn: &Connection, card_id: &str, today: NaiveDate) -> Result<Vec<Value>> {
    let info = card_info(conn, card_id, today)?;
    let dec = |m: i64| money::to_decimal(m, &info.currency);
    let rows = query(conn, "SELECT b.*, c.name AS category_name FROM card_benefits b LEFT JOIN categories c ON c.id = b.category_id WHERE b.card_id = ? ORDER BY b.kind, b.created_at", &[json!(card_id)])?;
    let mut out = vec![];
    for mut b in rows {
        let progress = match b["kind"].as_str() {
            Some("reward") => {
                let spend = spent(conn, &info.account_id, info.cycle_start, today, Some(&b))?;
                let cap = b["cap_minor"].as_i64();
                let e = earned(b["rate"].as_f64().unwrap_or(0.0), spend, cap);
                json!({ "spent": dec(spend), "earned": dec(e), "cap": cap.map(dec), "capped": cap.is_some_and(|c| e >= c), "since": info.cycle_start.to_string() })
            }
            Some("milestone") => {
                let spend = spent(conn, &info.account_id, info.year_start, today, None)?;
                let target = b["target_minor"].as_i64().unwrap_or(0);
                json!({ "spent": dec(spend), "target": dec(target), "left": dec((target - spend).max(0)), "reached": spend >= target, "since": info.year_start.to_string() })
            }
            _ => {
                let this_year = b["used_year"].as_str() == Some(info.year_start.to_string().as_str());
                let used = if this_year { b["used"].as_i64().unwrap_or(0) } else { 0 };
                json!({ "used": used, "allowance": b["allowance"], "left": b["allowance"].as_i64().map(|a| (a - used).max(0)), "since": info.year_start.to_string() })
            }
        };
        if let Some(o) = b.as_object_mut() {
            o.insert("progress".into(), progress);
            for (col, key) in [("cap_minor", "cap"), ("target_minor", "target")] {
                let v = o[col].as_i64().map(dec);
                o.insert(key.into(), json!(v));
            }
        }
        out.push(b);
    }
    Ok(out)
}

type Fields = (String, String, Option<String>, Option<String>, Option<f64>, Option<i64>, Option<i64>, Option<i64>, Option<String>);

fn fields(conn: &Connection, v: &Value, currency: &str) -> Result<Fields> {
    let kind = v["kind"].as_str().filter(|k| KINDS.contains(k)).ok_or_else(|| Error::Invalid("a benefit is a reward, a milestone or a perk".into()))?;
    let title: String = v["title"].as_str().map(str::trim).unwrap_or_default().chars().take(80).collect();
    if title.is_empty() { return Err(Error::Invalid("give the benefit a name".into())); }
    let money_of = |k: &str| -> Result<Option<i64>> { match &v[k] { Value::Null => Ok(None), Value::String(s) if s.trim().is_empty() => Ok(None), x => Ok(Some(money::parse_minor(x, currency)?)) } };
    let notes = v["notes"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(500).collect());
    let (mut category, mut merchant, mut rate, mut cap, mut target, mut allowance) = (None, None, None, None, None, None);
    match kind {
        "reward" => {
            let r = v["rate"].as_f64().or_else(|| v["rate"].as_str().and_then(|s| s.trim().parse().ok()))
                .ok_or_else(|| Error::Invalid("give the reward as a percent back".into()))?;
            if !(0.0..=100.0).contains(&r) || r == 0.0 { return Err(Error::Invalid("a reward is more than 0% and at most 100%".into())); }
            rate = Some(r);
            category = v["category_id"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
            if let Some(c) = &category {
                let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE id = ? AND type = 'expense')", [c], |r| r.get(0))?;
                if !ok { return Err(Error::Invalid("choose a spending category".into())); }
            } else {
                merchant = v["merchant"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(60).collect());
            }
            cap = money_of("cap")?;
        }
        "milestone" => {
            target = money_of("target")?.filter(|t| *t > 0);
            if target.is_none() { return Err(Error::Invalid("give the amount to spend for the milestone".into())); }
        }
        _ => {
            allowance = v["allowance"].as_i64().or_else(|| v["allowance"].as_str().and_then(|s| s.trim().parse().ok()));
            if allowance.is_some_and(|a| !(1..=1000).contains(&a)) { return Err(Error::Invalid("a perk can be used 1 to 1000 times a year".into())); }
        }
    }
    Ok((kind.into(), title, category, merchant, rate, cap, target, allowance, notes))
}

pub fn create(conn: &Connection, card_id: &str, v: &Value, today: NaiveDate) -> Result<Value> {
    let info = card_info(conn, card_id, today)?;
    let (kind, title, category, merchant, rate, cap, target, allowance, notes) = fields(conn, v, &info.currency)?;
    let id = new_id();
    conn.execute("INSERT INTO card_benefits (id, card_id, kind, title, category_id, merchant, rate, cap_minor, target_minor, allowance, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, card_id, kind, title, category, merchant, rate, cap, target, allowance, notes])?;
    Ok(json!({ "id": id }))
}

pub fn update(conn: &Connection, id: &str, v: &Value, today: NaiveDate) -> Result<()> {
    let card_id: String = conn.query_row("SELECT card_id FROM card_benefits WHERE id = ?", [id], |r| r.get(0)).map_err(|_| Error::Invalid("no such benefit".into()))?;
    let info = card_info(conn, &card_id, today)?;
    let (kind, title, category, merchant, rate, cap, target, allowance, notes) = fields(conn, v, &info.currency)?;
    conn.execute("UPDATE card_benefits SET kind = ?, title = ?, category_id = ?, merchant = ?, rate = ?, cap_minor = ?, target_minor = ?, allowance = ?, notes = ? WHERE id = ?",
        params![kind, title, category, merchant, rate, cap, target, allowance, notes, id])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM card_benefits WHERE id = ?", [id])? == 0 { return Err(Error::Invalid("no such benefit".into())); }
    Ok(())
}

/// Counts a perk as used (or takes one back, `by` = -1). Starts again each card year.
pub fn use_perk(conn: &Connection, id: &str, by: i64, today: NaiveDate) -> Result<i64> {
    let (card_id, kind, used, year): (String, String, i64, Option<String>) = conn.query_row(
        "SELECT card_id, kind, used, used_year FROM card_benefits WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|_| Error::Invalid("no such benefit".into()))?;
    if kind != "perk" { return Err(Error::Invalid("only perks are counted".into())); }
    let start = card_info(conn, &card_id, today)?.year_start.to_string();
    let now = if year.as_deref() == Some(start.as_str()) { used } else { 0 };
    let next = (now + by.signum()).max(0);
    conn.execute("UPDATE card_benefits SET used = ?, used_year = ? WHERE id = ?", params![next, start, id])?;
    Ok(next)
}

#[derive(Debug, Serialize)]
pub struct Pick {
    pub card_id: String,
    pub name: String,
    pub currency: String,
    /// The reward that applies, if any.
    pub reward: Option<String>,
    pub rate: f64,
    /// What this purchase would earn (after any cap already used up this statement).
    pub earns: Option<f64>,
    pub interest_free_days: Option<i64>,
    pub pay_by: Option<String>,
    pub available: Option<f64>,
    /// Enough limit left for the amount.
    pub fits: bool,
}

/// Credit cards ranked for a purchase today. `amount`, `category_id` and `place` are optional.
pub fn best_for(conn: &Connection, today: NaiveDate, amount: Option<&Value>, category_id: Option<&str>, place: Option<&str>) -> Result<Vec<Pick>> {
    let parent: Option<String> = match category_id {
        Some(c) => conn.query_row("SELECT parent_id FROM categories WHERE id = ?", [c], |r| r.get(0)).unwrap_or(None),
        None => None,
    };
    let place = place.map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty());
    let mut out = vec![];
    for c in crate::cards::list(conn)?.into_iter().filter(|c| c["kind"] == "credit") {
        let id = c["id"].as_str().unwrap_or_default().to_string();
        let info = card_info(conn, &id, today)?;
        let amt = amount.filter(|a| !a.is_null() && a.as_str() != Some("")).map(|a| money::parse_minor(a, &info.currency)).transpose()?;
        // The best reward that applies: a place or category match, or the card's everyday rate.
        let mut best: Option<(String, f64, Option<i64>, Value)> = None;
        for b in query(conn, "SELECT * FROM card_benefits WHERE card_id = ? AND kind = 'reward'", &[json!(id)])? {
            let applies = match (b["category_id"].as_str(), b["merchant"].as_str()) {
                (Some(cat), _) => category_id == Some(cat) || parent.as_deref() == Some(cat),
                (None, Some(m)) => place.as_deref().is_some_and(|p| p.contains(&m.to_lowercase())),
                (None, None) => true,
            };
            let rate = b["rate"].as_f64().unwrap_or(0.0);
            if applies && best.as_ref().is_none_or(|x| rate > x.1) {
                best = Some((b["title"].as_str().unwrap_or_default().into(), rate, b["cap_minor"].as_i64(), b));
            }
        }
        let earns = match (&best, amt) {
            (Some((_, rate, cap, b)), Some(a)) => {
                let room = cap.map(|cap| (cap - earned(*rate, spent(conn, &info.account_id, info.cycle_start, today, Some(b)).unwrap_or(0), Some(cap))).max(0));
                let e = earned(*rate, a, None);
                Some(money::to_decimal(room.map_or(e, |r| e.min(r)), &info.currency))
            }
            _ => None,
        };
        let cyc = card_cycles::for_card(conn, &id, today)?;
        let available = c["available_minor"].as_i64();
        out.push(Pick {
            name: c["name"].as_str().unwrap_or_default().into(), currency: info.currency.clone(),
            reward: best.as_ref().map(|b| b.0.clone()), rate: best.as_ref().map_or(0.0, |b| b.1), earns,
            interest_free_days: cyc.as_ref().map(|x| x.interest_free_days), pay_by: cyc.map(|x| x.buy_today_due),
            available: available.map(|a| money::to_decimal(a, &info.currency)),
            fits: match (available, amt) { (Some(av), Some(a)) => av >= a, _ => true },
            card_id: id,
        });
    }
    out.sort_by(|a, b| b.fits.cmp(&a.fits)
        .then(b.earns.unwrap_or(0.0).total_cmp(&a.earns.unwrap_or(0.0)))
        .then(b.rate.total_cmp(&a.rate))
        .then(b.interest_free_days.unwrap_or(-1).cmp(&a.interest_free_days.unwrap_or(-1))));
    Ok(out)
}
