//! Broker-hosted (cloud) watchlist commands

use crate::brokers::types::BrokerWatchlist;
use crate::error::Result;
use crate::services::BrokerWatchlistService;
use crate::state::AppState;
use tauri::State;

/// List the connected broker's cloud watchlists.
#[tauri::command]
pub async fn get_broker_watchlists(state: State<'_, AppState>) -> Result<Vec<BrokerWatchlist>> {
    BrokerWatchlistService::list(&state, None).await
}

/// Get a single broker watchlist (with its symbols).
#[tauri::command]
pub async fn get_broker_watchlist(
    state: State<'_, AppState>,
    id: String,
) -> Result<BrokerWatchlist> {
    BrokerWatchlistService::get(&state, None, &id).await
}

/// Create a broker watchlist with the given name and symbols.
#[tauri::command]
pub async fn create_broker_watchlist(
    state: State<'_, AppState>,
    name: String,
    symbols: Vec<String>,
) -> Result<BrokerWatchlist> {
    BrokerWatchlistService::create(&state, None, &name, symbols).await
}

/// Delete a broker watchlist by id.
#[tauri::command]
pub async fn delete_broker_watchlist(state: State<'_, AppState>, id: String) -> Result<()> {
    BrokerWatchlistService::delete(&state, None, &id).await
}
