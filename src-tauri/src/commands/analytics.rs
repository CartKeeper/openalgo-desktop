//! Portfolio analytics commands (WP2): returns, tax lots, allocation targets,
//! drift and the rebalance plan. All `analytics_*`.

use crate::analytics::allocation::{AllocationTarget, DriftReport, RebalancePlan};
use crate::analytics::lots::{LotMethod, LotReport};
use crate::error::AppError;
use crate::services::portfolio_analytics_service::{
    account_scope, PerformanceReport, PortfolioAnalyticsService,
};
use crate::state::AppState;
use tauri::State;

/// Tax lots of a client's trades: `method` is `fifo`, `lifo`, `hifo` or `average`;
/// `account_type` scopes to one account (`all`/absent = every account).
#[tauri::command]
pub async fn analytics_client_lots(
    state: State<'_, AppState>,
    client_id: i64,
    method: String,
    account_type: Option<String>,
) -> Result<LotReport, AppError> {
    let method = LotMethod::parse(&method)
        .ok_or_else(|| AppError::Validation(format!("Unknown lot method: {method}")))?;
    let account = account_scope(account_type);
    PortfolioAnalyticsService::client_lots(state.inner(), client_id, method, account.as_deref()).await
}

/// Time- and money-weighted returns over 1M, 3M, 6M, YTD, 1Y and since inception;
/// `account_type` scopes to one account (`all`/absent = every account).
#[tauri::command]
pub async fn analytics_client_performance(
    state: State<'_, AppState>,
    client_id: i64,
    account_type: Option<String>,
) -> Result<PerformanceReport, AppError> {
    let account = account_scope(account_type);
    PortfolioAnalyticsService::client_performance(state.inner(), client_id, account.as_deref()).await
}

/// The client's saved allocation targets.
#[tauri::command]
pub async fn analytics_list_targets(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<AllocationTarget>, AppError> {
    state.sqlite.list_allocation_targets(client_id)
}

/// Replace the client's allocation targets; rejected (with the sum) above 100% per scope.
#[tauri::command]
pub async fn analytics_save_targets(
    state: State<'_, AppState>,
    client_id: i64,
    targets: Vec<AllocationTarget>,
) -> Result<Vec<AllocationTarget>, AppError> {
    state.sqlite.replace_allocation_targets(client_id, &targets)
}

/// Each target's drift for a scenario.
#[tauri::command]
pub async fn analytics_scenario_drift(
    state: State<'_, AppState>,
    scenario_id: i64,
) -> Result<DriftReport, AppError> {
    PortfolioAnalyticsService::scenario_drift(state.inner(), scenario_id).await
}

/// Whole-share trades that bring a scenario back to its targets.
#[tauri::command]
pub async fn analytics_rebalance_plan(
    state: State<'_, AppState>,
    scenario_id: i64,
) -> Result<RebalancePlan, AppError> {
    PortfolioAnalyticsService::rebalance_plan(state.inner(), scenario_id).await
}
