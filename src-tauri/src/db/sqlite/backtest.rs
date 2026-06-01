//! backtest_runs persistence (run config + summary metrics only).

use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRunRecord {
    pub id: i64,
    pub created_at: String,
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
    pub from_date: String,
    pub to_date: String,
    pub strategy_kind: String,
    pub config_json: String,
    pub summary_json: String,
}

/// Insert a run; returns the new row id.
pub fn insert_run(
    conn: &Connection,
    created_at: &str,
    symbol: &str,
    exchange: &str,
    interval: &str,
    from_date: &str,
    to_date: &str,
    strategy_kind: &str,
    config_json: &str,
    summary_json: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO backtest_runs
         (created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            created_at, symbol, exchange, interval, from_date, to_date,
            strategy_kind, config_json, summary_json
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// List runs, newest first.
pub fn list_runs(conn: &Connection) -> Result<Vec<BacktestRunRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json
         FROM backtest_runs ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(BacktestRunRecord {
            id: r.get(0)?,
            created_at: r.get(1)?,
            symbol: r.get(2)?,
            exchange: r.get(3)?,
            interval: r.get(4)?,
            from_date: r.get(5)?,
            to_date: r.get(6)?,
            strategy_kind: r.get(7)?,
            config_json: r.get(8)?,
            summary_json: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Fetch one run's config_json by id (for reload).
pub fn get_run(conn: &Connection, id: i64) -> Result<Option<BacktestRunRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json
         FROM backtest_runs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |r| {
        Ok(BacktestRunRecord {
            id: r.get(0)?,
            created_at: r.get(1)?,
            symbol: r.get(2)?,
            exchange: r.get(3)?,
            interval: r.get(4)?,
            from_date: r.get(5)?,
            to_date: r.get(6)?,
            strategy_kind: r.get(7)?,
            config_json: r.get(8)?,
            summary_json: r.get(9)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}
