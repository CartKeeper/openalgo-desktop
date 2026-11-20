//! Ride entry date cache and dismissed protective-stop records

use crate::error::Result;
use rusqlite::{Connection, OptionalExtension};

/// Cached entry date for a symbol+quantity lot, if computed, with when it was
/// computed: `(entered_at, computed_at)`, `computed_at` as SQLite UTC
pub fn get_ride_entry(conn: &Connection, symbol: &str, quantity: f64) -> Result<Option<(String, String)>> {
    let entry = conn
        .query_row(
            "SELECT entered_at, computed_at FROM ride_entries WHERE symbol = ?1 AND quantity = ?2",
            rusqlite::params![symbol, quantity],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(entry)
}

/// Store or refresh the entry date for a symbol+quantity lot
pub fn upsert_ride_entry(conn: &Connection, symbol: &str, quantity: f64, entered_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO ride_entries (symbol, quantity, entered_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(symbol, quantity) DO UPDATE SET
             entered_at = excluded.entered_at,
             computed_at = datetime('now')",
        rusqlite::params![symbol, quantity, entered_at],
    )?;
    Ok(())
}

/// When the user dismissed a protective stop proposal for a symbol+quantity, if ever
pub fn is_protection_dismissed(conn: &Connection, symbol: &str, quantity: f64) -> Result<Option<String>> {
    let dismissed_at = conn
        .query_row(
            "SELECT dismissed_at FROM dismissed_protection WHERE symbol = ?1 AND quantity = ?2",
            rusqlite::params![symbol, quantity],
            |row| row.get(0),
        )
        .optional()?;
    Ok(dismissed_at)
}

/// Record that the user dismissed a protective stop proposal for a symbol+quantity
pub fn record_protection_dismissal(conn: &Connection, symbol: &str, quantity: f64) -> Result<()> {
    conn.execute(
        "INSERT INTO dismissed_protection (symbol, quantity) VALUES (?1, ?2)
         ON CONFLICT(symbol, quantity) DO UPDATE SET dismissed_at = datetime('now')",
        rusqlite::params![symbol, quantity],
    )?;
    Ok(())
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
    fn ride_entry_roundtrip_keyed_by_symbol_and_qty() {
        let conn = setup();
        upsert_ride_entry(&conn, "PFE", 11.0, "2026-01-05").unwrap();
        let (entered_at, computed_at) = get_ride_entry(&conn, "PFE", 11.0).unwrap().unwrap();
        assert_eq!(entered_at, "2026-01-05");
        // SQLite datetime('now'): `YYYY-MM-DD HH:MM:SS`, UTC.
        assert!(chrono::NaiveDateTime::parse_from_str(&computed_at, "%Y-%m-%d %H:%M:%S").is_ok());
        assert_eq!(get_ride_entry(&conn, "PFE", 12.0).unwrap(), None);
    }

    #[test]
    fn dismissal_expires_when_quantity_changes() {
        let conn = setup();
        record_protection_dismissal(&conn, "NVDA", 4.0).unwrap();
        assert!(is_protection_dismissed(&conn, "NVDA", 4.0).unwrap().is_some());
        assert_eq!(is_protection_dismissed(&conn, "NVDA", 5.0).unwrap(), None);
    }
}
