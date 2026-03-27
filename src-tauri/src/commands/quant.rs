//! Quantitative metrics Tauri commands
//!
//! Exposes portfolio risk analytics to the frontend via Tauri IPC.

use crate::error::AppError;
use crate::providers::yahoo::YahooClient;
use crate::services::quant_service::{self, CorrelationEntry, SymbolMetrics};
use crate::state::AppState;
use std::collections::HashMap;
use tauri::State;

/// Fetch historical close prices from Yahoo Finance.
/// Returns `(dates, close_prices)` for the given symbol.
async fn fetch_prices(
    state: &State<'_, AppState>,
    symbol: &str,
    period: &str,
) -> Result<(Vec<String>, Vec<f64>), AppError> {
    let client = YahooClient::new((*state.http_client).clone());
    let bars = client.get_historical(symbol, "1d", period).await?;

    if bars.is_empty() {
        return Err(AppError::Validation(format!(
            "No historical data returned for symbol '{}'",
            symbol
        )));
    }

    // bars: Vec<(date_string, open, high, low, close, volume)>
    let dates: Vec<String> = bars.iter().map(|(d, _, _, _, _, _)| d.clone()).collect();
    let closes: Vec<f64> = bars.iter().map(|(_, _, _, _, c, _)| *c).collect();

    Ok((dates, closes))
}

/// Compute all quantitative metrics for a single symbol.
///
/// - `symbol`: ticker to analyze (e.g., "AAPL")
/// - `benchmark`: optional benchmark ticker for beta/alpha (defaults to "SPY")
/// - `period`: Yahoo Finance range string (defaults to "1y")
#[tauri::command]
pub async fn get_symbol_metrics(
    state: State<'_, AppState>,
    symbol: String,
    benchmark: Option<String>,
    period: Option<String>,
) -> Result<SymbolMetrics, AppError> {
    let period = period.unwrap_or_else(|| "1y".to_string());

    let (dates, prices) = fetch_prices(&state, &symbol, &period).await?;

    if prices.len() < 2 {
        return Err(AppError::Validation(format!(
            "Insufficient price data for '{}': need at least 2 data points, got {}",
            symbol,
            prices.len()
        )));
    }

    let returns = quant_service::compute_returns(&prices);

    if returns.is_empty() {
        return Err(AppError::Validation(format!(
            "Could not compute returns for '{}': all prices may be zero or invalid",
            symbol
        )));
    }

    let risk_free_rate = 0.05; // 5% annualized, reasonable default

    let ann_ret = quant_service::annualized_return(&returns);
    let ann_vol = quant_service::annualized_volatility(&returns);
    let sharpe = quant_service::sharpe_ratio(&returns, risk_free_rate);
    let sortino = quant_service::sortino_ratio(&returns, risk_free_rate);
    let (mdd, peak_idx, trough_idx) = quant_service::max_drawdown(&prices);
    let var_95 = quant_service::value_at_risk(&returns, 0.95);
    let cvar_95 = quant_service::conditional_var(&returns, 0.95);
    let calmar = quant_service::calmar_ratio(&returns, &prices);

    // Extract dates for drawdown peak/trough
    let peak_date = dates.get(peak_idx).cloned();
    let trough_date = dates.get(trough_idx).cloned();

    // Compute beta and alpha if benchmark is available
    let benchmark_ticker = benchmark.unwrap_or_else(|| "SPY".to_string());
    let (beta_val, alpha_val) = match fetch_prices(&state, &benchmark_ticker, &period).await {
        Ok((bench_dates, bench_prices)) => {
            if bench_prices.len() < 2 {
                (None, None)
            } else {
                // Align by date: only keep dates present in both series
                let bench_map: HashMap<&str, f64> = bench_dates
                    .iter()
                    .zip(bench_prices.iter())
                    .map(|(d, &p)| (d.as_str(), p))
                    .collect();

                let mut aligned_asset_prices = Vec::new();
                let mut aligned_bench_prices = Vec::new();

                for (i, date) in dates.iter().enumerate() {
                    if let Some(&bp) = bench_map.get(date.as_str()) {
                        aligned_asset_prices.push(prices[i]);
                        aligned_bench_prices.push(bp);
                    }
                }

                if aligned_asset_prices.len() < 2 {
                    (None, None)
                } else {
                    let aligned_asset_returns =
                        quant_service::compute_returns(&aligned_asset_prices);
                    let aligned_bench_returns =
                        quant_service::compute_returns(&aligned_bench_prices);

                    if aligned_asset_returns.is_empty() || aligned_bench_returns.is_empty() {
                        (None, None)
                    } else {
                        let b =
                            quant_service::beta(&aligned_asset_returns, &aligned_bench_returns);
                        let a = quant_service::alpha(
                            &aligned_asset_returns,
                            &aligned_bench_returns,
                            risk_free_rate,
                        );
                        (Some(b), Some(a))
                    }
                }
            }
        }
        Err(_) => (None, None),
    };

    Ok(SymbolMetrics {
        symbol,
        annualized_return: ann_ret,
        annualized_volatility: ann_vol,
        sharpe_ratio: sharpe,
        sortino_ratio: sortino,
        max_drawdown: mdd,
        max_drawdown_peak_date: peak_date,
        max_drawdown_trough_date: trough_date,
        beta: beta_val,
        alpha: alpha_val,
        var_95,
        cvar_95,
        calmar_ratio: calmar,
    })
}

/// Compute the NxN correlation matrix for a set of symbols.
///
/// Returns a flat list of `CorrelationEntry` pairs (upper triangle + diagonal).
/// Dates are aligned so only trading days present in ALL symbols are used.
#[tauri::command]
pub async fn get_correlation_matrix(
    state: State<'_, AppState>,
    symbols: Vec<String>,
    period: Option<String>,
) -> Result<Vec<CorrelationEntry>, AppError> {
    if symbols.is_empty() {
        return Ok(Vec::new());
    }

    if symbols.len() == 1 {
        return Ok(vec![CorrelationEntry {
            symbol_a: symbols[0].clone(),
            symbol_b: symbols[0].clone(),
            correlation: 1.0,
        }]);
    }

    let period = period.unwrap_or_else(|| "1y".to_string());

    // Fetch all price series
    let mut all_data: Vec<(String, HashMap<String, f64>)> = Vec::new();

    for sym in &symbols {
        let (dates, prices) = fetch_prices(&state, sym, &period).await?;
        let map: HashMap<String, f64> = dates.into_iter().zip(prices.into_iter()).collect();
        all_data.push((sym.clone(), map));
    }

    // Find common dates across ALL symbols
    let common_dates: Vec<String> = {
        // Start with dates from the first symbol
        let first_dates: std::collections::HashSet<String> =
            all_data[0].1.keys().cloned().collect();

        let mut common: std::collections::HashSet<String> = first_dates;
        for (_, date_map) in all_data.iter().skip(1) {
            let sym_dates: std::collections::HashSet<String> = date_map.keys().cloned().collect();
            common = common.intersection(&sym_dates).cloned().collect();
        }

        let mut sorted: Vec<String> = common.into_iter().collect();
        sorted.sort();
        sorted
    };

    if common_dates.len() < 2 {
        return Err(AppError::Validation(
            "Insufficient overlapping dates between symbols to compute correlation".to_string(),
        ));
    }

    // Build aligned return series for each symbol
    let mut returns_matrix: Vec<Vec<f64>> = Vec::new();

    for (_, date_map) in &all_data {
        let aligned_prices: Vec<f64> = common_dates
            .iter()
            .filter_map(|d| date_map.get(d).copied())
            .collect();

        let returns = quant_service::compute_returns(&aligned_prices);
        if returns.is_empty() {
            return Err(AppError::Validation(
                "Could not compute returns for one or more symbols after date alignment"
                    .to_string(),
            ));
        }
        returns_matrix.push(returns);
    }

    // Compute correlation matrix
    let corr_matrix = quant_service::correlation_matrix(&returns_matrix);

    // Flatten into CorrelationEntry list (full matrix for easy frontend consumption)
    let mut entries = Vec::new();
    for i in 0..symbols.len() {
        for j in 0..symbols.len() {
            entries.push(CorrelationEntry {
                symbol_a: symbols[i].clone(),
                symbol_b: symbols[j].clone(),
                correlation: corr_matrix[i][j],
            });
        }
    }

    Ok(entries)
}

/// Get the drawdown time series for charting.
///
/// Returns a list of `(date, drawdown_pct)` tuples where drawdown_pct is
/// non-positive (0.0 at peaks, negative during drawdowns).
#[tauri::command]
pub async fn get_drawdown_chart(
    state: State<'_, AppState>,
    symbol: String,
    period: Option<String>,
) -> Result<Vec<(String, f64)>, AppError> {
    let period = period.unwrap_or_else(|| "1y".to_string());

    let (dates, prices) = fetch_prices(&state, &symbol, &period).await?;

    if prices.len() < 2 {
        return Err(AppError::Validation(format!(
            "Insufficient price data for '{}' to compute drawdown series",
            symbol
        )));
    }

    let dd_series = quant_service::drawdown_series(&prices);

    let result: Vec<(String, f64)> = dates
        .into_iter()
        .zip(dd_series.into_iter())
        .collect();

    Ok(result)
}
