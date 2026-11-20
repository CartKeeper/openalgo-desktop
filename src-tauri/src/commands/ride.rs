//! Ride (hitchhiker strategy) commands

use crate::error::AppError;
use crate::state::AppState;
use tauri::State;

/// Record that the user dismissed a proposed protective stop for a symbol at its held quantity.
///
/// The Ride Report marks the position as dismissed and the briefing stops proposing
/// it until the held quantity changes.
#[tauri::command]
pub async fn record_protection_dismissal(
    state: State<'_, AppState>,
    symbol: String,
    quantity: f64,
) -> Result<(), AppError> {
    state.sqlite.record_protection_dismissal(&symbol, quantity)?;
    tracing::info!("Recorded protection dismissal for {} x{}", symbol, quantity);
    Ok(())
}
