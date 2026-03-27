//! Client management CRUD operations

use crate::error::{AppError, Result};
use crate::providers::types::{Client, ClientPosition, ClientTrade, ImportBatch};
use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Clients
// ---------------------------------------------------------------------------

/// Add a new client
pub fn add_client(
    conn: &Connection,
    name: &str,
    email: Option<&str>,
    phone: Option<&str>,
    broker: Option<&str>,
    account_id: Option<&str>,
    notes: Option<&str>,
) -> Result<Client> {
    conn.execute(
        "INSERT INTO clients (name, email, phone, broker, account_id, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![name, email, phone, broker, account_id, notes],
    )?;
    let id = conn.last_insert_rowid();
    get_client_by_id(conn, id)
}

/// Get all clients
pub fn get_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, email, phone, broker, account_id, notes, created_at, updated_at
         FROM clients ORDER BY name ASC",
    )?;

    let clients = stmt
        .query_map([], |row| {
            Ok(Client {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                email: row.get(2)?,
                phone: row.get(3)?,
                broker: row.get(4)?,
                account_id: row.get(5)?,
                notes: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(clients)
}

/// Get a single client by ID
pub fn get_client_by_id(conn: &Connection, id: i64) -> Result<Client> {
    conn.query_row(
        "SELECT id, name, email, phone, broker, account_id, notes, created_at, updated_at
         FROM clients WHERE id = ?1",
        [id],
        |row| {
            Ok(Client {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                email: row.get(2)?,
                phone: row.get(3)?,
                broker: row.get(4)?,
                account_id: row.get(5)?,
                notes: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Client {} not found", id))
        }
        other => other.into(),
    })
}

/// Update a client
pub fn update_client(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    email: Option<&str>,
    phone: Option<&str>,
    broker: Option<&str>,
    account_id: Option<&str>,
    notes: Option<&str>,
) -> Result<Client> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = name {
        updates.push("name = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = email {
        updates.push("email = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = phone {
        updates.push("phone = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = broker {
        updates.push("broker = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = account_id {
        updates.push("account_id = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = notes {
        updates.push("notes = ?");
        params.push(Box::new(v.to_string()));
    }

    if updates.is_empty() {
        return get_client_by_id(conn, id);
    }

    updates.push("updated_at = datetime('now')");
    params.push(Box::new(id));

    let sql = format!("UPDATE clients SET {} WHERE id = ?", updates.join(", "));
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())?;

    get_client_by_id(conn, id)
}

/// Delete a client (cascades to trades and import batches)
pub fn delete_client(conn: &Connection, id: i64) -> Result<bool> {
    // Enable foreign keys for cascade
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let rows = conn.execute("DELETE FROM clients WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

// ---------------------------------------------------------------------------
// Trades
// ---------------------------------------------------------------------------

/// Add a single trade for a client
pub fn add_trade(
    conn: &Connection,
    client_id: i64,
    symbol: &str,
    exchange: &str,
    trade_date: &str,
    trade_type: &str,
    quantity: f64,
    price: f64,
    fees: f64,
    order_id: Option<&str>,
    notes: Option<&str>,
    import_batch_id: Option<i64>,
) -> Result<ClientTrade> {
    conn.execute(
        "INSERT INTO client_trades (client_id, import_batch_id, symbol, exchange, trade_date, trade_type, quantity, price, fees, order_id, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            client_id,
            import_batch_id,
            symbol,
            exchange,
            trade_date,
            trade_type,
            quantity,
            price,
            fees,
            order_id,
            notes,
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_trade_by_id(conn, id)
}

/// Get all trades for a client
pub fn get_trades(conn: &Connection, client_id: i64) -> Result<Vec<ClientTrade>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, import_batch_id, symbol, exchange, trade_date, trade_type, quantity, price, fees, order_id, notes, created_at
         FROM client_trades WHERE client_id = ?1 ORDER BY trade_date DESC, id DESC",
    )?;

    let trades = stmt
        .query_map([client_id], |row| {
            Ok(ClientTrade {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                import_batch_id: row.get(2)?,
                symbol: row.get(3)?,
                exchange: row.get(4)?,
                trade_date: row.get(5)?,
                trade_type: row.get(6)?,
                quantity: row.get(7)?,
                price: row.get(8)?,
                fees: row.get(9)?,
                order_id: row.get(10)?,
                notes: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(trades)
}

/// Get a single trade by ID
fn get_trade_by_id(conn: &Connection, id: i64) -> Result<ClientTrade> {
    conn.query_row(
        "SELECT id, client_id, import_batch_id, symbol, exchange, trade_date, trade_type, quantity, price, fees, order_id, notes, created_at
         FROM client_trades WHERE id = ?1",
        [id],
        |row| {
            Ok(ClientTrade {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                import_batch_id: row.get(2)?,
                symbol: row.get(3)?,
                exchange: row.get(4)?,
                trade_date: row.get(5)?,
                trade_type: row.get(6)?,
                quantity: row.get(7)?,
                price: row.get(8)?,
                fees: row.get(9)?,
                order_id: row.get(10)?,
                notes: row.get(11)?,
                created_at: row.get(12)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Trade {} not found", id))
        }
        other => other.into(),
    })
}

/// Delete a single trade
pub fn delete_trade(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM client_trades WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

// ---------------------------------------------------------------------------
// Import Batches
// ---------------------------------------------------------------------------

/// Create an import batch record
pub fn add_import_batch(
    conn: &Connection,
    client_id: i64,
    filename: &str,
    row_count: i64,
) -> Result<ImportBatch> {
    conn.execute(
        "INSERT INTO import_batches (client_id, filename, row_count)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![client_id, filename, row_count],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, client_id, filename, row_count, imported_at FROM import_batches WHERE id = ?1",
        [id],
        |row| {
            Ok(ImportBatch {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                filename: row.get(2)?,
                row_count: row.get(3)?,
                imported_at: row.get(4)?,
            })
        },
    )
    .map_err(Into::into)
}

/// Get all import batches for a client
pub fn get_import_batches(conn: &Connection, client_id: i64) -> Result<Vec<ImportBatch>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, filename, row_count, imported_at
         FROM import_batches WHERE client_id = ?1 ORDER BY imported_at DESC",
    )?;

    let batches = stmt
        .query_map([client_id], |row| {
            Ok(ImportBatch {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                filename: row.get(2)?,
                row_count: row.get(3)?,
                imported_at: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(batches)
}

/// Delete an import batch and all its associated trades. Returns count of deleted trades.
pub fn delete_import_batch(conn: &Connection, batch_id: i64) -> Result<usize> {
    let deleted = conn.execute(
        "DELETE FROM client_trades WHERE import_batch_id = ?1",
        [batch_id],
    )?;
    conn.execute("DELETE FROM import_batches WHERE id = ?1", [batch_id])?;
    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Computed Positions
// ---------------------------------------------------------------------------

/// Compute net positions from trade history for a client.
/// Uses weighted-average cost basis. Buy trades increase position; sell trades reduce it.
pub fn get_client_positions(conn: &Connection, client_id: i64) -> Result<Vec<ClientPosition>> {
    // Get all trades grouped by symbol+exchange, ordered by date for FIFO-style processing
    let mut stmt = conn.prepare(
        "SELECT symbol, exchange, trade_type, quantity, price, fees
         FROM client_trades
         WHERE client_id = ?1
         ORDER BY symbol, exchange, trade_date ASC, id ASC",
    )?;

    let rows: Vec<(String, String, String, f64, f64, f64)> = stmt
        .query_map([client_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Group by (symbol, exchange) and compute positions
    let mut positions: std::collections::BTreeMap<(String, String), (f64, f64, f64, i64, f64)> =
        std::collections::BTreeMap::new();

    for (symbol, exchange, trade_type, qty, price, fees) in &rows {
        let entry = positions
            .entry((symbol.clone(), exchange.clone()))
            .or_insert((0.0, 0.0, 0.0, 0, 0.0));
        // entry = (net_qty, cost_basis, total_fees, trade_count, realized_pnl)

        entry.2 += fees; // accumulate fees
        entry.3 += 1; // trade count

        let tt = trade_type.to_lowercase();
        if tt == "buy" {
            // Weighted average: new_cost = (old_qty * old_avg + new_qty * price) / (old_qty + new_qty)
            let old_cost = entry.0 * entry.1;
            let new_cost = qty * price;
            entry.0 += qty;
            if entry.0.abs() > 1e-10 {
                entry.1 = (old_cost + new_cost) / entry.0;
            }
        } else {
            // sell: realize P&L
            let sell_pnl = qty * (price - entry.1);
            entry.4 += sell_pnl;
            entry.0 -= qty;
        }
    }

    let result = positions
        .into_iter()
        .map(|((symbol, exchange), (net_qty, avg_price, total_fees, trade_count, realized_pnl))| {
            ClientPosition {
                symbol,
                exchange,
                net_quantity: net_qty,
                avg_price: if net_qty.abs() > 1e-10 { avg_price } else { 0.0 },
                total_fees,
                trade_count,
                realized_pnl,
            }
        })
        .collect();

    Ok(result)
}
