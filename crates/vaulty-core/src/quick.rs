//! Quick add: one line of plain language becomes a transaction.
//!
//! "250 swiggy yesterday", "+85000 salary on 1st", "petrol 2k from card", "uber 412.50 cash",
//! "कल 300 सब्ज़ी". The parser finds an amount, a date, an account and what's left over (the
//! merchant), then picks a category from the member's rules or a few common words. It never
//! guesses silently: the app shows what it understood before saving.

use crate::{error::Result, rules};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Parsed {
    pub kind: &'static str,
    /// Decimal text, e.g. "2000" or "412.50".
    pub amount: Option<String>,
    pub date: String,
    pub merchant: String,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    /// Which words were read as what, for showing back to the member.
    pub understood: Vec<(String, &'static str)>,
}

const FILLER: &[&str] = &["on", "for", "at", "from", "via", "paid", "pay", "spent", "spend", "rs", "rs.", "inr", "₹", "to", "with", "using", "the", "a", "an", "in", "by", "for", "पर", "से", "को", "का", "की", "के", "में"];
const INCOME_WORDS: &[&str] = &["received", "got", "income", "salary", "refund", "cashback", "earned", "interest", "dividend", "मिला", "मिले", "वेतन", "तनख़्वाह"];

/// Common words → built-in categories, for when no rule matches yet.
const KEYWORDS: &[(&str, &str)] = &[
    ("food", "cat_food"), ("lunch", "cat_food"), ("dinner", "cat_food"), ("breakfast", "cat_food"), ("coffee", "cat_food"), ("tea", "cat_food"),
    ("swiggy", "cat_food"), ("zomato", "cat_food"), ("groceries", "cat_food"), ("grocery", "cat_food"), ("vegetables", "cat_food"), ("खाना", "cat_food"), ("सब्ज़ी", "cat_food"), ("सब्जी", "cat_food"),
    ("petrol", "cat_transport"), ("diesel", "cat_transport"), ("fuel", "cat_transport"), ("uber", "cat_transport"), ("ola", "cat_transport"), ("auto", "cat_transport"),
    ("metro", "cat_transport"), ("cab", "cat_transport"), ("bus", "cat_transport"), ("taxi", "cat_transport"), ("parking", "cat_transport"), ("पेट्रोल", "cat_transport"),
    ("rent", "cat_home"), ("किराया", "cat_home"), ("electricity", "cat_bills"), ("bill", "cat_bills"), ("recharge", "cat_bills"), ("broadband", "cat_bills"), ("wifi", "cat_bills"), ("बिजली", "cat_bills"),
    ("medicine", "cat_health"), ("pharmacy", "cat_health"), ("doctor", "cat_health"), ("gym", "cat_health"), ("दवा", "cat_health"),
    ("movie", "cat_entertainment"), ("netflix", "cat_entertainment"), ("shopping", "cat_shopping"), ("clothes", "cat_shopping"), ("amazon", "cat_shopping"),
    ("school", "cat_education"), ("fees", "cat_education"), ("books", "cat_education"), ("flight", "cat_travel"), ("hotel", "cat_travel"), ("train", "cat_travel"),
    ("salary", "cat_salary"), ("वेतन", "cat_salary"), ("freelance", "cat_freelance"), ("gift", "cat_gift"), ("refund", "cat_refund"), ("interest", "cat_investment"), ("dividend", "cat_investment"),
];

/// "250", "1,250.50", "₹250", "rs250", "2k", "1.5k", "2 lakh", "1l".
fn amount_of(word: &str, next: Option<&str>) -> Option<(String, bool)> {
    let w = word.to_lowercase();
    let w = w.trim_start_matches(['₹', '+']).trim_start_matches("rs.").trim_start_matches("rs").trim_start_matches("inr").trim_end_matches(['/', '-']);
    let (num, mult) = if let Some(n) = w.strip_suffix('k') { (n, 1_000.0) }
        else if let Some(n) = w.strip_suffix("lakh").or_else(|| w.strip_suffix('l')) { (n, 100_000.0) }
        else { (w, 1.0) };
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') || !num.chars().any(|c| c.is_ascii_digit()) { return None; }
    let v: f64 = num.replace(',', "").parse().ok()?;
    let lakh_next = mult == 1.0 && matches!(next.map(str::to_lowercase).as_deref(), Some("lakh" | "lac" | "लाख"));
    let k_next = mult == 1.0 && matches!(next.map(str::to_lowercase).as_deref(), Some("k" | "thousand" | "हज़ार" | "हजार"));
    let total = v * if lakh_next { 100_000.0 } else if k_next { 1_000.0 } else { mult };
    if total <= 0.0 || total > 1e12 { return None; }
    let text = if total.fract() == 0.0 { format!("{total:.0}") } else { format!("{total:.2}") };
    Some((text, lakh_next || k_next))
}

fn weekday_of(w: &str) -> Option<Weekday> {
    Some(match w {
        "monday" | "mon" | "सोमवार" => Weekday::Mon, "tuesday" | "tue" | "मंगलवार" => Weekday::Tue, "wednesday" | "wed" | "बुधवार" => Weekday::Wed,
        "thursday" | "thu" | "गुरुवार" => Weekday::Thu, "friday" | "fri" | "शुक्रवार" => Weekday::Fri, "saturday" | "sat" | "शनिवार" => Weekday::Sat,
        "sunday" | "sun" | "रविवार" => Weekday::Sun, _ => return None,
    })
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// "1st", "5th", "21" (with "on" before it) → that day this month, or last month if it's still ahead.
fn day_of_month(w: &str, today: NaiveDate) -> Option<NaiveDate> {
    let n: u32 = w.trim_end_matches(|c: char| c.is_alphabetic()).parse().ok()?;
    if !(1..=31).contains(&n) { return None; }
    let this = today.with_day(n);
    match this { Some(d) if d <= today => Some(d), _ => {
        let prev = today.with_day(1)? - Duration::days(1);
        prev.with_day(n.min(prev.day()))
    } }
}

pub fn parse(conn: &Connection, text: &str, today: NaiveDate, accounts: &[Value]) -> Result<Parsed> {
    let raw: Vec<&str> = text.split_whitespace().collect();
    let lower: Vec<String> = raw.iter().map(|w| w.to_lowercase().trim_matches(',').to_string()).collect();
    let mut used = vec![false; raw.len()];
    let mut understood = Vec::new();
    let mut kind = "expense";
    let mut amount = None;
    let mut date = today;
    let mut account_id = None;

    if text.trim_start().starts_with('+') { kind = "income"; }
    // Amount: the first word that is a number.
    for i in 0..raw.len() {
        if let Some((a, ate_next)) = amount_of(&lower[i], lower.get(i + 1).map(String::as_str)) {
            amount = Some(a);
            used[i] = true;
            if ate_next { used[i + 1] = true; }
            understood.push((raw[i].to_string(), "amount"));
            break;
        }
    }
    // Dates.
    for i in 0..raw.len() {
        if used[i] { continue; }
        let w = lower[i].as_str();
        let d = match w {
            "today" | "आज" => Some(today),
            "yesterday" | "kal" | "कल" => Some(today - Duration::days(1)),
            "parso" | "परसों" => Some(today - Duration::days(2)),
            _ => None,
        };
        if let Some(d) = d { date = d; used[i] = true; understood.push((raw[i].to_string(), "date")); continue; }
        if w == "day" && lower.get(i + 1).map(String::as_str) == Some("before") && lower.get(i + 2).map(String::as_str) == Some("yesterday") {
            date = today - Duration::days(2); used[i] = true; used[i + 1] = true; used[i + 2] = true; understood.push(("day before yesterday".into(), "date")); continue;
        }
        if let Some(wd) = weekday_of(w) {
            let back = (7 + today.weekday().num_days_from_monday() as i64 - wd.num_days_from_monday() as i64) % 7;
            let back = if back == 0 || (i > 0 && lower[i - 1] == "last") { if back == 0 { 7 } else { back } } else { back };
            date = today - Duration::days(back);
            used[i] = true; if i > 0 && lower[i - 1] == "last" { used[i - 1] = true; }
            understood.push((raw[i].to_string(), "date")); continue;
        }
        // "5 oct", "oct 5"
        if let Some(m) = MONTHS.iter().position(|m| w.starts_with(m) && w.len() <= 9) {
            let num = |j: usize| lower.get(j).and_then(|x| x.trim_end_matches(|c: char| c.is_alphabetic()).parse::<u32>().ok()).filter(|n| (1..=31).contains(n));
            let (day, j) = match (i.checked_sub(1).filter(|j| !used[*j]).and_then(|j| num(j).map(|n| (n, j))), num(i + 1).map(|n| (n, i + 1))) {
                (Some(x), _) => x, (None, Some(x)) => x, _ => continue,
            };
            let mut y = today.year();
            if NaiveDate::from_ymd_opt(y, m as u32 + 1, day).is_some_and(|d| d > today) { y -= 1; }
            if let Some(d) = NaiveDate::from_ymd_opt(y, m as u32 + 1, day) {
                date = d; used[i] = true; used[j] = true; understood.push((format!("{} {}", raw[j.min(i)], raw[j.max(i)]), "date"));
            }
            continue;
        }
        // "on 5th"
        if i > 0 && lower[i - 1] == "on" && w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if let Some(d) = day_of_month(w, today) { date = d; used[i] = true; understood.push((raw[i].to_string(), "date")); }
        }
    }
    // Account: a word that starts an account's name ("cash", "hdfc", "card" for a credit card).
    for i in 0..raw.len() {
        if used[i] || lower[i].chars().count() < 3 { continue; }
        let w = &lower[i];
        let hit = accounts.iter().find(|a| a["name"].as_str().is_some_and(|n| n.to_lowercase().split_whitespace().any(|p| p.starts_with(w.as_str()))))
            .or_else(|| (w == "card").then(|| accounts.iter().find(|a| a["type"] == "credit")).flatten());
        if let Some(a) = hit {
            account_id = a["id"].as_str().map(String::from);
            used[i] = true;
            understood.push((raw[i].to_string(), "account"));
            break;
        }
    }
    // Income words.
    for i in 0..raw.len() { if !used[i] && INCOME_WORDS.contains(&lower[i].as_str()) { kind = "income"; } }
    // What's left is the merchant (filler words dropped).
    let merchant_words: Vec<&str> = (0..raw.len()).filter(|i| !used[*i] && !FILLER.contains(&lower[*i].as_str()) && lower[*i] != "last")
        .map(|i| raw[i].trim_matches(',')).filter(|w| !w.is_empty() && *w != "+").collect();
    let merchant = merchant_words.join(" ");
    // "gift from mom" is money in; "gift for mom" is a purchase. Words that are only ever income
    // (salary, refund…) already set the type above.
    if kind == "expense" {
        let from_person = (0..raw.len()).any(|i| lower[i] == "from" && lower.get(i + 1).is_some_and(|n| !used.get(i + 1).copied().unwrap_or(true)
            && !accounts.iter().any(|a| a["name"].as_str().is_some_and(|x| x.to_lowercase().starts_with(n.as_str())))));
        if from_person && merchant_words.iter().any(|w| ["gift", "gifts", "उपहार", "तोहफ़ा"].contains(&w.to_lowercase().as_str())) { kind = "income"; }
    }
    let mut category_id = if merchant.is_empty() { None } else { rules::suggest(conn, &merchant, kind)? };
    if category_id.is_none() {
        let want_income = kind == "income";
        category_id = merchant_words.iter().find_map(|w| {
            let w = w.to_lowercase();
            KEYWORDS.iter().find(|(k, _)| *k == w).map(|(_, c)| c.to_string())
        }).filter(|c| ["cat_salary", "cat_freelance", "cat_gift", "cat_refund", "cat_investment"].contains(&c.as_str()) == want_income);
    }
    if !merchant.is_empty() { understood.push((merchant.clone(), "merchant")); }
    Ok(Parsed { kind, amount, date: date.to_string(), merchant: capitalise(&merchant), account_id, category_id, understood })
}

/// "swiggy" → "Swiggy"; leaves "BESCOM" and mixed case alone.
fn capitalise(s: &str) -> String {
    if s.chars().any(|c| c.is_uppercase()) { return s.to_string(); }
    s.split(' ').map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default() }).collect::<Vec<_>>().join(" ")
}
