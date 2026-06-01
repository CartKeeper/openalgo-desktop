//! Market status commands (real-time clock + trading calendar from the broker)

use crate::brokers::types::{MarketCalendarDay, MarketClock};
use crate::error::Result;
use crate::services::MarketStatusService;
use crate::state::AppState;
use tauri::State;

/// Real-time market open/closed status and next open/close times.
#[tauri::command]
pub async fn get_market_clock(state: State<'_, AppState>) -> Result<MarketClock> {
    MarketStatusService::get_clock(&state, None).await
}

/// Trading calendar between optional start/end dates ("YYYY-MM-DD").
#[tauri::command]
pub async fn get_market_calendar(
    state: State<'_, AppState>,
    start: Option<String>,
    end: Option<String>,
) -> Result<Vec<MarketCalendarDay>> {
    MarketStatusService::get_calendar(&state, None, start.as_deref(), end.as_deref()).await
}
