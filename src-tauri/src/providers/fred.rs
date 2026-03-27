//! Federal Reserve Economic Data (FRED) API client
//!
//! Provides economic indicators, interest rates, and calendar data.
//! Free tier: 120 requests/min. API key required.

use super::ProviderError;
use super::types::*;
use reqwest::Client;

const FRED_BASE_URL: &str = "https://api.stlouisfed.org/fred";

/// FRED API client
pub struct FredClient {
    client: Client,
    api_key: String,
}

impl FredClient {
    /// Create a new FRED client
    pub fn new(client: Client, api_key: String) -> Self {
        Self { client, api_key }
    }

    /// Get observations for a FRED series
    pub async fn get_series(
        &self,
        series_id: &str,
        observation_start: Option<&str>,
        observation_end: Option<&str>,
    ) -> Result<Vec<FredObservation>, ProviderError> {
        let mut url = format!(
            "{}/series/observations?series_id={}&api_key={}&file_type=json",
            FRED_BASE_URL, series_id, self.api_key
        );

        if let Some(start) = observation_start {
            url.push_str(&format!("&observation_start={}", start));
        }
        if let Some(end) = observation_end {
            url.push_str(&format!("&observation_end={}", end));
        }

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let observations = resp["observations"]
            .as_array()
            .ok_or_else(|| ProviderError::Parse("Invalid FRED response".to_string()))?;

        Ok(observations.iter().map(|item| {
            let value_str = item["value"].as_str().unwrap_or(".");
            let value = if value_str == "." { None } else { value_str.parse().ok() };

            FredObservation {
                date: item["date"].as_str().unwrap_or("").to_string(),
                value,
            }
        }).collect())
    }

    /// Search for FRED series
    pub async fn search_series(&self, query: &str, limit: i32) -> Result<Vec<FredSeries>, ProviderError> {
        let url = format!(
            "{}/series/search?search_text={}&api_key={}&file_type=json&limit={}&order_by=search_rank",
            FRED_BASE_URL,
            urlencoding::encode(query),
            self.api_key,
            limit
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let serieses = resp["seriess"]
            .as_array()
            .ok_or_else(|| ProviderError::Parse("Invalid FRED search response".to_string()))?;

        Ok(serieses.iter().map(|item| FredSeries {
            id: item["id"].as_str().unwrap_or("").to_string(),
            title: item["title"].as_str().unwrap_or("").to_string(),
            frequency: item["frequency_short"].as_str().map(String::from),
            units: item["units_short"].as_str().map(String::from),
            seasonal_adjustment: item["seasonal_adjustment_short"].as_str().map(String::from),
            last_updated: item["last_updated"].as_str().map(String::from),
            observation_start: item["observation_start"].as_str().map(String::from),
            observation_end: item["observation_end"].as_str().map(String::from),
            popularity: item["popularity"].as_i64().map(|n| n as i32),
        }).collect())
    }

    /// Get upcoming data releases (economic calendar)
    pub async fn get_releases(&self) -> Result<Vec<(String, String, String)>, ProviderError> {
        let url = format!(
            "{}/releases?api_key={}&file_type=json",
            FRED_BASE_URL, self.api_key
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let releases = resp["releases"]
            .as_array()
            .ok_or_else(|| ProviderError::Parse("Invalid FRED releases response".to_string()))?;

        Ok(releases.iter().map(|item| {
            (
                item["id"].as_i64().map(|n| n.to_string()).unwrap_or_default(),
                item["name"].as_str().unwrap_or("").to_string(),
                item["press_release"].as_bool().map(|b| b.to_string()).unwrap_or_default(),
            )
        }).collect())
    }
}
