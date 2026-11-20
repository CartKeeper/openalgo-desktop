//! Technical indicator computation commands
//!
//! Computes TA-Lib indicators for a given symbol using OHLCV data
//! from DuckDB (primary) or Yahoo Finance (fallback), and exposes the
//! indicator catalog the Analysis tab picks from.

use std::collections::HashMap;

use serde::Deserialize;
use tauri::State;

use crate::error::AppError;
use crate::providers::yahoo::YahooClient;
use crate::services::indicator_catalog::{catalog, IndicatorSpec};
use crate::services::indicators_service::{compute_indicator, IndicatorResult, OhlcvData};
use crate::state::AppState;

/// One indicator to compute: a catalog name (case-insensitive) and optional
/// numeric parameters keyed by the catalog's parameter names. Missing
/// parameters take the catalog defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct IndicatorRequest {
    pub name: String,
    #[serde(default)]
    pub params: Option<HashMap<String, f64>>,
}

/// The indicator catalog: every indicator and candlestick pattern the app can
/// compute, with groups, parameters (defaults and ranges) and output names.
#[tauri::command]
pub fn indicator_catalog() -> Result<Vec<IndicatorSpec>, AppError> {
    Ok(catalog().to_vec())
}

/// Compute one or more technical indicators for a symbol.
///
/// Each entry in `indicators` is `{ "name": "SMA", "params": { "period": 20 } }`
/// (`params` optional); any catalog name is accepted, see [`indicator_catalog`].
///
/// Data source priority:
/// 1. DuckDB historical data (if available for the symbol/exchange/timeframe)
/// 2. Yahoo Finance historical data (fallback)
#[tauri::command]
pub async fn compute_indicators(
    state: State<'_, AppState>,
    symbol: String,
    exchange: String,
    timeframe: String,
    indicators: Vec<IndicatorRequest>,
) -> Result<Vec<IndicatorResult>, AppError> {
    // Attempt to load OHLCV data from DuckDB first
    let ohlcv_data = load_ohlcv_from_duckdb(&state, &symbol, &exchange, &timeframe)
        .or_else(|_| load_ohlcv_from_duckdb_wide_range(&state, &symbol, &exchange, &timeframe));

    let ohlcv_data = match ohlcv_data {
        Ok(data) if !data.is_empty() => data,
        _ => {
            // Fall back to Yahoo Finance historical data
            tracing::info!(
                "DuckDB data not available for {}:{}, falling back to Yahoo Finance",
                exchange,
                symbol
            );
            load_ohlcv_from_yahoo(&state, &symbol, &timeframe).await?
        }
    };

    if ohlcv_data.is_empty() {
        return Err(AppError::NotFound(format!(
            "No OHLCV data available for {}:{} ({})",
            exchange, symbol, timeframe
        )));
    }

    // Compute each requested indicator
    let mut results = Vec::with_capacity(indicators.len());
    for request in &indicators {
        let params = request.params.clone().unwrap_or_default();
        results.push(compute_indicator(&ohlcv_data, &request.name, &params)?);
    }

    Ok(results)
}

/// Load OHLCV data from DuckDB using a reasonable date range.
fn load_ohlcv_from_duckdb(
    state: &State<'_, AppState>,
    symbol: &str,
    exchange: &str,
    timeframe: &str,
) -> Result<Vec<OhlcvData>, AppError> {
    // Use a 2-year lookback for sufficient indicator warm-up data
    let to_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let from_date = (chrono::Utc::now() - chrono::Duration::days(730))
        .format("%Y-%m-%d")
        .to_string();

    let rows = state
        .duckdb
        .query_market_data(symbol, exchange, timeframe, &from_date, &to_date)?;

    let ohlcv: Vec<OhlcvData> = rows
        .into_iter()
        .map(|row| OhlcvData {
            timestamp: row.timestamp,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume as f64,
        })
        .collect();

    Ok(ohlcv)
}

/// Load OHLCV data from DuckDB with the widest possible date range (fallback).
fn load_ohlcv_from_duckdb_wide_range(
    state: &State<'_, AppState>,
    symbol: &str,
    exchange: &str,
    timeframe: &str,
) -> Result<Vec<OhlcvData>, AppError> {
    let rows = state.duckdb.query_market_data(
        symbol,
        exchange,
        timeframe,
        "1900-01-01",
        "2100-01-01",
    )?;

    let ohlcv: Vec<OhlcvData> = rows
        .into_iter()
        .map(|row| OhlcvData {
            timestamp: row.timestamp,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume as f64,
        })
        .collect();

    Ok(ohlcv)
}

/// Load OHLCV data from Yahoo Finance as a fallback.
async fn load_ohlcv_from_yahoo(
    state: &State<'_, AppState>,
    symbol: &str,
    timeframe: &str,
) -> Result<Vec<OhlcvData>, AppError> {
    let client = YahooClient::new((*state.http_client).clone());

    // Map our timeframe format to Yahoo's interval format
    let interval = match timeframe {
        "1m" => "1m",
        "2m" => "2m",
        "5m" => "5m",
        "15m" => "15m",
        "30m" => "30m",
        "60m" | "1h" => "60m",
        "1d" => "1d",
        "1wk" => "1wk",
        "1mo" => "1mo",
        other => other,
    };

    // Determine appropriate range based on timeframe
    let range = match timeframe {
        "1m" | "2m" => "7d",
        "5m" | "15m" => "60d",
        "30m" | "60m" | "1h" => "6mo",
        _ => "1y",
    };

    let bars = client.get_historical(symbol, interval, range).await?;

    let ohlcv: Vec<OhlcvData> = bars
        .into_iter()
        .map(|(timestamp, open, high, low, close, volume)| OhlcvData {
            timestamp,
            open,
            high,
            low,
            close,
            volume: volume as f64,
        })
        .collect();

    Ok(ohlcv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::indicator_catalog::catalog;

    #[test]
    fn indicator_catalog_command_returns_the_whole_catalog() {
        let specs = indicator_catalog().expect("the catalog command cannot fail");
        assert_eq!(specs.len(), catalog().len());
        assert_eq!(specs[0].name, catalog()[0].name);
        assert!(specs.iter().any(|s| s.name == "ADX" && s.group == "Momentum"));
    }

    #[test]
    fn indicator_requests_accept_the_old_named_fields_and_a_bare_name() {
        let legacy: IndicatorRequest =
            serde_json::from_str(r#"{ "name": "MACD", "params": { "fast": 12, "slow": 26, "signal": 9 } }"#)
                .unwrap();
        assert_eq!(legacy.name, "MACD");
        let params = legacy.params.expect("params present");
        assert_eq!(params.get("fast"), Some(&12.0));
        assert_eq!(params.len(), 3);

        let bare: IndicatorRequest = serde_json::from_str(r#"{ "name": "VWAP" }"#).unwrap();
        assert_eq!(bare.name, "VWAP");
        assert!(bare.params.is_none());

        let typed: IndicatorRequest =
            serde_json::from_str(r#"{ "name": "CDLMORNINGSTAR", "params": { "penetration": 0.3 } }"#).unwrap();
        assert_eq!(typed.params.unwrap().get("penetration"), Some(&0.3));
    }
}
