//! Double-entry ledger and integer money.
use serde_json::{json, Value};
use vaulty_core::{accounts, categories, db, export, household, ledger, money, rows, transactions};

fn conn() -> rusqlite::Connection {
    db::open_in_memory().unwrap()
}

fn balance(c: &rusqlite::Connection, account_id: &str) -> Value {
    accounts::get(c, account_id).unwrap()["current_balance"].clone()
}

/// The ledger's core invariant: every entry balances and every currency nets to zero.
fn assert_books_balance(c: &rusqlite::Connection) {
    assert!(ledger::unbalanced_entries(c).unwrap().is_empty(), "unbalanced entries");
    let mut per_currency = std::collections::BTreeMap::<String, i64>::new();
    for line in ledger::trial_balance(c, ledger::PERSONAL_WORKSPACE).unwrap() {
        *per_currency.entry(line.currency).or_default() += line.total_minor;
    }
    assert!(per_currency.values().all(|v| *v == 0), "trial balance not zero: {per_currency:?}");
}

#[test]
fn currency_table_matches_rust() {
    let c = conn();
    for row in rows::query(&c, "SELECT code, exponent FROM currencies", &[]).unwrap() {
        let code = row["code"].as_str().unwrap();
        assert_eq!(money::exponent(code) as i64, row["exponent"].as_i64().unwrap(), "{code}");
    }
    for code in money::ZERO_DECIMAL.iter().chain(money::THREE_DECIMAL) {
        let n: i64 = c.query_row("SELECT COUNT(*) FROM currencies WHERE code = ?", [code], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "{code} missing from currencies table");
    }
}

#[test]
fn amounts_are_exact() {
    let c = conn();
    let acc = accounts::create(&c, &json!({"name": "Wallet", "type": "cash", "currency": "INR"})).unwrap();
    let id = acc["id"].as_str().unwrap();
    for amt in [0.1, 0.2, 0.7] {
        transactions::create(&c, &json!({"account_id": id, "type": "income", "amount": amt, "date": "2026-10-01"})).unwrap();
    }
    assert_eq!(accounts::get(&c, id).unwrap()["current_balance_minor"], json!(100)); // ₹1.00 exactly
    assert_eq!(balance(&c, id), json!(1.0));
    assert!(transactions::create(&c, &json!({"account_id": id, "type": "expense", "amount": "1.005", "date": "2026-10-01"})).is_err());
    assert!(transactions::create(&c, &json!({"account_id": id, "type": "expense", "amount": 0, "date": "2026-10-01"})).is_err());
    assert_books_balance(&c);
}

#[test]
fn credit_cards_are_liabilities_and_transfers_move_money() {
    let c = conn();
    let bank = accounts::create(&c, &json!({"name": "Savings", "type": "checking", "currency": "INR", "initial_balance": "50000"})).unwrap();
    let card = accounts::create(&c, &json!({"name": "Card", "type": "credit", "currency": "INR", "initial_balance": 5000})).unwrap();
    let (bank, card) = (bank["id"].as_str().unwrap(), card["id"].as_str().unwrap());

    // Opening "amount owed" shows as a negative contribution to net worth.
    assert_eq!(balance(&c, card), json!(-5000.0));
    assert_eq!(accounts::get(&c, card).unwrap()["initial_balance"], json!(5000.0));

    transactions::create(&c, &json!({"account_id": card, "type": "expense", "amount": 1000, "date": "2026-10-02", "category_id": "cat_food"})).unwrap();
    assert_eq!(balance(&c, card), json!(-6000.0));

    // Paying the card from savings is a transfer, not spending.
    let pay = transactions::create(&c, &json!({"account_id": bank, "to_account_id": card, "type": "transfer", "amount": 2000, "date": "2026-10-05"})).unwrap();
    assert_eq!(pay["to_account_name"], json!("Card"));
    assert_eq!(balance(&c, card), json!(-4000.0));
    assert_eq!(balance(&c, bank), json!(48000.0));
    // The transfer appears in both accounts' lists.
    assert_eq!(transactions::get_all(&c, Some(&json!({"accountId": card}))).unwrap().len(), 2);
    assert!(transactions::create(&c, &json!({"account_id": bank, "to_account_id": bank, "type": "transfer", "amount": 1, "date": "2026-10-05"})).is_err());
    assert_books_balance(&c);
}

#[test]
fn editing_moves_postings_and_opening_balance_can_change() {
    let c = conn();
    let a = accounts::create(&c, &json!({"name": "A", "type": "cash", "currency": "USD", "initial_balance": 100})).unwrap();
    let b = accounts::create(&c, &json!({"name": "B", "type": "cash", "currency": "USD"})).unwrap();
    let (a, b) = (a["id"].as_str().unwrap(), b["id"].as_str().unwrap());
    let t = transactions::create(&c, &json!({"account_id": a, "type": "expense", "amount": 30, "date": "2026-10-01"})).unwrap();
    let tid = t["id"].as_str().unwrap();
    assert_eq!(balance(&c, a), json!(70.0));

    transactions::update(&c, tid, &json!({"account_id": b, "type": "expense", "amount": 12.5, "date": "2026-10-01"})).unwrap();
    assert_eq!(balance(&c, a), json!(100.0));
    assert_eq!(balance(&c, b), json!(-12.5));

    accounts::update(&c, a, &json!({"name": "A", "type": "cash", "initial_balance": 250})).unwrap();
    assert_eq!(balance(&c, a), json!(250.0));
    // Currency can't change once there are postings.
    assert!(accounts::update(&c, a, &json!({"name": "A", "type": "cash", "currency": "EUR"})).is_err());

    transactions::delete(&c, tid).unwrap();
    assert_eq!(balance(&c, b), json!(0.0));
    assert_books_balance(&c);
}

#[test]
fn categories_must_match_and_deleting_one_keeps_its_transactions() {
    let c = conn();
    let a = accounts::create(&c, &json!({"name": "A", "type": "cash", "currency": "INR"})).unwrap();
    let a = a["id"].as_str().unwrap();
    assert!(transactions::create(&c, &json!({"account_id": a, "type": "expense", "amount": 5, "date": "2026-10-01", "category_id": "cat_salary"})).is_err());

    let pets = categories::create(&c, &json!({"name": "Pets", "type": "expense"})).unwrap();
    let pets = pets["id"].as_str().unwrap();
    let t = transactions::create(&c, &json!({"account_id": a, "type": "expense", "amount": 499, "date": "2026-10-01", "category_id": pets})).unwrap();
    assert_eq!(t["category_name"], json!("Pets"));

    categories::delete(&c, pets).unwrap();
    let after = transactions::get_one(&c, t["id"].as_str().unwrap()).unwrap().unwrap();
    assert_eq!(after["category_id"], Value::Null);
    assert_eq!(after["amount"], json!(499.0));
    assert_books_balance(&c);
}

#[test]
fn zero_decimal_and_three_decimal_currencies() {
    let c = conn();
    let yen = accounts::create(&c, &json!({"name": "Yen", "type": "cash", "currency": "JPY", "initial_balance": 10000})).unwrap();
    let dinar = accounts::create(&c, &json!({"name": "Dinar", "type": "cash", "currency": "KWD", "initial_balance": "1.250"})).unwrap();
    assert_eq!(yen["current_balance_minor"], json!(10000));
    assert_eq!(dinar["current_balance_minor"], json!(1250));
    assert!(transactions::create(&c, &json!({"account_id": yen["id"], "type": "expense", "amount": 1.5, "date": "2026-10-01"})).is_err());
    assert_books_balance(&c);
}

/// Data created with the pre-ledger schema (migrations 0001–0003) converts without changing any balance.
#[test]
fn migration_from_float_schema_preserves_balances() {
    let c = rusqlite::Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    db::migrate(&c, &db::DATA_MIGRATIONS[..3]).unwrap();
    c.execute_batch(
        "INSERT INTO accounts (id, name, type, currency, initial_balance, current_balance, created_at) VALUES
           ('bank', 'Bank', 'checking', 'INR', 1000.10, 0, '2026-01-05 10:00:00'),
           ('card', 'Card', 'credit',   'INR', -200,    0, '2026-01-05 10:00:00'),
           ('yen',  'Yen',  'cash',     'JPY', 5000,    0, '2026-01-05 10:00:00'),
           ('none', 'Empty','cash',     'USD', 0,       0, '2026-01-05 10:00:00');
         INSERT INTO transactions (id, account_id, category_id, type, amount, date, merchant, is_cleared, visibility) VALUES
           ('t1', 'bank', 'cat_food',   'expense', 250.75, '2026-02-01', 'Swiggy', 1, 'public'),
           ('t2', 'bank', 'cat_salary', 'income',  0.1,    '2026-02-02', 'Interest', 1, 'private'),
           ('t3', 'bank', NULL,         'expense', 0.2,    '2026-02-03', NULL, 1, 'shared'),
           ('t4', 'card', 'cat_shopping','expense', 99.99, '2026-02-04', 'Store', 1, 'private'),
           ('t5', 'bank', NULL,         'transfer', 40,    '2026-02-05', NULL, 1, 'private'),
           ('t6', 'yen',  'cat_travel', 'expense', 1200,   '2026-02-06', 'Train', 1, 'private'),
           ('t7', 'bank', 'cat_food',   'expense', 10,     '2026-02-07', 'Pending', 0, 'private'),
           ('t8', 'bank', 'cat_food',   'expense', 0,      '2026-02-08', 'Zero', 1, 'private');",
    )
    .unwrap();
    // Old rule: balance = initial + cleared income − cleared expense (transfers had no effect).
    let old: Vec<(String, f64)> = c
        .prepare(
            "SELECT a.id, a.initial_balance + COALESCE((SELECT SUM(CASE type WHEN 'income' THEN amount WHEN 'expense' THEN -amount ELSE 0 END)
                                                         FROM transactions t WHERE t.account_id = a.id AND t.is_cleared = 1), 0)
             FROM accounts a ORDER BY a.id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    db::migrate(&c, db::DATA_MIGRATIONS).unwrap();

    for (id, expected) in old {
        let got = balance(&c, &id).as_f64().unwrap();
        assert!((got - expected).abs() < 1e-9, "{id}: {got} vs {expected}");
    }
    assert_eq!(balance(&c, "bank"), json!(749.25)); // 1000.10 − 250.75 + 0.10 − 0.20
    assert_eq!(transactions::get_all(&c, None).unwrap().len(), 8);
    let t1 = transactions::get_one(&c, "t1").unwrap().unwrap();
    assert_eq!((t1["amount_minor"].clone(), t1["visibility"].clone(), t1["category_name"].clone()), (json!(25075), json!("public"), json!("Food & Dining")));
    assert_eq!(transactions::get_one(&c, "t6").unwrap().unwrap()["amount_minor"], json!(1200));
    assert_books_balance(&c);
    // And the app keeps working on the migrated data.
    transactions::create(&c, &json!({"account_id": "bank", "type": "expense", "amount": 49.25, "date": "2026-03-01"})).unwrap();
    assert_eq!(balance(&c, "bank"), json!(700.0));
}

#[test]
fn version_1_backups_still_import() {
    let v1 = json!({
        "version": 1,
        "accounts": [
            {"id": "acc1", "name": "Main", "type": "checking", "currency": "INR", "initial_balance": 1000.5, "current_balance": 900.5},
            {"id": "card1", "name": "Card", "type": "credit", "currency": "INR", "initial_balance": -300, "current_balance": -350}
        ],
        "categories": [{"id": "cat_food", "name": "Food & Dining", "type": "expense", "is_system": 1}],
        "transactions": [
            {"id": "x1", "account_id": "acc1", "category_id": "cat_food", "type": "expense", "amount": 100, "date": "2026-01-02", "visibility": "public"},
            {"id": "x2", "account_id": "card1", "category_id": null, "type": "expense", "amount": 50, "date": "2026-01-03"}
        ]
    });
    let c = conn();
    let counts = export::from_json(&c, &v1).unwrap();
    assert_eq!((counts.accounts, counts.transactions), (2, 2));
    assert_eq!(balance(&c, "acc1"), json!(900.5));
    assert_eq!(balance(&c, "card1"), json!(-350.0));
    // Importing again changes nothing.
    let again = export::from_json(&c, &v1).unwrap();
    assert_eq!((again.accounts, again.transactions), (0, 0));
    assert_eq!(balance(&c, "acc1"), json!(900.5));
    assert_books_balance(&c);
}

#[test]
fn version_2_backup_roundtrip_is_idempotent() {
    let a = conn();
    let acc = accounts::create(&a, &json!({"name": "Main", "type": "checking", "currency": "INR", "initial_balance": 10})).unwrap();
    transactions::create(&a, &json!({"account_id": acc["id"], "type": "expense", "amount": 4, "date": "2026-10-01"})).unwrap();
    let dump = export::to_json(&a).unwrap();
    assert_eq!(dump["version"], json!(2));

    let b = conn();
    let counts = export::from_json(&b, &dump).unwrap();
    assert_eq!((counts.accounts, counts.transactions), (1, 1));
    let again = export::from_json(&b, &dump).unwrap();
    assert_eq!(again.transactions, 0);
    assert_eq!(balance(&b, acc["id"].as_str().unwrap()), json!(6.0));
    assert_books_balance(&b);

    let (csv, n) = export::to_csv(&b).unwrap();
    assert_eq!(n, 1);
    assert!(csv.lines().nth(1).unwrap().starts_with("2026-10-01,expense,4.00,INR,Main"), "{csv}");
}

#[test]
fn household_publishes_exact_amounts() {
    let c = conn();
    let h = household::Household::open_in_memory().unwrap();
    let on = household::SharingPrefs { enabled: true, default_visibility: household::Visibility::Private, shared_with: vec![] };
    h.set_prefs("A", &on).unwrap();
    h.set_prefs("C", &on).unwrap();
    let acc = accounts::create(&c, &json!({"name": "Pocket", "type": "cash", "currency": "INR"})).unwrap();
    let row = transactions::create(&c, &json!({"account_id": acc["id"], "type": "expense", "amount": "149.99", "date": "2026-10-10", "visibility": "public"})).unwrap();
    h.publish("C", &row).unwrap();
    let feed = h.feed("A", household::FeedView::All, &Default::default()).unwrap();
    assert_eq!(feed[0]["amount_minor"], json!(14999));
    assert_eq!(feed[0]["amount"], json!(149.99));
}

#[test]
fn household_migration_converts_real_amounts() {
    let c = rusqlite::Connection::open_in_memory().unwrap();
    db::migrate(&c, &db::HOUSEHOLD_MIGRATIONS[..1]).unwrap();
    c.execute_batch(
        "INSERT INTO members (user_id, enabled) VALUES ('A', 1), ('C', 1);
         INSERT INTO shared_items (owner_id, item_id, visibility, date, type, amount, currency) VALUES
           ('C', 'i1', 'public', '2026-10-01', 'expense', 250.75, 'INR'),
           ('C', 'i2', 'public', '2026-10-02', 'expense', 1200, 'JPY');",
    )
    .unwrap();
    db::migrate(&c, db::HOUSEHOLD_MIGRATIONS).unwrap();
    let rows = rows::query(&c, "SELECT item_id, amount_minor, amount_scale FROM shared_items ORDER BY item_id", &[]).unwrap();
    assert_eq!(rows[0]["amount_minor"], json!(25075));
    assert_eq!(rows[1]["amount_minor"], json!(1200));
    assert_eq!(rows[1]["amount_scale"], json!(1));
}
