//! FinanceDatabase symbol metadata commands (`symbol_metadata_*`).
//!
//! `symbol_metadata_refresh` downloads the pinned files and loads them,
//! `symbol_metadata_status` reports what is loaded, and
//! `symbol_metadata_search` queries it for the Search page's Company data table.

use crate::db::duckdb::DuckDb;
use crate::error::Result;
use crate::services::symbol_metadata_service::{
    self, HttpFetcher, MetadataStatus, SymbolMetadataRow,
};
use crate::state::AppState;
use tauri::State;

/// Rows returned when the caller passes no limit.
pub const DEFAULT_SEARCH_LIMIT: u32 = 25;
/// Most rows a single search returns.
pub const MAX_SEARCH_LIMIT: u32 = 200;

/// Default 25, clamped to 1..=200.
pub fn clamp_limit(limit: Option<u32>) -> usize {
    limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT) as usize
}

/// The search behind `symbol_metadata_search`, separated so it is testable
/// against a temporary DuckDB without Tauri state.
pub fn run_search(
    db: &DuckDb,
    query: &str,
    limit: Option<u32>,
    include_delisted: Option<bool>,
) -> Result<Vec<SymbolMetadataRow>> {
    db.search_symbol_metadata(query, clamp_limit(limit), include_delisted.unwrap_or(false))
}

/// Download the pinned FinanceDatabase files (OTC pink sheets only when
/// `include_otc`) and load them in one transaction. Returns the new status.
#[tauri::command]
pub async fn symbol_metadata_refresh(
    state: State<'_, AppState>,
    include_otc: bool,
) -> Result<MetadataStatus> {
    let fetcher = HttpFetcher(state.http_client.as_ref());
    symbol_metadata_service::refresh(state.duckdb.as_ref(), &fetcher, include_otc).await
}

/// Row count, pinned commit/data date and last refresh time.
#[tauri::command]
pub fn symbol_metadata_status(state: State<'_, AppState>) -> Result<MetadataStatus> {
    symbol_metadata_service::status(state.duckdb.as_ref())
}

/// Case-insensitive search by symbol prefix or name; limit default 25, max 200.
#[tauri::command]
pub fn symbol_metadata_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
    include_delisted: Option<bool>,
) -> Result<Vec<SymbolMetadataRow>> {
    run_search(state.duckdb.as_ref(), &query, limit, include_delisted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::duckdb::DuckDb;
    use crate::services::symbol_metadata_service::SymbolMetadataRow;

    fn temp_db() -> (tempfile::TempDir, DuckDb) {
        let dir = tempfile::tempdir().expect("temp dir");
        let db = DuckDb::new(&dir.path().join("symbols.duckdb")).expect("open duckdb");
        (dir, db)
    }

    fn row(symbol: &str, exchange: &str, name: &str, asset_type: &str, delisted: bool) -> SymbolMetadataRow {
        SymbolMetadataRow {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            name: name.to_string(),
            sector: None,
            industry: None,
            industry_group: None,
            country: None,
            market_cap: None,
            size_bucket: None,
            isin: None,
            cusip: None,
            figi: None,
            website: None,
            summary: None,
            asset_type: asset_type.to_string(),
            delisted,
        }
    }

    fn dual_listing_db() -> (tempfile::TempDir, DuckDb) {
        let (dir, db) = temp_db();
        let rows = [
            row("A", "NYSE", "Agilent Technologies, Inc.", "equity", false),
            row("A", "ARCA", "A Series Trust ETF", "etf", false),
            row("AOLD", "NYSE", "Agilent Old Shell", "equity", true),
        ];
        db.replace_symbol_metadata(&rows, "FinanceDatabase", "2026-11-08", &[]).unwrap();
        (dir, db)
    }

    #[test]
    fn limit_defaults_to_25_and_is_clamped_between_1_and_200() {
        assert_eq!(clamp_limit(None), 25);
        assert_eq!(clamp_limit(Some(10)), 10);
        assert_eq!(clamp_limit(Some(0)), 1);
        assert_eq!(clamp_limit(Some(200)), 200);
        assert_eq!(clamp_limit(Some(5_000)), 200);
    }

    #[test]
    fn search_returns_a_dual_listing_on_both_exchanges() {
        let (_dir, db) = dual_listing_db();
        let found = run_search(&db, "A", None, None).unwrap();
        assert!(found.iter().all(|r| r.symbol == "A"), "{found:?}");
        let mut exchanges: Vec<&str> = found.iter().map(|r| r.exchange.as_str()).collect();
        exchanges.sort();
        assert_eq!(exchanges, ["ARCA", "NYSE"]);
        let etf = found.iter().find(|r| r.exchange == "ARCA").unwrap();
        assert_eq!(etf.asset_type, "etf");
    }

    #[test]
    fn search_respects_the_limit() {
        let (_dir, db) = dual_listing_db();
        assert_eq!(run_search(&db, "A", Some(1), None).unwrap().len(), 1);
        assert_eq!(run_search(&db, "A", Some(50), None).unwrap().len(), 2);
    }

    #[test]
    fn search_with_a_blank_query_returns_nothing() {
        let (_dir, db) = dual_listing_db();
        assert!(run_search(&db, "", None, None).unwrap().is_empty());
        assert!(run_search(&db, "   ", None, None).unwrap().is_empty());
    }

    #[test]
    fn search_includes_delisted_rows_only_when_asked() {
        let (_dir, db) = dual_listing_db();
        let listed = run_search(&db, "agilent", None, None).unwrap();
        assert_eq!(listed.iter().map(|r| r.symbol.as_str()).collect::<Vec<_>>(), ["A"]);
        let all = run_search(&db, "agilent", None, Some(true)).unwrap();
        assert_eq!(all.iter().map(|r| r.symbol.as_str()).collect::<Vec<_>>(), ["A", "AOLD"]);
    }
}
