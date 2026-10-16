//! Opt-in market ticker: regional presets, preferences, provider response parsing and a cache of
//! the last prices. Fetching happens in the app shell; this module never touches the network.
//!
//! Prices here are for display only and never enter the ledger.

use crate::{error::{Error, Result}, rows::*};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const STOCK_PROVIDERS: &[&str] = &["none", "twelvedata"];
const MAX_ITEMS: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item { pub symbol: String, pub label: String }

fn item(symbol: &str, label: &str) -> Item { Item { symbol: symbol.into(), label: label.into() } }

/// Popular listings per country, in Twelve Data's `SYMBOL:EXCHANGE` form. Index funds stand in
/// for the indices themselves, which most free data plans don't include.
pub fn regional(country: &str) -> Vec<Item> {
    match country {
        "IN" => vec![
            item("NIFTYBEES:NSE", "Nifty 50 ETF"), item("BANKBEES:NSE", "Bank Nifty ETF"),
            item("RELIANCE:NSE", "Reliance"), item("HDFCBANK:NSE", "HDFC Bank"), item("TCS:NSE", "TCS"),
            item("INFY:NSE", "Infosys"), item("ICICIBANK:NSE", "ICICI Bank"), item("SBIN:NSE", "SBI"),
            item("BHARTIARTL:NSE", "Bharti Airtel"), item("ITC:NSE", "ITC"),
        ],
        "US" => vec![
            item("SPY", "S&P 500 ETF"), item("QQQ", "Nasdaq-100 ETF"), item("DIA", "Dow Jones ETF"),
            item("AAPL", "Apple"), item("MSFT", "Microsoft"), item("NVDA", "Nvidia"), item("AMZN", "Amazon"),
            item("GOOGL", "Alphabet"), item("META", "Meta"), item("JPM", "JPMorgan"),
        ],
        "GB" => vec![
            item("ISF:LSE", "FTSE 100 ETF"), item("AZN:LSE", "AstraZeneca"), item("SHEL:LSE", "Shell"),
            item("HSBA:LSE", "HSBC"), item("ULVR:LSE", "Unilever"), item("RR.:LSE", "Rolls-Royce"),
        ],
        _ => vec![],
    }
}

/// Countries with a regional preset.
pub fn regional_countries() -> Vec<&'static str> { vec!["IN", "US", "GB"] }

const CRYPTO_NAMES: &[(&str, &str)] = &[
    ("bitcoin", "BTC"), ("ethereum", "ETH"), ("solana", "SOL"), ("ripple", "XRP"), ("binancecoin", "BNB"),
    ("dogecoin", "DOGE"), ("cardano", "ADA"), ("tether", "USDT"), ("usd-coin", "USDC"), ("tron", "TRX"),
    ("polkadot", "DOT"), ("litecoin", "LTC"), ("chainlink", "LINK"), ("avalanche-2", "AVAX"), ("matic-network", "POL"),
];
pub fn crypto_label(id: &str) -> String {
    CRYPTO_NAMES.iter().find(|(i, _)| *i == id).map_or_else(|| id.to_uppercase(), |(_, s)| (*s).to_string())
}
pub fn known_crypto() -> Vec<Item> { CRYPTO_NAMES.iter().map(|(i, s)| item(i, s)).collect() }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Show { pub regional: bool, pub crypto: bool, pub watchlist: bool }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Prefs {
    pub enabled: bool,
    pub show: Show,
    /// Country whose regional list is shown.
    pub region: String,
    /// CoinGecko coin ids.
    pub crypto: Vec<String>,
    pub crypto_currency: String,
    /// The person's own symbols (`AAPL`, `TCS:NSE`…).
    pub watchlist: Vec<Item>,
    pub stock_provider: String,
    pub refresh_minutes: u32,
    pub speed: String,
}

impl Prefs {
    pub fn defaults(country: &str, currency: &str) -> Self {
        Prefs {
            enabled: false,
            show: Show { regional: true, crypto: true, watchlist: true },
            region: if regional_countries().contains(&country) { country.into() } else { "US".into() },
            crypto: vec!["bitcoin".into(), "ethereum".into(), "solana".into()],
            crypto_currency: currency.into(),
            watchlist: vec![],
            stock_provider: "none".into(),
            refresh_minutes: 15,
            speed: "normal".into(),
        }
    }
}

fn clean_symbol(s: &str) -> Option<String> {
    let s = s.trim().to_uppercase();
    let ok = !s.is_empty() && s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric() || ".:-/^&_ ".contains(c));
    ok.then_some(s)
}

fn clean_coin(s: &str) -> Option<String> {
    let s = s.trim().to_lowercase();
    (!s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then_some(s)
}

/// Merges a partial update into the stored prefs, validating everything that reaches a URL.
pub fn merge(current: &Prefs, patch: &Value) -> Result<Prefs> {
    let obj = patch.as_object().ok_or_else(|| Error::Invalid("ticker settings must be an object".into()))?;
    let mut p = current.clone();
    let bad = |m: &str| Error::Invalid(format!("ticker: {m}"));
    for (k, v) in obj {
        match k.as_str() {
            "enabled" => p.enabled = v.as_bool().ok_or_else(|| bad("enabled must be true or false"))?,
            "show" => {
                let o = v.as_object().ok_or_else(|| bad("show must be an object"))?;
                for (sk, sv) in o {
                    let on = sv.as_bool().ok_or_else(|| bad("show switches are true or false"))?;
                    match sk.as_str() {
                        "regional" => p.show.regional = on,
                        "crypto" => p.show.crypto = on,
                        "watchlist" => p.show.watchlist = on,
                        _ => return Err(bad("show has regional, crypto and watchlist")),
                    }
                }
            }
            "region" => {
                let r = v.as_str().unwrap_or_default().to_uppercase();
                if !regional_countries().contains(&r.as_str()) { return Err(bad("no regional list for that country yet")); }
                p.region = r;
            }
            "crypto" => {
                let list = v.as_array().ok_or_else(|| bad("crypto must be a list"))?;
                p.crypto = list.iter().map(|x| x.as_str().and_then(clean_coin).ok_or_else(|| bad("coin ids look like `bitcoin`")))
                    .collect::<Result<Vec<_>>>()?;
                p.crypto.dedup();
            }
            "crypto_currency" => {
                let c = v.as_str().unwrap_or_default().to_uppercase();
                if c.len() != 3 || !c.bytes().all(|b| b.is_ascii_uppercase()) { return Err(bad("currency must be a 3-letter code")); }
                p.crypto_currency = c;
            }
            "watchlist" => {
                let list = v.as_array().ok_or_else(|| bad("watchlist must be a list"))?;
                p.watchlist = list.iter().map(|x| {
                    let (sym, label) = match x {
                        Value::String(s) => (s.as_str(), None),
                        Value::Object(o) => (o.get("symbol").and_then(Value::as_str).unwrap_or_default(), o.get("label").and_then(Value::as_str)),
                        _ => ("", None),
                    };
                    let symbol = clean_symbol(sym).ok_or_else(|| bad("symbols look like AAPL or TCS:NSE"))?;
                    let label = label.map(str::trim).filter(|l| !l.is_empty()).map(|l| l.chars().take(40).collect()).unwrap_or_else(|| symbol.clone());
                    Ok(Item { symbol, label })
                }).collect::<Result<Vec<_>>>()?;
            }
            "stock_provider" => {
                let s = v.as_str().unwrap_or_default();
                if !STOCK_PROVIDERS.contains(&s) { return Err(bad("unknown stock data provider")); }
                p.stock_provider = s.into();
            }
            "refresh_minutes" => p.refresh_minutes = v.as_u64().filter(|m| (5..=1440).contains(m)).ok_or_else(|| bad("refresh every 5 to 1440 minutes"))? as u32,
            "speed" => {
                let s = v.as_str().unwrap_or_default();
                if !["slow", "normal", "fast"].contains(&s) { return Err(bad("speed is slow, normal or fast")); }
                p.speed = s.into();
            }
            other => return Err(bad(&format!("unknown setting `{other}`"))),
        }
    }
    if p.crypto.len() > MAX_ITEMS || p.watchlist.len() > MAX_ITEMS { return Err(bad("at most 30 coins and 30 watchlist symbols")); }
    Ok(p)
}

/// Stored prefs, or defaults for the profile's country and currency.
pub fn prefs(conn: &Connection) -> Result<Prefs> {
    let raw: String = conn.query_row("SELECT prefs FROM market_prefs WHERE id = 'default'", [], |r| r.get(0))?;
    let settings = crate::settings::get(conn)?;
    let locale = settings["locale"].as_str().unwrap_or("en-US");
    let country = crate::settings::country_of(locale).unwrap_or("US");
    let currency = settings["currency"].as_str().unwrap_or("USD");
    let base = Prefs::defaults(country, currency);
    let stored: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
    // Stored values are merged over defaults so new fields get sensible values.
    Ok(merge(&base, &stored).unwrap_or(base))
}

/// Prefs plus whether keys are set (the keys themselves never leave the backend).
pub fn prefs_view(conn: &Connection) -> Result<Value> {
    let p = prefs(conn)?;
    let (stock, crypto): (Option<String>, Option<String>) =
        conn.query_row("SELECT stock_key, crypto_key FROM market_prefs WHERE id = 'default'", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(json!({ "prefs": p, "has_stock_key": stock.is_some(), "has_crypto_key": crypto.is_some(),
               "regional": regional(&p.region), "regional_countries": regional_countries(), "known_crypto": known_crypto() }))
}

pub fn update_prefs(conn: &Connection, patch: &Value) -> Result<Value> {
    let next = merge(&prefs(conn)?, patch)?;
    conn.execute("UPDATE market_prefs SET prefs = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 'default'", [serde_json::to_string(&next)?])?;
    prefs_view(conn)
}

/// Sets or clears (`None` / empty) a provider key. `which` is `stock` or `crypto`.
pub fn set_key(conn: &Connection, which: &str, key: Option<&str>) -> Result<()> {
    let col = match which { "stock" => "stock_key", "crypto" => "crypto_key", _ => return Err(Error::Invalid("key is for stock or crypto".into())) };
    let key = key.map(str::trim).filter(|k| !k.is_empty());
    if key.is_some_and(|k| k.len() > 200 || !k.chars().all(|c| c.is_ascii_graphic())) { return Err(Error::Invalid("that doesn't look like an API key".into())); }
    conn.execute(&format!("UPDATE market_prefs SET {col} = ? WHERE id = 'default'"), [key])?;
    Ok(())
}

pub fn key(conn: &Connection, which: &str) -> Result<Option<String>> {
    let col = if which == "stock" { "stock_key" } else { "crypto_key" };
    Ok(conn.query_row(&format!("SELECT {col} FROM market_prefs WHERE id = 'default'"), [], |r| r.get(0))?)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quote {
    pub kind: String,
    pub symbol: String,
    pub label: String,
    pub price: f64,
    pub change_pct: Option<f64>,
    pub currency: Option<String>,
    pub source: String,
    pub as_of: String,
}

/// What the shell should fetch for the current prefs.
pub struct Plan { pub stocks: Vec<Item>, pub crypto: Vec<String>, pub vs: String }

pub fn plan(p: &Prefs) -> Plan {
    let mut stocks = vec![];
    if p.stock_provider != "none" {
        if p.show.regional { stocks.extend(regional(&p.region)); }
        if p.show.watchlist { for w in &p.watchlist { if !stocks.iter().any(|s| s.symbol == w.symbol) { stocks.push(w.clone()); } } }
    }
    Plan { stocks, crypto: if p.show.crypto { p.crypto.clone() } else { vec![] }, vs: p.crypto_currency.to_lowercase() }
}

/// `GET /api/v3/simple/price?ids=…&vs_currencies=…&include_24hr_change=true`
pub fn coingecko_url(ids: &[String], vs: &str) -> String {
    format!("https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={vs}&include_24hr_change=true", ids.join(","))
}

pub fn parse_coingecko(body: &Value, ids: &[String], vs: &str, as_of: &str) -> Vec<Quote> {
    ids.iter().filter_map(|id| {
        let o = body.get(id)?;
        let price = o.get(vs)?.as_f64()?;
        Some(Quote {
            kind: "crypto".into(), symbol: id.clone(), label: crypto_label(id), price,
            change_pct: o.get(format!("{vs}_24h_change")).and_then(Value::as_f64),
            currency: Some(vs.to_uppercase()), source: "CoinGecko".into(), as_of: as_of.into(),
        })
    }).collect()
}

/// `GET /quote?symbol=A,B:NSE&apikey=…`; the key is added by the shell.
pub fn twelvedata_url(items: &[Item]) -> String {
    let symbols: Vec<String> = items.iter().map(|i| i.symbol.replace(' ', "%20").replace('&', "%26").replace('^', "%5E")).collect();
    format!("https://api.twelvedata.com/quote?symbol={}", symbols.join(","))
}

/// Twelve Data answers one symbol with a flat object and several with a map keyed by symbol.
/// Returns the quotes plus `(symbol, message)` for each symbol that failed.
pub fn parse_twelvedata(body: &Value, items: &[Item], as_of: &str) -> (Vec<Quote>, Vec<(String, String)>) {
    let num = |v: Option<&Value>| v.and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())));
    let one = |it: &Item, o: &Value| -> std::result::Result<Quote, String> {
        if o.get("status").and_then(Value::as_str) == Some("error") {
            return Err(o.get("message").and_then(Value::as_str).unwrap_or("not available").to_string());
        }
        let price = num(o.get("close")).ok_or("no price")?;
        Ok(Quote {
            kind: "stock".into(), symbol: it.symbol.clone(), label: it.label.clone(), price,
            change_pct: num(o.get("percent_change")),
            currency: o.get("currency").and_then(Value::as_str).map(str::to_owned),
            source: "Twelve Data".into(), as_of: as_of.into(),
        })
    };
    let (mut ok, mut failed) = (vec![], vec![]);
    let whole_error = body.get("status").and_then(Value::as_str) == Some("error");
    for it in items {
        let o = if whole_error || (items.len() == 1 && body.get("symbol").is_some()) { Some(body) } else { body.get(&it.symbol) };
        match o.map(|o| one(it, o)) {
            Some(Ok(q)) => ok.push(q),
            Some(Err(m)) => failed.push((it.symbol.clone(), m)),
            None => failed.push((it.symbol.clone(), "not in the response".into())),
        }
    }
    (ok, failed)
}

/// Adds or updates cached quotes; symbols not fetched this time keep their last price.
pub fn save_quotes(conn: &Connection, quotes: &[Quote]) -> Result<()> {
    atomic(conn, |c| {
        for q in quotes {
            c.execute("INSERT OR REPLACE INTO market_quotes (kind, symbol, label, price, change_pct, currency, source, as_of) VALUES (?,?,?,?,?,?,?,?)",
                rusqlite::params![q.kind, q.symbol, q.label, q.price, q.change_pct, q.currency, q.source, q.as_of])?;
        }
        // Forget symbols the person no longer follows.
        let p = prefs(c)?;
        let pl = plan(&p);
        let keep: Vec<String> = pl.stocks.iter().map(|s| format!("stock:{}", s.symbol)).chain(pl.crypto.iter().map(|c| format!("crypto:{c}"))).collect();
        let cached: Vec<String> = c.prepare("SELECT kind || ':' || symbol FROM market_quotes")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for k in cached.iter().filter(|k| !keep.contains(k)) {
            let (kind, symbol) = k.split_once(':').unwrap_or_default();
            c.execute("DELETE FROM market_quotes WHERE kind = ? AND symbol = ?", [kind, symbol])?;
        }
        Ok(())
    })
}

/// The `n` stocks to fetch next: never-fetched first, then the stalest, so a rate-limited plan
/// still cycles through a long list.
pub fn next_stocks(conn: &Connection, stocks: &[Item], n: usize) -> Result<Vec<Item>> {
    let mut aged: Vec<(String, Item)> = stocks.iter().map(|s| {
        let as_of: Option<String> = conn.query_row("SELECT as_of FROM market_quotes WHERE kind = 'stock' AND symbol = ?", [&s.symbol], |r| r.get(0)).ok();
        (as_of.unwrap_or_default(), s.clone())
    }).collect();
    aged.sort_by(|a, b| a.0.cmp(&b.0)); // stable: ties keep ticker order
    Ok(aged.into_iter().take(n).map(|(_, s)| s).collect())
}

/// Cached quotes in ticker order: regional and watchlist stocks first, then crypto as listed.
pub fn cached(conn: &Connection) -> Result<Vec<Quote>> {
    let p = prefs(conn)?;
    let rows = query(conn, "SELECT * FROM market_quotes", &[])?;
    let all: Vec<Quote> = rows.into_iter().filter_map(|r| serde_json::from_value(r).ok()).collect();
    let pl = plan(&p);
    let mut out = vec![];
    for s in &pl.stocks { if let Some(q) = all.iter().find(|q| q.kind == "stock" && q.symbol == s.symbol) { out.push(q.clone()); } }
    for c in &pl.crypto { if let Some(q) = all.iter().find(|q| q.kind == "crypto" && &q.symbol == c) { out.push(q.clone()); } }
    Ok(out)
}
