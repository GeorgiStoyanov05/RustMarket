use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct FinnhubClient {
    http: Client,
    api_key: String,
}

impl FinnhubClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
        }
    }

    fn has_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub async fn search(&self, q: &str) -> Result<SearchResponse, String> {
        if !self.has_key() {
            return Err("FINNHUB_API_KEY is missing in .env".to_string());
        }

        let url = "https://finnhub.io/api/v1/search";
        let res = self
            .http
            .get(url)
            .query(&[("q", q), ("token", &self.api_key)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Finnhub search failed: {status} {body}"));
        }

        res.json::<SearchResponse>().await.map_err(|e| e.to_string())
    }

    pub async fn quote(&self, symbol: &str) -> Result<QuoteResponse, String> {
        if !self.has_key() {
            return Err("FINNHUB_API_KEY is missing in .env".to_string());
        }

        let url = "https://finnhub.io/api/v1/quote";
        let res = self
            .http
            .get(url)
            .query(&[("symbol", symbol), ("token", &self.api_key)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Finnhub quote failed: {status} {body}"));
        }

        res.json::<QuoteResponse>().await.map_err(|e| e.to_string())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResponse {
    pub count: i64,
    pub result: Vec<SearchItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchItem {
    pub description: String,

    #[serde(rename = "displaySymbol")]
    pub display_symbol: String,

    pub symbol: String,

    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QuoteResponse {
    // current
    pub c: f64,
    // change
    pub d: f64,
    // percent change
    pub dp: f64,
    // high
    pub h: f64,
    // low
    pub l: f64,
    // open
    pub o: f64,
    // previous close
    pub pc: f64,
    // timestamp
    pub t: i64,
}
