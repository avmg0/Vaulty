use serde_json::json;
use vaulty_core::{accounts, db, statements::{self, DateOrder}, transactions};

const INDIAN_BANK: &str = "\u{feff}Statement of account,,,,,,
Name: JOHN DOE,,,,,,
Account No: XXXXXXXX1234,,,,,,
,,,,,,
Date,Narration,Chq./Ref.No.,Value Dt,Withdrawal Amt.,Deposit Amt.,Closing Balance
01/10/26,SALARY OCT 2026 ACME PVT LTD,NEFT123,01/10/26,,\"85,000.00\",\"1,35,000.00\"
03/10/26,UPI/412345678901/SWIGGY/swiggy@icici/Food,412345678901,03/10/26,320.00,,\"1,34,680.00\"
15/10/26,UPI/412345678902/BESCOM ELECTRICITY/bescom@ybl,412345678902,15/10/26,\"1,850.00\",,\"1,32,830.00\"
15/10/26,UPI/412345678903/SWIGGY/swiggy@icici,412345678903,15/10/26,320.00,,\"1,32,510.00\"
,,,,,,
Statement Summary :-,,,,,,
";

const CARD: &str = "Transaction Date;Details;Amount;Dr/Cr
2026-10-04;AMAZON PAY INDIA;1,299.00;Dr
2026-10-09;REFUND AMAZON;299.00;Cr
2026-10-12;PETROL PUMP 0042;2,000.00;Dr
";

const US: &str = "Date,Description,Amount
10/13/2026,Coffee Shop,-4.50
10/14/2026,Payroll,2500.00
10/15/2026,Grocery Store,-82.10
";

fn setup() -> (rusqlite::Connection, String) {
    let c = db::open_in_memory().unwrap();
    let a = accounts::create(&c, &json!({ "name": "Savings", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    (c, a["id"].as_str().unwrap().to_string())
}

#[test]
fn indian_bank_csv_with_preamble_and_duplicates() {
    let (c, acc) = setup();
    // One of the two Swiggy orders is already recorded, a day off.
    transactions::create(&c, &json!({ "account_id": acc, "type": "expense", "amount": "320", "date": "2026-10-02", "merchant": "Swiggy" })).unwrap();
    let t = statements::read(INDIAN_BANK.as_bytes(), "statement.csv").unwrap();
    assert_eq!(t.header_row, 3, "blank lines are dropped");
    let m = statements::guess(&t, DateOrder::Mdy).unwrap();
    assert_eq!((m.date, m.description, m.debit, m.credit, m.amount, m.reference), (0, 1, Some(4), Some(5), None, Some(2)));
    assert_eq!(m.date_order, DateOrder::Dmy, "15/10 can only be day-first");
    let rows = statements::preview(&c, &t, &m, &acc).unwrap();
    let new: Vec<_> = rows.iter().filter(|r| r.status == "new").collect();
    let dup: Vec<_> = rows.iter().filter(|r| r.status == "duplicate").collect();
    assert_eq!(new.len(), 3);
    assert_eq!(dup.len(), 1, "only one Swiggy matches the one already recorded");
    assert_eq!(dup[0].date.as_deref(), Some("2026-10-03"));
    let salary = rows.iter().find(|r| r.description.starts_with("SALARY")).unwrap();
    assert_eq!((salary.kind, salary.amount.as_deref(), salary.date.as_deref()), (Some("income"), Some("85000.00"), Some("2026-10-01")));
    assert_eq!(rows.iter().find(|r| r.description.contains("BESCOM")).unwrap().merchant, "BESCOM ELECTRICITY");
    assert!(rows.iter().any(|r| r.status == "skipped"), "summary line skipped");

    // Import everything new except the electricity bill.
    let bescom = rows.iter().find(|r| r.description.contains("BESCOM")).unwrap().index;
    let added = statements::import(&c, &t, &m, &acc, &[bescom], &[]).unwrap();
    assert_eq!(added, 2);
    let bal = accounts::get_all(&c).unwrap()[0]["current_balance"].as_f64().unwrap();
    assert_eq!(bal, 50000.0 - 320.0 + 85000.0 - 320.0);
    // Importing the same file again finds them all.
    let again = statements::preview(&c, &t, &m, &acc).unwrap();
    assert_eq!(again.iter().filter(|r| r.status == "new").count(), 1, "only the skipped bill is still new");
    // The mapping is remembered for this account.
    assert!(statements::saved_mapping(&c, &acc, &t).unwrap().is_some());
}

#[test]
fn card_statement_with_dr_cr_column_and_us_style() {
    let (c, acc) = setup();
    let t = statements::read(CARD.as_bytes(), "card.csv").unwrap();
    let m = statements::guess(&t, DateOrder::Dmy).unwrap();
    assert_eq!((m.amount, m.side), (Some(2), Some(3)));
    let rows = statements::preview(&c, &t, &m, &acc).unwrap();
    assert_eq!(rows.iter().map(|r| r.kind.unwrap()).collect::<Vec<_>>(), ["expense", "income", "expense"]);
    assert_eq!(rows[0].amount.as_deref(), Some("1299.00"));

    let t = statements::read(US.as_bytes(), "us.csv").unwrap();
    let m = statements::guess(&t, DateOrder::Dmy).unwrap();
    assert_eq!(m.date_order, DateOrder::Mdy);
    let rows = statements::preview(&c, &t, &m, &acc).unwrap();
    assert_eq!(rows.iter().map(|r| (r.kind.unwrap(), r.date.clone().unwrap())).collect::<Vec<_>>(),
        [("expense", "2026-10-13".into()), ("income", "2026-10-14".into()), ("expense", "2026-10-15".into())]);
}

#[test]
fn excel_statement() {
    let (c, acc) = setup();
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/statement.xlsx")).unwrap();
    let t = statements::read(&bytes, "Statement.XLSX").unwrap();
    assert_eq!(t.header_row, 2, "blank row dropped, two lines of preamble");
    let m = statements::guess(&t, DateOrder::Dmy).unwrap();
    let rows = statements::preview(&c, &t, &m, &acc).unwrap();
    let new: Vec<_> = rows.iter().filter(|r| r.status == "new").map(|r| (r.date.clone().unwrap(), r.kind.unwrap(), r.amount.clone().unwrap(), r.merchant.clone())).collect();
    assert_eq!(new, [("2026-10-01".to_string(), "income", "85000".to_string(), "Salary October".to_string()),
                     ("2026-10-03".to_string(), "expense", "245.5".to_string(), "Corner Cafe".to_string()),
                     ("2026-10-05".to_string(), "expense", "22000".to_string(), "Rent".to_string())]);
}

#[test]
fn parsing_helpers() {
    use statements::{parse_amount, parse_date};
    assert_eq!(parse_amount("1,23,456.78"), Some((false, "123456.78".into())));
    assert_eq!(parse_amount("(1,234.00)"), Some((true, "1234.00".into())));
    assert_eq!(parse_amount("500.00 Dr"), Some((true, "500.00".into())));
    assert_eq!(parse_amount("₹ 1,000 CR"), Some((false, "1000".into())));
    assert_eq!(parse_amount("1.234,56"), Some((false, "1234.56".into())));
    assert_eq!(parse_amount("-"), None);
    assert_eq!(parse_amount("abc"), None);
    // Dates, references and other text in an amount column are not amounts.
    assert_eq!(parse_amount("10-10"), None);
    assert_eq!(parse_amount("0-0.001"), None);
    assert_eq!(parse_amount("12/500"), None);
    assert_eq!(parse_amount("Rs. 500"), Some((false, "500".into())));
    assert_eq!(parse_amount("INR 1,250.50 Dr."), Some((true, "1250.50".into())));
    assert_eq!(parse_amount("500-"), Some((true, "500".into())));
    assert_eq!(parse_amount("+75.25"), Some((false, "75.25".into())));
    let d = |s, o| parse_date(s, o).map(|d| d.to_string());
    assert_eq!(d("15-Oct-2026", DateOrder::Mdy).as_deref(), Some("2026-10-15"));
    assert_eq!(d("Oct 15, 2026", DateOrder::Dmy).as_deref(), Some("2026-10-15"));
    assert_eq!(d("2026-10-15 10:22:01", DateOrder::Dmy).as_deref(), Some("2026-10-15"));
    assert_eq!(d("05/10/2026", DateOrder::Mdy).as_deref(), Some("2026-05-10"));
    assert_eq!(d("31/02/2026", DateOrder::Dmy), None);
    assert_eq!(d("1O/10/26", DateOrder::Dmy), None, "letter O typed for zero");
    assert_eq!(d("15 October 2026", DateOrder::Mdy).as_deref(), Some("2026-10-15"));
    assert_eq!(statements::merchant_from("POS 4242 XXXX CORNER CAFE"), "CORNER CAFE");
    assert_eq!(statements::merchant_from("UPI/412345678901/SWIGGY/swiggy@icici/Food"), "SWIGGY");
    assert!(statements::read(&vec![b'a'; statements::MAX_BYTES + 1], "big.csv").is_err());
}

#[test]
fn pdf_bank_statement_with_password() {
    let (c, acc) = setup();
    let pdf = std::fs::read(format!("{}/tests/fixtures/statements/bank-aes256.pdf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(statements::read(&pdf, "statement.pdf").unwrap_err().to_string().contains("needs its password"));
    assert!(statements::read_pdf(&pdf, Some("wrong")).is_err());
    let t = statements::read_pdf(&pdf, Some("Jane1990")).unwrap();
    assert_eq!(t.headers(), ["Date", "Narration", "Chq./Ref.No.", "Value Dt", "Withdrawal Amt.", "Deposit Amt.", "Closing Balance"]);
    assert_eq!(t.rows.len() - 1, 36, "every transaction, across the page break, without the repeated header");
    // A narration over two lines is one cell; a deposit leaves the withdrawal empty.
    assert_eq!(t.rows[1][1], "UPI-SWIGGY-swiggy@icici-ICIC0000103-627412345678-Payment for order");
    assert_eq!((t.rows[2][4].as_str(), t.rows[2][5].as_str()), ("", "85,000.00"));
    let m = statements::guess(&t, DateOrder::Mdy).unwrap();
    assert_eq!((m.date, m.description, m.debit, m.credit, m.reference, m.date_order), (0, 1, Some(4), Some(5), Some(2), DateOrder::Dmy));
    let rows = statements::preview(&c, &t, &m, &acc).unwrap();
    assert_eq!(rows.iter().filter(|r| r.status == "new").count(), 36);
    let salary = rows.iter().find(|r| r.description.starts_with("SALARY")).unwrap();
    assert_eq!((salary.kind, salary.amount.as_deref(), salary.date.as_deref()), (Some("income"), Some("85000.00"), Some("2026-10-01")));
    let wiring = rows.iter().find(|r| r.description.contains("Wiring advance")).unwrap();
    assert_eq!((wiring.kind, wiring.amount.as_deref()), (Some("expense"), Some("15000.00")));
    assert!(!rows.iter().any(|r| r.description.contains("Page")), "footers aren't rows or part of one");
    let last = rows.last().unwrap();
    assert_eq!((last.kind, last.amount.as_deref(), last.date.as_deref()), (Some("income"), Some("12000.00"), Some("2026-10-29")));
}

#[test]
fn damaged_pdfs_are_refused_without_crashing() {
    let pdf = std::fs::read(format!("{}/tests/fixtures/statements/bank-aes256.pdf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    for cut in [10, 200, pdf.len() / 2, pdf.len() - 30] { assert!(statements::read_pdf(&pdf[..cut], Some("Jane1990")).is_err()); }
    let mut flipped = pdf.clone();
    for i in (400..flipped.len()).step_by(97) { flipped[i] ^= 0x5A; }
    let _ = statements::read_pdf(&flipped, Some("Jane1990"));
    assert!(statements::read(b"%PDF-1.4 nonsense", "x.pdf").is_err());
}
