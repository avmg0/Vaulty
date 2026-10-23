use serde_json::json;
use vaulty_core::{accounts, categories, db, history, projects as pr, transactions};

fn setup() -> rusqlite::Connection {
    let c = db::open_in_memory().unwrap();
    vaulty_core::settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    c
}

#[test]
fn bills_payments_and_whats_owed() {
    let c = setup();
    let p = pr::create(&c, &json!({ "name": "Home renovation", "kind": "family", "budget": "500000",
        "sections": ["Electrical work", "Plumbing", "Painting"],
        "columns": [{ "name": "Room", "type": "choice", "options": ["Kitchen", "Bedroom"] }, { "name": "Warranty years", "type": "number" }],
        "hidden": ["bill_no"] })).unwrap();
    let id = p["id"].as_str().unwrap();
    assert_eq!(p["sections"].as_array().unwrap().len(), 3);
    let elec = p["sections"].as_array().unwrap().iter().find(|s| s["name"] == "Electrical work").unwrap()["id"].as_str().unwrap().to_string();
    let room = p["columns"][0]["id"].as_str().unwrap().to_string();
    let years = p["columns"][1]["id"].as_str().unwrap().to_string();

    // Qty × rate, 18% GST worked out; a labour charge typed as an amount.
    let wire = pr::add_item(&c, id, &json!({ "section_id": elec, "date": "2026-10-02", "item": "Copper wire 2.5 sq mm", "qty": "4", "unit": "coil",
        "rate": "1850.50", "tax_percent": 18, "vendor": "Jimmy Doe Electricals", "custom": { room.clone(): "Kitchen", years.clone(): "2" } })).unwrap();
    assert_eq!((wire["amount"].as_f64(), wire["tax"].as_f64(), wire["total"].as_f64()), (Some(7402.0), Some(1332.36), Some(8734.36)));
    assert_eq!((wire["custom"][&room].as_str(), wire["custom"][&years].as_f64()), (Some("Kitchen"), Some(2.0)));
    pr::add_item(&c, id, &json!({ "section_id": elec, "date": "2026-10-05", "item": "Wiring labour", "kind": "labour", "amount": "12000", "vendor": "jimmy doe electricals " })).unwrap();
    pr::add_item(&c, id, &json!({ "date": "2026-10-06", "item": "Debris removal", "kind": "transport", "amount": "1500", "vendor": "John Doe" })).unwrap();
    assert!(pr::add_item(&c, id, &json!({ "date": "2026-10-06", "item": "x", "custom": { room.clone(): "Garage" }, "amount": "1" })).is_err(), "not one of the choices");
    assert!(pr::add_item(&c, id, &json!({ "date": "2026-10-06", "item": "x" })).is_err(), "no amount");
    assert!(pr::add_item(&c, id, &json!({ "date": "2026-10-06", "item": "x", "qty": "0", "rate": "5" })).is_err());

    // An advance and an instalment to the electrician, one recorded as a transaction too.
    let acct = accounts::create(&c, &json!({ "name": "Everyday savings", "type": "savings", "currency": "INR", "initial_balance": "100000" })).unwrap();
    let home = categories::get_all(&c).unwrap().into_iter().find(|x| x["type"] == "expense").unwrap();
    let adv = pr::add_payment(&c, id, &json!({ "date": "2026-10-01", "amount": "15000", "vendor": "Jimmy Doe Electricals", "mode": "upi", "account_name": "JIMMY D",
        "section_id": elec, "record": { "account_id": acct["id"], "category_id": home["id"] } })).unwrap();
    assert!(adv["entry_id"].is_string());
    assert_eq!(adv["from_account"], "Everyday savings");
    let t = transactions::get_one(&c, adv["entry_id"].as_str().unwrap()).unwrap().unwrap();
    assert_eq!((t["amount"].as_f64(), t["tags"].as_str()), (Some(15000.0), Some("Home renovation")));
    pr::add_payment(&c, id, &json!({ "date": "2026-10-07", "amount": "4000", "vendor": "Jimmy Doe Electricals", "mode": "cash" })).unwrap();
    pr::add_payment(&c, id, &json!({ "date": "2026-10-06", "amount": "2000", "vendor": "John Doe", "mode": "cash" })).unwrap();

    let p = pr::get(&c, id).unwrap();
    assert_eq!((p["billed"].as_f64(), p["paid"].as_f64(), p["tax"].as_f64()), (Some(22234.36), Some(21000.0), Some(1332.36)));
    // Jimmy is owed 1,734.36; John was paid 500 in advance. Only what's owed counts as owed.
    let v = p["vendors"].as_array().unwrap();
    assert_eq!((v[0]["name"].as_str(), v[0]["balance"].as_f64(), v[0]["items"].as_i64()), (Some("Jimmy Doe Electricals"), Some(1734.36), Some(2)));
    assert_eq!((v[1]["name"].as_str(), v[1]["balance"].as_f64()), (Some("John Doe"), Some(-500.0)));
    assert_eq!(p["owed"].as_f64(), Some(1734.36));
    assert_eq!(p["budget_left"].as_f64(), Some(500000.0 - 22234.36));
    let e = p["sections"].as_array().unwrap().iter().find(|s| s["name"] == "Electrical work").unwrap();
    assert_eq!((e["billed"].as_f64(), e["items"].as_i64(), e["paid"].as_f64()), (Some(20734.36), Some(2), Some(15000.0)));
    assert_eq!((p["unsectioned"]["billed"].as_f64(), p["by_kind"]["labour"].as_f64(), p["by_mode"]["cash"].as_f64()), (Some(1500.0), Some(12000.0), Some(6000.0)));

    let items = pr::items(&c, id).unwrap();
    assert_eq!(items.iter().map(|i| i["item"].as_str().unwrap()).collect::<Vec<_>>(), ["Copper wire 2.5 sq mm", "Wiring labour", "Debris removal"]);
    assert_eq!(pr::list(&c).unwrap()[0]["items"], 3);

    // Removing a section keeps its items, without a section.
    history::record(&c, "project.section_delete", |c| pr::delete_section(c, &elec)).unwrap();
    assert_eq!(pr::get(&c, id).unwrap()["unsectioned"]["items"], 3);
    history::undo_last(&c).unwrap();
    assert_eq!(pr::get(&c, id).unwrap()["unsectioned"]["items"], 1);

    // Deleting the project keeps the transaction; undo brings everything back.
    history::record(&c, "project.delete", |c| pr::delete(c, id)).unwrap();
    assert!(transactions::get_one(&c, adv["entry_id"].as_str().unwrap()).unwrap().is_some());
    assert!(pr::list(&c).unwrap().is_empty());
    history::undo_last(&c).unwrap();
    assert_eq!(pr::get(&c, id).unwrap()["payments"], 3);
}

#[test]
fn linking_payments_to_bank_rows() {
    let c = setup();
    let p = pr::create(&c, &json!({ "name": "Wedding" })).unwrap();
    let id = p["id"].as_str().unwrap();
    let acct = accounts::create(&c, &json!({ "name": "Salary account", "type": "savings", "currency": "INR", "initial_balance": "500000" })).unwrap();
    // Rows as a bank statement import would bring them in.
    let row = |amount: &str, date: &str, merchant: &str| transactions::create(&c, &json!({ "type": "expense", "account_id": acct["id"], "amount": amount, "date": date, "merchant": merchant })).unwrap();
    let caterer = row("85000", "2026-11-03", "UPI/JENNY DOE CATERERS/okhdfc");
    let other = row("85000", "2026-11-01", "NEFT/SOMEONE ELSE");
    row("85000", "2026-12-20", "UPI/JENNY DOE CATERERS");
    let pay = pr::add_payment(&c, id, &json!({ "date": "2026-11-02", "amount": "85000", "vendor": "Caterer", "account_name": "Jenny Doe Caterers", "mode": "upi" })).unwrap();

    let s = pr::link_suggestions(&c, id).unwrap();
    let cands = s[0]["candidates"].as_array().unwrap();
    assert_eq!(cands.len(), 2, "the December one is too far off");
    assert_eq!((cands[0]["id"].as_str(), cands[0]["named"].as_bool()), (caterer["id"].as_str(), Some(true)));
    assert_eq!(cands[1]["id"].as_str(), other["id"].as_str());

    let pid = pay["id"].as_str().unwrap();
    let mut v = pay.clone();
    v["entry_id"] = caterer["id"].clone();
    let linked = pr::update_payment(&c, pid, &v).unwrap();
    assert_eq!(linked["entry_id"], caterer["id"]);
    assert!(pr::link_suggestions(&c, id).unwrap().is_empty());
    // The same transaction can't back two payments, and amounts must agree.
    let p2 = pr::add_payment(&c, id, &json!({ "date": "2026-11-02", "amount": "85000", "mode": "upi" })).unwrap();
    let mut v2 = p2.clone();
    v2["entry_id"] = caterer["id"].clone();
    assert!(pr::update_payment(&c, p2["id"].as_str().unwrap(), &v2).unwrap_err().to_string().contains("already linked"));
    let mut v3 = linked.clone();
    v3["amount"] = json!("80000");
    assert!(pr::update_payment(&c, pid, &v3).unwrap_err().to_string().contains("isn't the same"));
    v3["entry_id"] = json!("");
    assert!(pr::update_payment(&c, pid, &v3).unwrap()["entry_id"].is_null(), "unlinked");
    let names = pr::names(&c).unwrap();
    assert_eq!(names["account_names"][0], "Jenny Doe Caterers");
}

#[test]
fn checks_on_projects_and_columns() {
    let c = setup();
    assert!(pr::create(&c, &json!({ "name": "" })).is_err());
    assert!(pr::create(&c, &json!({ "name": "x", "kind": "secret" })).is_err());
    assert!(pr::create(&c, &json!({ "name": "x", "start_date": "2026-10-10", "end_date": "2026-10-01" })).is_err());
    assert!(pr::create(&c, &json!({ "name": "x", "columns": [{ "name": "A" }, { "name": "a" }] })).is_err(), "same name");
    assert!(pr::create(&c, &json!({ "name": "x", "columns": [{ "name": "Pick", "type": "choice" }] })).is_err(), "no choices");
    assert!(pr::create(&c, &json!({ "name": "x", "hidden": ["amount"] })).is_err(), "amount always shows");
    let p = pr::create(&c, &json!({ "name": "Trip", "sections": ["Travel", "Travel", "Stay"] })).unwrap();
    assert_eq!(p["sections"].as_array().unwrap().len(), 2, "duplicate names are skipped");
    let id = p["id"].as_str().unwrap();
    assert!(pr::add_section(&c, id, &json!({ "name": "stay" })).is_err(), "names ignore case");
    // Columns kept by id when renamed; their values stay.
    let p = pr::update(&c, id, &json!({ "name": "Goa trip", "status": "done", "columns": [{ "name": "Paid by", "type": "text" }] })).unwrap();
    let col = p["columns"][0]["id"].as_str().unwrap().to_string();
    let item = pr::add_item(&c, id, &json!({ "date": "2026-10-01", "item": "Flights", "amount": "18000", "custom": { col.clone(): "Jane" } })).unwrap();
    let p = pr::update(&c, id, &json!({ "name": "Goa trip", "status": "done", "columns": [{ "id": col, "name": "Who paid", "type": "text" }] })).unwrap();
    assert_eq!(p["columns"][0]["name"], "Who paid");
    assert_eq!(pr::items(&c, id).unwrap()[0]["custom"][&col], "Jane");
    assert_eq!(item["total"].as_f64(), Some(18000.0));
}

#[test]
fn workbook_has_every_row_and_totals() {
    use calamine::{Reader, Xlsx};
    let c = setup();
    let p = pr::create(&c, &json!({ "name": "Shop fit-out", "kind": "business", "sections": ["Furniture"], "columns": [{ "name": "Pieces", "type": "number" }], "hidden": ["bill_no"] })).unwrap();
    let id = p["id"].as_str().unwrap();
    let col = p["columns"][0]["id"].as_str().unwrap().to_string();
    for i in 1..=3 {
        pr::add_item(&c, id, &json!({ "date": format!("2026-10-0{i}"), "item": format!("Shelf {i}"), "qty": i, "rate": "1000", "tax_percent": "18", "vendor": "Jimmy Doe", "custom": { col.clone(): i } })).unwrap();
    }
    pr::add_payment(&c, id, &json!({ "date": "2026-10-04", "amount": "5000", "vendor": "Jimmy Doe", "mode": "bank" })).unwrap();
    let bytes = vaulty_core::project_export::to_xlsx(&c, id, &json!({ "items": "बिल", "mode.bank": "Bank transfer" })).unwrap();
    let mut wb: Xlsx<_> = Xlsx::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(wb.sheet_names(), ["Summary", "बिल", "Payments"]);
    let items = wb.worksheet_range("बिल").unwrap();
    let head: Vec<String> = items.rows().next().unwrap().iter().map(|c| c.to_string()).collect();
    assert!(head.contains(&"Pieces".to_string()) && !head.contains(&"Bill no.".to_string()), "{head:?}");
    assert_eq!(items.rows().count(), 5, "heading, three rows, totals");
    let formulas = wb.worksheet_formula("बिल").unwrap();
    let f: Vec<String> = formulas.rows().flatten().map(|c| c.to_string()).filter(|s| !s.is_empty()).collect();
    assert!(f.iter().any(|s| s.contains("SUM(")), "{f:?}");
    let pays = wb.worksheet_range("Payments").unwrap();
    assert_eq!(pays.get((1, 4)).unwrap().to_string(), "Bank transfer");
}

#[test]
fn standard_columns_can_be_renamed() {
    use calamine::{Reader, Xlsx};
    let c = setup();
    let p = pr::create(&c, &json!({ "name": "Line-following robot", "labels": { "item": "Part", "vendor": "Supplier", "qty": " " } })).unwrap();
    assert_eq!(p["labels"], json!({ "item": "Part", "vendor": "Supplier" }), "blank names fall back to the app's words");
    let id = p["id"].as_str().unwrap();
    assert!(pr::update(&c, id, &json!({ "name": "x", "labels": { "budget": "Money" } })).is_err(), "not a column");
    assert!(pr::update(&c, id, &json!({ "name": "x", "labels": { "item": "x".repeat(41) } })).is_err());
    pr::add_item(&c, id, &json!({ "date": "2026-10-20", "item": "Servo motor SG90", "qty": 4, "rate": "180", "vendor": "Robu" })).unwrap();
    let bytes = vaulty_core::project_export::to_xlsx(&c, id, &json!({})).unwrap();
    let mut wb: Xlsx<_> = Xlsx::new(std::io::Cursor::new(bytes)).unwrap();
    let head: Vec<String> = wb.worksheet_range("Bill items").unwrap().rows().next().unwrap().iter().map(|c| c.to_string()).collect();
    assert!(head.contains(&"Part".into()) && head.contains(&"Supplier".into()) && head.contains(&"Qty".into()), "{head:?}");
    let p = pr::update(&c, id, &json!({ "name": "Line-following robot", "labels": {} })).unwrap();
    assert_eq!(p["labels"], json!({}));
}
