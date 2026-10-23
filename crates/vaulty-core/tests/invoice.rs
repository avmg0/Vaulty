use chrono::NaiveDate;
use vaulty_core::invoice;

fn read(name: &str) -> invoice::Draft {
    let bytes = std::fs::read(format!("{}/tests/fixtures/invoices/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    invoice::read_pdf(&bytes, None).unwrap()
}
type Row<'a> = (&'a str, Option<&'a str>, Option<&'a str>, Option<&'a str>, &'a str, Option<&'a str>);
fn d(s: &str) -> Option<NaiveDate> { NaiveDate::parse_from_str(s, "%Y-%m-%d").ok() }

#[test]
fn gst_invoice_with_cgst_and_sgst() {
    let b = read("gst-cgst.pdf");
    assert_eq!((b.vendor.as_deref(), b.gstin.as_deref(), b.bill_no.as_deref(), b.date), (Some("Jimmy Doe Electricals"), Some("27ABCDE1234F1Z5"), Some("JDE/2026-27/0114"), d("2026-10-02")));
    let rows: Vec<Row> = b.items.iter()
        .map(|i| (i.item.as_str(), i.qty.as_deref(), i.unit.as_deref(), i.rate.as_deref(), i.amount.as_str(), i.hsn.as_deref())).collect();
    assert_eq!(rows, [
        ("Copper wire 2.5 sq mm FR", Some("4"), Some("Coil"), Some("1850.50"), "7402.00", Some("8544")),
        ("Modular switch 6A", Some("24"), Some("Pcs"), Some("62.00"), "1488.00", Some("8536")),
        ("MCB 32A double pole", Some("2"), Some("Nos"), Some("1145.00"), "2290.00", Some("8536")),
        ("LED panel 15W round", Some("12"), Some("Pcs"), Some("450.00"), "5400.00", Some("9405")),
    ]);
    assert!(b.items.iter().all(|i| i.tax_percent.as_deref() == Some("18")));
    assert_eq!((b.taxable.as_deref(), b.tax.as_deref(), b.tax_rate.as_deref(), b.total.as_deref()), (Some("16580.00"), Some("2984.40"), Some("18"), Some("19564.40")));
    assert_eq!((b.items_total.as_str(), b.matches), ("19564.40", Some(true)));
}

#[test]
fn inter_state_invoice_with_igst() {
    let b = read("gst-igst.pdf");
    assert_eq!(b.items.len(), 4);
    assert_eq!((b.tax_rate.as_deref(), b.tax.as_deref(), b.matches), (Some("18"), Some("2984.40"), Some(true)));
    assert_eq!(b.gstin.as_deref(), Some("27ABCDE1234F1Z5"), "the seller's, which comes first");
}

#[test]
fn shop_bill_without_gst() {
    let b = read("shop.pdf");
    assert_eq!((b.vendor.as_deref(), b.bill_no.as_deref(), b.date, b.gstin), (Some("SHARMA HARDWARE STORE"), Some("4471"), d("2026-10-06"), None));
    assert_eq!(b.items.iter().map(|i| (i.item.as_str(), i.amount.as_str())).collect::<Vec<_>>(),
        [("Wall putty 20kg", "2340.00"), ("Primer 4L", "2300.00"), ("Sandpaper sheet", "250.00"), ("Brush 4 inch", "290.00")]);
    assert!(b.items.iter().all(|i| i.tax_percent.is_none()));
    assert_eq!((b.total.as_deref(), b.matches), (Some("5180.00"), Some(true)));
}

#[test]
fn lines_that_dont_add_up_are_flagged() {
    let lines: Vec<String> = ["Example Traders", "Invoice No: 77", "Description  Qty  Rate  Amount", "Labour charges  3,500.00", "Paint  2  900.00  1,800.00", "Total  6,000.00"]
        .iter().map(|s| s.to_string()).collect();
    let b = invoice::parse(&lines);
    assert_eq!(b.items.len(), 2, "a description with an amount counts inside the item table");
    assert_eq!((b.items[0].qty.as_deref(), b.items[0].amount.as_str()), (None, "3500.00"));
    assert_eq!((b.items_total.as_str(), b.matches), ("5300.00", Some(false)));
    assert_eq!(invoice::parse(&[]).matches, None);
}
