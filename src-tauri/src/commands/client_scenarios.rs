//! Client scenario commands for what-if portfolio modeling

use crate::error::AppError;
use crate::providers::types::{ClientScenario, ScenarioPosition};
use crate::state::AppState;
use tauri::State;
use tracing::info;

// ---------------------------------------------------------------------------
// Scenario CRUD
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn create_client_scenario(
    state: State<'_, AppState>,
    client_id: i64,
    name: String,
    description: Option<String>,
    is_baseline: Option<bool>,
) -> Result<ClientScenario, AppError> {
    info!("create_client_scenario called: client_id={}, name={}", client_id, name);
    state.sqlite.create_client_scenario(
        client_id,
        &name,
        description.as_deref(),
        is_baseline.unwrap_or(false),
    )
}

#[tauri::command]
pub async fn get_client_scenarios(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientScenario>, AppError> {
    info!("get_client_scenarios called: client_id={}", client_id);
    state.sqlite.get_client_scenarios(client_id)
}

#[tauri::command]
pub async fn get_client_scenario(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ClientScenario, AppError> {
    state.sqlite.get_client_scenario_by_id(id)
}

#[tauri::command]
pub async fn update_client_scenario(
    state: State<'_, AppState>,
    id: i64,
    name: Option<String>,
    description: Option<String>,
) -> Result<ClientScenario, AppError> {
    state.sqlite.update_client_scenario(
        id,
        name.as_deref(),
        description.as_deref(),
    )
}

#[tauri::command]
pub async fn delete_client_scenario(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_client_scenario(id)
}

#[tauri::command]
pub async fn clone_client_scenario(
    state: State<'_, AppState>,
    source_id: i64,
    client_id: i64,
    name: String,
    description: Option<String>,
) -> Result<ClientScenario, AppError> {
    state.sqlite.clone_client_scenario(
        source_id,
        client_id,
        &name,
        description.as_deref(),
    )
}

#[tauri::command]
pub async fn sync_baseline_scenario(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientScenario>, AppError> {
    tracing::info!("sync_baseline_scenario called with client_id={}", client_id);
    state.sqlite.sync_baseline_scenarios(client_id)
}

// ---------------------------------------------------------------------------
// Scenario Positions
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_scenario_positions(
    state: State<'_, AppState>,
    scenario_id: i64,
) -> Result<Vec<ScenarioPosition>, AppError> {
    state.sqlite.get_scenario_positions(scenario_id)
}

#[tauri::command]
pub async fn add_scenario_position(
    state: State<'_, AppState>,
    scenario_id: i64,
    symbol: String,
    exchange: String,
    quantity: f64,
    avg_price: f64,
    side: Option<String>,
    notes: Option<String>,
) -> Result<ScenarioPosition, AppError> {
    state.sqlite.add_scenario_position(
        scenario_id,
        &symbol,
        &exchange,
        quantity,
        avg_price,
        side.as_deref().unwrap_or("long"),
        notes.as_deref(),
    )
}

#[tauri::command]
pub async fn update_scenario_position(
    state: State<'_, AppState>,
    id: i64,
    quantity: Option<f64>,
    avg_price: Option<f64>,
    side: Option<String>,
    notes: Option<String>,
) -> Result<ScenarioPosition, AppError> {
    state.sqlite.update_scenario_position(
        id,
        quantity,
        avg_price,
        side.as_deref(),
        notes.as_deref(),
    )
}

#[tauri::command]
pub async fn delete_scenario_position(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_scenario_position(id)
}

#[tauri::command]
pub async fn apply_scenario_trade(
    state: State<'_, AppState>,
    scenario_id: i64,
    symbol: String,
    exchange: String,
    side: String,
    quantity: f64,
    price: f64,
) -> Result<ScenarioPosition, AppError> {
    state.sqlite.apply_scenario_trade(
        scenario_id,
        &symbol,
        &exchange,
        &side,
        quantity,
        price,
    )
}
