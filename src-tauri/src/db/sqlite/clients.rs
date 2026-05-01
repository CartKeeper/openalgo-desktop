//! Client management CRUD operations

use crate::error::{AppError, Result};
use crate::providers::types::{
    Client, ClientHolding, ClientOpenOrder, ClientPosition, ClientTrade, ComplianceViolation,
    ImportBatch,
};
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
    account_type: Option<&str>,
    notes: Option<&str>,
) -> Result<Client> {
    conn.execute(
        "INSERT INTO clients (name, email, phone, broker, account_id, account_type, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, email, phone, broker, account_id, account_type, notes],
    )?;
    let id = conn.last_insert_rowid();
    get_client_by_id(conn, id)
}

/// Get all clients
pub fn get_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, email, phone, broker, account_id, account_type, notes, created_at, updated_at
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
                account_type: row.get(6)?,
                notes: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(clients)
}

/// Get a single client by ID
pub fn get_client_by_id(conn: &Connection, id: i64) -> Result<Client> {
    conn.query_row(
        "SELECT id, name, email, phone, broker, account_id, account_type, notes, created_at, updated_at
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
                account_type: row.get(6)?,
                notes: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
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
    account_type: Option<&str>,
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
    if let Some(v) = account_type {
        updates.push("account_type = ?");
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
    account_type: Option<&str>,
) -> Result<ImportBatch> {
    conn.execute(
        "INSERT INTO import_batches (client_id, filename, row_count, account_type)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![client_id, filename, row_count, account_type],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, client_id, filename, row_count, account_type, imported_at FROM import_batches WHERE id = ?1",
        [id],
        |row| {
            Ok(ImportBatch {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                filename: row.get(2)?,
                row_count: row.get(3)?,
                account_type: row.get(4)?,
                imported_at: row.get(5)?,
            })
        },
    )
    .map_err(Into::into)
}

/// Get all import batches for a client
pub fn get_import_batches(conn: &Connection, client_id: i64) -> Result<Vec<ImportBatch>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, filename, row_count, account_type, imported_at
         FROM import_batches WHERE client_id = ?1 ORDER BY imported_at DESC",
    )?;

    let batches = stmt
        .query_map([client_id], |row| {
            Ok(ImportBatch {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                filename: row.get(2)?,
                row_count: row.get(3)?,
                account_type: row.get(4)?,
                imported_at: row.get(5)?,
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

/// Update the account_type on an existing import batch.
pub fn update_import_batch_account_type(
    conn: &Connection,
    batch_id: i64,
    account_type: Option<&str>,
) -> Result<ImportBatch> {
    conn.execute(
        "UPDATE import_batches SET account_type = ?1 WHERE id = ?2",
        rusqlite::params![account_type, batch_id],
    )?;
    conn.query_row(
        "SELECT id, client_id, filename, row_count, account_type, imported_at FROM import_batches WHERE id = ?1",
        [batch_id],
        |row| {
            Ok(ImportBatch {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                filename: row.get(2)?,
                row_count: row.get(3)?,
                account_type: row.get(4)?,
                imported_at: row.get(5)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Import batch {} not found", batch_id))
        }
        other => other.into(),
    })
}

// ---------------------------------------------------------------------------
// Account Discovery
// ---------------------------------------------------------------------------

/// A distinct account found from import batches for a client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClientAccount {
    pub account_type: String,
    pub trade_count: i64,
    pub batch_count: i64,
}

/// Get distinct accounts (from import batches) for a client.
/// Returns each unique account_type along with how many trades and batches it has.
pub fn get_client_accounts(conn: &Connection, client_id: i64) -> Result<Vec<ClientAccount>> {
    let mut stmt = conn.prepare(
        "SELECT
            COALESCE(ib.account_type, 'unspecified') AS acct_type,
            COUNT(DISTINCT ct.id) AS trade_count,
            COUNT(DISTINCT ib.id) AS batch_count
         FROM import_batches ib
         LEFT JOIN client_trades ct ON ct.import_batch_id = ib.id
         WHERE ib.client_id = ?1
         GROUP BY acct_type
         ORDER BY acct_type ASC",
    )?;

    let accounts = stmt
        .query_map([client_id], |row| {
            Ok(ClientAccount {
                account_type: row.get(0)?,
                trade_count: row.get(1)?,
                batch_count: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Also count manual trades (no import_batch_id) as "manual"
    let manual_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM client_trades WHERE client_id = ?1 AND import_batch_id IS NULL",
        [client_id],
        |row| row.get(0),
    )?;

    let mut result = accounts;
    if manual_count > 0 {
        result.push(ClientAccount {
            account_type: "manual".to_string(),
            trade_count: manual_count,
            batch_count: 0,
        });
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Computed Positions
// ---------------------------------------------------------------------------

/// Internal helper: fetch trade rows with optional account_type filter.
fn fetch_trade_rows(
    conn: &Connection,
    client_id: i64,
    account_type: Option<&str>,
) -> Result<Vec<(String, String, String, f64, f64, f64)>> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match account_type {
        Some("manual") => {
            // Manual trades only (no import batch)
            (
                "SELECT ct.symbol, ct.exchange, ct.trade_type, ct.quantity, ct.price, ct.fees
                 FROM client_trades ct
                 WHERE ct.client_id = ?1 AND ct.import_batch_id IS NULL
                 ORDER BY ct.symbol, ct.exchange, ct.trade_date ASC, ct.id ASC".to_string(),
                vec![Box::new(client_id) as Box<dyn rusqlite::ToSql>],
            )
        }
        Some(acct) => {
            // Filter by account_type via import_batches join
            (
                "SELECT ct.symbol, ct.exchange, ct.trade_type, ct.quantity, ct.price, ct.fees
                 FROM client_trades ct
                 JOIN import_batches ib ON ct.import_batch_id = ib.id
                 WHERE ct.client_id = ?1 AND COALESCE(ib.account_type, 'unspecified') = ?2
                 ORDER BY ct.symbol, ct.exchange, ct.trade_date ASC, ct.id ASC".to_string(),
                vec![
                    Box::new(client_id) as Box<dyn rusqlite::ToSql>,
                    Box::new(acct.to_string()) as Box<dyn rusqlite::ToSql>,
                ],
            )
        }
        None => {
            // All trades (original behavior)
            (
                "SELECT symbol, exchange, trade_type, quantity, price, fees
                 FROM client_trades
                 WHERE client_id = ?1
                 ORDER BY symbol, exchange, trade_date ASC, id ASC".to_string(),
                vec![Box::new(client_id) as Box<dyn rusqlite::ToSql>],
            )
        }
    };

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
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

    Ok(rows)
}

/// Internal helper: compute positions from trade rows.
fn compute_positions(rows: Vec<(String, String, String, f64, f64, f64)>, account_type: Option<String>) -> Vec<ClientPosition> {
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
            let old_cost = entry.0 * entry.1;
            let new_cost = qty * price;
            entry.0 += qty;
            if entry.0.abs() > 1e-10 {
                entry.1 = (old_cost + new_cost) / entry.0;
            }
        } else if tt == "sell" {
            let sell_pnl = qty * (price - entry.1);
            entry.4 += sell_pnl;
            entry.0 -= qty;
        }
    }

    positions
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
                account_type: account_type.clone(),
            }
        })
        .collect()
}

/// Compute net positions from trade history for a client (all accounts aggregated).
/// Uses weighted-average cost basis. Buy trades increase position; sell trades reduce it.
pub fn get_client_positions(conn: &Connection, client_id: i64) -> Result<Vec<ClientPosition>> {
    let rows = fetch_trade_rows(conn, client_id, None)?;
    Ok(compute_positions(rows, None))
}

/// Compute net positions filtered by account_type.
pub fn get_client_positions_by_account(
    conn: &Connection,
    client_id: i64,
    account_type: &str,
) -> Result<Vec<ClientPosition>> {
    let rows = fetch_trade_rows(conn, client_id, Some(account_type))?;
    Ok(compute_positions(rows, Some(account_type.to_string())))
}

/// Compute net positions per account (not aggregated across accounts).
/// Returns separate position rows for each account_type so the "All Accounts"
/// view can show which account each position belongs to.
pub fn get_client_positions_by_each_account(
    conn: &Connection,
    client_id: i64,
) -> Result<Vec<ClientPosition>> {
    let accounts = get_client_accounts(conn, client_id)?;
    let mut all_positions = Vec::new();

    for acct in &accounts {
        let rows = fetch_trade_rows(conn, client_id, Some(&acct.account_type))?;
        let positions = compute_positions(rows, Some(acct.account_type.clone()));
        all_positions.extend(positions);
    }

    Ok(all_positions)
}

/// Get trades filtered by account_type.
pub fn get_trades_by_account(
    conn: &Connection,
    client_id: i64,
    account_type: &str,
) -> Result<Vec<ClientTrade>> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if account_type == "manual" {
        (
            "SELECT ct.id, ct.client_id, ct.import_batch_id, ct.symbol, ct.exchange, ct.trade_date, ct.trade_type, ct.quantity, ct.price, ct.fees, ct.order_id, ct.notes, ct.created_at
             FROM client_trades ct
             WHERE ct.client_id = ?1 AND ct.import_batch_id IS NULL
             ORDER BY ct.trade_date DESC, ct.id DESC".to_string(),
            vec![Box::new(client_id) as Box<dyn rusqlite::ToSql>],
        )
    } else {
        (
            "SELECT ct.id, ct.client_id, ct.import_batch_id, ct.symbol, ct.exchange, ct.trade_date, ct.trade_type, ct.quantity, ct.price, ct.fees, ct.order_id, ct.notes, ct.created_at
             FROM client_trades ct
             JOIN import_batches ib ON ct.import_batch_id = ib.id
             WHERE ct.client_id = ?1 AND COALESCE(ib.account_type, 'unspecified') = ?2
             ORDER BY ct.trade_date DESC, ct.id DESC".to_string(),
            vec![
                Box::new(client_id) as Box<dyn rusqlite::ToSql>,
                Box::new(account_type.to_string()) as Box<dyn rusqlite::ToSql>,
            ],
        )
    };

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let trades = stmt
        .query_map(params_refs.as_slice(), |row| {
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

// ---------------------------------------------------------------------------
// Documents (raw broker file uploads)
// ---------------------------------------------------------------------------

pub fn add_client_document(
    conn: &Connection,
    client_id: i64,
    doc_type: &str,
    filename: &str,
    content: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO client_documents (client_id, doc_type, filename, content, byte_size)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![client_id, doc_type, filename, content, content.len() as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

// ---------------------------------------------------------------------------
// Holdings (replaced wholesale on each import)
// ---------------------------------------------------------------------------

pub fn replace_client_holdings(
    conn: &mut Connection,
    client_id: i64,
    holdings: &[ClientHolding],
) -> Result<usize> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM client_holdings WHERE client_id = ?1", [client_id])?;
    let mut inserted = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO client_holdings
             (client_id, symbol, description, quantity, avg_cost, total_cost, realized_pnl, last_activity_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for h in holdings {
            stmt.execute(rusqlite::params![
                client_id,
                h.symbol,
                h.description,
                h.quantity,
                h.avg_cost,
                h.total_cost,
                h.realized_pnl,
                h.last_activity_date,
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

pub fn get_client_holdings(conn: &Connection, client_id: i64) -> Result<Vec<ClientHolding>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, symbol, description, quantity, avg_cost, total_cost,
                realized_pnl, last_activity_date, updated_at
         FROM client_holdings WHERE client_id = ?1 ORDER BY symbol ASC",
    )?;
    let rows = stmt
        .query_map([client_id], |row| {
            Ok(ClientHolding {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                symbol: row.get(2)?,
                description: row.get(3)?,
                quantity: row.get(4)?,
                avg_cost: row.get(5)?,
                total_cost: row.get(6)?,
                realized_pnl: row.get(7)?,
                last_activity_date: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Open orders (replaced wholesale on each import)
// ---------------------------------------------------------------------------

pub fn replace_client_open_orders(
    conn: &mut Connection,
    client_id: i64,
    orders: &[ClientOpenOrder],
) -> Result<usize> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM client_open_orders WHERE client_id = ?1", [client_id])?;
    let mut inserted = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO client_open_orders
             (client_id, order_number, symbol, description, action, quantity, order_type,
              limit_price, stop_price, time_in_force, status, placed_at, last_activity_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;
        for o in orders {
            stmt.execute(rusqlite::params![
                client_id,
                o.order_number,
                o.symbol,
                o.description,
                o.action,
                o.quantity,
                o.order_type,
                o.limit_price,
                o.stop_price,
                o.time_in_force,
                o.status,
                o.placed_at,
                o.last_activity_at,
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

pub fn get_client_open_orders(conn: &Connection, client_id: i64) -> Result<Vec<ClientOpenOrder>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, order_number, symbol, description, action, quantity, order_type,
                limit_price, stop_price, time_in_force, status, placed_at, last_activity_at, updated_at
         FROM client_open_orders WHERE client_id = ?1 ORDER BY placed_at DESC",
    )?;
    let rows = stmt
        .query_map([client_id], |row| {
            Ok(ClientOpenOrder {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                order_number: row.get(2)?,
                symbol: row.get(3)?,
                description: row.get(4)?,
                action: row.get(5)?,
                quantity: row.get(6)?,
                order_type: row.get(7)?,
                limit_price: row.get(8)?,
                stop_price: row.get(9)?,
                time_in_force: row.get(10)?,
                status: row.get(11)?,
                placed_at: row.get(12)?,
                last_activity_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Compliance violations (replaced wholesale on each import)
// ---------------------------------------------------------------------------

pub fn replace_client_compliance_violations(
    conn: &mut Connection,
    client_id: i64,
    violations: &[ComplianceViolation],
) -> Result<usize> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM client_compliance_violations WHERE client_id = ?1",
        [client_id],
    )?;
    let mut inserted = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO client_compliance_violations
             (client_id, rule_set, violation_type, severity, symbol, quantity, message, resolved)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for v in violations {
            stmt.execute(rusqlite::params![
                client_id,
                v.rule_set,
                v.violation_type,
                v.severity,
                v.symbol,
                v.quantity,
                v.message,
                v.resolved as i64,
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

pub fn get_client_compliance_violations(
    conn: &Connection,
    client_id: i64,
) -> Result<Vec<ComplianceViolation>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, rule_set, violation_type, severity, symbol, quantity, message,
                detected_at, resolved, resolved_reason, resolved_at
         FROM client_compliance_violations WHERE client_id = ?1
         ORDER BY resolved ASC, detected_at DESC",
    )?;
    let rows = stmt
        .query_map([client_id], |row| {
            let resolved: i64 = row.get(9)?;
            Ok(ComplianceViolation {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                rule_set: row.get(2)?,
                violation_type: row.get(3)?,
                severity: row.get(4)?,
                symbol: row.get(5)?,
                quantity: row.get(6)?,
                message: row.get(7)?,
                detected_at: row.get(8)?,
                resolved: resolved != 0,
                resolved_reason: row.get(10)?,
                resolved_at: row.get(11)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Mark a single violation resolved with an audit reason (or unresolve when reason is None).
pub fn resolve_compliance_violation(
    conn: &Connection,
    id: i64,
    reason: Option<&str>,
) -> Result<ComplianceViolation> {
    if let Some(r) = reason {
        conn.execute(
            "UPDATE client_compliance_violations
             SET resolved = 1, resolved_reason = ?1, resolved_at = datetime('now')
             WHERE id = ?2",
            rusqlite::params![r, id],
        )?;
    } else {
        conn.execute(
            "UPDATE client_compliance_violations
             SET resolved = 0, resolved_reason = NULL, resolved_at = NULL
             WHERE id = ?1",
            rusqlite::params![id],
        )?;
    }
    conn.query_row(
        "SELECT id, client_id, rule_set, violation_type, severity, symbol, quantity, message,
                detected_at, resolved, resolved_reason, resolved_at
         FROM client_compliance_violations WHERE id = ?1",
        [id],
        |row| {
            let resolved: i64 = row.get(9)?;
            Ok(ComplianceViolation {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                rule_set: row.get(2)?,
                violation_type: row.get(3)?,
                severity: row.get(4)?,
                symbol: row.get(5)?,
                quantity: row.get(6)?,
                message: row.get(7)?,
                detected_at: row.get(8)?,
                resolved: resolved != 0,
                resolved_reason: row.get(10)?,
                resolved_at: row.get(11)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Violation {} not found", id))
        }
        other => other.into(),
    })
}

/// Count violations still flagged (resolved = 0). Used for the ClientDetail banner.
pub fn count_unresolved_violations(conn: &Connection, client_id: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM client_compliance_violations
         WHERE client_id = ?1 AND resolved = 0",
        [client_id],
        |row| row.get(0),
    )?;
    Ok(count)
}
