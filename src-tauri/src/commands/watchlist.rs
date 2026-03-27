use crate::db::sqlite::WatchlistItem;
use crate::error::AppError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn add_watchlist_symbol(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<WatchlistItem, AppError> {
    state.sqlite.add_watchlist_symbol(&symbol.to_uppercase())
}

#[tauri::command]
pub async fn remove_watchlist_symbol(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<bool, AppError> {
    state.sqlite.remove_watchlist_symbol(&symbol.to_uppercase())
}

#[tauri::command]
pub async fn get_watchlist_symbols(
    state: State<'_, AppState>,
) -> Result<Vec<WatchlistItem>, AppError> {
    state.sqlite.get_watchlist_symbols()
}
