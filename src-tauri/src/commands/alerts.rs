use crate::db::sqlite::alerts::{AlertConfig, AlertGlobalSettings, AlertHistoryEntry};
use crate::error::AppError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_alert(
    state: State<'_, AppState>,
    alert_type: String,
    symbol: String,
    threshold_value: Option<f64>,
    threshold_period_minutes: Option<i64>,
    cooldown_minutes: i64,
) -> Result<AlertConfig, AppError> {
    let alert = AlertConfig {
        id: None,
        alert_type,
        symbol: symbol.to_uppercase(),
        enabled: true,
        threshold_value,
        threshold_period_minutes,
        cooldown_minutes,
        last_triggered_at: None,
        created_at: None,
        updated_at: None,
    };
    state.sqlite.create_alert(&alert)
}

#[tauri::command]
pub async fn get_alerts(
    state: State<'_, AppState>,
) -> Result<Vec<AlertConfig>, AppError> {
    state.sqlite.get_alerts()
}

#[tauri::command]
pub async fn update_alert(
    state: State<'_, AppState>,
    id: i64,
    alert_type: String,
    symbol: String,
    threshold_value: Option<f64>,
    threshold_period_minutes: Option<i64>,
    cooldown_minutes: i64,
) -> Result<AlertConfig, AppError> {
    let alert = AlertConfig {
        id: Some(id),
        alert_type,
        symbol: symbol.to_uppercase(),
        enabled: true,
        threshold_value,
        threshold_period_minutes,
        cooldown_minutes,
        last_triggered_at: None,
        created_at: None,
        updated_at: None,
    };
    state.sqlite.update_alert(id, &alert)
}

#[tauri::command]
pub async fn delete_alert(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_alert(id)
}

#[tauri::command]
pub async fn toggle_alert(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), AppError> {
    state.sqlite.toggle_alert(id, enabled)
}

#[tauri::command]
pub async fn get_alert_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AlertHistoryEntry>, AppError> {
    state.sqlite.get_alert_history(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub async fn acknowledge_alert(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.acknowledge_alert(id)
}

#[tauri::command]
pub async fn acknowledge_all_alerts(
    state: State<'_, AppState>,
) -> Result<i64, AppError> {
    state.sqlite.acknowledge_all_alerts()
}

#[tauri::command]
pub async fn get_unacknowledged_count(
    state: State<'_, AppState>,
) -> Result<i64, AppError> {
    state.sqlite.get_unacknowledged_count()
}

#[tauri::command]
pub async fn get_alert_settings(
    state: State<'_, AppState>,
) -> Result<AlertGlobalSettings, AppError> {
    state.sqlite.get_alert_settings()
}

#[tauri::command]
pub async fn update_alert_settings(
    state: State<'_, AppState>,
    settings: AlertGlobalSettings,
) -> Result<AlertGlobalSettings, AppError> {
    state.sqlite.update_alert_settings(&settings)
}
