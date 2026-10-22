use chrono::NaiveDate;
use serde_json::json;
use vaulty_core::{accounts, db, recurring, report_export, settings, transactions};

#[test]
fn workbook_has_the_report_in_the_members_words() {
    let c = db::open_in_memory().unwrap();
    settings::update(&c, &json!({ "currency": "INR" })).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
    let bank = accounts::create(&c, &json!({ "name": "Bank", "type": "checking", "currency": "INR", "initial_balance": "50000" })).unwrap();
    let food = vaulty_core::categories::get_all(&c).unwrap().into_iter().find(|x| x["type"] == "expense").unwrap();
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "expense", "amount": "1250.50", "date": "2026-10-05", "category_id": food["id"], "merchant": "Swiggy" })).unwrap();
    transactions::create(&c, &json!({ "account_id": bank["id"], "type": "income", "amount": "90000", "date": "2026-10-01" })).unwrap();
    recurring::create(&c, &json!({ "name": "House rent", "type": "expense", "amount": "28000", "account_id": bank["id"], "frequency": "monthly", "start_date": "2026-11-01" }), today).unwrap();

    let bytes = report_export::to_xlsx(&c, "2026-10-01", "2026-10-31", today, &json!({ "summary": "सारांश", "months": "By/month: [x]" })).unwrap();
    assert_eq!(&bytes[..2], b"PK", "an xlsx is a zip");

    // Read it back as the import code would.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.xlsx");
    std::fs::write(&path, &bytes).unwrap();
    use calamine::Reader;
    let mut wb = calamine::open_workbook_auto(&path).unwrap();
    assert_eq!(wb.sheet_names(), ["सारांश", "Bymonth x", "Forecast"]);
    let summary = wb.worksheet_range("सारांश").unwrap();
    let cells: Vec<String> = summary.cells().map(|(_, _, v)| v.to_string()).collect();
    for want in ["Income", "90000", "1250.5", "Swiggy", "2026-10-01 – 2026-10-31"] {
        assert!(cells.iter().any(|c| c.contains(want)), "missing {want} in {cells:?}");
    }
    let forecast = wb.worksheet_range("Forecast").unwrap();
    assert!(forecast.cells().any(|(_, _, v)| matches!(v, calamine::Data::String(s) if s == "House rent")));
    assert_eq!(forecast.height(), 4 + 91, "today plus 90 days");
}
