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

#[test]
fn bills_kept_encrypted_with_the_project() {
    use vaulty_core::{documents, Key};
    const PDF: &[u8] = b"%PDF-1.7\n1 0 obj << /Type /Catalog >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n";
    let key = Key::new([7u8; 32]);
    let c = setup();
    let dir = tempfile::tempdir().unwrap();
    let p = pr::create(&c, &json!({ "name": "Kitchen" })).unwrap();
    let id = p["id"].as_str().unwrap();
    let item = pr::add_item(&c, id, &json!({ "date": "2026-10-02", "item": "Chimney", "amount": "18000" })).unwrap();
    let iid = item["id"].as_str().unwrap();
    let f = pr::add_file(&c, &key, dir.path(), id, "/Users/jane/Bills/chimney-invoice.pdf", PDF, &json!({ "item_id": iid })).unwrap();
    let fid = f["id"].as_str().unwrap().to_string();
    assert_eq!((f["name"].as_str(), f["item"].as_str()), (Some("chimney-invoice.pdf"), Some("Chimney")));
    assert!(pr::add_file(&c, &key, dir.path(), id, "x.txt", b"hello", &json!({})).is_err(), "only PDFs and photos");
    assert!(pr::add_file(&c, &key, dir.path(), id, "x.pdf", PDF, &json!({ "item_id": "nope" })).is_err());
    let raw = std::fs::read(dir.path().join(format!("{fid}.vdoc"))).unwrap();
    assert!(!raw.windows(5).any(|w| w == b"%PDF-"), "encrypted on disk");
    assert_eq!(pr::read_file(&c, &key, dir.path(), &fid).unwrap().2, PDF);
    assert_eq!(pr::items(&c, id).unwrap()[0]["files"], 1);
    assert_eq!(pr::get(&c, id).unwrap()["files"], 1);

    // Backups carry project bills, and purge leaves them alone.
    let backup = documents::files_for_backup(&c, &key, dir.path()).unwrap().unwrap();
    assert!(backup.get(&fid).is_some());
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
    std::fs::File::options().write(true).open(dir.path().join(format!("{fid}.vdoc"))).unwrap().set_modified(old).unwrap();
    assert_eq!(documents::purge(&c, dir.path()).unwrap(), 0);

    // Deleting the item keeps the bill in the project; it can be pointed elsewhere.
    pr::delete_item(&c, iid).unwrap();
    assert!(pr::files(&c, id).unwrap()[0]["item_id"].is_null());
    let pay = pr::add_payment(&c, id, &json!({ "date": "2026-10-03", "amount": "18000", "mode": "card" })).unwrap();
    let f = pr::link_file(&c, &fid, &json!({ "payment_id": pay["id"] })).unwrap();
    assert_eq!(f["payment_date"], "2026-10-03");
    assert!(pr::link_file(&c, &fid, &json!({ "payment_id": pay["id"], "item_id": "x" })).is_err());
    history::record(&c, "project.file_remove", |c| pr::remove_file(c, &fid)).unwrap();
    assert!(pr::files(&c, id).unwrap().is_empty());
    history::undo_last(&c).unwrap();
    assert_eq!(pr::read_file(&c, &key, dir.path(), &fid).unwrap().0, "chimney-invoice.pdf");
}

#[test]
fn reading_a_kept_bill_into_items() {
    use vaulty_core::Key;
    let key = Key::new([7u8; 32]);
    let c = setup();
    let dir = tempfile::tempdir().unwrap();
    let p = pr::create(&c, &json!({ "name": "Rewiring" })).unwrap();
    let id = p["id"].as_str().unwrap();
    let pdf = std::fs::read(format!("{}/tests/fixtures/invoices/gst-cgst.pdf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let f = pr::add_file(&c, &key, dir.path(), id, "invoice.pdf", &pdf, &json!({})).unwrap();
    let fid = f["id"].as_str().unwrap();
    let draft = pr::read_bill(&c, &key, dir.path(), fid, |_, _| panic!("a PDF with text isn't sent for recognition")).unwrap();
    assert_eq!(draft.items.len(), 4);
    let items: Vec<serde_json::Value> = draft.items.iter().map(|i| json!({ "date": "2026-10-02", "item": i.item, "qty": i.qty, "unit": i.unit, "rate": i.rate,
        "amount": i.amount, "tax_percent": i.tax_percent, "vendor": draft.vendor, "bill_no": draft.bill_no })).collect();
    let added = history::record(&c, "project.items_add", |c| pr::add_items(c, id, &json!({ "items": items, "file_id": fid }))).unwrap();
    assert_eq!(added.len(), 4);
    let p = pr::get(&c, id).unwrap();
    assert_eq!((p["billed"].as_f64(), p["tax"].as_f64()), (Some(19564.4), Some(2984.4)), "the bill's own total");
    assert_eq!(pr::files(&c, id).unwrap()[0]["item_id"], added[0]["id"]);
    history::undo_last(&c).unwrap();
    assert!(pr::items(&c, id).unwrap().is_empty(), "one undo takes the whole bill out");
    // A photo goes to the system's text recognition; its pieces are laid out into lines.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";
    let img = pr::add_file(&c, &key, dir.path(), id, "bill.png", PNG, &json!({})).unwrap();
    let iid = img["id"].as_str().unwrap();
    let piece = |t: &str, x: f64, y: f64| vaulty_core::ocr::Piece { text: t.into(), x, y, w: t.len() as f64 * 0.012, h: 0.02 };
    let draft = pr::read_bill(&c, &key, dir.path(), iid, |bytes, mime| {
        assert_eq!((bytes, mime), (PNG, "image/png"));
        Ok(vec![piece("SHARMA HARDWARE", 0.3, 0.05), piece("Primer 4L", 0.05, 0.3), piece("2", 0.5, 0.3), piece("1150.00", 0.62, 0.3), piece("2300.00", 0.8, 0.3),
                piece("TOTAL", 0.05, 0.4), piece("2300.00", 0.8, 0.4)])
    }).unwrap();
    assert_eq!((draft.items.len(), draft.items[0].item.as_str(), draft.matches), (1, "Primer 4L", Some(true)));
    assert!(pr::read_bill(&c, &key, dir.path(), iid, |_, _| Ok(vec![])).is_err(), "nothing recognised");
}

#[test]
fn splitting_a_trip_with_friends() {
    let c = setup();
    let p = pr::create(&c, &json!({ "name": "Goa trip", "kind": "group" })).unwrap();
    let id = p["id"].as_str().unwrap();
    assert_eq!(pr::split(&c, id).unwrap()["members"], json!([]));
    assert!(pr::set_members(&c, id, &json!([{ "name": "Me" }, { "name": "John" }])).is_err(), "someone must be you");
    assert!(pr::set_members(&c, id, &json!([{ "name": "Jane", "is_me": true }, { "name": "jane" }])).is_err());
    let ms = pr::set_members(&c, id, &json!([{ "name": "Jane", "is_me": true }, { "name": "John" }, { "name": "Jenny", "share": 2 }])).unwrap();
    let mid = |n: &str| ms.iter().find(|m| m["name"] == n).unwrap()["id"].as_str().unwrap().to_string();
    let (john, jenny) = (mid("John"), mid("Jenny"));
    // I paid the hotel, John the flights, Jenny dinner.
    pr::add_payment(&c, id, &json!({ "date": "2026-05-01", "amount": "24000", "vendor": "Hotel", "mode": "card" })).unwrap();
    pr::add_payment(&c, id, &json!({ "date": "2026-05-01", "amount": "15000", "vendor": "Airline", "mode": "upi", "paid_by": john })).unwrap();
    pr::add_payment(&c, id, &json!({ "date": "2026-05-03", "amount": "1000.01", "vendor": "Shack", "mode": "cash", "paid_by": jenny })).unwrap();
    assert!(pr::add_payment(&c, id, &json!({ "date": "2026-05-03", "amount": "10", "mode": "cash", "paid_by": "nobody" })).is_err());
    let acct = accounts::create(&c, &json!({ "name": "Savings", "type": "savings", "currency": "INR", "initial_balance": "1000" })).unwrap();
    assert!(pr::add_payment(&c, id, &json!({ "date": "2026-05-03", "amount": "10", "mode": "cash", "paid_by": john, "record": { "account_id": acct["id"] } })).is_err(), "not my money");

    let s = pr::split(&c, id).unwrap();
    assert_eq!(s["total"].as_f64(), Some(40000.01));
    let get = |n: &str| s["members"].as_array().unwrap().iter().find(|m| m["name"] == n).unwrap().clone();
    // Shares 1:1:2 of ₹40,000.01; the odd paisa goes to someone, and the shares add up exactly.
    let owes: f64 = ["Jane", "John", "Jenny"].iter().map(|n| get(n)["owes"].as_f64().unwrap()).sum();
    assert!((owes - 40000.01).abs() < 1e-6);
    assert_eq!((get("Jane")["paid"].as_f64(), get("Jane")["owes"].as_f64()), (Some(24000.0), Some(10000.0)));
    assert_eq!(get("Jenny")["balance"].as_f64(), Some(1000.01 - 20000.01));
    let settle = s["settle"].as_array().unwrap();
    assert_eq!(settle.len(), 2, "{settle:?}");
    assert_eq!((settle[0]["from"].as_str(), settle[0]["to"].as_str(), settle[0]["amount"].as_f64(), settle[0]["to_me"].as_bool()), (Some("Jenny"), Some("Jane"), Some(14000.0), Some(true)));
    assert_eq!((settle[1]["from"].as_str(), settle[1]["to"].as_str(), settle[1]["amount"].as_f64()), (Some("Jenny"), Some("John"), Some(5000.0)));
    let pays = pr::payments(&c, id).unwrap();
    assert_eq!(pays.iter().filter_map(|y| y["paid_by_name"].as_str()).collect::<Vec<_>>(), ["John", "Jenny"]);

    // Removing John makes his payment mine; renaming keeps ids.
    let ms = pr::set_members(&c, id, &json!([{ "id": mid("Jane"), "name": "Jane D", "is_me": true }, { "id": jenny, "name": "Jenny" }])).unwrap();
    assert_eq!(ms.len(), 2);
    assert_eq!(pr::split(&c, id).unwrap()["members"][0]["paid"].as_f64(), Some(39000.0));
}

#[test]
fn sharing_a_project_as_a_file_and_back() {
    use vaulty_core::{backup, project_share as share, Key};
    const PDF: &[u8] = b"%PDF-1.7\n1 0 obj << /Type /Catalog >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n";
    // Jane's Vaulty and John's Vaulty.
    let (jane, john) = (setup(), setup());
    let (kj, kn) = (Key::new([1u8; 32]), Key::new([2u8; 32]));
    let (dj, dn) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let p = pr::create(&jane, &json!({ "name": "Goa trip", "kind": "group", "sections": ["Travel", "Stay"],
        "columns": [{ "name": "Booked on", "type": "date" }], "labels": { "vendor": "Booked with" } })).unwrap();
    let pid = p["id"].as_str().unwrap();
    let col = p["columns"][0]["id"].as_str().unwrap().to_string();
    let ms = pr::set_members(&jane, pid, &json!([{ "name": "Jane", "is_me": true }, { "name": "John" }])).unwrap();
    let john_id = ms[1]["id"].as_str().unwrap().to_string();
    pr::add_item(&jane, pid, &json!({ "date": "2026-05-01", "item": "Flights", "amount": "15000", "custom": { col.clone(): "2026-04-01" } })).unwrap();
    let acct = accounts::create(&jane, &json!({ "name": "Savings", "type": "savings", "currency": "INR", "initial_balance": "50000" })).unwrap();
    let hotel = pr::add_payment(&jane, pid, &json!({ "date": "2026-05-01", "amount": "24000", "vendor": "Hotel", "mode": "card", "record": { "account_id": acct["id"] } })).unwrap();
    pr::add_payment(&jane, pid, &json!({ "date": "2026-05-01", "amount": "15000", "vendor": "Airline", "mode": "upi", "paid_by": john_id })).unwrap();
    pr::add_file(&jane, &kj, dj.path(), pid, "hotel.pdf", PDF, &json!({ "payment_id": hotel["id"] })).unwrap();

    // Jane sends it, encrypted with a password.
    let value = share::export(&jane, &kj, dj.path(), pid, true).unwrap();
    assert!(value["payments"].as_array().unwrap().iter().all(|y| y.get("entry_id").is_none()), "her transactions stay hers");
    let file = backup::encrypt_value(&value, "goa-2026-trip", None).unwrap();
    assert!(!file.windows(5).any(|w| w == b"Hotel"));
    let got = backup::decrypt(&file, "goa-2026-trip").unwrap();

    // John opens it and says which person he is.
    let peek = share::peek(&john, &got).unwrap();
    assert_eq!((peek["name"].as_str(), peek["members"].as_array().unwrap().len(), peek["files"].as_u64(), peek["updates"].is_null()), (Some("Goa trip"), 2, Some(1), true));
    assert!(share::import(&john, &kn, dn.path(), &got, None).is_err(), "he must say who he is");
    let r = share::import(&john, &kn, dn.path(), &got, Some(&john_id)).unwrap();
    assert_eq!((r["updated"].as_bool(), r["files"].as_u64()), (Some(false), Some(1)));
    let jid = r["id"].as_str().unwrap().to_string();
    let s = pr::split(&john, &jid).unwrap();
    let me = s["members"].as_array().unwrap().iter().find(|m| m["is_me"] == true).unwrap();
    assert_eq!((me["name"].as_str(), me["paid"].as_f64()), (Some("John"), Some(15000.0)), "the flights John paid are his own on his side");
    assert_eq!(pr::get(&john, &jid).unwrap()["labels"]["vendor"], "Booked with");
    assert_eq!(pr::items(&john, &jid).unwrap()[0]["custom"][&col], "2026-04-01");
    let fid = pr::files(&john, &jid).unwrap()[0]["id"].as_str().unwrap().to_string();
    assert_eq!(pr::read_file(&john, &kn, dn.path(), &fid).unwrap().2, PDF, "the bill, under John's key");

    // John adds dinner and sends it back; Jane's project is brought up to date, links kept.
    pr::add_payment(&john, &jid, &json!({ "date": "2026-05-03", "amount": "3000", "vendor": "Shack", "mode": "cash" })).unwrap();
    let back = share::export(&john, &kn, dn.path(), &jid, false).unwrap();
    assert_eq!(share::peek(&jane, &back).unwrap()["updates"]["id"].as_str(), Some(pid));
    let jane_me = ms[0]["id"].as_str().unwrap().to_string();
    let r = share::import(&jane, &kj, dj.path(), &back, Some(&jane_me)).unwrap();
    assert_eq!((r["id"].as_str(), r["updated"].as_bool()), (Some(pid), Some(true)));
    let pays = pr::payments(&jane, pid).unwrap();
    assert_eq!(pays.len(), 3);
    assert!(pays.iter().find(|y| y["vendor"] == "Hotel").unwrap()["entry_id"].is_string(), "her link to her transaction survives");
    assert_eq!(pays.iter().find(|y| y["vendor"] == "Shack").unwrap()["paid_by_name"], "John");
    let s = pr::split(&jane, pid).unwrap();
    assert_eq!(s["total"].as_f64(), Some(42000.0));

    // Files that aren't shared projects are refused.
    assert!(share::peek(&jane, &json!({ "format": "something" })).is_err());
    assert!(share::peek(&jane, &json!({ "format": "vaulty-project", "version": 99, "share_id": "x" })).unwrap_err().to_string().contains("newer"));
}

#[test]
fn recording_the_split_in_lend_and_borrow() {
    use vaulty_core::people;
    let c = setup();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
    let p = pr::create(&c, &json!({ "name": "Goa trip", "kind": "group" })).unwrap();
    let id = p["id"].as_str().unwrap();
    let ms = pr::set_members(&c, id, &json!([{ "name": "Jane", "is_me": true }, { "name": "John" }, { "name": "Jenny" }])).unwrap();
    let john = ms[1]["id"].as_str().unwrap().to_string();
    let acct = accounts::create(&c, &json!({ "name": "Savings", "type": "savings", "currency": "INR", "initial_balance": "50000" })).unwrap();
    let food = categories::get_all(&c).unwrap().into_iter().find(|x| x["type"] == "expense").unwrap();
    // I paid ₹30,000 (recorded in my transactions); each share is ₹10,000.
    pr::add_payment(&c, id, &json!({ "date": "2026-05-01", "amount": "30000", "vendor": "Hotel", "mode": "card", "record": { "account_id": acct["id"], "category_id": food["id"] } })).unwrap();
    let existing = people::create(&c, &json!({ "name": "John Doe" })).unwrap();

    let pre = pr::settle_preview(&c, id).unwrap();
    assert_eq!(pre.iter().map(|r| (r["name"].as_str().unwrap(), r["change"].as_f64().unwrap())).collect::<Vec<_>>(), [("John", 10000.0), ("Jenny", 10000.0)]);
    let made = history::record(&c, "project.settle", |c| pr::record_settlement(c, id, &json!({ "category_id": food["id"], "people": { john.clone(): existing["id"] } }), today)).unwrap();
    assert_eq!(made, 2);
    let list = people::list(&c).unwrap();
    let owed = |n: &str| list.iter().find(|p| p["name"] == n).unwrap()["balances"][0]["amount"].as_f64();
    assert_eq!((owed("John Doe"), owed("Jenny")), (Some(10000.0), Some(10000.0)), "Jenny was added as a new person");
    // My food spending is now my share: 30,000 − 20,000.
    let spent: i64 = c.query_row("SELECT sum(p.amount_minor) FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id WHERE l.category_id = ?", [food["id"].as_str()], |r| r.get(0)).unwrap();
    assert_eq!(spent, 1_000_000);
    assert!(pr::settle_preview(&c, id).unwrap().iter().all(|r| r["change"].as_f64() == Some(0.0)), "nothing left to record");
    assert_eq!(pr::record_settlement(&c, id, &json!({}), today).unwrap(), 0);

    // John pays ₹6,000 for dinner: shares are now ₹12,000; John owes ₹6,000, Jenny ₹12,000.
    pr::add_payment(&c, id, &json!({ "date": "2026-05-03", "amount": "6000", "mode": "cash", "paid_by": john })).unwrap();
    let pre = pr::settle_preview(&c, id).unwrap();
    assert_eq!(pre.iter().map(|r| r["change"].as_f64().unwrap()).collect::<Vec<_>>(), [-4000.0, 2000.0]);
    history::record(&c, "project.settle", |c| pr::record_settlement(c, id, &json!({ "category_id": food["id"] }), today)).unwrap();
    let list = people::list(&c).unwrap();
    let owed = |n: &str| list.iter().find(|p| p["name"] == n).unwrap()["balances"][0]["amount"].as_f64();
    assert_eq!((owed("John Doe"), owed("Jenny")), (Some(6000.0), Some(12000.0)), "only the difference is added, to the same people");
    // Undo takes a recording back out.
    history::undo_last(&c).unwrap();
    assert_eq!(pr::settle_preview(&c, id).unwrap()[0]["change"].as_f64(), Some(-4000.0));
}

#[test]
fn warranties_become_documents_and_improvement_costs_by_year() {
    use vaulty_core::documents;
    let c = setup();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
    let p = pr::create(&c, &json!({ "name": "Kitchen renovation", "improvement": true })).unwrap();
    let id = p["id"].as_str().unwrap();
    assert_eq!(p["improvement"], 1);
    let it = pr::add_item(&c, id, &json!({ "date": "2026-03-20", "item": "Chimney 90 cm", "amount": "18000", "tax_percent": 18, "vendor": "Example Appliances",
        "bill_no": "EA-77", "warranty_until": "2031-03-19" })).unwrap();
    assert_eq!(it["warranty_until"], "2031-03-19");
    let doc_id = it["document_id"].as_str().unwrap().to_string();
    let doc = documents::get(&c, &doc_id, today).unwrap();
    assert_eq!((doc["title"].as_str(), doc["kind"].as_str(), doc["issuer"].as_str(), doc["number"].as_str(), doc["expires_on"].as_str()),
        (Some("Chimney 90 cm"), Some("warranty"), Some("Example Appliances"), Some("EA-77"), Some("2031-03-19")));
    assert!(doc["kept_at"].as_str().unwrap().contains("Kitchen renovation"));
    // Changing the date updates the same document; an impossible date is refused.
    let mut v = json!({ "date": "2026-03-20", "item": "Chimney 90 cm", "amount": "18000", "tax_percent": 18, "vendor": "Example Appliances", "warranty_until": "2033-03-19" });
    let it = pr::update_item(&c, it["id"].as_str().unwrap(), &v).unwrap();
    assert_eq!(it["document_id"].as_str(), Some(doc_id.as_str()));
    assert_eq!(documents::get(&c, &doc_id, today).unwrap()["expires_on"], "2033-03-19");
    v["warranty_until"] = json!("2025-01-01");
    assert!(pr::update_item(&c, it["id"].as_str().unwrap(), &v).is_err());
    assert_eq!(documents::list(&c, today).unwrap().len(), 1);

    pr::add_item(&c, id, &json!({ "date": "2026-04-02", "item": "Tiles", "amount": "40000" })).unwrap();
    let p = pr::get(&c, id).unwrap();
    assert_eq!(p["improvement_by_year"], json!([{ "fy": "2025-26", "amount": 21240.0 }, { "fy": "2026-27", "amount": 40000.0 }]));
    let plain = pr::create(&c, &json!({ "name": "Trip" })).unwrap();
    assert_eq!(pr::get(&c, plain["id"].as_str().unwrap()).unwrap()["improvement_by_year"], json!([]));
}
