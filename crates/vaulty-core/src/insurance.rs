//! Insurance: life, health, motor, home, travel and other policies. Each keeps its cover, the
//! premium and how often it's paid, when it's next due (reminded and on the calendar), who's
//! covered, the nominee and the insurer's contact. Paying a premium can add the transaction and
//! moves the due date on; premiums paid feed the tax planner (health → 80D, life → 80C). Claims
//! keep what was asked for and what was paid out. A summary sets life cover against a year's
//! income, as a common rule of thumb does.
use crate::{error::{Error, Result}, money, rows::*, transactions};
use chrono::{Months, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub const KINDS: &[&str] = &["term_life", "life", "health", "motor", "home", "travel", "accident", "other"];
pub const FREQUENCIES: &[&str] = &["monthly", "quarterly", "half_yearly", "yearly", "single"];
pub const STATUSES: &[&str] = &["active", "lapsed", "ended"];
pub const CLAIM_STATUSES: &[&str] = &["filed", "in_progress", "settled", "partly_settled", "rejected"];
/// Reminded this many days before a premium is due, and for this many days after (the grace period).
pub const DUE_AHEAD: i64 = 15;
pub const GRACE: i64 = 30;
const MAX_POLICIES: i64 = 300;

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }
fn text(v: &Value, key: &str, max: usize) -> Result<Option<String>> {
    match v[key].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) if s.chars().count() > max => Err(bad("one of the details is too long")),
        s => Ok(s.map(String::from)),
    }
}
fn date(v: &Value, key: &str) -> Result<Option<String>> {
    match v[key].as_str().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map(|d| Some(d.to_string())).map_err(|_| bad("give the date as YYYY-MM-DD")),
    }
}
fn amount(v: &Value, key: &str, cur: &str) -> Result<Option<i64>> {
    match &v[key] {
        Value::Null => Ok(None),
        Value::String(s) if s.trim().is_empty() => Ok(None),
        x => { let m = money::parse_minor(x, cur)?; if m < 0 { Err(bad("amounts can't be negative")) } else { Ok(Some(m)) } }
    }
}
fn one_of<'a>(v: &'a Value, key: &str, allowed: &[&str], default: &'a str, err: &str) -> Result<&'a str> {
    let s = v[key].as_str().unwrap_or(default);
    if allowed.contains(&s) { Ok(s) } else { Err(bad(err)) }
}
fn day(s: &str) -> Option<NaiveDate> { NaiveDate::parse_from_str(s, "%Y-%m-%d").ok() }

/// Months between premiums; none for a single premium.
pub fn months_of(frequency: &str) -> Option<u32> {
    match frequency { "monthly" => Some(1), "quarterly" => Some(3), "half_yearly" => Some(6), "yearly" => Some(12), _ => None }
}
/// A premium's yearly cost.
fn yearly(premium: i64, frequency: &str) -> i64 { months_of(frequency).map_or(0, |m| premium * 12 / m as i64) }

fn write(conn: &Connection, id: &str, v: &Value, insert: bool) -> Result<()> {
    let kind = one_of(v, "kind", KINDS, "other", "unknown kind of policy")?;
    let name = text(v, "name", 100)?.ok_or_else(|| bad("give the policy a name"))?;
    let cur = v["currency"].as_str().filter(|c| c.len() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase())).ok_or_else(|| bad("choose a currency"))?.to_string();
    let frequency = one_of(v, "frequency", FREQUENCIES, "yearly", "unknown premium frequency")?;
    let status = one_of(v, "status", STATUSES, "active", "unknown policy status")?;
    let document = v["document_id"].as_str().filter(|s| !s.is_empty());
    if let Some(d) = document {
        let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM vault_documents WHERE id = ?)", [d], |r| r.get(0))?;
        if !ok { return Err(bad("no such document")); }
    }
    let (start, end) = (date(v, "start_date")?, date(v, "end_date")?);
    if let (Some(s), Some(e)) = (&start, &end) { if e < s { return Err(bad("the policy ends before it starts")); } }
    let row = params![id, kind, name, text(v, "insurer", 100)?, text(v, "policy_no", 60)?, text(v, "holder", 100)?, text(v, "covers", 200)?,
        v["for_parents"].as_bool().unwrap_or(false) && kind == "health", cur, amount(v, "cover", &cur)?, amount(v, "premium", &cur)?.unwrap_or(0), frequency,
        date(v, "next_due")?, start, end, text(v, "nominee", 200)?, text(v, "contact", 200)?, document, status, text(v, "notes", 2000)?];
    if insert {
        conn.execute("INSERT INTO policies (id, kind, name, insurer, policy_no, holder, covers, for_parents, currency, cover_minor, premium_minor, frequency,
            next_due, start_date, end_date, nominee, contact, document_id, status, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", row)?;
    } else if conn.execute("UPDATE policies SET kind = ?2, name = ?3, insurer = ?4, policy_no = ?5, holder = ?6, covers = ?7, for_parents = ?8, currency = ?9,
            cover_minor = ?10, premium_minor = ?11, frequency = ?12, next_due = ?13, start_date = ?14, end_date = ?15, nominee = ?16, contact = ?17,
            document_id = ?18, status = ?19, notes = ?20, updated_at = CURRENT_TIMESTAMP WHERE id = ?1", row)? == 0 {
        return Err(bad("no such policy"));
    }
    Ok(())
}

pub fn create(conn: &Connection, v: &Value) -> Result<Value> {
    let n: i64 = conn.query_row("SELECT count(*) FROM policies", [], |r| r.get(0))?;
    if n >= MAX_POLICIES { return Err(bad("you can keep up to 300 policies")); }
    let id = new_id();
    write(conn, &id, v, true)?;
    Ok(json!({ "id": id }))
}
pub fn update(conn: &Connection, id: &str, v: &Value) -> Result<()> { write(conn, id, v, false) }
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM policies WHERE id = ?", [id])? == 0 { return Err(bad("no such policy")); }
    Ok(())
}

/// Every policy with `cover`, `premium`, `premium_yearly`, `days_to_due` (negative when late),
/// `due` (soon, late, lapsing — past the grace period — or none), `paid_this_year` (the last 12
/// months), payments and claims.
pub fn list(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let year_ago = (today - chrono::Days::new(365)).to_string();
    let payments = query(conn, "SELECT id, policy_id, date, amount_minor, entry_id FROM policy_payments ORDER BY date DESC", &[])?;
    let claims = query(conn, "SELECT * FROM policy_claims ORDER BY date DESC", &[])?;
    query(conn, "SELECT p.*, d.title AS document_title FROM policies p LEFT JOIN vault_documents d ON d.id = p.document_id
                  ORDER BY p.status = 'active' DESC, p.kind, p.name COLLATE NOCASE", &[])?
        .into_iter().map(|mut p| {
            let id = p["id"].as_str().unwrap_or_default().to_string();
            let cur = p["currency"].as_str().unwrap_or("INR").to_string();
            let d = |m: i64| money::to_decimal(m, &cur);
            let premium = p["premium_minor"].as_i64().unwrap_or(0);
            let freq = p["frequency"].as_str().unwrap_or("yearly").to_string();
            let days = p["next_due"].as_str().and_then(day).map(|n| (n - today).num_days());
            let active = p["status"] == "active";
            let due = match days { Some(x) if active && x < -GRACE => "lapsing", Some(x) if active && x < 0 => "late", Some(x) if active && x <= DUE_AHEAD => "soon", _ => "none" };
            let mine: Vec<Value> = payments.iter().filter(|x| x["policy_id"] == id.as_str()).map(|x| {
                let mut x = x.clone();
                x["amount"] = json!(d(x["amount_minor"].as_i64().unwrap_or(0)));
                x
            }).collect();
            let paid_year: i64 = mine.iter().filter(|x| x["date"].as_str().unwrap_or_default() > year_ago.as_str()).filter_map(|x| x["amount_minor"].as_i64()).sum();
            let my_claims: Vec<Value> = claims.iter().filter(|c| c["policy_id"] == id.as_str()).map(|c| {
                let mut c = c.clone();
                c["claimed"] = json!(d(c["claimed_minor"].as_i64().unwrap_or(0)));
                c["settled"] = json!(c["settled_minor"].as_i64().map(d));
                c
            }).collect();
            let o = p.as_object_mut().ok_or_else(|| bad("bad policy"))?;
            o.insert("cover".into(), json!(o.get("cover_minor").and_then(Value::as_i64).map(d)));
            o.insert("premium".into(), json!(d(premium)));
            o.insert("premium_yearly".into(), json!(d(yearly(premium, &freq))));
            o.insert("days_to_due".into(), json!(days));
            o.insert("due".into(), json!(due));
            o.insert("paid_this_year".into(), json!(d(paid_year)));
            o.insert("for_parents".into(), json!(o.get("for_parents").and_then(Value::as_i64) == Some(1)));
            o.insert("open_claims".into(), json!(my_claims.iter().filter(|c| matches!(c["status"].as_str(), Some("filed" | "in_progress"))).count()));
            o.insert("payments".into(), json!(mine));
            o.insert("claims".into(), json!(my_claims));
            Ok(p)
        }).collect()
}

/// Per currency, for active policies: life cover, health cover, yearly premiums, and the last
/// 12 months' income (from transactions), with life cover as a multiple of it.
pub fn summary(conn: &Connection, today: NaiveDate) -> Result<Vec<Value>> {
    let since = (today - chrono::Days::new(365)).to_string();
    let mut out = vec![];
    let curs: Vec<String> = conn.prepare("SELECT DISTINCT currency FROM policies WHERE status = 'active' ORDER BY currency")?
        .query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    for cur in curs {
        let sum = |kinds: &str| -> Result<i64> {
            Ok(conn.query_row(&format!("SELECT coalesce(sum(cover_minor), 0) FROM policies WHERE status = 'active' AND currency = ? AND kind IN ({kinds})"), [&cur], |r| r.get(0))?)
        };
        let (life, health) = (sum("'term_life', 'life'")?, sum("'health'")?);
        let mut premiums = 0;
        let mut st = conn.prepare("SELECT premium_minor, frequency FROM policies WHERE status = 'active' AND currency = ?")?;
        for r in st.query_map([&cur], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? { let (p, f) = r?; premiums += yearly(p, &f); }
        let income: i64 = conn.query_row(
            "SELECT coalesce(-sum(p.amount_minor), 0) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id JOIN entries e ON e.id = p.entry_id
              WHERE l.kind = 'income' AND p.currency = ?1 AND e.date > ?2 AND e.date <= ?3", params![cur, since, today.to_string()], |r| r.get(0))?;
        let d = |m: i64| money::to_decimal(m, &cur);
        out.push(json!({ "currency": cur, "life_cover": d(life), "health_cover": d(health), "premiums_yearly": d(premiums), "income_year": d(income.max(0)),
            "life_multiple": (income > 0).then(|| (life as f64 / income as f64 * 10.0).round() / 10.0) }));
    }
    Ok(out)
}

/// Records a premium paid: `date`, `amount` (the policy's premium if not given), and with
/// `record` ({account_id, category_id}) the transaction too. The due date moves on by the
/// policy's frequency (a single premium clears it).
pub fn pay(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    let (name, insurer, cur, premium, freq, next): (String, Option<String>, String, i64, String, Option<String>) = conn.query_row(
        "SELECT name, insurer, currency, premium_minor, frequency, next_due FROM policies WHERE id = ?", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).optional()?.ok_or_else(|| bad("no such policy"))?;
    let d = date(v, "date")?.ok_or_else(|| bad("give the date paid"))?;
    let amt = amount(v, "amount", &cur)?.unwrap_or(premium);
    if amt <= 0 { return Err(bad("give the amount paid")); }
    let pid = new_id();
    atomic(conn, |tx| {
        let entry = match &v["record"] {
            r @ Value::Object(_) => {
                let account = r["account_id"].as_str().ok_or_else(|| bad("choose the account it was paid from"))?;
                let acc_cur: String = tx.query_row("SELECT currency FROM accounts WHERE id = ?", [account], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such account"))?;
                if acc_cur != cur { return Err(bad("that account is in a different currency from the policy")); }
                let t = transactions::create(tx, &json!({ "type": "expense", "account_id": account, "category_id": r["category_id"], "date": d,
                    "amount": money::to_string(amt, &cur), "merchant": insurer.clone().unwrap_or_else(|| name.clone()), "description": name, "tags": "insurance" }))?;
                t["id"].as_str().map(str::to_string)
            }
            _ => None,
        };
        tx.execute("INSERT INTO policy_payments (id, policy_id, date, amount_minor, entry_id) VALUES (?, ?, ?, ?, ?)", params![pid, id, d, amt, entry])?;
        // The next due date after this one: from the date that was due (so paying early or late
        // keeps the policy's own dates), or from the payment if none was set.
        let base = next.as_deref().and_then(day).or_else(|| day(&d));
        let moved = match (months_of(&freq), base) { (Some(m), Some(b)) => b.checked_add_months(Months::new(m)).map(|x| x.to_string()), _ => None };
        tx.execute("UPDATE policies SET next_due = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?", params![moved, id])?;
        Ok(())
    })?;
    Ok(json!({ "id": pid }))
}

/// Removes a recorded premium (its transaction, if one was added, stays).
pub fn delete_payment(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM policy_payments WHERE id = ?", [id])? == 0 { return Err(bad("no such payment")); }
    Ok(())
}

/// A claim's checked fields: date, what, claimed, settled, status, settled on, reference, notes.
type ClaimFields = (String, String, i64, Option<i64>, String, Option<String>, Option<String>, Option<String>);
fn claim_fields(conn: &Connection, policy_id: &str, v: &Value) -> Result<ClaimFields> {
    let cur: String = conn.query_row("SELECT currency FROM policies WHERE id = ?", [policy_id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such policy"))?;
    let d = date(v, "date")?.ok_or_else(|| bad("give the date of the claim"))?;
    let what = text(v, "what", 200)?.ok_or_else(|| bad("say what the claim is for"))?;
    let claimed = amount(v, "claimed", &cur)?.ok_or_else(|| bad("give the amount claimed"))?;
    let status = one_of(v, "status", CLAIM_STATUSES, "filed", "unknown claim status")?.to_string();
    let settled = amount(v, "settled", &cur)?;
    Ok((d, what, claimed, settled, status, date(v, "settled_on")?, text(v, "reference", 60)?, text(v, "notes", 1000)?))
}
pub fn add_claim(conn: &Connection, policy_id: &str, v: &Value) -> Result<Value> {
    let (d, what, claimed, settled, status, on, reference, notes) = claim_fields(conn, policy_id, v)?;
    let id = new_id();
    conn.execute("INSERT INTO policy_claims (id, policy_id, date, what, claimed_minor, settled_minor, status, settled_on, reference, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, policy_id, d, what, claimed, settled, status, on, reference, notes])?;
    Ok(json!({ "id": id }))
}
pub fn update_claim(conn: &Connection, id: &str, v: &Value) -> Result<()> {
    let policy: String = conn.query_row("SELECT policy_id FROM policy_claims WHERE id = ?", [id], |r| r.get(0)).optional()?.ok_or_else(|| bad("no such claim"))?;
    let (d, what, claimed, settled, status, on, reference, notes) = claim_fields(conn, &policy, v)?;
    conn.execute("UPDATE policy_claims SET date = ?2, what = ?3, claimed_minor = ?4, settled_minor = ?5, status = ?6, settled_on = ?7, reference = ?8, notes = ?9 WHERE id = ?1",
        params![id, d, what, claimed, settled, status, on, reference, notes])?;
    Ok(())
}
pub fn delete_claim(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM policy_claims WHERE id = ?", [id])? == 0 { return Err(bad("no such claim")); }
    Ok(())
}

/// Premiums due between `from` and `to` (each active policy's next due date and the ones after
/// it, by frequency): (policy id, name, date, amount in minor units, currency).
pub type PremiumDue = (String, String, NaiveDate, i64, String);
pub fn dues(conn: &Connection, from: NaiveDate, to: NaiveDate) -> Result<Vec<PremiumDue>> {
    let mut out = vec![];
    let mut st = conn.prepare("SELECT id, name, next_due, premium_minor, currency, frequency FROM policies WHERE status = 'active' AND next_due IS NOT NULL")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?)))?;
    for r in rows {
        let (id, name, next, premium, cur, freq) = r?;
        let Some(mut d) = day(&next) else { continue };
        for _ in 0..60 {
            if d > to { break; }
            if d >= from { out.push((id.clone(), name.clone(), d, premium, cur.clone())); }
            match months_of(&freq).and_then(|m| d.checked_add_months(Months::new(m))) { Some(n) => d = n, None => break }
        }
    }
    Ok(out)
}

/// Premiums paid in a period, by tax use: health for self and family, health for parents, and
/// life (in minor units, for `currency`).
pub fn premiums_paid(conn: &Connection, currency: &str, from: &str, to: &str) -> Result<(i64, i64, i64)> {
    let q = |cond: &str| -> Result<i64> {
        Ok(conn.query_row(&format!("SELECT coalesce(sum(x.amount_minor), 0) FROM policy_payments x JOIN policies p ON p.id = x.policy_id
            WHERE p.currency = ?1 AND x.date BETWEEN ?2 AND ?3 AND {cond}"), params![currency, from, to], |r| r.get(0))?)
    };
    Ok((q("p.kind = 'health' AND p.for_parents = 0")?, q("p.kind = 'health' AND p.for_parents = 1")?, q("p.kind IN ('term_life', 'life')")?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn yearly_premiums() {
        assert_eq!(yearly(1000, "monthly"), 12000);
        assert_eq!(yearly(5000, "half_yearly"), 10000);
        assert_eq!(yearly(9999, "single"), 0);
    }
}
