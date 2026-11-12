//! Per-symbol chart preferences: saved indicators and drawings (WP1)
//!
//! One `chart_preferences` row per `exchange:symbol` (the table's UNIQUE key).
//! The Historify chart page saves the active indicator list and the drawing
//! JSON here and restores them when the symbol is opened again.

use super::SqliteDb;
use crate::error::Result;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// The saved chart state of an `exchange:symbol`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPreference {
    pub symbol: String,
    pub exchange: String,
    pub timeframe: String,
    pub chart_type: String,
    /// JSON list of the active indicators (frontend `ActiveIndicator[]`), if any
    pub indicators: Option<String>,
    /// `DrawingManager.exportJSON()` output, if any
    pub drawings: Option<String>,
    /// SQLite `datetime('now')`: `YYYY-MM-DD HH:MM:SS`, UTC
    pub updated_at: String,
}

/// The saved preference of a symbol, if one exists
pub fn get_preference(
    conn: &Connection,
    exchange: &str,
    symbol: &str,
) -> Result<Option<ChartPreference>> {
    let pref = conn
        .query_row(
            "SELECT symbol, exchange, timeframe, chart_type, indicators, drawings, updated_at
             FROM chart_preferences
             WHERE exchange = ?1 AND symbol = ?2",
            rusqlite::params![exchange, symbol],
            |row| {
                Ok(ChartPreference {
                    symbol: row.get(0)?,
                    exchange: row.get(1)?,
                    timeframe: row.get(2)?,
                    chart_type: row.get(3)?,
                    indicators: row.get(4)?,
                    drawings: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()?;
    Ok(pref)
}

/// Upsert the saved indicators and drawings of a symbol. Inserting leaves
/// `timeframe` and `chart_type` at the table defaults; updating touches only
/// the two JSON columns and `updated_at`.
pub fn save_preference(
    conn: &Connection,
    exchange: &str,
    symbol: &str,
    indicators: Option<&str>,
    drawings: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO chart_preferences (symbol, exchange, indicators, drawings)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(exchange, symbol) DO UPDATE SET
             indicators = excluded.indicators,
             drawings = excluded.drawings,
             updated_at = datetime('now')",
        rusqlite::params![symbol, exchange, indicators, drawings],
    )?;
    Ok(())
}

impl SqliteDb {
    /// The saved chart preference of an `exchange:symbol`, if any
    pub fn get_chart_preference(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> Result<Option<ChartPreference>> {
        let conn = self.conn.lock();
        get_preference(&conn, exchange, symbol)
    }

    /// Upsert the saved indicators and drawings of an `exchange:symbol`
    pub fn save_chart_preference(
        &self,
        exchange: &str,
        symbol: &str,
        indicators: Option<&str>,
        drawings: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        save_preference(&conn, exchange, symbol, indicators, drawings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn missing_preference_is_none() {
        let conn = setup();
        assert!(get_preference(&conn, "NASDAQ", "AAPL").unwrap().is_none());
    }

    #[test]
    fn save_upserts_one_row_per_exchange_symbol_and_latest_wins() {
        let conn = setup();
        save_preference(&conn, "NASDAQ", "AAPL", Some("[1]"), Some("[d]")).unwrap();
        save_preference(&conn, "NASDAQ", "AAPL", Some("[2]"), Some("[e]")).unwrap();

        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chart_preferences WHERE exchange = 'NASDAQ' AND symbol = 'AAPL'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1);

        let pref = get_preference(&conn, "NASDAQ", "AAPL").unwrap().unwrap();
        assert_eq!(pref.exchange, "NASDAQ");
        assert_eq!(pref.symbol, "AAPL");
        assert_eq!(pref.indicators.as_deref(), Some("[2]"));
        assert_eq!(pref.drawings.as_deref(), Some("[e]"));
        // Inserting leaves the table defaults for the columns this WP does not set.
        assert_eq!(pref.timeframe, "1D");
        assert_eq!(pref.chart_type, "candle");
        // SQLite datetime('now'): `YYYY-MM-DD HH:MM:SS`, UTC.
        assert!(chrono::NaiveDateTime::parse_from_str(&pref.updated_at, "%Y-%m-%d %H:%M:%S").is_ok());
    }

    #[test]
    fn preferences_are_keyed_by_exchange_and_symbol() {
        let conn = setup();
        save_preference(&conn, "NASDAQ", "AAPL", None, Some("[nasdaq]")).unwrap();
        save_preference(&conn, "NYSE", "AAPL", None, Some("[nyse]")).unwrap();
        assert_eq!(
            get_preference(&conn, "NASDAQ", "AAPL").unwrap().unwrap().drawings.as_deref(),
            Some("[nasdaq]")
        );
        assert_eq!(
            get_preference(&conn, "NYSE", "AAPL").unwrap().unwrap().drawings.as_deref(),
            Some("[nyse]")
        );
        assert!(get_preference(&conn, "NYSE", "MSFT").unwrap().is_none());
    }

    #[test]
    fn save_with_none_clears_a_column() {
        let conn = setup();
        save_preference(&conn, "NASDAQ", "AAPL", Some("[1]"), Some("[d]")).unwrap();
        save_preference(&conn, "NASDAQ", "AAPL", None, None).unwrap();
        let pref = get_preference(&conn, "NASDAQ", "AAPL").unwrap().unwrap();
        assert_eq!(pref.indicators, None);
        assert_eq!(pref.drawings, None);
    }
}
