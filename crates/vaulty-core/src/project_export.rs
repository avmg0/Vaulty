//! A project as an Excel workbook: a summary (budget, billed, paid, owed, by section and by
//! person), the bill items with the project's own columns, and the payments. Totals are Excel
//! formulas, so rows added later in Excel are counted too.
//!
//! Labels come from the interface in the member's language; any left out are in English.
use crate::{error::{Error, Result}, projects};
use rust_xlsxwriter::{Format, FormatAlign, Formula, Workbook, XlsxError};
use serde_json::Value;

const DEFAULTS: &[(&str, &str)] = &[
    ("summary", "Summary"), ("items", "Bill items"), ("payments", "Payments"), ("sections", "By section"), ("people", "By person"),
    ("budget", "Budget"), ("billed", "Billed"), ("paid", "Paid"), ("owed", "Still owed"), ("balance", "Balance"), ("tax", "Tax"),
    ("currency", "Currency"), ("status", "Status"), ("period", "Dates"), ("no", "S.No."), ("date", "Date"), ("section", "Section"),
    ("item", "Item"), ("kind", "Kind"), ("qty", "Qty"), ("unit", "Unit"), ("rate", "Rate"), ("amount", "Amount"), ("total", "Total"),
    ("vendor", "Paid to / bought from"), ("bill_no", "Bill no."), ("notes", "Notes"), ("paidTo", "Paid to"), ("accountName", "Name on account"),
    ("mode", "Mode"), ("improvement", "Cost of improvement, by financial year"), ("fy", "Financial year"), ("warranty", "Warranty until"), ("reference", "Reference"), ("fromAccount", "Paid from"), ("none", "No section"), ("name", "Name"), ("yes", "Yes"), ("no_", "No"),
];

struct Labels<'a>(&'a Value);
impl Labels<'_> {
    fn get(&self, key: &str) -> String {
        let fallback = DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_string()).unwrap_or_else(|| key.rsplit('.').next().unwrap_or(key).replace('_', " "));
        self.0.get(key).and_then(Value::as_str).map(|s| s.trim().chars().take(200).collect::<String>()).filter(|s| !s.is_empty()).unwrap_or(fallback)
    }
    fn sheet(&self, key: &str) -> String {
        let s: String = self.get(key).chars().filter(|c| !"[]:*?/\\'".contains(*c)).take(31).collect();
        if s.trim().is_empty() { key.to_string() } else { s }
    }
}

fn xl(_: XlsxError) -> Error { Error::Invalid("the spreadsheet couldn't be written".into()) }

/// Column letters for formulas: 0 → A, 26 → AA.
fn col(mut i: u16) -> String {
    let mut s = String::new();
    loop { s.insert(0, (b'A' + (i % 26) as u8) as char); if i < 26 { break; } i = i / 26 - 1; }
    s
}

fn num(v: &Value) -> Option<f64> { v.as_f64() }
fn st(v: &Value) -> &str { v.as_str().unwrap_or("") }

pub fn to_xlsx(conn: &rusqlite::Connection, project_id: &str, labels: &Value) -> Result<Vec<u8>> {
    let p = projects::get(conn, project_id)?;
    let items = projects::items(conn, project_id)?;
    let pays = projects::payments(conn, project_id)?;
    let l = Labels(labels);
    let cur = st(&p["currency"]).to_string();
    let head = Format::new().set_bold().set_background_color("#E8EEF7").set_align(FormatAlign::Left).set_text_wrap();
    let money = Format::new().set_num_format("#,##0.00");
    let bold_money = Format::new().set_bold().set_num_format("#,##0.00");
    let bold = Format::new().set_bold();
    let qty = Format::new().set_num_format("#,##0.####");
    let hidden: Vec<&str> = p["hidden"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let shows = |c: &str| !hidden.contains(&c);
    // The project's own name for a standard column, else the interface's.
    let own = |key: &str| p["labels"][key].as_str().map(str::to_string).unwrap_or_else(|| l.get(key));
    let mut wb = Workbook::new();

    // Summary.
    (|| -> std::result::Result<(), XlsxError> {
        let ws = wb.add_worksheet().set_name(l.sheet("summary"))?;
        ws.set_column_width(0, 34)?; for c in 1..=4 { ws.set_column_width(c, 16)?; }
        ws.write_string_with_format(0, 0, st(&p["name"]), &Format::new().set_bold().set_font_size(16))?;
        let mut row = 1;
        ws.write_string(row, 0, format!("{}: {}", l.get("currency"), cur))?; row += 1;
        ws.write_string(row, 0, format!("{}: {}", l.get("status"), l.get(&format!("status.{}", st(&p["status"])))))?; row += 1;
        if p["start_date"].is_string() || p["end_date"].is_string() {
            ws.write_string(row, 0, format!("{}: {} – {}", l.get("period"), st(&p["start_date"]), st(&p["end_date"])))?; row += 1;
        }
        row += 1;
        for key in ["budget", "billed", "tax", "paid", "owed"] {
            if let Some(v) = num(&p[key]) {
                ws.write_string_with_format(row, 0, l.get(key), &head)?;
                ws.write_number_with_format(row, 1, v, &bold_money)?;
                row += 1;
            }
        }
        if let Some(years) = p["improvement_by_year"].as_array().filter(|y| !y.is_empty()) {
            row += 1;
            ws.write_string_with_format(row, 0, l.get("improvement"), &bold)?; row += 1;
            for (i, h) in [l.get("fy"), l.get("billed")].iter().enumerate() { ws.write_string_with_format(row, i as u16, h, &head)?; }
            row += 1;
            for y in years {
                ws.write_string(row, 0, st(&y["fy"]))?;
                ws.write_number_with_format(row, 1, num(&y["amount"]).unwrap_or(0.0), &money)?;
                row += 1;
            }
        }
        row += 1;
        ws.write_string_with_format(row, 0, l.get("sections"), &bold)?; row += 1;
        for (i, h) in [l.get("section"), l.get("budget"), l.get("billed"), l.get("paid")].iter().enumerate() { ws.write_string_with_format(row, i as u16, h, &head)?; }
        row += 1;
        for s in p["sections"].as_array().into_iter().flatten() {
            ws.write_string(row, 0, st(&s["name"]))?;
            if let Some(b) = num(&s["budget"]) { ws.write_number_with_format(row, 1, b, &money)?; }
            ws.write_number_with_format(row, 2, num(&s["billed"]).unwrap_or(0.0), &money)?;
            ws.write_number_with_format(row, 3, num(&s["paid"]).unwrap_or(0.0), &money)?;
            row += 1;
        }
        if p["unsectioned"]["items"].as_i64().unwrap_or(0) > 0 {
            ws.write_string(row, 0, l.get("none"))?;
            ws.write_number_with_format(row, 2, num(&p["unsectioned"]["billed"]).unwrap_or(0.0), &money)?;
            row += 1;
        }
        row += 1;
        let people = p["vendors"].as_array().cloned().unwrap_or_default();
        if !people.is_empty() {
            ws.write_string_with_format(row, 0, l.get("people"), &bold)?; row += 1;
            for (i, h) in [l.get("name"), l.get("billed"), l.get("paid"), l.get("balance")].iter().enumerate() { ws.write_string_with_format(row, i as u16, h, &head)?; }
            row += 1;
            for v in people {
                ws.write_string(row, 0, st(&v["name"]))?;
                for (c, k) in [(1, "billed"), (2, "paid"), (3, "balance")] { ws.write_number_with_format(row, c, num(&v[k]).unwrap_or(0.0), &money)?; }
                row += 1;
            }
        }
        Ok(())
    })().map_err(xl)?;

    // Bill items.
    (|| -> std::result::Result<(), XlsxError> {
        let ws = wb.add_worksheet().set_name(l.sheet("items"))?;
        let custom = p["columns"].as_array().cloned().unwrap_or_default();
        // (heading, width, kind) — kind says how to write the cell and whether to total it.
        let mut cols: Vec<(String, f64, &str)> = vec![(l.get("no"), 6.0, "no"), (own("date"), 12.0, "date")];
        if shows("section") { cols.push((own("section"), 18.0, "section")); }
        cols.push((own("item"), 32.0, "item"));
        if shows("kind") { cols.push((own("kind"), 12.0, "kind")); }
        if shows("qty") { cols.push((own("qty"), 9.0, "qty")); cols.push((own("unit"), 8.0, "unit")); }
        if shows("rate") { cols.push((format!("{} ({cur})", own("rate")), 13.0, "rate")); }
        cols.push((format!("{} ({cur})", own("amount")), 14.0, "amount"));
        if shows("tax") { cols.push((format!("{} ({cur})", own("tax")), 12.0, "tax")); cols.push((format!("{} ({cur})", own("total")), 14.0, "total")); }
        if shows("vendor") { cols.push((own("vendor"), 24.0, "vendor")); }
        if shows("bill_no") { cols.push((own("bill_no"), 12.0, "bill_no")); }
        for c in &custom { cols.push((st(&c["name"]).to_string(), 14.0, "custom")); }
        if shows("notes") { cols.push((own("notes"), 30.0, "notes")); }
        if items.iter().any(|i| i["warranty_until"].is_string()) { cols.push((l.get("warranty"), 13.0, "warranty")); }
        for (i, (h, w, _)) in cols.iter().enumerate() {
            ws.set_column_width(i as u16, *w)?;
            ws.write_string_with_format(0, i as u16, h, &head)?;
        }
        let mut totals: Vec<u16> = vec![];
        for (n, it) in items.iter().enumerate() {
            let r = n as u32 + 1;
            let mut ci = 0usize;
            for (i, (_, _, kind)) in cols.iter().enumerate() {
                let c = i as u16;
                match *kind {
                    "no" => { ws.write_number(r, c, (n + 1) as f64)?; }
                    "date" => { ws.write_string(r, c, st(&it["date"]))?; }
                    "section" => { ws.write_string(r, c, st(&it["section_name"]))?; }
                    "item" => { ws.write_string(r, c, st(&it["item"]))?; }
                    "kind" => { ws.write_string(r, c, l.get(&format!("itemKind.{}", st(&it["kind"]))))?; }
                    "qty" => { if let Some(q) = num(&it["qty"]) { ws.write_number_with_format(r, c, q, &qty)?; } }
                    "unit" => { ws.write_string(r, c, st(&it["unit"]))?; }
                    "rate" => { if let Some(x) = num(&it["rate"]) { ws.write_number_with_format(r, c, x, &money)?; } }
                    "amount" | "tax" | "total" => {
                        ws.write_number_with_format(r, c, num(&it[*kind]).unwrap_or(0.0), &money)?;
                        if n == 0 { totals.push(c); }
                    }
                    "vendor" => { ws.write_string(r, c, st(&it["vendor"]))?; }
                    "bill_no" => { ws.write_string(r, c, st(&it["bill_no"]))?; }
                    "notes" => { ws.write_string(r, c, st(&it["notes"]))?; }
                    "warranty" => { ws.write_string(r, c, st(&it["warranty_until"]))?; }
                    _ => {
                        let def = &custom[ci];
                        ci += 1;
                        let v = &it["custom"][st(&def["id"])];
                        match (st(&def["type"]), v) {
                            ("number", Value::Number(x)) => {
                                ws.write_number(r, c, x.as_f64().unwrap_or(0.0))?;
                                if n == 0 { totals.push(c); }
                            }
                            ("yes_no", Value::Bool(b)) => { ws.write_string(r, c, if *b { l.get("yes") } else { l.get("no_") })?; }
                            (_, Value::String(s)) => { ws.write_string(r, c, s)?; }
                            _ => {}
                        }
                    }
                }
            }
        }
        // Number columns of the project's own are totalled even if the first row left them empty.
        for (i, (_, _, kind)) in cols.iter().enumerate() {
            if *kind == "custom" {
                let def = &custom[cols[..i].iter().filter(|c| c.2 == "custom").count()];
                if st(&def["type"]) == "number" && !totals.contains(&(i as u16)) { totals.push(i as u16); }
            }
        }
        let last = items.len() as u32 + 1;
        ws.write_string_with_format(last, cols.iter().position(|c| c.2 == "item").unwrap_or(1) as u16, l.get("total"), &bold)?;
        if !items.is_empty() {
            for c in totals {
                let f = Formula::new(format!("=SUM({0}2:{0}{1})", col(c), last));
                ws.write_formula_with_format(last, c, f, &bold_money)?;
            }
        }
        ws.set_freeze_panes(1, 0)?;
        if !items.is_empty() { ws.autofilter(0, 0, last - 1, cols.len() as u16 - 1)?; }
        Ok(())
    })().map_err(xl)?;

    // Payments.
    (|| -> std::result::Result<(), XlsxError> {
        let ws = wb.add_worksheet().set_name(l.sheet("payments"))?;
        let cols = [(l.get("no"), 6.0), (l.get("date"), 12.0), (l.get("paidTo"), 24.0), (l.get("accountName"), 24.0), (l.get("mode"), 10.0),
            (l.get("section"), 18.0), (format!("{} ({cur})", l.get("amount")), 14.0), (l.get("reference"), 18.0), (l.get("fromAccount"), 20.0), (l.get("notes"), 30.0)];
        for (i, (h, w)) in cols.iter().enumerate() { ws.set_column_width(i as u16, *w)?; ws.write_string_with_format(0, i as u16, h, &head)?; }
        for (n, y) in pays.iter().enumerate() {
            let r = n as u32 + 1;
            ws.write_number(r, 0, (n + 1) as f64)?;
            ws.write_string(r, 1, st(&y["date"]))?;
            ws.write_string(r, 2, st(&y["vendor"]))?;
            ws.write_string(r, 3, st(&y["account_name"]))?;
            ws.write_string(r, 4, l.get(&format!("mode.{}", st(&y["mode"]))))?;
            ws.write_string(r, 5, st(&y["section_name"]))?;
            ws.write_number_with_format(r, 6, num(&y["amount"]).unwrap_or(0.0), &money)?;
            ws.write_string(r, 7, st(&y["reference"]))?;
            ws.write_string(r, 8, st(&y["from_account"]))?;
            ws.write_string(r, 9, st(&y["notes"]))?;
        }
        let last = pays.len() as u32 + 1;
        ws.write_string_with_format(last, 2, l.get("total"), &bold)?;
        if !pays.is_empty() { ws.write_formula_with_format(last, 6, Formula::new(format!("=SUM(G2:G{last})")), &bold_money)?; }
        ws.set_freeze_panes(1, 0)?;
        Ok(())
    })().map_err(xl)?;

    wb.save_to_buffer().map_err(xl)
}

#[cfg(test)]
mod tests {
    #[test]
    fn column_letters() {
        assert_eq!([0, 25, 26, 27, 701, 702].map(super::col), ["A", "Z", "AA", "AB", "ZZ", "AAA"]);
    }
}
