//! Property tests: random and hostile input for everything that reads files or free text, and
//! random sequences of real operations for the ledger and history.
//!
//! Each property runs a fixed number of cases so CI stays fast; set PROPTEST_CASES for a longer
//! local run (e.g. `PROPTEST_CASES=5000 cargo test -p vaulty-core --test fuzz`).
use chrono::NaiveDate;
use proptest::prelude::*;
use rusqlite::Connection;
use serde_json::{json, Value};
use vaulty_core::{accounts, backup, db, export, history, ledger, limits, money, quick, statements, tax, transactions};

fn cases(n: u32) -> ProptestConfig {
    let n = std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(n);
    ProptestConfig { cases: n, failure_persistence: None, ..ProptestConfig::default() }
}

/// Text that looks a bit like bank statement cells: digits, separators, currency signs, letters.
fn cell() -> impl Strategy<Value = String> {
    prop_oneof![
        "[0-9]{1,2}[/.-][0-9]{1,2}[/.-][0-9]{2,4}",
        "-?[₹$€]?[0-9,]{1,12}(\\.[0-9]{0,3})?( ?(Dr|Cr|DR|CR))?",
        "\\(?[0-9]{1,7}\\.[0-9]{2}\\)?",
        "(UPI|NEFT|POS|ATM|IMPS)/[A-Za-z0-9 @/._-]{0,40}",
        "[A-Za-z .&/-]{0,30}",
        any::<String>(),
    ]
}

/// A CSV made of plausible rows, sometimes with a recognisable header.
fn csv() -> impl Strategy<Value = Vec<u8>> {
    (any::<bool>(), prop::collection::vec(prop::collection::vec(cell(), 1..8), 0..40), prop::sample::select(vec![',', ';', '\t', '|'])).prop_map(|(header, rows, sep)| {
        let mut out = String::new();
        if header { out.push_str(&["Date", "Narration", "Withdrawal Amt.", "Deposit Amt.", "Balance"].join(&sep.to_string())); out.push('\n'); }
        for r in rows {
            let r: Vec<String> = r.into_iter().map(|c| if c.contains([sep, '"', '\n']) { format!("\"{}\"", c.replace('"', "\"\"")) } else { c }).collect();
            out.push_str(&r.join(&sep.to_string()));
            out.push('\n');
        }
        out.into_bytes()
    })
}

fn bank() -> (Connection, String) {
    let c = db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "100000" })).unwrap();
    (c, a["id"].as_str().unwrap().to_string())
}

// ------------------------------------------------------------ files and text from outside

thread_local! { static READ_ONLY: Connection = db::open_in_memory().unwrap(); }

proptest! {
    #![proptest_config(cases(300))]

    /// Any bytes at all, under any file name: an error is fine, a panic is not.
    #[test]
    fn statement_reader_survives_any_bytes(bytes in prop::collection::vec(any::<u8>(), 0..4096),
                                           name in prop::sample::select(vec!["s.csv", "s.txt", "s.xlsx", "s.xls", "s.ods", "s"])) {
        let _ = statements::read(&bytes, name);
    }

}

proptest! {
    // These open a database per case, so fewer of them.
    #![proptest_config(cases(80))]

    /// Plausible statements go all the way through: read, guess the columns, preview, import.
    /// Whatever gets imported must leave the books balanced.
    #[test]
    fn statements_import_without_unbalancing_the_books(file in csv()) {
        let Ok(table) = statements::read(&file, "s.csv") else { return Ok(()) };
        prop_assert!(table.rows.iter().all(|r| r.len() <= 60 && r.iter().all(|c| c.chars().count() <= 500)));
        let Ok(mapping) = statements::guess(&table, statements::DateOrder::Dmy) else { return Ok(()) };
        let (c, acc) = bank();
        match statements::preview(&c, &table, &mapping, &acc) {
            Ok(rows) => if let Ok(n) = statements::import(&c, &table, &mapping, &acc, &[], &[]) { prop_assert!(n <= rows.len()); },
            Err(_) => prop_assert!(statements::import(&c, &table, &mapping, &acc, &[], &[]).is_err()),
        }
        prop_assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
    }
}

proptest! {
    #![proptest_config(cases(300))]

    #[test]
    fn date_and_amount_parsers_survive_any_text(s in cell()) {
        let _ = statements::parse_date(&s, statements::DateOrder::Dmy);
        let _ = statements::parse_date(&s, statements::DateOrder::Mdy);
        let _ = statements::merchant_from(&s);
        if let Some((_, amount)) = statements::parse_amount(&s) {
            // Whatever it reads as an amount is a plain positive number; with at most two
            // decimals it's a valid rupee amount for the ledger.
            let (int, frac) = amount.split_once('.').unwrap_or((&amount, ""));
            prop_assert!(!int.is_empty() && int.chars().all(|c| c.is_ascii_digit()) && frac.chars().all(|c| c.is_ascii_digit()), "{s:?} -> {amount:?}");
            if frac.len() <= 2 { prop_assert!(money::parse_minor(&json!(amount), "INR").is_ok(), "{s:?} -> {amount:?}"); }
        }
    }

    /// Amounts written the ordinary way read back exactly.
    #[test]
    fn amounts_round_trip(minor in 0i64..1_000_000_000_000, cur in prop::sample::select(vec!["INR", "USD", "JPY", "KWD", "EUR"])) {
        let text = money::to_string(minor, cur);
        prop_assert_eq!(money::parse_minor(&json!(text), cur).unwrap(), minor);
    }

    #[test]
    fn money_parser_survives_any_value(s in any::<String>(), f in any::<f64>(), i in any::<i64>()) {
        for v in [json!(s), json!(f), json!(i), Value::Null, json!([s])] {
            if let Ok(m) = money::parse_minor(&v, "INR") { prop_assert!((0..=money::MAX_MINOR).contains(&m.abs())); }
        }
    }

    #[test]
    fn quick_add_survives_any_text(text in prop_oneof![any::<String>(), "[0-9kKlL., ]{0,20}[a-z ]{0,40}", "(spent|paid|got|from|गिफ्ट|कल|yesterday|on|for) [a-z0-9 ]{0,30}"]) {
        let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        // Parsing only reads, so one database serves every case.
        let parsed = READ_ONLY.with(|c| quick::parse(c, &text, today, &[]));
        if let Ok(p) = parsed {
            if let Some(a) = p.amount { prop_assert!(money::parse_minor(&json!(a), "INR").is_ok()); }
        }
    }

    #[test]
    fn backup_header_and_decrypt_reject_junk_without_panicking(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let _ = backup::header(&bytes);
        // Junk never gets as far as the (slow) key derivation, so this is cheap.
        prop_assert!(backup::decrypt(&bytes, "irrelevant password").is_err());
    }

    #[test]
    fn size_limits_never_panic(v in json_value()) {
        let _ = limits::check(&v);
    }

    #[test]
    fn tax_estimates_survive_any_input(v in json_value(), country in prop::sample::select(vec!["IN", "US", "GB", "XX", ""])) {
        if let Ok(out) = tax::compare(country, None, &json!({ "input": v.clone() })) { prop_assert!(out.is_object()); }
        let _ = tax::compare(country, Some("2026-27"), &v);
    }
}

/// Arbitrary JSON, a few levels deep.
fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![Just(Value::Null), any::<bool>().prop_map(Value::from), any::<i64>().prop_map(Value::from),
        any::<f64>().prop_filter("finite", |f| f.is_finite()).prop_map(Value::from), ".{0,40}".prop_map(Value::from)];
    leaf.prop_recursive(4, 64, 8, |inner| prop_oneof![
        prop::collection::vec(inner.clone(), 0..8).prop_map(Value::from),
        prop::collection::hash_map("[a-z_]{1,12}", inner, 0..8).prop_map(|m| Value::Object(m.into_iter().collect())),
    ])
}

// ------------------------------------------------------------ backups

proptest! {
    // Each case runs the full password key derivation, so fewer cases.
    #![proptest_config(cases(12))]

    /// Changing any single byte of a real backup makes it fail to open (never open to something else).
    #[test]
    fn a_changed_byte_never_opens(pos in any::<prop::sample::Index>(), flip in 1u8..=255) {
        let (file, pw) = &*BACKUP;
        let mut bad = file.clone();
        let i = pos.index(bad.len());
        bad[i] ^= flip;
        match backup::decrypt(&bad, pw) {
            Err(_) => {}
            // Only formatting outside the signed fields (like whitespace) may change harmlessly.
            Ok(v) => prop_assert_eq!(v, backup::decrypt(file, pw).unwrap(), "byte {} changed what the backup contains", i),
        }
    }
}

static BACKUP: std::sync::LazyLock<(Vec<u8>, String)> = std::sync::LazyLock::new(|| {
    let (c, acc) = bank();
    transactions::create(&c, &json!({ "account_id": acc, "type": "expense", "amount": "250", "date": "2026-10-10", "merchant": "Corner shop" })).unwrap();
    let pw = format!("fuzz-{}", uuid::Uuid::new_v4());
    (backup::encrypt(&c, &pw, Some("hint")).unwrap(), pw)
});

proptest! {
    #![proptest_config(cases(80))]

    /// A restore file someone edited by hand, or crafted: rows with random fields in every table.
    /// It may be refused, but it must never leave the books unbalanced or half-restored.
    #[test]
    fn crafted_restores_keep_the_books_balanced(tamper in prop::collection::vec((prop::sample::select(vec!["accounts", "categories", "transactions", "entries", "postings", "ledger_accounts", "budgets", "recurring"]), json_value()), 0..12)) {
        let (src, _) = bank();
        let mut data = export::to_json(&src).unwrap();
        for (table, row) in tamper {
            match data.get_mut(table).and_then(Value::as_array_mut) { Some(rows) => rows.push(row), None => { data[table] = json!([row]); } }
        }
        let (c, _) = bank();
        let before = export::to_json(&c).unwrap();
        match export::from_json(&c, &data) {
            Ok(_) => prop_assert!(ledger::unbalanced_entries(&c).unwrap().is_empty(), "a restore left unbalanced entries"),
            Err(_) => prop_assert_eq!(strip_time(export::to_json(&c).unwrap()), strip_time(before), "a failed restore changed data"),
        }
    }
}

fn strip_time(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() { o.remove("exported_at"); }
    v
}

// ------------------------------------------------------------ ledger and history

/// A live transaction in the model: id, kind, amount, from account, to account.
type Tx = (String, &'static str, i64, usize, Option<usize>);

#[derive(Debug, Clone)]
enum Op {
    Spend { amount: u32, account: usize },
    Earn { amount: u32, account: usize },
    Move { amount: u32, from: usize, to: usize },
    Edit { which: prop::sample::Index, amount: u32 },
    Delete { which: prop::sample::Index },
    Undo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (1u32..5_000_000, 0usize..3).prop_map(|(amount, account)| Op::Spend { amount, account }),
        2 => (1u32..5_000_000, 0usize..3).prop_map(|(amount, account)| Op::Earn { amount, account }),
        2 => (1u32..5_000_000, 0usize..3, 0usize..3).prop_map(|(amount, from, to)| Op::Move { amount, from, to }),
        2 => (any::<prop::sample::Index>(), 1u32..5_000_000).prop_map(|(which, amount)| Op::Edit { which, amount }),
        1 => any::<prop::sample::Index>().prop_map(|which| Op::Delete { which }),
        2 => Just(Op::Undo),
    ]
}

/// Every account's balance, in minor units.
fn balances(c: &Connection) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = accounts::get_all(c).unwrap().into_iter()
        .map(|a| (a["id"].as_str().unwrap().to_string(), money::parse_minor(&a["current_balance"], "INR").unwrap())).collect();
    out.sort();
    out
}

proptest! {
    #![proptest_config(cases(60))]

    /// Random sequences of real actions, each recorded in history like the app does. After every
    /// step: entries balance, account balances match what the actions add up to, and the history
    /// chain verifies. At the end, undoing everything returns to the starting balances.
    #[test]
    fn the_ledger_always_balances_and_undo_restores(ops in prop::collection::vec(op(), 1..30)) {
        let c = db::open_in_memory().unwrap();
        let ids: Vec<String> = ["Bank", "Cash", "Card"].iter().enumerate().map(|(i, n)| {
            let a = history::record(&c, "account.create", |c| accounts::create(c, &json!({ "name": n, "type": if i == 1 { "cash" } else { "checking" }, "currency": "INR", "initial_balance": "1000" }))).unwrap();
            a["id"].as_str().unwrap().to_string()
        }).collect();
        let start = balances(&c);
        let mut expected: std::collections::HashMap<String, i64> = start.iter().cloned().collect();
        // Each live transaction: (id, kind, amount, from, to). Undo pops what the last action did.
        let mut live: Vec<Tx> = Vec::new();
        let mut done: Vec<Vec<Tx>> = Vec::new();
        let apply = |exp: &mut std::collections::HashMap<String, i64>, t: &Tx, sign: i64| {
            let (_, kind, amt, from, to) = t;
            match *kind {
                "expense" => *exp.get_mut(&ids[*from]).unwrap() -= sign * amt,
                "income" => *exp.get_mut(&ids[*from]).unwrap() += sign * amt,
                _ => { *exp.get_mut(&ids[*from]).unwrap() -= sign * amt; *exp.get_mut(&ids[to.unwrap()]).unwrap() += sign * amt; }
            }
        };
        let rupees = |minor: i64| format!("{}.{:02}", minor / 100, minor % 100);

        for op in ops {
            match op {
                Op::Spend { amount, account } | Op::Earn { amount, account } => {
                    let kind = if matches!(op, Op::Spend { .. }) { "expense" } else { "income" };
                    let t = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": ids[account], "type": kind, "amount": rupees(amount as i64), "date": "2026-10-10" }))).unwrap();
                    let row = (t["id"].as_str().unwrap().to_string(), kind, amount as i64, account, None);
                    apply(&mut expected, &row, 1);
                    live.push(row.clone());
                    done.push(snapshot(&live));
                }
                Op::Move { amount, from, to } => {
                    let r = history::record(&c, "transaction.create", |c| transactions::create(c, &json!({ "account_id": ids[from], "to_account_id": ids[to], "type": "transfer", "amount": rupees(amount as i64), "date": "2026-10-10" })));
                    if from == to { prop_assert!(r.is_err(), "a transfer to the same account was accepted"); continue; }
                    let t = r.unwrap();
                    let row = (t["id"].as_str().unwrap().to_string(), "transfer", amount as i64, from, Some(to));
                    apply(&mut expected, &row, 1);
                    live.push(row);
                    done.push(snapshot(&live));
                }
                Op::Edit { which, amount } => {
                    if live.is_empty() { continue; }
                    let i = which.index(live.len());
                    let old = live[i].clone();
                    let mut body = json!({ "account_id": ids[old.3], "type": old.1, "amount": rupees(amount as i64), "date": "2026-10-11" });
                    if let Some(to) = old.4 { body["to_account_id"] = json!(ids[to]); }
                    history::record(&c, "transaction.update", |c| transactions::update(c, &old.0, &body)).unwrap();
                    apply(&mut expected, &old, -1);
                    live[i].2 = amount as i64;
                    apply(&mut expected, &live[i], 1);
                    done.push(snapshot(&live));
                }
                Op::Delete { which } => {
                    if live.is_empty() { continue; }
                    let old = live.remove(which.index(live.len()));
                    history::record(&c, "transaction.delete", |c| transactions::delete(c, &old.0)).unwrap();
                    apply(&mut expected, &old, -1);
                    done.push(snapshot(&live));
                }
                Op::Undo => {
                    if history::undo_last(&c).unwrap().is_none() { continue; }
                    // Undo can reach back past this test's transactions to the account setup; stop there.
                    if done.is_empty() { prop_assert!(accounts::get_all(&c).unwrap().len() < 3); return Ok(()); }
                    done.pop();
                    live = done.last().cloned().unwrap_or_default();
                    expected = start.iter().cloned().collect();
                    for t in &live { apply(&mut expected, t, 1); }
                }
            }
            prop_assert!(ledger::unbalanced_entries(&c).unwrap().is_empty(), "unbalanced after {:?}", op);
            let mut want: Vec<(String, i64)> = expected.clone().into_iter().collect();
            want.sort();
            prop_assert_eq!(balances(&c), want, "balances wrong after {:?}", op);
            let v = history::verify(&c).unwrap();
            prop_assert!(v.broken_at.is_none() && v.changed_outside.is_empty(), "history check failed after {:?}: {:?}", op, v.reason);
        }

        // Undo everything this test did; the accounts end where they started.
        for _ in 0..done.len() { history::undo_last(&c).unwrap(); }
        prop_assert_eq!(balances(&c), start);
        prop_assert!(ledger::unbalanced_entries(&c).unwrap().is_empty());
    }
}

fn snapshot<T: Clone>(live: &[T]) -> Vec<T> { live.to_vec() }
