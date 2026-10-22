//! The Reports page as an Excel workbook: a summary (totals, spending by category, income
//! sources, top places), income and spending by month, and the cash-flow forecast.
//!
//! Labels come from the interface in the member's language; any left out are in English.
//! Amounts are written as numbers (so they can be added up in Excel), in the main currency.
use crate::{error::{Error, Result}, reports};
use chrono::NaiveDate;
use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet, XlsxError};
use serde_json::Value;

/// How far ahead the forecast sheet goes.
pub const FORECAST_DAYS: i64 = 90;

const DEFAULTS: &[(&str, &str)] = &[
    ("title", "Vaulty report"), ("summary", "Summary"), ("months", "By month"), ("forecast", "Forecast"),
    ("period", "Period"), ("currency", "Currency"), ("income", "Income"), ("spending", "Spending"), ("net", "Net"),
    ("savingsRate", "Saved"), ("categories", "Spending by category"), ("category", "Category"), ("amount", "Amount"),
    ("count", "Transactions"), ("share", "Share"), ("incomeSources", "Income by source"), ("source", "Source"),
    ("places", "Where the money went"), ("place", "Place"), ("month", "Month"), ("date", "Date"),
    ("scheduled", "Balance with scheduled payments"), ("expected", "Expected, with typical spending"),
    ("balance", "Everyday money now"), ("typicalDay", "Typical day's spending"), ("comingUp", "Coming up"),
    ("item", "Item"), ("uncategorised", "Uncategorised"), ("hidden", "Hidden"),
    ("otherCurrencies", "Transactions in other currencies, not included"),
];

struct Labels<'a>(&'a Value);
impl Labels<'_> {
    fn get(&self, key: &str) -> String {
        let fallback = DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(key);
        self.0.get(key).and_then(Value::as_str).map(|s| s.trim().chars().take(200).collect::<String>())
            .filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
    }
    /// Sheet names: at most 31 characters, none of `[]:*?/\`.
    fn sheet(&self, key: &str) -> String {
        let s: String = self.get(key).chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
        if s.trim().is_empty() { key.to_string() } else { s }
    }
    fn name(&self, n: &str) -> String {
        match n { "\u{0}uncategorised" => self.get("uncategorised"), "\u{0}hidden" => self.get("hidden"), _ => n.to_string() }
    }
}

struct Styles { title: Format, head: Format, money: Format, pct: Format, bold_money: Format }

fn xl(_: XlsxError) -> Error { Error::Invalid("the spreadsheet couldn't be written".into()) }

fn table(ws: &mut Worksheet, s: &Styles, row: &mut u32, heading: &str, cols: &[String], rows: &[(String, f64, Option<i64>, Option<f64>)]) -> std::result::Result<(), XlsxError> {
    if rows.is_empty() { return Ok(()); }
    ws.write_string_with_format(*row, 0, heading, &s.title)?;
    *row += 1;
    for (i, c) in cols.iter().enumerate() { ws.write_string_with_format(*row, i as u16, c, &s.head)?; }
    *row += 1;
    for (name, amount, count, share) in rows {
        ws.write_string(*row, 0, name)?;
        ws.write_number_with_format(*row, 1, *amount, &s.money)?;
        if let Some(n) = count { ws.write_number(*row, 2, *n as f64)?; }
        if let Some(p) = share { ws.write_number_with_format(*row, 3, *p, &s.pct)?; }
        *row += 1;
    }
    *row += 1;
    Ok(())
}

/// The workbook for `from`–`to`, plus a forecast from `today`.
pub fn to_xlsx(conn: &rusqlite::Connection, from: &str, to: &str, today: NaiveDate, labels: &Value) -> Result<Vec<u8>> {
    let o = reports::overview(conn, from, to)?;
    let f = reports::forecast(conn, today, FORECAST_DAYS)?;
    let l = Labels(labels);
    let s = Styles {
        title: Format::new().set_bold().set_font_size(13),
        head: Format::new().set_bold().set_background_color("#E8EEF7").set_align(FormatAlign::Left),
        money: Format::new().set_num_format("#,##0.00"),
        pct: Format::new().set_num_format("0.0%"),
        bold_money: Format::new().set_bold().set_num_format("#,##0.00"),
    };
    let amount = format!("{} ({})", l.get("amount"), o.currency);
    let mut wb = Workbook::new();

    // Summary.
    (|| -> std::result::Result<(), XlsxError> {
        let ws = wb.add_worksheet().set_name(l.sheet("summary"))?;
        ws.set_column_width(0, 36)?; ws.set_column_width(1, 16)?; ws.set_column_width(2, 14)?; ws.set_column_width(3, 10)?;
        ws.write_string_with_format(0, 0, l.get("title"), &Format::new().set_bold().set_font_size(16))?;
        ws.write_string(1, 0, format!("{}: {from} – {to}", l.get("period")))?;
        ws.write_string(2, 0, format!("{}: {}", l.get("currency"), o.currency))?;
        let mut row = 4;
        for (key, v) in [("income", o.income), ("spending", o.spending), ("net", o.income - o.spending)] {
            ws.write_string_with_format(row, 0, l.get(key), &s.head)?;
            ws.write_number_with_format(row, 1, (v * 100.0).round() / 100.0, &s.bold_money)?;
            row += 1;
        }
        if let Some(r) = o.savings_rate {
            ws.write_string_with_format(row, 0, l.get("savingsRate"), &s.head)?;
            ws.write_number_with_format(row, 1, r, &s.pct)?;
            row += 1;
        }
        row += 1;
        let share = |total: f64, a: f64| (total > 0.0).then(|| a / total);
        let slices = |v: &[reports::Slice], total: f64| -> Vec<_> { v.iter().map(|x| (l.name(&x.name), x.amount, Some(x.count), share(total, x.amount))).collect() };
        table(ws, &s, &mut row, &l.get("categories"), &[l.get("category"), amount.clone(), l.get("count"), l.get("share")], &slices(&o.categories, o.spending))?;
        table(ws, &s, &mut row, &l.get("incomeSources"), &[l.get("source"), amount.clone(), l.get("count"), l.get("share")], &slices(&o.income_sources, o.income))?;
        table(ws, &s, &mut row, &l.get("places"), &[l.get("place"), amount.clone(), l.get("count"), l.get("share")], &slices(&o.places, o.spending))?;
        if o.other_currencies > 0 {
            ws.write_string(row, 0, l.get("otherCurrencies"))?;
            ws.write_number(row, 1, o.other_currencies as f64)?;
        }
        Ok(())
    })().map_err(xl)?;

    // By month.
    (|| -> std::result::Result<(), XlsxError> {
        let ws = wb.add_worksheet().set_name(l.sheet("months"))?;
        ws.set_column_width(0, 12)?;
        for c in 1..=3 { ws.set_column_width(c, 16)?; }
        for (i, h) in [l.get("month"), format!("{} ({})", l.get("income"), o.currency), format!("{} ({})", l.get("spending"), o.currency), format!("{} ({})", l.get("net"), o.currency)].iter().enumerate() {
            ws.write_string_with_format(0, i as u16, h, &s.head)?;
        }
        for (i, m) in o.months.iter().enumerate() {
            let r = i as u32 + 1;
            ws.write_string(r, 0, &m.month)?;
            ws.write_number_with_format(r, 1, m.income, &s.money)?;
            ws.write_number_with_format(r, 2, m.spending, &s.money)?;
            ws.write_number_with_format(r, 3, m.net, &s.money)?;
        }
        ws.set_freeze_panes(1, 0)?;
        Ok(())
    })().map_err(xl)?;

    // Forecast.
    (|| -> std::result::Result<(), XlsxError> {
        let ws = wb.add_worksheet().set_name(l.sheet("forecast"))?;
        ws.set_column_width(0, 14)?; ws.set_column_width(1, 30)?; ws.set_column_width(2, 30)?;
        ws.set_column_width(4, 14)?; ws.set_column_width(5, 30)?; ws.set_column_width(6, 16)?;
        ws.write_string_with_format(0, 0, l.get("balance"), &s.head)?;
        ws.write_number_with_format(0, 1, f.balance, &s.bold_money)?;
        ws.write_string_with_format(1, 0, l.get("typicalDay"), &s.head)?;
        ws.write_number_with_format(1, 1, f.typical_day, &s.money)?;
        for (i, h) in [l.get("date"), l.get("scheduled"), l.get("expected")].iter().enumerate() { ws.write_string_with_format(3, i as u16, h, &s.head)?; }
        for (i, p) in f.points.iter().enumerate() {
            let r = i as u32 + 4;
            ws.write_string(r, 0, &p.date)?;
            ws.write_number_with_format(r, 1, p.scheduled, &s.money)?;
            ws.write_number_with_format(r, 2, p.expected, &s.money)?;
        }
        // Coming up, beside the daily figures.
        ws.write_string_with_format(3, 4, l.get("date"), &s.head)?;
        ws.write_string_with_format(3, 5, l.get("comingUp"), &s.head)?;
        ws.write_string_with_format(3, 6, &amount, &s.head)?;
        for (i, c) in f.changes.iter().enumerate() {
            let r = i as u32 + 4;
            ws.write_string(r, 4, &c.date)?;
            ws.write_string(r, 5, &c.title)?;
            ws.write_number_with_format(r, 6, c.amount, &s.money)?;
        }
        ws.set_freeze_panes(4, 0)?;
        Ok(())
    })().map_err(xl)?;

    wb.save_to_buffer().map_err(xl)
}
