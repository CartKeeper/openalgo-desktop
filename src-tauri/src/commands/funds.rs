//! Funds/margin commands

use crate::brokers::types::{Funds, PortfolioHistory};
use crate::error::Result;
use crate::services::FundsService;
use crate::state::AppState;
use tauri::State;

/// Get available funds/margin
///
/// Routes to sandbox funds when in analyze mode.
#[tauri::command]
pub async fn get_funds(state: State<'_, AppState>) -> Result<Funds> {
    let result = FundsService::get_funds(&state, None).await?;
    tracing::info!("Funds retrieved in {} mode", result.mode);
    Ok(result.funds)
}

/// Get account equity history (time series) for charting.
///
/// `period` defaults to "1M", `timeframe` to "1D". Returns an empty series
/// in analyze (sandbox) mode.
#[tauri::command]
pub async fn get_portfolio_history(
    state: State<'_, AppState>,
    period: Option<String>,
    timeframe: Option<String>,
) -> Result<PortfolioHistory> {
    let period = period.unwrap_or_else(|| "1M".to_string());
    let timeframe = timeframe.unwrap_or_else(|| "1D".to_string());
    let history = FundsService::get_portfolio_history(&state, None, &period, &timeframe).await?;
    Ok(history)
}
