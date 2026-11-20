//! Chart preference commands (WP1): drawings and indicators saved per symbol

use crate::db::sqlite::chart_preferences::ChartPreference;
use crate::error::AppError;
use crate::state::AppState;
use tauri::State;

fn require_key(exchange: &str, symbol: &str) -> Result<(), AppError> {
    if exchange.trim().is_empty() || symbol.trim().is_empty() {
        return Err(AppError::Validation(
            "exchange and symbol are required for chart preferences".to_string(),
        ));
    }
    Ok(())
}

/// The saved chart state (indicators, drawings) of an `exchange:symbol`, if any.
#[tauri::command]
pub async fn chart_get_preference(
    state: State<'_, AppState>,
    exchange: String,
    symbol: String,
) -> Result<Option<ChartPreference>, AppError> {
    require_key(&exchange, &symbol)?;
    state.sqlite.get_chart_preference(&exchange, &symbol)
}

/// Upsert the saved indicators and drawings of an `exchange:symbol`.
#[tauri::command]
pub async fn chart_save_preference(
    state: State<'_, AppState>,
    exchange: String,
    symbol: String,
    indicators: Option<String>,
    drawings: Option<String>,
) -> Result<(), AppError> {
    require_key(&exchange, &symbol)?;
    state.sqlite.save_chart_preference(
        &exchange,
        &symbol,
        indicators.as_deref(),
        drawings.as_deref(),
    )
}
