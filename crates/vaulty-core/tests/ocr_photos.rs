//! Photos of bills read by Tesseract (when it's installed, as on Linux), laid out into lines and
//! read like a PDF bill. The same layout code serves Apple's and Windows' recognition.
use vaulty_core::{invoice, ocr};

fn tesseract(name: &str) -> Option<Vec<ocr::Piece>> {
    let path = format!("{}/tests/fixtures/invoices/{name}", env!("CARGO_MANIFEST_DIR"));
    let out = std::process::Command::new("tesseract").args([path.as_str(), "stdout", "--psm", "6", "tsv"]).output().ok()?;
    out.status.success().then(|| ocr::from_tesseract_tsv(&String::from_utf8_lossy(&out.stdout)))
}

#[test]
fn shop_bill_photo() {
    let Some(pieces) = tesseract("shop-photo.png") else { eprintln!("tesseract isn't installed; skipped"); return };
    let b = invoice::parse(&ocr::lines(&pieces));
    assert_eq!((b.vendor.as_deref(), b.bill_no.as_deref(), b.total.as_deref()), (Some("SHARMA HARDWARE STORE"), Some("4471"), Some("5180.00")));
    assert_eq!(b.items.iter().map(|i| (i.item.as_str(), i.qty.as_deref(), i.amount.as_str())).collect::<Vec<_>>(),
        [("Wall putty 20kg", Some("3"), "2340.00"), ("Primer 4L", Some("2"), "2300.00"), ("Sandpaper sheet", Some("20"), "250.00"), ("Brush 4 inch", Some("2"), "290.00")]);
    assert_eq!(b.matches, Some(true));
}

#[test]
fn gst_invoice_photo() {
    let Some(pieces) = tesseract("gst-photo.jpg") else { eprintln!("tesseract isn't installed; skipped"); return };
    let lines = ocr::lines(&pieces);
    let b = invoice::parse(&lines);
    assert_eq!((b.gstin.as_deref(), b.bill_no.as_deref(), b.total.as_deref(), b.tax_rate.as_deref()), (Some("27ABCDE1234F1Z5"), Some("JDE/2026-27/0114"), Some("19564.40"), Some("18")), "{lines:#?}");
    assert_eq!(b.items.len(), 4, "{lines:#?}");
    assert_eq!(b.matches, Some(true), "{lines:#?}");
}
