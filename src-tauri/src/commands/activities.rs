//! Account activities commands

use crate::brokers::types::AccountActivity;
use crate::error::Result;
use crate::services::ActivitiesService;
use crate::state::AppState;
use tauri::State;

/// Get recent account activity (trade fills + non-trade events such as
/// deposits, transfers, dividends, and fees) from the connected broker.
///
/// Returns an empty list in analyze (sandbox) mode.
#[tauri::command]
pub async fn get_account_activities(
    state: State<'_, AppState>,
    page_size: Option<u32>,
) -> Result<Vec<AccountActivity>> {
    let activities =
        ActivitiesService::get_activities(&state, None, page_size.unwrap_or(50)).await?;
    tracing::info!("Retrieved {} account activities", activities.len());
    Ok(activities)
}
