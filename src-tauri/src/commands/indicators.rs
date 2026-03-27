//! Technical indicator computation commands
//!
//! Computes technical indicators for a given symbol using OHLCV data
//! from DuckDB (primary) or Yahoo Finance (fallback).

use crate::error::AppError;
use crate::providers::yahoo::YahooClient;
use crate::services::indicators_service::{compute_indicator, IndicatorResult, OhlcvData};
use crate::state::AppState;
use tauri::State;

/// Compute one or more technical indicators for a symbol.
///
/// Each entry in `indicators` should be a JSON object:
/// ```json
/// { "name": "SMA", "params": { "period": 20 } }
/// ```
///
/// Supported indicators: SMA, EMA, RSI, MACD, BOLLINGER, VWAP.
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
    indicators: Vec<serde_json::Value>,
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
    for indicator_spec in &indicators {
        let name = indicator_spec
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::Validation("Each indicator must have a 'name' field".to_string())
            })?;

        let params = indicator_spec
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        let result = compute_indicator(&ohlcv_data, name, &params)
            .map_err(|e| AppError::Validation(e))?;
        results.push(result);
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
