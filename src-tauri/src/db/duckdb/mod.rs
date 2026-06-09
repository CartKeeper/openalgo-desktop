//! DuckDB database module for historical data (Historify)

pub mod models;
mod migrations;

use crate::error::Result;
use duckdb::Connection;
use models::MarketDataRow;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// DuckDB database wrapper
pub struct DuckDb {
    conn: Mutex<Connection>,
    /// Path to the DuckDB file (used for file-size stats).
    pub path: PathBuf,
}

impl DuckDb {
    /// Create new DuckDB connection
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        let db = Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        };

        // Run migrations
        db.run_migrations()?;

        Ok(db)
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock();
        migrations::run_migrations(&conn)
    }

    /// Query market data
    pub fn query_market_data(
        &self,
        symbol: &str,
        exchange: &str,
        timeframe: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Vec<MarketDataRow>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            // CAST(timestamp AS VARCHAR): the column is a TIMESTAMP, but MarketDataRow
            // reads it into a String — duckdb-rs cannot convert TIMESTAMP -> String, so
            // without this cast every row mapping errors and the query returns nothing.
            // CAST(? AS TIMESTAMP) on the bounds lets the string date params compare.
            "SELECT CAST(timestamp AS VARCHAR), open, high, low, close, volume
             FROM market_data
             WHERE symbol = ? AND exchange = ? AND timeframe = ?
               AND timestamp >= CAST(? AS TIMESTAMP) AND timestamp <= CAST(? AS TIMESTAMP)
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt
            .query_map(
                duckdb::params![symbol, exchange, timeframe, from_date, to_date],
                |row| {
                    Ok(MarketDataRow {
                        timestamp: row.get(0)?,
                        open: row.get(1)?,
                        high: row.get(2)?,
                        low: row.get(3)?,
                        close: row.get(4)?,
                        volume: row.get(5)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Insert market data
    pub fn insert_market_data(
        &self,
        symbol: &str,
        exchange: &str,
        timeframe: &str,
        data: &[MarketDataRow],
    ) -> Result<usize> {
        let mut conn = self.conn.lock();

        let tx = conn.transaction()?;

        let mut stmt = tx.prepare(
            "INSERT INTO market_data (symbol, exchange, timeframe, timestamp, open, high, low, close, volume)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (symbol, exchange, timeframe, timestamp) DO UPDATE SET
               open = excluded.open, high = excluded.high, low = excluded.low,
               close = excluded.close, volume = excluded.volume",
        )?;

        let mut count = 0;
        for row in data {
            stmt.execute(duckdb::params![
                symbol,
                exchange,
                timeframe,
                row.timestamp,
                row.open,
                row.high,
                row.low,
                row.close,
                row.volume,
            ])?;
            count += 1;
        }

        drop(stmt);
        tx.commit()?;

        Ok(count)
    }

    // =========================================================================
    // Historify watchlist
    // =========================================================================

    /// Return all watchlist items ordered by order_index then created_at DESC.
    pub fn historify_get_watchlist(&self) -> Result<Vec<HistorifyWatchlistItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, symbol, exchange, name, CAST(created_at AS VARCHAR)
             FROM watchlist
             ORDER BY order_index ASC, created_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HistorifyWatchlistItem {
                    id: row.get::<_, i64>(0)?,
                    symbol: row.get(1)?,
                    exchange: row.get(2)?,
                    name: row.get(3)?,
                    added_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Insert one symbol into the watchlist; silently ignores duplicates.
    /// Returns the new row id.
    pub fn historify_add_watchlist(
        &self,
        symbol: &str,
        exchange: &str,
        name: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO watchlist (id, symbol, exchange, name, list_name, order_index)
             SELECT
               (SELECT COALESCE(MAX(id), 0) + 1 FROM watchlist),
               ?, ?, ?,
               'historify',
               (SELECT COALESCE(MAX(order_index), 0) + 1 FROM watchlist)
             WHERE NOT EXISTS (
               SELECT 1 FROM watchlist
               WHERE symbol = ? AND exchange = ? AND list_name = 'historify'
             )",
            duckdb::params![symbol, exchange, name, symbol, exchange],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM watchlist WHERE symbol = ? AND exchange = ? AND list_name = 'historify'",
            duckdb::params![symbol, exchange],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(id)
    }

    /// Remove a symbol from the watchlist. Returns true if a row was deleted.
    pub fn historify_remove_watchlist(&self, symbol: &str, exchange: &str) -> Result<bool> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let affected = tx.execute(
            "DELETE FROM watchlist WHERE symbol = ? AND exchange = ? AND list_name = 'historify'",
            duckdb::params![symbol, exchange],
        )?;
        tx.commit()?;
        Ok(affected > 0)
    }

    /// Bulk-add a list of (symbol, exchange) pairs. Returns the count inserted.
    pub fn historify_bulk_add_watchlist(&self, pairs: &[(String, String)]) -> Result<usize> {
        let mut count = 0;
        for (symbol, exchange) in pairs {
            let added = self.historify_add_watchlist(symbol, exchange, symbol)?;
            if added > 0 {
                count += 1;
            }
        }
        Ok(count)
    }

    // =========================================================================
    // Data catalog
    // =========================================================================

    /// Return all catalog entries. Timeframe column aliased → interval;
    /// from_date/to_date aliased to first_date/last_date and also exposed as
    /// epoch-seconds timestamps (first_timestamp / last_timestamp).
    pub fn historify_get_catalog(&self) -> Result<Vec<CatalogItemResponse>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT symbol, exchange, timeframe,
                    CAST(from_date AS VARCHAR) AS from_date,
                    CAST(to_date   AS VARCHAR) AS to_date,
                    row_count
             FROM data_catalog
             ORDER BY symbol, exchange, timeframe",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let symbol: String = row.get(0)?;
                let exchange: String = row.get(1)?;
                let timeframe: String = row.get(2)?;
                let from_str: Option<String> = row.get(3)?;
                let to_str: Option<String> = row.get(4)?;
                let row_count: i64 = row.get(5)?;
                Ok((symbol, exchange, timeframe, from_str, to_str, row_count))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let items = rows
            .into_iter()
            .map(|(symbol, exchange, timeframe, from_str, to_str, row_count)| {
                let first_date = from_str.clone().unwrap_or_default();
                let last_date = to_str.clone().unwrap_or_default();
                // Convert YYYY-MM-DD → epoch seconds (best-effort; zero on parse error)
                let first_timestamp = date_to_epoch(&first_date);
                let last_timestamp = date_to_epoch(&last_date);
                CatalogItemResponse {
                    symbol,
                    exchange,
                    interval: timeframe,
                    first_timestamp,
                    last_timestamp,
                    record_count: row_count,
                    first_date: Some(first_date),
                    last_date: Some(last_date),
                }
            })
            .collect();

        Ok(items)
    }

    /// Upsert a data_catalog row. On conflict, widen the date range and update
    /// row_count.
    pub fn upsert_data_catalog(
        &self,
        symbol: &str,
        exchange: &str,
        timeframe: &str,
        from_date: &str,
        to_date: &str,
        row_count: i64,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO data_catalog (id, symbol, exchange, timeframe, from_date, to_date, row_count, last_updated)
             SELECT
               (SELECT COALESCE(MAX(id), 0) + 1 FROM data_catalog),
               ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP
             WHERE NOT EXISTS (SELECT 1 FROM data_catalog WHERE symbol = ? AND exchange = ? AND timeframe = ?)
             ;",
            duckdb::params![symbol, exchange, timeframe, from_date, to_date, row_count, symbol, exchange, timeframe],
        )?;
        conn.execute(
            "UPDATE data_catalog
             SET from_date    = CASE WHEN from_date > CAST(? AS DATE) THEN CAST(? AS DATE) ELSE from_date END,
                 to_date      = CASE WHEN to_date   < CAST(? AS DATE) THEN CAST(? AS DATE) ELSE to_date   END,
                 row_count    = ?,
                 last_updated = CURRENT_TIMESTAMP
             WHERE symbol = ? AND exchange = ? AND timeframe = ?",
            duckdb::params![from_date, from_date, to_date, to_date, row_count, symbol, exchange, timeframe],
        )?;
        Ok(())
    }

    // =========================================================================
    // Stats
    // =========================================================================

    /// Return aggregate stats for the Historify header.
    pub fn historify_get_stats(&self, db_path: &Path) -> Result<HistorifyStats> {
        let conn = self.conn.lock();

        let (total_records, total_symbols): (i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(row_count), 0), COUNT(DISTINCT symbol || ':' || exchange)
             FROM data_catalog",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let watchlist_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM watchlist", [], |row| row.get(0))
            .unwrap_or(0);

        let database_size_mb = std::fs::metadata(db_path)
            .map(|m| m.len() as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0);

        Ok(HistorifyStats {
            database_size_mb,
            total_records,
            total_symbols,
            watchlist_count,
        })
    }

    // =========================================================================
    // Download jobs
    // =========================================================================

    /// Return the most recent `limit` download jobs (newest first).
    pub fn historify_get_jobs(&self, limit: i64) -> Result<Vec<HistorifyJobResponse>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(job_type, 'custom'), status,
                    total_items, completed_items, COALESCE(failed_items, 0),
                    COALESCE(interval, 'D'), COALESCE(start_date, ''), COALESCE(end_date, ''),
                    CAST(created_at AS VARCHAR),
                    CAST(started_at AS VARCHAR),
                    CAST(completed_at AS VARCHAR),
                    error_message
             FROM download_jobs
             ORDER BY id DESC
             LIMIT ?",
        )?;
        let rows = stmt
            .query_map(duckdb::params![limit], |row| {
                Ok(HistorifyJobResponse {
                    id: row.get::<_, i64>(0)?.to_string(),
                    job_type: row.get(1)?,
                    status: row.get(2)?,
                    total_symbols: row.get(3)?,
                    completed_symbols: row.get(4)?,
                    failed_symbols: row.get(5)?,
                    interval: row.get(6)?,
                    start_date: row.get(7)?,
                    end_date: row.get(8)?,
                    created_at: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    started_at: row.get::<_, Option<String>>(10)?,
                    completed_at: row.get::<_, Option<String>>(11)?,
                    error_message: row.get(12)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Create a new download job and its job_items rows.
    /// Returns the new job id.
    pub fn create_download_job(
        &self,
        job_type: &str,
        interval: &str,
        start_date: &str,
        end_date: &str,
        symbols: &[(String, String)], // (symbol, exchange)
    ) -> Result<i64> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let total = symbols.len() as i32;
        let job_name = format!("{job_type}_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));

        tx.execute(
            "INSERT INTO download_jobs
               (id, name, status, total_items, completed_items, failed_items,
                job_type, interval, start_date, end_date, started_at)
             VALUES (
               (SELECT COALESCE(MAX(id), 0) + 1 FROM download_jobs),
               ?, 'running', ?, 0, 0, ?, ?, ?, ?,
               CURRENT_TIMESTAMP
             )",
            duckdb::params![job_name, total, job_type, interval, start_date, end_date],
        )?;

        let job_id: i64 = tx.query_row(
            "SELECT MAX(id) FROM download_jobs",
            [],
            |row| row.get(0),
        )?;

        for (symbol, exchange) in symbols {
            tx.execute(
                "INSERT INTO job_items (id, job_id, symbol, exchange, timeframe, status)
                 VALUES (
                   (SELECT COALESCE(MAX(id), 0) + 1 FROM job_items),
                   ?, ?, ?, ?, 'pending'
                 )",
                duckdb::params![job_id, symbol, exchange, interval],
            )?;
        }

        tx.commit()?;
        Ok(job_id)
    }

    /// Update overall job status (and set completed_at when finished).
    pub fn update_job_status(
        &self,
        job_id: i64,
        status: &str,
        completed: i32,
        failed: i32,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let is_terminal = matches!(status, "completed" | "failed" | "completed_with_errors");
        if is_terminal {
            conn.execute(
                "UPDATE download_jobs
                 SET status = ?, completed_items = ?, failed_items = ?,
                     completed_at = CURRENT_TIMESTAMP
                 WHERE id = ?",
                duckdb::params![status, completed, failed, job_id],
            )?;
        } else {
            conn.execute(
                "UPDATE download_jobs
                 SET status = ?, completed_items = ?, failed_items = ?
                 WHERE id = ?",
                duckdb::params![status, completed, failed, job_id],
            )?;
        }
        Ok(())
    }

    /// Update a single job_item's status.
    pub fn update_job_item_status(
        &self,
        job_id: i64,
        symbol: &str,
        exchange: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE job_items
             SET status = ?, error = ?
             WHERE job_id = ? AND symbol = ? AND exchange = ?",
            duckdb::params![status, error, job_id, symbol, exchange],
        )?;
        Ok(())
    }
}

// ============================================================================
// Response / DTO types shared between db and commands layers
// ============================================================================

/// Watchlist item shape returned by historify_get_watchlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorifyWatchlistItem {
    pub id: i64,
    pub symbol: String,
    pub exchange: String,
    pub name: String,
    /// ISO timestamp string (CAST from DuckDB TIMESTAMP)
    pub added_at: String,
}

/// Catalog item shape returned by historify_get_catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItemResponse {
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
    pub first_timestamp: i64,
    pub last_timestamp: i64,
    pub record_count: i64,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
}

/// Aggregate stats for the Historify header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorifyStats {
    pub database_size_mb: f64,
    pub total_records: i64,
    pub total_symbols: i64,
    pub watchlist_count: i64,
}

/// Download job row returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorifyJobResponse {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub total_symbols: i32,
    pub completed_symbols: i32,
    pub failed_symbols: i32,
    pub interval: String,
    pub start_date: String,
    pub end_date: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse a YYYY-MM-DD string to seconds-since-epoch (UTC midnight).
/// Returns 0 on any parse error.
fn date_to_epoch(date: &str) -> i64 {
    use std::str::FromStr;
    // date is "YYYY-MM-DD" or "YYYY-MM-DD 00:00:00" from DuckDB DATE cast
    let ymd = date.split_whitespace().next().unwrap_or(date);
    let parts: Vec<&str> = ymd.split('-').collect();
    if parts.len() != 3 {
        return 0;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        i32::from_str(parts[0]),
        u32::from_str(parts[1]),
        u32::from_str(parts[2]),
    ) else {
        return 0;
    };
    // Days since Unix epoch using Gregorian calendar formula
    let days_since_epoch = days_from_ymd(y, m, d);
    days_since_epoch * 86400
}

/// Returns days since 1970-01-01 for a Gregorian calendar date.
fn days_from_ymd(y: i32, m: u32, d: u32) -> i64 {
    // Algorithm from "Calendrical Calculations" / Julian Day Number
    let m = m as i32;
    let d = d as i32;
    let a = (14 - m) / 12;
    let yr = y + 4800 - a;
    let mo = m + 12 * a - 3;
    let jdn = d + (153 * mo + 2) / 5 + 365 * yr + yr / 4 - yr / 100 + yr / 400 - 32045;
    // Unix epoch is JDN 2440588
    (jdn - 2440588) as i64
}
