use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub id: i64,
    pub symbol: String,
    pub added_at: String,
}

/// Add a symbol to the user's watchlist
pub fn add_symbol(conn: &Connection, symbol: &str) -> Result<WatchlistItem> {
    conn.execute(
        "INSERT OR IGNORE INTO user_watchlist (symbol) VALUES (?1)",
        [symbol],
    )?;

    let item = conn.query_row(
        "SELECT id, symbol, added_at FROM user_watchlist WHERE symbol = ?1",
        [symbol],
        |row| {
            Ok(WatchlistItem {
                id: row.get(0)?,
                symbol: row.get(1)?,
                added_at: row.get(2)?,
            })
        },
    )?;

    Ok(item)
}

/// Remove a symbol from the watchlist
pub fn remove_symbol(conn: &Connection, symbol: &str) -> Result<bool> {
    let affected = conn.execute(
        "DELETE FROM user_watchlist WHERE symbol = ?1",
        [symbol],
    )?;
    Ok(affected > 0)
}

/// Get all watchlist symbols
pub fn get_symbols(conn: &Connection) -> Result<Vec<WatchlistItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, added_at FROM user_watchlist ORDER BY added_at DESC",
    )?;

    let items = stmt
        .query_map([], |row| {
            Ok(WatchlistItem {
                id: row.get(0)?,
                symbol: row.get(1)?,
                added_at: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(items)
}
