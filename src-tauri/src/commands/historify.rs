//! Historical data (Historify) commands

use crate::db::duckdb::models::MarketDataRow;
use crate::db::duckdb::{CatalogItemResponse, HistorifyJobResponse, HistorifyStats, HistorifyWatchlistItem};
use crate::error::{AppError, Result};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct MarketDataQuery {
    pub symbol: String,
    pub exchange: String,
    pub timeframe: String, // "1m", "5m", "15m", "1h", "1d"
    pub from_date: String, // ISO date
    pub to_date: String,   // ISO date
}

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub symbol: String,
    pub exchange: String,
    pub timeframe: String,
    pub from_date: String,
    pub to_date: String,
}

#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub success: bool,
    pub rows_downloaded: usize,
    pub message: String,
}

/// Get historical market data from DuckDB
#[tauri::command]
pub async fn get_market_data(
    state: State<'_, AppState>,
    query: MarketDataQuery,
) -> Result<Vec<MarketDataRow>> {
    state.duckdb.query_market_data(
        &query.symbol,
        &query.exchange,
        &query.timeframe,
        &query.from_date,
        &query.to_date,
    )
}

/// Download historical data from broker
#[tauri::command]
pub async fn download_historical_data(
    state: State<'_, AppState>,
    request: DownloadRequest,
) -> Result<DownloadResponse> {
    tracing::info!(
        "Downloading historical data for {}:{} {}",
        request.exchange,
        request.symbol,
        request.timeframe
    );

    let rows = crate::services::HistoryService::download_history(
        &state,
        &request.symbol,
        &request.exchange,
        &request.timeframe,
        &request.from_date,
        &request.to_date,
        None,
    )
    .await?;
    Ok(DownloadResponse {
        success: true,
        rows_downloaded: rows,
        message: if rows == 0 {
            "No bars returned for that symbol/range".into()
        } else {
            format!("Downloaded {rows} bars")
        },
    })
}

// ============================================================================
// Historify — Watchlist
// ============================================================================

/// Return all symbols in the Historify watchlist.
#[tauri::command]
pub async fn historify_get_watchlist(
    state: State<'_, AppState>,
) -> Result<Vec<HistorifyWatchlistItem>> {
    state.duckdb.historify_get_watchlist()
}

#[derive(Debug, Deserialize)]
pub struct AddWatchlistRequest {
    pub symbol: String,
    /// Hint exchange from the UI; will be overridden by Alpaca resolve if a
    /// broker session is active.
    pub exchange: String,
}

/// Add one symbol to the Historify watchlist, resolving its exchange from
/// Alpaca if a broker session is active.
#[tauri::command]
pub async fn historify_add_watchlist(
    state: State<'_, AppState>,
    request: AddWatchlistRequest,
) -> Result<HistorifyWatchlistItem> {
    let symbol = request.symbol.trim().to_uppercase();
    let hint_exchange = request.exchange.trim().to_uppercase();

    // Best-effort: resolve exchange from Alpaca if the active session is Alpaca.
    let resolved_exchange = if let Some(session) = state.get_broker_session() {
        if session.broker_id == "alpaca" {
            crate::brokers::alpaca::resolve_alpaca_exchange(
                &session.auth_token,
                &symbol,
                &hint_exchange,
            )
            .await
        } else {
            hint_exchange.clone()
        }
    } else {
        hint_exchange.clone()
    };

    // Use symbol as the display name (we don't have a name at add time unless
    // we searched; the frontend can supply it via the search result).
    state.duckdb.historify_add_watchlist(&symbol, &resolved_exchange, &symbol)?;

    // Return the freshly-inserted row so the frontend can update state.
    let list = state.duckdb.historify_get_watchlist()?;
    list.into_iter()
        .find(|w| w.symbol == symbol && w.exchange == resolved_exchange)
        .ok_or_else(|| AppError::NotFound(format!("{symbol}:{resolved_exchange} not found after insert")))
}

#[derive(Debug, Deserialize)]
pub struct RemoveWatchlistRequest {
    pub symbol: String,
    pub exchange: String,
}

/// Remove a symbol from the Historify watchlist.
#[tauri::command]
pub async fn historify_remove_watchlist(
    state: State<'_, AppState>,
    request: RemoveWatchlistRequest,
) -> Result<bool> {
    state
        .duckdb
        .historify_remove_watchlist(&request.symbol, &request.exchange)
}

#[derive(Debug, Deserialize)]
pub struct BulkAddRequest {
    /// Each element: { symbol, exchange }
    pub symbols: Vec<BulkSymbol>,
}

#[derive(Debug, Deserialize)]
pub struct BulkSymbol {
    pub symbol: String,
    pub exchange: String,
}

/// Bulk-add symbols to the Historify watchlist. Returns the count added.
#[tauri::command]
pub async fn historify_bulk_add_watchlist(
    state: State<'_, AppState>,
    request: BulkAddRequest,
) -> Result<usize> {
    let pairs: Vec<(String, String)> = request
        .symbols
        .into_iter()
        .map(|s| (s.symbol.trim().to_uppercase(), s.exchange.trim().to_uppercase()))
        .collect();
    state.duckdb.historify_bulk_add_watchlist(&pairs)
}

// ============================================================================
// Historify — Catalog + Stats
// ============================================================================

/// Return all data catalog entries (one row per symbol+exchange+interval).
#[tauri::command]
pub async fn historify_get_catalog(
    state: State<'_, AppState>,
) -> Result<Vec<CatalogItemResponse>> {
    state.duckdb.historify_get_catalog()
}

/// Return aggregate DB stats for the header strip.
#[tauri::command]
pub async fn historify_get_stats(state: State<'_, AppState>) -> Result<HistorifyStats> {
    let db_path = state.duckdb.path.clone();
    state.duckdb.historify_get_stats(&db_path)
}

// ============================================================================
// Historify — Static lookups
// ============================================================================

#[derive(Debug, Serialize)]
pub struct StorageIntervalsResponse {
    pub storage_intervals: Vec<String>,
    pub computed_intervals: Vec<String>,
    pub all_intervals: Vec<String>,
}

/// Return the static interval lists (storage + computed).
#[tauri::command]
pub async fn historify_get_storage_intervals() -> Result<StorageIntervalsResponse> {
    let storage = vec!["1m".to_string(), "D".to_string(), "W".to_string()];
    let computed = vec!["5m".to_string(), "15m".to_string(), "30m".to_string(), "1h".to_string()];
    let all = storage.iter().chain(computed.iter()).cloned().collect();
    Ok(StorageIntervalsResponse {
        storage_intervals: storage,
        computed_intervals: computed,
        all_intervals: all,
    })
}

/// Return the static list of supported US exchanges.
#[tauri::command]
pub async fn historify_get_exchanges() -> Result<Vec<String>> {
    Ok(vec![
        "NASDAQ".to_string(),
        "NYSE".to_string(),
        "ARCA".to_string(),
        "BATS".to_string(),
        "AMEX".to_string(),
        "OTC".to_string(),
    ])
}

// ============================================================================
// Historify — Jobs
// ============================================================================

/// Return the most recent download jobs (limit defaults to 50).
#[tauri::command]
pub async fn historify_get_jobs(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<HistorifyJobResponse>> {
    state.duckdb.historify_get_jobs(limit.unwrap_or(50))
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub job_type: Option<String>,
    pub symbols: Vec<BulkSymbol>,
    pub interval: String,
    pub start_date: String,
    pub end_date: String,
    pub incremental: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateJobResponse {
    pub success: bool,
    pub job_id: i64,
    pub total_symbols: usize,
    pub message: String,
}

/// Create a download job and synchronously download history for each symbol.
/// Updates job + item statuses and upserts data_catalog entries.
#[tauri::command]
pub async fn historify_create_job(
    state: State<'_, AppState>,
    request: CreateJobRequest,
) -> Result<CreateJobResponse> {
    if request.symbols.is_empty() {
        return Err(AppError::Validation("No symbols provided".to_string()));
    }

    let job_type = request.job_type.as_deref().unwrap_or("custom");
    let pairs: Vec<(String, String)> = request
        .symbols
        .iter()
        .map(|s| (s.symbol.trim().to_uppercase(), s.exchange.trim().to_uppercase()))
        .collect();
    let total = pairs.len();

    let job_id = state.duckdb.create_download_job(
        job_type,
        &request.interval,
        &request.start_date,
        &request.end_date,
        &pairs,
    )?;

    let mut completed = 0i32;
    let mut failed = 0i32;

    for (symbol, exchange) in &pairs {
        let result = crate::services::HistoryService::download_history(
            &state,
            symbol,
            exchange,
            &request.interval,
            &request.start_date,
            &request.end_date,
            None,
        )
        .await;

        match result {
            Ok(rows) if rows > 0 => {
                // Upsert data_catalog with the date range that was stored
                let _ = state.duckdb.upsert_data_catalog(
                    symbol,
                    exchange,
                    &request.interval,
                    &request.start_date,
                    &request.end_date,
                    rows as i64,
                );
                let _ = state.duckdb.update_job_item_status(
                    job_id, symbol, exchange, "completed", None,
                );
                completed += 1;
            }
            Ok(_) => {
                // 0 rows — treat as success but note it
                let _ = state.duckdb.update_job_item_status(
                    job_id, symbol, exchange, "completed", Some("0 bars returned"),
                );
                completed += 1;
            }
            Err(e) => {
                let err_str = e.to_string();
                tracing::warn!("historify_create_job: {symbol}:{exchange} failed: {err_str}");
                let _ = state.duckdb.update_job_item_status(
                    job_id, symbol, exchange, "failed", Some(&err_str),
                );
                failed += 1;
            }
        }
    }

    let final_status = match (failed, completed) {
        (0, _) => "completed",
        (_, 0) => "failed",
        _ => "completed_with_errors",
    };

    let _ = state
        .duckdb
        .update_job_status(job_id, final_status, completed, failed);

    Ok(CreateJobResponse {
        success: failed < total as i32,
        job_id,
        total_symbols: total,
        message: format!(
            "{} downloaded, {} failed",
            completed, failed
        ),
    })
}
