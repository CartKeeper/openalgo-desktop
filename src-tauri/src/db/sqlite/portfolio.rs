//! Manual portfolio CRUD operations

use crate::error::{AppError, Result};
use crate::providers::types::PortfolioPosition;
use rusqlite::Connection;

/// Add a portfolio position
pub fn add_position(
    conn: &Connection,
    symbol: &str,
    exchange: &str,
    quantity: f64,
    avg_price: f64,
    position_type: &str,
    asset_class: Option<&str>,
    notes: Option<&str>,
) -> Result<PortfolioPosition> {
    conn.execute(
        "INSERT INTO portfolio_positions (symbol, exchange, quantity, avg_price, position_type, asset_class, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![symbol, exchange, quantity, avg_price, position_type, asset_class, notes],
    )?;

    let id = conn.last_insert_rowid();
    get_position_by_id(conn, id)
}

/// Update a portfolio position
pub fn update_position(
    conn: &Connection,
    id: i64,
    quantity: Option<f64>,
    avg_price: Option<f64>,
    notes: Option<&str>,
) -> Result<PortfolioPosition> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(q) = quantity {
        updates.push("quantity = ?");
        params.push(Box::new(q));
    }
    if let Some(p) = avg_price {
        updates.push("avg_price = ?");
        params.push(Box::new(p));
    }
    if let Some(n) = notes {
        updates.push("notes = ?");
        params.push(Box::new(n.to_string()));
    }

    if updates.is_empty() {
        return get_position_by_id(conn, id);
    }

    updates.push("updated_at = datetime('now')");
    params.push(Box::new(id));

    let sql = format!(
        "UPDATE portfolio_positions SET {} WHERE id = ?",
        updates.join(", ")
    );

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())?;

    get_position_by_id(conn, id)
}

/// Delete a portfolio position
pub fn delete_position(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM portfolio_positions WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

/// Get all portfolio positions
pub fn get_positions(conn: &Connection) -> Result<Vec<PortfolioPosition>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, exchange, quantity, avg_price, position_type, asset_class, notes, added_at, updated_at
         FROM portfolio_positions ORDER BY added_at DESC",
    )?;

    let positions = stmt
        .query_map([], |row| {
            Ok(PortfolioPosition {
                id: Some(row.get(0)?),
                symbol: row.get(1)?,
                exchange: row.get(2)?,
                quantity: row.get(3)?,
                avg_price: row.get(4)?,
                current_price: None,
                pnl: None,
                pnl_percent: None,
                position_type: row.get(5)?,
                asset_class: row.get(6)?,
                notes: row.get(7)?,
                added_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(positions)
}

/// Get a single position by ID
fn get_position_by_id(conn: &Connection, id: i64) -> Result<PortfolioPosition> {
    conn.query_row(
        "SELECT id, symbol, exchange, quantity, avg_price, position_type, asset_class, notes, added_at, updated_at
         FROM portfolio_positions WHERE id = ?1",
        [id],
        |row| {
            Ok(PortfolioPosition {
                id: Some(row.get(0)?),
                symbol: row.get(1)?,
                exchange: row.get(2)?,
                quantity: row.get(3)?,
                avg_price: row.get(4)?,
                current_price: None,
                pnl: None,
                pnl_percent: None,
                position_type: row.get(5)?,
                asset_class: row.get(6)?,
                notes: row.get(7)?,
                added_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Position {} not found", id)),
        other => other.into(),
    })
}

/// Import positions from CSV data (parsed rows)
pub fn import_positions(
    conn: &Connection,
    positions: &[PortfolioPosition],
) -> Result<usize> {
    let mut count = 0;
    for pos in positions {
        conn.execute(
            "INSERT OR REPLACE INTO portfolio_positions (symbol, exchange, quantity, avg_price, position_type, asset_class, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                pos.symbol,
                pos.exchange,
                pos.quantity,
                pos.avg_price,
                pos.position_type,
                pos.asset_class,
                pos.notes,
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Export all positions (same as get_positions, used for CSV export)
pub fn export_positions(conn: &Connection) -> Result<Vec<PortfolioPosition>> {
    get_positions(conn)
}

/// Get or set generic mode flag
pub fn get_generic_mode(conn: &Connection) -> Result<bool> {
    let mode: bool = conn.query_row(
        "SELECT generic_mode FROM settings WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(mode)
}

pub fn set_generic_mode(conn: &Connection, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE settings SET generic_mode = ?1 WHERE id = 1",
        [enabled],
    )?;
    Ok(())
}
