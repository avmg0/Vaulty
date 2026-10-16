//! Fetches ticker prices. Only runs when the person has turned the ticker on; sends nothing but
//! the symbols (and their own API key to the provider they chose).

use serde_json::{json, Value};
use std::time::Duration;
use vaulty_core::{error::{Error, Result}, market};

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Vaulty/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| Error::Invalid(format!("network: {e}")))
}

async fn get_json(req: reqwest::RequestBuilder) -> std::result::Result<Value, String> {
    let resp = req.send().await.map_err(|e| if e.is_timeout() { "timed out".to_string() } else if e.is_connect() { "no connection".to_string() } else { e.to_string() })?;
    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS { return Err("rate limited; try again later".into()); }
    let body: Value = resp.json().await.map_err(|_| format!("unexpected reply ({status})"))?;
    if !status.is_success() && body.get("status").is_none() { return Err(format!("provider error ({status})")); }
    Ok(body)
}

/// `stock_batch` is the part of `plan.stocks` to fetch now (see `market::next_stocks`).
pub struct Job { pub plan: market::Plan, pub stock_batch: Vec<market::Item>, pub provider: String, pub stock_key: Option<String>, pub crypto_key: Option<String> }

/// Fetches the plan's crypto and this round's stocks. Returns the quotes and per-source problems.
pub async fn fetch(job: Job) -> Result<(Vec<market::Quote>, Vec<Value>)> {
    let http = client()?;
    let now = vaulty_core::rows::now();
    let (mut quotes, mut problems) = (vec![], vec![]);

    if !job.plan.crypto.is_empty() {
        let mut req = http.get(market::coingecko_url(&job.plan.crypto, &job.plan.vs));
        if let Some(k) = &job.crypto_key { req = req.header("x-cg-demo-api-key", k); }
        match get_json(req).await {
            Ok(body) => { quotes.extend(market::parse_coingecko(&body, &job.plan.crypto, &job.plan.vs, &now)); }
            Err(m) => problems.push(json!({ "source": "CoinGecko", "message": m })),
        }
    }

    if !job.stock_batch.is_empty() && job.provider == "twelvedata" {
        match &job.stock_key {
            None => problems.push(json!({ "source": "Twelve Data", "message": "add your API key in Settings" })),
            Some(key) => {
                let batch = &job.stock_batch;
                let req = http.get(market::twelvedata_url(batch)).header("Authorization", format!("apikey {key}"));
                match get_json(req).await {
                    Ok(body) => {
                        let (ok, failed) = market::parse_twelvedata(&body, batch, &now);
                        quotes.extend(ok);
                        problems.extend(failed.into_iter().map(|(s, m)| json!({ "source": "Twelve Data", "symbol": s, "message": m })));
                    }
                    Err(m) => problems.push(json!({ "source": "Twelve Data", "message": m })),
                }
            }
        }
    }
    Ok((quotes, problems))
}
