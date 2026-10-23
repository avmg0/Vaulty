//! The tax planner, kept: the figures for each country and year, tax already paid (TDS, advance
//! tax, self-assessment), what's still to pay or coming back, advance-tax instalments with their
//! dates, and figures filled from the person's own records: spending in categories linked to a
//! deduction (health insurance premiums → the health insurance deduction), salary credited, and
//! ELSS fund purchases (tax-saving investments). Also the HRA exemption and rent receipts.
use crate::{error::{Error, Result}, money, rows::*, tax};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

pub const PAYMENT_KINDS: &[&str] = &["tds", "advance", "self_assessment"];
/// What a category can feed besides a deduction.
pub const INCOME_TARGETS: &[&str] = &["salary", "other_income"];

fn bad(m: &str) -> Error { Error::Invalid(m.into()) }
fn plan_id(country: &str, year: &str) -> String { format!("{}:{}", country.to_uppercase(), year) }
fn day(s: &str) -> Result<NaiveDate> { NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| bad("give the date")) }

/// The year's rules, and its dates.
fn year(country: &str, year_id: &str) -> Result<(tax::Pack, tax::Year)> {
    let p = tax::pack(country)?;
    let y = tax::year_of(&p, Some(year_id))?.clone();
    Ok((p, y))
}

/// Every deduction id in the year (any regime).
fn deduction_ids(y: &tax::Year) -> Vec<String> {
    let mut ids: Vec<String> = vec![];
    for r in &y.regimes { for d in &r.deductions { if !ids.contains(&d.id) { ids.push(d.id.clone()); } } }
    ids
}

// ---------------------------------------------------------------- figures from records

/// What the person's records say for the year: per linked target (a deduction, salary, other
/// income) the amount in the year's currency, and ELSS purchases separately.
pub fn records(conn: &Connection, country: &str, year_id: &str) -> Result<Value> {
    let (p, y) = year(country, year_id)?;
    let cur = p.currency.clone();
    let mut by: Map<String, Value> = Map::new();
    let mut st = conn.prepare(
        "SELECT k.target, l.kind, sum(po.amount_minor)
           FROM tax_links k JOIN ledger_accounts l ON l.category_id = k.category_id
           JOIN postings po ON po.ledger_account_id = l.id JOIN entries e ON e.id = po.entry_id
          WHERE po.currency = ?1 AND e.date BETWEEN ?2 AND ?3
          GROUP BY k.target, l.kind")?;
    let rows = st.query_map(params![cur, y.start, y.end], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?;
    let mut sums: std::collections::BTreeMap<String, i64> = Default::default();
    for r in rows {
        let (target, kind, sum) = r?;
        // Spending is positive on expense ledgers; income is negative on income ledgers.
        *sums.entry(target).or_default() += if kind == "income" { -sum } else { sum };
    }
    // ELSS (tax-saving) fund purchases in the year count towards tax-saving investments.
    let elss: i64 = conn.query_row(
        "SELECT coalesce(sum(abs(t.amount_minor)), 0) FROM holding_txns t JOIN holdings h ON h.id = t.holding_id
          WHERE h.currency = ?1 AND t.kind IN ('purchase', 'sip') AND t.date BETWEEN ?2 AND ?3
            AND (lower(h.name) LIKE '%elss%' OR lower(h.name) LIKE '%tax saver%' OR lower(h.name) LIKE '%tax saving%')",
        params![cur, y.start, y.end], |r| r.get(0))?;
    if elss > 0 && deduction_ids(&y).iter().any(|d| d == "investments") { *sums.entry("investments".into()).or_default() += elss; }
    for (k, v) in &sums { by.insert(k.clone(), json!(money::to_decimal((*v).max(0), &cur))); }
    Ok(json!({ "by": by, "elss": money::to_decimal(elss, &cur) }))
}

/// Category links, with names.
pub fn links(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, "SELECT k.id, k.category_id, k.target, c.name AS category_name, c.icon AS category_icon, c.type AS category_type
                   FROM tax_links k JOIN categories c ON c.id = k.category_id ORDER BY k.target, c.name COLLATE NOCASE", &[])
}

/// Replaces the links: `[{category_id, target}]`. Spending categories feed deductions; income
/// categories feed salary or other income.
pub fn set_links(conn: &Connection, country: &str, year_id: &str, list: &Value) -> Result<Vec<Value>> {
    let (_, y) = year(country, year_id)?;
    let ids = deduction_ids(&y);
    let list = list.as_array().cloned().unwrap_or_default();
    if list.len() > 200 { return Err(bad("link at most 200 categories")); }
    atomic(conn, |tx| {
        tx.execute("DELETE FROM tax_links", [])?;
        for l in &list {
            let cat = l["category_id"].as_str().ok_or_else(|| bad("no such category"))?;
            let target = l["target"].as_str().unwrap_or_default();
            let kind: Option<String> = tx.query_row("SELECT type FROM categories WHERE id = ?", [cat], |r| r.get(0)).optional()?;
            let kind = kind.ok_or_else(|| bad("no such category"))?;
            let ok = if kind == "income" { INCOME_TARGETS.contains(&target) } else { ids.iter().any(|i| i == target) };
            if !ok { return Err(bad("that category can't count towards that")); }
            tx.execute("INSERT INTO tax_links (id, category_id, target) VALUES (?, ?, ?) ON CONFLICT(category_id) DO UPDATE SET target = excluded.target",
                params![new_id(), cat, target])?;
        }
        Ok(())
    })?;
    links(conn)
}

// ---------------------------------------------------------------- HRA

/// The exempt part of house rent allowance (India, s.10(13A)), for the year: the least of the
/// HRA received, rent paid less 10% of salary (basic + DA), and 50% of salary in a metro city
/// (40% elsewhere). Amounts in minor units.
pub fn hra_exempt(salary: i64, hra: i64, rent: i64, metro: bool) -> i64 {
    let over = (rent - salary / 10).max(0);
    let share = salary * if metro { 50 } else { 40 } / 100;
    hra.min(over).min(share).max(0)
}

fn hra_view(h: &Value, cur: &str, y: &tax::Year) -> Result<Value> {
    let amt = |k: &str| -> Result<i64> {
        match &h[k] { Value::Null => Ok(0), Value::String(s) if s.trim().is_empty() => Ok(0), v => money::parse_minor(v, cur).map(|m| m.max(0)) }
    };
    let (salary, received, monthly) = (amt("salary")?, amt("received")?, amt("monthly_rent")?);
    let start = day(&y.start)?;
    let months = |k: &str, default: NaiveDate| -> NaiveDate {
        h[k].as_str().and_then(|m| NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").ok()).unwrap_or(default)
    };
    let from = months("from", start);
    let to = months("to", day(&y.end)?.with_day(1).unwrap_or(start));
    let count = ((to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32 + 1).clamp(0, 12) as i64;
    let rent = monthly * count;
    let metro = h["metro"].as_bool().unwrap_or(false);
    Ok(json!({
        "months": count, "rent": money::to_decimal(rent, cur),
        "exempt": money::to_decimal(hra_exempt(salary, received, rent, metro), cur),
        // The landlord's PAN is needed when the year's rent is over ₹1 lakh.
        "pan_needed": cur == "INR" && rent > 100_000 * money::scale(cur),
    }))
}

// ---------------------------------------------------------------- the plan

fn stored(conn: &Connection, id: &str) -> Result<Option<(Value, Option<String>, Value)>> {
    conn.query_row("SELECT input, regime, hra FROM tax_plans WHERE id = ?", [id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)))
        .optional()?
        .map(|(i, r, h)| Ok((serde_json::from_str(&i).unwrap_or(json!({})), r, serde_json::from_str(&h).unwrap_or(json!({})))))
        .transpose()
}

/// Saves the planner's figures (`input`), the chosen regime (`regime`, empty for whichever is
/// lower) and the HRA details (`hra`). Figures are checked as the planner reads them.
pub fn save(conn: &Connection, country: &str, year_id: &str, v: &Value) -> Result<()> {
    let (p, y) = year(country, year_id)?;
    let input = v.get("input").cloned().unwrap_or(json!({}));
    tax::parse_input(&input, &p.currency)?;
    let regime = v["regime"].as_str().filter(|r| !r.is_empty());
    if let Some(r) = regime { if !y.regimes.iter().any(|x| x.id == r) { return Err(bad("unknown tax regime")); } }
    let hra = v.get("hra").cloned().unwrap_or(json!({}));
    hra_view(&hra, &p.currency, &y)?;
    for k in ["landlord", "landlord_pan", "address", "tenant"] {
        if hra[k].as_str().is_some_and(|s| s.chars().count() > 200) { return Err(bad("one of the details is too long (up to 200 characters)")); }
    }
    conn.execute(
        "INSERT INTO tax_plans (id, country, year_id, input, regime, hra) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET input = excluded.input, regime = excluded.regime, hra = excluded.hra, updated_at = CURRENT_TIMESTAMP",
        params![plan_id(country, year_id), p.country, y.id, input.to_string(), regime, hra.to_string()])?;
    Ok(())
}

/// Records tax paid: `kind` (tds, advance, self_assessment), `date`, `amount`, `note`.
pub fn add_payment(conn: &Connection, country: &str, year_id: &str, v: &Value) -> Result<Value> {
    let (p, y) = year(country, year_id)?;
    let id = plan_id(country, year_id);
    if stored(conn, &id)?.is_none() { save(conn, country, year_id, &json!({}))?; }
    let kind = v["kind"].as_str().unwrap_or("advance");
    if !PAYMENT_KINDS.contains(&kind) { return Err(bad("unknown kind of tax payment")); }
    let d = day(v["date"].as_str().unwrap_or_default())?;
    // Tax for a year can be paid after it ends (self-assessment), but not before it starts.
    if d < day(&y.start)? { return Err(bad("that date is before the tax year starts")); }
    let amount = money::parse_minor(v.get("amount").unwrap_or(&Value::Null), &p.currency)?;
    if amount <= 0 { return Err(bad("give the amount paid")); }
    let note = v["note"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(200).collect::<String>());
    let pid = new_id();
    conn.execute("INSERT INTO tax_payments (id, plan_id, kind, date, amount_minor, note) VALUES (?, ?, ?, ?, ?, ?)",
        params![pid, id, kind, d.to_string(), amount, note])?;
    Ok(json!({ "id": pid }))
}

pub fn delete_payment(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM tax_payments WHERE id = ?", [id])? == 0 { return Err(bad("no such payment")); }
    Ok(())
}

/// The plan: figures, records, payments, the estimate for each regime, the regime used, what's
/// paid and what's left (or coming back), and the advance-tax instalments.
pub fn plan(conn: &Connection, country: &str, year_id: &str, today: NaiveDate) -> Result<Value> {
    let (p, y) = year(country, year_id)?;
    let cur = p.currency.clone();
    let id = plan_id(country, year_id);
    let (input, regime, hra) = stored(conn, &id)?.unwrap_or((json!({}), None, json!({})));
    let cmp = tax::compare(country, Some(year_id), &input)?;
    let used = regime.clone().or_else(|| cmp["recommended"].as_str().map(str::to_string)).unwrap_or_default();
    let total: i64 = cmp["estimates"].as_array().into_iter().flatten().find(|e| e["regime"] == used.as_str()).and_then(|e| e["total_minor"].as_i64()).unwrap_or(0);
    let payments = query(conn, "SELECT id, kind, date, amount_minor, note FROM tax_payments WHERE plan_id = ? ORDER BY date, created_at", &[json!(id)])?;
    let sum = |k: Option<&str>| -> i64 { payments.iter().filter(|x| k.is_none_or(|k| x["kind"] == k)).filter_map(|x| x["amount_minor"].as_i64()).sum() };
    let (tds, paid) = (sum(Some("tds")), sum(None));
    // Advance tax: what's due by each date, against what was paid by then.
    let mut schedule = vec![];
    if let Some(a) = &y.advance_tax {
        let after_tds = (total - tds).max(0);
        let applies = after_tds >= a.threshold * money::scale(&cur);
        let start = day(&y.start)?;
        for i in &a.instalments {
            let (m, d) = i.date.split_once('-').and_then(|(m, d)| Some((m.parse::<u32>().ok()?, d.parse::<u32>().ok()?))).unwrap_or((3, 15));
            let yr = if m >= start.month() { start.year() } else { start.year() + 1 };
            let Some(due) = NaiveDate::from_ymd_opt(yr, m, d) else { continue };
            let needed = if applies { (after_tds as f64 * i.percent / 100.0).round() as i64 } else { 0 };
            let by_then: i64 = payments.iter().filter(|x| x["kind"] != "tds" && x["date"].as_str().is_some_and(|dd| dd <= due.to_string().as_str()))
                .filter_map(|x| x["amount_minor"].as_i64()).sum();
            let short = (needed - by_then).max(0);
            schedule.push(json!({ "date": due.to_string(), "percent": i.percent, "needed": money::to_decimal(needed, &cur), "paid": money::to_decimal(by_then, &cur),
                "short": money::to_decimal(short, &cur), "short_minor": short, "past": due < today }));
        }
    }
    let d = |m: i64| money::to_decimal(m, &cur);
    Ok(json!({
        "id": id, "country": p.country, "currency": cur, "year": { "id": y.id, "label": y.label, "start": y.start, "end": y.end },
        "input": input, "regime": regime, "hra": hra, "hra_result": hra_view(&hra, &cur, &y)?,
        "records": records(conn, country, year_id)?, "links": links(conn)?,
        "payments": payments.into_iter().map(|mut x| { let a = x["amount_minor"].as_i64().unwrap_or(0); if let Some(o) = x.as_object_mut() { o.insert("amount".into(), json!(d(a))); } x }).collect::<Vec<_>>(),
        "comparison": cmp, "used": used, "total": d(total), "tds": d(tds), "paid": d(paid), "balance": d(total - paid),
        "advance": schedule, "advance_note": y.advance_tax.as_ref().and_then(|a| a.note.clone()),
    }))
}

/// Advance-tax instalments with something still to pay, for reminders and the calendar: the
/// year containing `today` (and the one before, until its last date passes), for each saved plan.
pub fn advance_due(conn: &Connection, today: NaiveDate) -> Result<Vec<(String, NaiveDate, i64, String)>> {
    let plans: Vec<(String, String)> = conn.prepare("SELECT country, year_id FROM tax_plans")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut out = vec![];
    for (country, year_id) in plans {
        let Ok(p) = plan(conn, &country, &year_id, today) else { continue };
        let cur = p["currency"].as_str().unwrap_or("INR").to_string();
        for i in p["advance"].as_array().into_iter().flatten() {
            let short = i["short_minor"].as_i64().unwrap_or(0);
            if short <= 0 { continue; }
            if let Ok(d) = day(i["date"].as_str().unwrap_or_default()) { out.push((p["year"]["label"].as_str().unwrap_or_default().to_string(), d, short, cur.clone())); }
        }
    }
    Ok(out)
}
