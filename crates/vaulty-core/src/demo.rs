//! Sample data for demo mode: six months of believable finances for a fictional household
//! (John Doe, Jane Doe and Jenny Doe), so screenshots, testing and new contributors never need
//! real data. The numbers come from a fixed seed, so every run produces the same history.
//!
//! Merchants are generic descriptions rather than real businesses, and no real bank is named.

use crate::error::Result;
use crate::{cards, history, transactions};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::Connection;
use serde_json::{json, Value};

/// Which family member the sample data is for; each gets a different, fitting picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persona {
    /// Salaried adult with rent, bills, a credit card and a couple of hidden purchases.
    Earner,
    /// Freelancer with irregular income.
    Freelancer,
    /// A child with pocket money and small spends.
    Child,
}

/// Small deterministic generator (xorshift), so the sample history is identical on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Whole number in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

struct Seeder<'c> {
    conn: &'c Connection,
    today: NaiveDate,
}

impl Seeder<'_> {
    fn account(&self, name: &str, kind: &str, opening: i64) -> Result<String> {
        let a = crate::accounts::create(self.conn, &json!({ "name": name, "type": kind, "currency": "INR", "initial_balance": opening.to_string() }))?;
        Ok(a["id"].as_str().unwrap_or_default().to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn tx(&self, account: &str, kind: &str, amount: i64, date: NaiveDate, merchant: &str, category: Option<&str>, extra: Value) -> Result<()> {
        if date > self.today {
            return Ok(());
        }
        let mut t = json!({ "account_id": account, "type": kind, "amount": amount.to_string(), "date": date.to_string(),
                            "merchant": merchant, "category_id": category });
        if let (Some(o), Some(e)) = (t.as_object_mut(), extra.as_object()) {
            o.extend(e.clone());
        }
        transactions::create(self.conn, &t)?;
        Ok(())
    }
}

/// First day of the month `back` months before `d`'s month.
fn month_start(d: NaiveDate, back: u32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 - back as i32;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).expect("valid month")
}

fn day(m: NaiveDate, d: u32) -> NaiveDate {
    let last = (month_start(m, 0) + Duration::days(40)).with_day(1).expect("valid") - Duration::days(1);
    m.with_day(d.min(last.day())).expect("valid day")
}

/// Fills an empty profile with sample accounts, a card and six months of transactions, recorded
/// as one undoable action ("Sample data added").
pub fn seed(conn: &Connection, persona: Persona, today: NaiveDate) -> Result<()> {
    history::record(conn, "demo.seed", |conn| {
        let s = Seeder { conn, today };
        match persona {
            Persona::Earner => earner(&s),
            Persona::Freelancer => freelancer(&s),
            Persona::Child => child(&s),
        }
    })
}

fn earner(s: &Seeder) -> Result<()> {
    let mut r = Rng(0x5eed_0001);
    let bank = s.account("Salary account", "savings", 85_000)?;
    let cash = s.account("Cash wallet", "cash", 3_000)?;
    let card = cards::create(s.conn, &json!({ "kind": "credit", "name": "Rewards credit card", "owed": "0", "network": "visa", "last4": "4242",
        "expiry_month": 8, "expiry_year": s.today.year() + 3, "credit_limit": "200000", "statement_day": 2, "due_day": 20,
        "look": { "preset": "midnight" } }))?;
    let card = card["account_id"].as_str().unwrap_or_default().to_string();
    let public = json!({ "visibility": "public" });
    for back in (0..6).rev() {
        let m = month_start(s.today, back);
        s.tx(&bank, "income", 85_000, day(m, 1), "Monthly salary", Some("cat_salary"), json!({}))?;
        s.tx(&bank, "expense", 22_000, day(m, 5), "House rent", Some("cat_home"), public.clone())?;
        s.tx(&bank, "expense", r.range(1_400, 2_600), day(m, 8), "Electricity bill", Some("cat_bills"), public.clone())?;
        s.tx(&bank, "expense", 799, day(m, 10), "Broadband", Some("cat_bills"), json!({}))?;
        s.tx(&bank, "expense", 5_000, day(m, 12), "Mutual fund SIP", None, json!({}))?;
        s.tx(&card, "expense", 649, day(m, 14), "Music and video streaming", Some("cat_entertainment"), json!({}))?;
        for week in 0..4 {
            let d = day(m, 3 + week * 7);
            s.tx(&card, "expense", r.range(1_800, 3_400), d, r.pick(&["Neighbourhood supermarket", "Weekly vegetables", "Grocery delivery"]), Some("cat_food"), public.clone())?;
            s.tx(&card, "expense", r.range(250, 900), d + Duration::days(2), r.pick(&["Food delivery", "Cafe", "Lunch with colleagues"]), Some("cat_food"), json!({}))?;
            s.tx(&cash, "expense", r.range(80, 450), d + Duration::days(1), r.pick(&["Auto rickshaw", "Metro card top-up", "Cab ride"]), Some("cat_transport"), json!({}))?;
        }
        s.tx(&card, "expense", r.range(1_200, 4_500), day(m, 18), r.pick(&["Clothing store", "Online shopping", "Electronics accessories"]), Some("cat_shopping"), json!({}))?;
        s.tx(&bank, "transfer", 2_000, day(m, 2), "Cash withdrawal", None, json!({ "to_account_id": cash }))?;
        if back % 2 == 0 {
            s.tx(&card, "expense", r.range(600, 1_500), day(m, 22), "Pharmacy", Some("cat_health"), json!({}))?;
        }
    }
    // Paid off the card last month.
    let last = month_start(s.today, 1);
    s.tx(&bank, "transfer", 18_000, day(last, 20), "Credit card payment", None, json!({ "to_account_id": card }))?;
    // Two purchases kept in the Hidden section.
    let this = month_start(s.today, 0);
    s.tx(&card, "expense", 12_500, day(this, 1).max(s.today - Duration::days(6)), "Anniversary gift", Some("cat_shopping"), json!({ "hidden": true }))?;
    s.tx(&bank, "expense", 2_500, day(month_start(s.today, 1), 16), "Counselling session", Some("cat_health"), json!({ "hidden": true }))?;
    Ok(())
}

fn freelancer(s: &Seeder) -> Result<()> {
    let mut r = Rng(0x5eed_0002);
    let bank = s.account("Everyday account", "savings", 140_000)?;
    for back in (0..6).rev() {
        let m = month_start(s.today, back);
        for _ in 0..r.range(1, 3) {
            s.tx(&bank, "income", r.range(18, 60) * 1_000, day(m, r.range(3, 26) as u32), r.pick(&["Design project", "Website build", "Consulting"]), Some("cat_freelance"), json!({}))?;
        }
        s.tx(&bank, "expense", r.range(3_000, 6_500), day(m, 9), "Groceries", Some("cat_food"), json!({ "visibility": "public" }))?;
        s.tx(&bank, "expense", 1_299, day(m, 11), "Design software subscription", Some("cat_education"), json!({}))?;
        s.tx(&bank, "expense", r.range(700, 2_200), day(m, 19), r.pick(&["Yoga class", "Gym membership", "Sports shoes"]), Some("cat_health"), json!({}))?;
        s.tx(&bank, "expense", r.range(400, 1_800), day(m, 24), r.pick(&["Bookshop", "Weekend movie", "Dinner out"]), Some("cat_entertainment"), json!({}))?;
    }
    Ok(())
}

fn child(s: &Seeder) -> Result<()> {
    let mut r = Rng(0x5eed_0003);
    let piggy = s.account("Pocket money", "cash", 500)?;
    for back in (0..6).rev() {
        let m = month_start(s.today, back);
        s.tx(&piggy, "income", 1_000, day(m, 1), "Pocket money from parents", Some("cat_gift"), json!({}))?;
        s.tx(&piggy, "expense", r.range(100, 350), day(m, 9), r.pick(&["Comic books", "Art supplies", "Stickers"]), Some("cat_shopping"), json!({}))?;
        s.tx(&piggy, "expense", r.range(60, 200), day(m, 17), r.pick(&["Ice cream", "School canteen", "Snacks"]), Some("cat_food"), json!({}))?;
    }
    Ok(())
}
