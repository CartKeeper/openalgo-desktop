//! Staged "Analyze with Claude" command (WP3).
//!
//! Runs the staged pipeline for a client scenario and streams one progress
//! event per stage. The result is a normal `CopilotResponse` whose markdown
//! ends with an ACTIONS_JSON block the app has already validated; nothing is
//! applied to the scenario — the trades go to Review Recommendations.

use tauri::{AppHandle, Emitter, State};
use tracing::warn;

use crate::error::AppError;
use crate::services::copilot_service::CopilotResponse;
use crate::services::staged_analysis::{self, StageProgress};
use crate::state::AppState;

/// The Tauri event carrying `StageProgress` payloads while a run is in flight.
pub const PROGRESS_EVENT: &str = "staged-analysis-progress";

/// Run the full staged analysis of a scenario, with an optional focus the
/// portfolio manager is told about. About nine Claude calls; up to ten minutes.
#[tauri::command]
pub async fn staged_run_analysis(
    app: AppHandle,
    state: State<'_, AppState>,
    scenario_id: i64,
    focus: Option<String>,
) -> Result<CopilotResponse, AppError> {
    let api_key = state
        .sqlite
        .get_provider_key("anthropic", &state.security)?
        .ok_or_else(|| {
            AppError::Provider(
                "Anthropic API key not configured. Please add your API key in Settings > Providers.".to_string(),
            )
        })?;

    let emit = |progress: StageProgress| {
        if let Err(e) = app.emit(PROGRESS_EVENT, &progress) {
            warn!("staged analysis: progress event failed: {}", e);
        }
    };
    staged_analysis::run(&state, &api_key, scenario_id, focus, &emit).await
}
