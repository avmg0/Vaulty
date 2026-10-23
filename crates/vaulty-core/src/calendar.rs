//! The calendar: bills and income from recurring payments, card due and statement dates, the
//! member's own events and holidays, and bills already paid, for any range up to about a year.
//! Also `.ics` export (to add Vaulty's dates to another calendar) and import (a holiday list, or
//! events from another calendar).
//!
//! Banks close on holidays (and in India on Sundays and the 2nd and 4th Saturday), so a bill or
//! card payment due on such a day is flagged with the last working day before it.
use crate::{error::{Error, Result}, money, recurring, rows::*};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

/// The longest range one request may cover.
pub const MAX_DAYS: i64 = 400;
/// Limits for imported calendars.
pub const MAX_ICS_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_ICS_EVENTS: usize = 5_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Item {
    /// bill, income, transfer, paid, card_due, statement, task, expiry, event, reminder, holiday
    pub kind: String,
    pub date: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// What it comes from: a recurring payment, card, event or transaction id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Due before today and not paid.
    pub overdue: bool,
    /// Banks are closed on this date; pay by `pay_by` instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pay_by: Option<String>,
}

impl Item {
    fn new(kind: &str, date: NaiveDate, title: impl Into<String>) -> Self {
        Item { kind: kind.into(), date: date.to_string(), title: title.into(), time: None, amount: None, currency: None, icon: None, source_id: None, overdue: false, pay_by: None }
    }
}

fn day(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| Error::Invalid(format!("`{s}` isn't a date (YYYY-MM-DD)")))
}

/// Holidays that fall on the same date every year. Festival dates move and bank holidays differ
/// by state, so only these are built in; members add or import the rest.
fn fixed_holidays(country: &str) -> &'static [(u32, u32, &'static str)] {
    match country {
        "IN" => &[(1, 26, "Republic Day"), (8, 15, "Independence Day"), (10, 2, "Gandhi Jayanti")],
        _ => &[],
    }
}

/// Whether banks are closed: weekends (in India, Sundays and the 2nd and 4th Saturdays) and holidays.
pub fn bank_closed(d: NaiveDate, country: &str, holidays: &[NaiveDate]) -> bool {
    let weekend = match (country, d.weekday()) {
        (_, Weekday::Sun) => true,
        ("IN", Weekday::Sat) => matches!((d.day() - 1) / 7, 1 | 3),
        (_, Weekday::Sat) => true,
        _ => false,
    };
    weekend || holidays.contains(&d)
}

/// The last day before `d` when banks are open.
pub fn last_working_day_before(d: NaiveDate, country: &str, holidays: &[NaiveDate]) -> NaiveDate {
    let mut p = d - Duration::days(1);
    while bank_closed(p, country, holidays) { p -= Duration::days(1); }
    p
}

/// Every month from `from` to `to` as (year, month).
fn months(from: NaiveDate, to: NaiveDate) -> Vec<(i32, u32)> {
    let mut out = vec![];
    let (mut y, mut m) = (from.year(), from.month());
    while (y, m) <= (to.year(), to.month()) {
        out.push((y, m));
        (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    }
    out
}

/// `day` of the month, or its last day if the month is shorter (a "31st" due date in February).
fn clamp_day(y: i32, m: u32, day: u32) -> Option<NaiveDate> {
    (1..=day.min(31)).rev().find_map(|d| NaiveDate::from_ymd_opt(y, m, d))
}

/// Everything on the calendar from `from` to `to` (inclusive), sorted by date.
pub fn range(conn: &Connection, from: &str, to: &str, country: &str, today: NaiveDate) -> Result<Vec<Item>> {
    let (from, to) = (day(from)?, day(to)?);
    if to < from { return Err(Error::Invalid("the end date is before the start".into())); }
    if (to - from).num_days() > MAX_DAYS { return Err(Error::Invalid("pick a range of at most a year".into())); }
    let mut out = vec![];

    // Holidays first: they decide which due dates move.
    for y in from.year()..=to.year() {
        for (m, d, name) in fixed_holidays(country) {
            if let Some(date) = NaiveDate::from_ymd_opt(y, *m, *d).filter(|d| (from..=to).contains(d)) {
                let mut it = Item::new("holiday", date, *name);
                it.source_id = Some(format!("fixed:{country}"));
                out.push(it);
            }
        }
    }
    for ev in events(conn)? {
        for date in expand(&ev, from, to)? {
            let kind = match ev["type"].as_str() { Some("holiday") => "holiday", Some("reminder") => "reminder", _ => "event" };
            let mut it = Item::new(kind, date, ev["title"].as_str().unwrap_or_default());
            it.time = ev["time"].as_str().map(str::to_string);
            it.amount = ev["amount"].as_f64();
            it.icon = ev["icon"].as_str().map(str::to_string);
            it.source_id = ev["id"].as_str().map(str::to_string);
            out.push(it);
        }
    }
    // Open to-dos with a due date.
    for t in crate::tasks::due_between(conn, &from.to_string(), &to.to_string())? {
        let mut it = Item::new("task", day(t["due_date"].as_str().unwrap_or_default())?, t["title"].as_str().unwrap_or_default());
        it.source_id = t["id"].as_str().map(str::to_string);
        it.overdue = it.date < today.to_string();
        out.push(it);
    }
    // Documents that expire (passports, policies…).
    for d in crate::documents::expiring_between(conn, &from.to_string(), &to.to_string())? {
        let mut it = Item::new("expiry", day(d["expires_on"].as_str().unwrap_or_default())?, d["title"].as_str().unwrap_or_default());
        it.source_id = d["id"].as_str().map(str::to_string);
        out.push(it);
    }
    // Advance tax instalments still to pay.
    for (label, date, short, cur) in crate::tax_plan::advance_due(conn, today)? {
        if date < from || date > to { continue; }
        let mut it = Item::new("tax", date, &label);
        it.amount = Some(crate::money::to_decimal(short, &cur));
        it.currency = Some(cur);
        it.overdue = date < today;
        out.push(it);
    }
    let holidays: Vec<NaiveDate> = out.iter().filter(|i| i.kind == "holiday").filter_map(|i| day(&i.date).ok()).collect();

    // Bills and income still to come (overdue ones start before today).
    for r in recurring::upcoming(conn, from.min(today), to)? {
        let date = day(r["date"].as_str().unwrap_or_default())?;
        if date < from { continue; }
        let kind = match r["type"].as_str() { Some("income") => "income", Some("transfer") => "transfer", _ => "bill" };
        let mut it = Item::new(kind, date, r["name"].as_str().unwrap_or_default());
        it.amount = r["amount"].as_f64();
        it.currency = r["currency"].as_str().map(str::to_string);
        it.icon = r["category_icon"].as_str().map(str::to_string);
        it.source_id = r["recurring_id"].as_str().map(str::to_string);
        it.overdue = date < today;
        out.push(it);
    }
    // Bills already paid in the range (hidden transactions are left out).
    let paid = query(conn,
        "SELECT t.id, t.date, MAX(t.amount) AS amount, t.currency, r.name, r.id AS rid FROM transactions t
           JOIN recurring_payments r ON r.id = t.recurring_id
          WHERE t.date BETWEEN ?1 AND ?2 AND COALESCE(t.hidden, 0) = 0 GROUP BY t.id", &[json!(from.to_string()), json!(to.to_string())])?;
    for p in paid {
        let mut it = Item::new("paid", day(p["date"].as_str().unwrap_or_default())?, p["name"].as_str().unwrap_or_default());
        it.amount = p["amount"].as_f64().map(f64::abs);
        it.currency = p["currency"].as_str().map(str::to_string);
        it.source_id = p["id"].as_str().map(str::to_string);
        out.push(it);
    }
    // Credit card statement and due dates, every month.
    let cards = query(conn, "SELECT c.id, c.name, c.statement_day, c.due_day, c.account_id, a.currency FROM cards c LEFT JOIN accounts a ON a.id = c.account_id
                              WHERE c.kind = 'credit'", &[])?;
    let next_due_of = |due_day: u32| months(today, today + Duration::days(62)).into_iter().filter_map(|(y, m)| clamp_day(y, m, due_day)).find(|d| *d >= today);
    for c in cards {
        let name = c["name"].as_str().unwrap_or_default().to_string();
        let owed = owed_minor(conn, c["account_id"].as_str())?;
        let statement_due = crate::card_cycles::next_due(conn, c["id"].as_str().unwrap_or_default(), today)?;
        let cur = c["currency"].as_str().unwrap_or("INR").to_string();
        for (y, m) in months(from, to) {
            if let Some(sd) = c["statement_day"].as_i64().and_then(|d| clamp_day(y, m, d as u32)).filter(|d| (from..=to).contains(d)) {
                let mut it = Item::new("statement", sd, name.clone());
                it.source_id = c["id"].as_str().map(str::to_string);
                out.push(it);
            }
            if let Some(due_day) = c["due_day"].as_i64() {
                if let Some(dd) = clamp_day(y, m, due_day as u32).filter(|d| (from..=to).contains(d)) {
                    let mut it = Item::new("card_due", dd, name.clone());
                    it.source_id = c["id"].as_str().map(str::to_string);
                    it.currency = Some(cur.clone());
                    // Only the next due date knows its amount: what the latest statement asks for
                    // (or, without statement days, what's owed now).
                    match &statement_due {
                        Some((d, left, _)) if *d == dd => it.amount = Some(money::to_decimal(*left, &cur)),
                        None if Some(dd) == next_due_of(due_day as u32) && owed > 0 => it.amount = Some(money::to_decimal(owed, &cur)),
                        _ => {}
                    }
                    out.push(it);
                }
            }
        }
    }
    for it in out.iter_mut().filter(|i| matches!(i.kind.as_str(), "bill" | "card_due" | "transfer")) {
        let d = day(&it.date)?;
        if bank_closed(d, country, &holidays) { it.pay_by = Some(last_working_day_before(d, country, &holidays).to_string()); }
    }
    let order = |k: &str| ["holiday", "card_due", "bill", "transfer", "income", "task", "expiry", "statement", "paid", "reminder", "event"].iter().position(|x| *x == k).unwrap_or(9);
    out.sort_by(|a, b| a.date.cmp(&b.date).then(order(&a.kind).cmp(&order(&b.kind))).then(a.title.cmp(&b.title)));
    Ok(out)
}

fn owed_minor(conn: &Connection, account_id: Option<&str>) -> Result<i64> {
    let Some(id) = account_id else { return Ok(0) };
    let bal = crate::accounts::get_all(conn)?.into_iter().find(|a| a["id"] == id)
        .and_then(|a| money::parse_minor(&a["current_balance"], a["currency"].as_str().unwrap_or("INR")).ok()).unwrap_or(0);
    Ok((-bal).max(0))
}

// ------------------------------------------------------------ the member's own events

const TYPES: &[&str] = &["custom", "holiday", "reminder"];
const REPEATS: &[&str] = &["monthly", "yearly"];

pub fn events(conn: &Connection) -> Result<Vec<Value>> {
    query(conn, "SELECT id, title, date, time, type, amount, icon, notes, recurring_pattern AS repeat, linked_type, linked_id FROM calendar_events ORDER BY date, time, title", &[])
}

/// The dates an event falls on between `from` and `to`.
fn expand(ev: &Value, from: NaiveDate, to: NaiveDate) -> Result<Vec<NaiveDate>> {
    let start = day(ev["date"].as_str().unwrap_or_default())?;
    Ok(match ev["repeat"].as_str() {
        Some("yearly") => (from.year()..=to.year())
            .filter_map(|y| clamp_day(y, start.month(), start.day()).filter(|_| y >= start.year()))
            .filter(|d| (from..=to).contains(d) && *d >= start).collect(),
        Some("monthly") => months(from, to).into_iter().filter_map(|(y, m)| clamp_day(y, m, start.day()))
            .filter(|d| (from..=to).contains(d) && *d >= start).collect(),
        _ => vec![start].into_iter().filter(|d| (from..=to).contains(d)).collect(),
    })
}

struct Fields { title: String, date: String, time: Option<String>, kind: String, amount: Option<f64>, repeat: Option<String>, icon: Option<String>, notes: Option<String> }

fn checked(v: &Value) -> Result<Fields> {
    let title = v["title"].as_str().map(str::trim).unwrap_or_default().to_string();
    if title.is_empty() || title.chars().count() > 80 { return Err(Error::Invalid("a name needs 1 to 80 characters".into())); }
    let date = day(v["date"].as_str().unwrap_or_default())?.to_string();
    let time = match v["time"].as_str().filter(|t| !t.is_empty()) {
        Some(t) => Some(chrono::NaiveTime::parse_from_str(t, "%H:%M").map_err(|_| Error::Invalid("times look like 14:30".into()))?.format("%H:%M").to_string()),
        None => None,
    };
    let kind = v["type"].as_str().unwrap_or("custom");
    if !TYPES.contains(&kind) { return Err(Error::Invalid(format!("unknown event type `{kind}`"))); }
    let amount = match v.get("amount").filter(|a| !a.is_null() && a.as_str() != Some("")) {
        Some(a) => Some(money::to_decimal(money::parse_minor(a, "INR")?, "INR")),
        None => None,
    };
    let repeat = v["repeat"].as_str().filter(|r| !r.is_empty() && *r != "none").map(str::to_string);
    if repeat.as_deref().is_some_and(|r| !REPEATS.contains(&r)) { return Err(Error::Invalid("repeat is none, monthly or yearly".into())); }
    let icon = v["icon"].as_str().filter(|i| !i.is_empty()).map(|i| i.chars().take(4).collect());
    let notes = v["notes"].as_str().filter(|n| !n.trim().is_empty()).map(str::to_string);
    Ok(Fields { title, date, time, kind: kind.to_string(), amount, repeat, icon, notes })
}

pub fn create_event(conn: &Connection, v: &Value) -> Result<Value> {
    let Fields { title, date, time, kind, amount, repeat, icon, notes } = checked(v)?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO calendar_events (id, title, date, time, type, amount, icon, notes, recurring_pattern, is_recurring) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, title, date, time, kind, amount, icon, notes, repeat, repeat.is_some()],
    )?;
    event(conn, &id)
}

pub fn update_event(conn: &Connection, id: &str, v: &Value) -> Result<Value> {
    event(conn, id)?;
    let Fields { title, date, time, kind, amount, repeat, icon, notes } = checked(v)?;
    conn.execute(
        "UPDATE calendar_events SET title = ?, date = ?, time = ?, type = ?, amount = ?, icon = ?, notes = ?, recurring_pattern = ?, is_recurring = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![title, date, time, kind, amount, icon, notes, repeat, repeat.is_some(), id],
    )?;
    event(conn, id)
}

pub fn delete_event(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM calendar_events WHERE id = ?", [id])? == 0 { return Err(Error::Invalid("no such event".into())); }
    Ok(())
}

fn event(conn: &Connection, id: &str) -> Result<Value> {
    query_one(conn, "SELECT id, title, date, time, type, amount, icon, notes, recurring_pattern AS repeat FROM calendar_events WHERE id = ?", &[json!(id)])?
        .ok_or_else(|| Error::Invalid("no such event".into()))
}

// ------------------------------------------------------------ .ics

/// RFC 5545 text escaping.
fn ics_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n").replace('\r', "")
}

/// Lines longer than 75 bytes continue on the next line after a space (never splitting a character).
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut len = 0;
    for c in line.chars() {
        if len + c.len_utf8() > 75 { out.push_str("\r\n "); len = 1; }
        out.push(c);
        len += c.len_utf8();
    }
    out
}

/// An `.ics` file of the given items. Amounts are left out unless asked for, since calendars are
/// often synced to online services.
pub fn to_ics(items: &[Item], include_amounts: bool) -> String {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut lines = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".into(), "PRODID:-//Vaulty//Calendar//EN".into(), "CALSCALE:GREGORIAN".into()];
    for it in items.iter().filter(|i| i.kind != "paid") {
        let date = it.date.replace('-', "");
        let mut summary = it.title.clone();
        if include_amounts { if let (Some(a), Some(c)) = (it.amount, &it.currency) { summary = format!("{summary} ({a:.2} {c})"); } }
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}-{}-{}@vaulty", it.kind, it.source_id.as_deref().unwrap_or("item"), date));
        lines.push(format!("DTSTAMP:{stamp}"));
        match it.time.as_deref() {
            Some(t) => lines.push(format!("DTSTART:{date}T{}00", t.replace(':', ""))),
            None => {
                lines.push(format!("DTSTART;VALUE=DATE:{date}"));
                if let Ok(d) = day(&it.date) { lines.push(format!("DTEND;VALUE=DATE:{}", (d + Duration::days(1)).format("%Y%m%d"))); }
            }
        }
        lines.push(format!("SUMMARY:{}", ics_text(&summary)));
        if let Some(p) = &it.pay_by { lines.push(format!("DESCRIPTION:{}", ics_text(&format!("Banks are closed that day; pay by {p}")))); }
        lines.push(format!("CATEGORIES:{}", it.kind));
        lines.push("END:VEVENT".into());
    }
    lines.push("END:VCALENDAR".into());
    lines.iter().map(|l| fold(l)).collect::<Vec<_>>().join("\r\n") + "\r\n"
}

/// An event read from an `.ics` file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Imported {
    pub uid: Option<String>,
    pub title: String,
    pub date: String,
    pub time: Option<String>,
    pub repeat: Option<String>,
    pub notes: Option<String>,
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() { Some('n') | Some('N') => out.push('\n'), Some(x) => out.push(x), None => {} }
        } else { out.push(c); }
    }
    out
}

/// Reads the events in an `.ics` file. Repeats are kept when they're plain monthly or yearly ones;
/// other rules import the first date only.
pub fn parse_ics(bytes: &[u8]) -> Result<Vec<Imported>> {
    if bytes.len() > MAX_ICS_BYTES { return Err(Error::Invalid("that calendar file is larger than 5 MB".into())); }
    let text = String::from_utf8_lossy(bytes);
    if !text.contains("BEGIN:VCALENDAR") { return Err(Error::Invalid("that isn't a calendar (.ics) file".into())); }
    // Unfold continuation lines.
    let mut lines: Vec<String> = vec![];
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() { lines.last_mut().unwrap().push_str(&raw[1..]); } else { lines.push(raw.to_string()); }
    }
    let mut out = vec![];
    let mut cur: Option<Vec<(String, String, String)>> = None;
    for l in lines {
        if l == "BEGIN:VEVENT" { cur = Some(vec![]); continue; }
        if l == "END:VEVENT" {
            if let Some(props) = cur.take() {
                if let Some(ev) = to_imported(&props) { out.push(ev); }
                if out.len() > MAX_ICS_EVENTS { return Err(Error::Invalid(format!("that calendar has more than {MAX_ICS_EVENTS} events"))); }
            }
            continue;
        }
        if let (Some(props), Some((name, value))) = (cur.as_mut(), l.split_once(':')) {
            let (key, params) = name.split_once(';').unwrap_or((name, ""));
            props.push((key.to_ascii_uppercase(), params.to_ascii_uppercase(), value.to_string()));
        }
    }
    Ok(out)
}

fn to_imported(props: &[(String, String, String)]) -> Option<Imported> {
    let get = |k: &str| props.iter().find(|(key, _, _)| key == k);
    let title: String = unescape(&get("SUMMARY")?.2).trim().chars().take(80).collect();
    let (_, params, start) = get("DTSTART")?;
    let digits: String = start.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 { return None; }
    let date = NaiveDate::parse_from_str(&digits[..8], "%Y%m%d").ok()?;
    if !(1900..=2200).contains(&date.year()) || title.is_empty() { return None; }
    // A time of day, as written (time zones aren't converted; all-day dates have none).
    let time = (!params.contains("VALUE=DATE") && digits.len() >= 12).then(|| format!("{}:{}", &digits[8..10], &digits[10..12]))
        .filter(|t| chrono::NaiveTime::parse_from_str(t, "%H:%M").is_ok());
    let repeat = get("RRULE").and_then(|(_, _, r)| {
        let r = r.to_ascii_uppercase();
        let plain = !r.contains("INTERVAL=") || r.contains("INTERVAL=1;") || r.ends_with("INTERVAL=1");
        if !plain || r.contains("BYDAY") || r.contains("BYSETPOS") { return None; }
        if r.contains("FREQ=YEARLY") { Some("yearly".to_string()) } else if r.contains("FREQ=MONTHLY") { Some("monthly".to_string()) } else { None }
    });
    let notes = get("DESCRIPTION").map(|(_, _, d)| unescape(d).chars().take(2000).collect::<String>()).filter(|n: &String| !n.trim().is_empty());
    Some(Imported { uid: get("UID").map(|(_, _, u)| u.chars().take(200).collect()), title, date: date.to_string(), time, repeat, notes })
}

/// Adds imported events (as holidays, or as ordinary events). Ones already imported (same UID, or
/// same name and date) are skipped. Returns how many were added.
pub fn import_events(conn: &Connection, events: &[Imported], as_holidays: bool) -> Result<usize> {
    let kind = if as_holidays { "holiday" } else { "custom" };
    let mut added = 0;
    for e in events {
        let dup: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE (linked_type = 'ics' AND linked_id = ?1 AND ?1 IS NOT NULL) OR (title = ?2 AND date = ?3))",
            params![e.uid, e.title, e.date], |r| r.get(0))?;
        if dup { continue; }
        conn.execute(
            "INSERT INTO calendar_events (id, title, date, time, type, notes, recurring_pattern, is_recurring, linked_type, linked_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ics', ?)",
            params![uuid::Uuid::new_v4().to_string(), e.title, e.date, e.time, kind, e.notes, e.repeat, e.repeat.is_some(), e.uid],
        )?;
        added += 1;
    }
    Ok(added)
}
