//! Yahoo Finance API client
//!
//! Provides real-time quotes, historical prices, options chains, and symbol search.
//! No API key required (unofficial endpoints, rate limited).

use super::ProviderError;
use super::types::*;
use reqwest::Client;

const YAHOO_SEARCH_URL: &str = "https://query2.finance.yahoo.com/v1/finance/search";
const YAHOO_CHART_URL: &str = "https://query2.finance.yahoo.com/v8/finance/chart";

/// Yahoo Finance API client
pub struct YahooClient {
    client: Client,
}

impl YahooClient {
    /// Create a new Yahoo Finance client
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Get real-time quotes for one or more symbols via v8 chart endpoint
    ///
    /// The v7 quote endpoint was deprecated. We use v8/finance/chart with range=5d
    /// to get current price data from the `meta` field.
    pub async fn get_quotes(&self, symbols: &[&str]) -> Result<Vec<GenericQuote>, ProviderError> {
        let mut quotes = Vec::with_capacity(symbols.len());

        for &symbol in symbols {
            let url = format!("{}/{}?range=5d&interval=1d", YAHOO_CHART_URL, symbol);

            let resp: serde_json::Value = self.client
                .get(&url)
                .header("User-Agent", "Mozilla/5.0")
                .send()
                .await?
                .json()
                .await?;

            let result = resp["chart"]["result"]
                .as_array()
                .and_then(|a| a.first());

            if let Some(r) = result {
                let meta = &r["meta"];
                let price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
                let prev_close = meta["chartPreviousClose"].as_f64()
                    .or_else(|| meta["previousClose"].as_f64());

                let (change, change_pct) = if let Some(pc) = prev_close {
                    if pc > 0.0 {
                        let c = price - pc;
                        (Some(c), Some(c / pc * 100.0))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                quotes.push(GenericQuote {
                    symbol: meta["symbol"].as_str().unwrap_or(symbol).to_string(),
                    name: meta["longName"].as_str()
                        .or_else(|| meta["shortName"].as_str())
                        .map(String::from),
                    exchange: meta["exchangeName"].as_str()
                        .or_else(|| meta["fullExchangeName"].as_str())
                        .map(String::from),
                    currency: meta["currency"].as_str().map(String::from),
                    price,
                    change,
                    change_percent: change_pct,
                    open: None,
                    high: meta["regularMarketDayHigh"].as_f64(),
                    low: meta["regularMarketDayLow"].as_f64(),
                    previous_close: prev_close,
                    volume: meta["regularMarketVolume"].as_i64(),
                    market_cap: None, // Not available in chart meta
                    pe_ratio: None,   // Not available in chart meta
                    fifty_two_week_high: meta["fiftyTwoWeekHigh"].as_f64(),
                    fifty_two_week_low: meta["fiftyTwoWeekLow"].as_f64(),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                });
            }
        }

        if quotes.is_empty() && !symbols.is_empty() {
            return Err(ProviderError::Parse(format!(
                "No quote data returned for: {}",
                symbols.join(", ")
            )));
        }

        Ok(quotes)
    }

    /// Search for symbols globally
    pub async fn search_symbols(&self, query: &str, limit: i32) -> Result<Vec<SymbolSearchResult>, ProviderError> {
        let url = format!(
            "{}?q={}&quotesCount={}&newsCount=0&enableFuzzyQuery=false",
            YAHOO_SEARCH_URL, query, limit
        );

        let resp: serde_json::Value = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?
            .json()
            .await?;

        let quotes = resp["quotes"]
            .as_array()
            .ok_or_else(|| ProviderError::Parse("Invalid search response".to_string()))?;

        Ok(quotes.iter().map(|item| SymbolSearchResult {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            name: item["shortname"].as_str()
                .or(item["longname"].as_str())
                .unwrap_or("")
                .to_string(),
            exchange: item["exchange"].as_str().map(String::from),
            asset_type: item["quoteType"].as_str().map(String::from),
            exchange_display: item["exchDisp"].as_str().map(String::from),
        }).collect())
    }

    /// Get historical OHLCV data
    pub async fn get_historical(
        &self,
        symbol: &str,
        interval: &str,
        range: &str,
    ) -> Result<Vec<(String, f64, f64, f64, f64, i64)>, ProviderError> {
        let url = format!(
            "{}/{}?interval={}&range={}",
            YAHOO_CHART_URL, symbol, interval, range
        );

        let resp: serde_json::Value = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?
            .json()
            .await?;

        let result = &resp["chart"]["result"][0];
        let timestamps = result["timestamp"].as_array()
            .ok_or_else(|| ProviderError::Parse("No timestamp data".to_string()))?;
        let indicators = &result["indicators"]["quote"][0];

        let opens = indicators["open"].as_array();
        let highs = indicators["high"].as_array();
        let lows = indicators["low"].as_array();
        let closes = indicators["close"].as_array();
        let volumes = indicators["volume"].as_array();

        let mut bars = Vec::new();
        for (i, ts) in timestamps.iter().enumerate() {
            let timestamp = ts.as_i64().unwrap_or(0);
            let dt = chrono::DateTime::from_timestamp(timestamp, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();

            let open = opens.and_then(|a| a.get(i)).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let high = highs.and_then(|a| a.get(i)).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let low = lows.and_then(|a| a.get(i)).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let close = closes.and_then(|a| a.get(i)).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let volume = volumes.and_then(|a| a.get(i)).and_then(|v| v.as_i64()).unwrap_or(0);

            if open > 0.0 {
                bars.push((dt, open, high, low, close, volume));
            }
        }

        Ok(bars)
    }
}
