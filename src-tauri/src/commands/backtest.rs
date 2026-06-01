//! Backtest commands.

use crate::db::sqlite::BacktestRunRecord;
use crate::error::{AppError, Result};
use crate::services::backtest_service::{BacktestConfig, BacktestResult, BacktestService, StrategySpec};
use crate::state::AppState;
use tauri::State;

/// Run a backtest from a config (no persistence).
#[tauri::command]
pub async fn run_backtest(
    state: State<'_, AppState>,
    config: BacktestConfig,
) -> Result<BacktestResult> {
    BacktestService::run_for_config(&state, config).await
}

/// Persist a completed run's config + summary; returns the new id.
#[tauri::command]
pub async fn save_backtest_run(
    state: State<'_, AppState>,
    config: BacktestConfig,
    summary_json: String,
) -> Result<i64> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let strategy_kind = match &config.strategy {
        StrategySpec::SmaCrossover { .. } => "SmaCrossover",
        StrategySpec::EmaCrossover { .. } => "EmaCrossover",
        StrategySpec::RsiThreshold { .. } => "RsiThreshold",
        StrategySpec::MacdCross { .. } => "MacdCross",
        StrategySpec::BollingerReversion { .. } => "BollingerReversion",
    };
    let config_json = serde_json::to_string(&config)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    state.sqlite.insert_backtest_run(
        &created_at,
        &config.symbol,
        &config.exchange,
        &config.interval,
        &config.from_date,
        &config.to_date,
        strategy_kind,
        &config_json,
        &summary_json,
    )
}

/// List saved runs (newest first).
#[tauri::command]
pub async fn list_backtest_runs(
    state: State<'_, AppState>,
) -> Result<Vec<BacktestRunRecord>> {
    state.sqlite.list_backtest_runs()
}

/// Fetch one saved run by id.
#[tauri::command]
pub async fn get_backtest_run(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<BacktestRunRecord>> {
    state.sqlite.get_backtest_run(id)
}
