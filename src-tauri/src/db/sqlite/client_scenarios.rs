//! Client scenario CRUD operations for what-if portfolio modeling

use crate::error::{AppError, Result};
use crate::providers::types::{ClientScenario, ScenarioPosition};
use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

/// Create a new scenario for a client
pub fn create_scenario(
    conn: &Connection,
    client_id: i64,
    name: &str,
    description: Option<&str>,
    is_baseline: bool,
) -> Result<ClientScenario> {
    conn.execute(
        "INSERT INTO client_scenarios (client_id, name, description, is_baseline)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![client_id, name, description, is_baseline as i32],
    )?;
    let id = conn.last_insert_rowid();
    get_scenario_by_id(conn, id)
}

/// Map an account_type value to a human-readable label
fn account_type_label(value: &str) -> &str {
    match value {
        "individual" => "Individual Brokerage",
        "joint" => "Joint Brokerage",
        "margin" => "Margin Account",
        "401k" => "401(k)",
        "roth_401k" => "Roth 401(k)",
        "traditional_ira" => "Traditional IRA",
        "roth_ira" => "Roth IRA",
        "sep_ira" => "SEP IRA",
        "simple_ira" => "SIMPLE IRA",
        "529" => "529 Plan",
        "trust" => "Trust",
        "custodial" => "Custodial (UGMA/UTMA)",
        "other" => "Other",
        _ => value,
    }
}

/// Get all scenarios for a client
pub fn get_scenarios(conn: &Connection, client_id: i64) -> Result<Vec<ClientScenario>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, name, description, is_baseline, cloned_from_id, account_type, created_at, updated_at, cash
         FROM client_scenarios WHERE client_id = ?1 ORDER BY is_baseline DESC, name ASC",
    )?;

    let scenarios = stmt
        .query_map([client_id], |row| {
            Ok(ClientScenario {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                is_baseline: row.get::<_, i32>(4)? != 0,
                cloned_from_id: row.get(5)?,
                account_type: row.get(6)?,
                cash: row.get(9)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(scenarios)
}

/// Get a single scenario by ID
pub fn get_scenario_by_id(conn: &Connection, id: i64) -> Result<ClientScenario> {
    conn.query_row(
        "SELECT id, client_id, name, description, is_baseline, cloned_from_id, account_type, created_at, updated_at, cash
         FROM client_scenarios WHERE id = ?1",
        [id],
        |row| {
            Ok(ClientScenario {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                is_baseline: row.get::<_, i32>(4)? != 0,
                cloned_from_id: row.get(5)?,
                account_type: row.get(6)?,
                cash: row.get(9)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Scenario {} not found", id))
        }
        other => other.into(),
    })
}

/// Update a scenario
pub fn update_scenario(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<ClientScenario> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = name {
        updates.push("name = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = description {
        updates.push("description = ?");
        params.push(Box::new(v.to_string()));
    }

    if updates.is_empty() {
        return get_scenario_by_id(conn, id);
    }

    updates.push("updated_at = datetime('now')");
    params.push(Box::new(id));

    let sql = format!(
        "UPDATE client_scenarios SET {} WHERE id = ?",
        updates.join(", ")
    );
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())?;

    get_scenario_by_id(conn, id)
}

/// Delete a scenario (cascades to positions)
pub fn delete_scenario(conn: &Connection, id: i64) -> Result<bool> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let rows = conn.execute("DELETE FROM client_scenarios WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

/// Clone a scenario: copies all positions from source into a new scenario
pub fn clone_scenario(
    conn: &Connection,
    source_id: i64,
    client_id: i64,
    name: &str,
    description: Option<&str>,
) -> Result<ClientScenario> {
    // Copy account_type and cash from the source scenario
    let (source_account_type, source_cash): (Option<String>, Option<f64>) = conn
        .query_row(
            "SELECT account_type, cash FROM client_scenarios WHERE id = ?1",
            [source_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((None, None));

    conn.execute(
        "INSERT INTO client_scenarios (client_id, name, description, is_baseline, cloned_from_id, account_type, cash)
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)",
        rusqlite::params![client_id, name, description, source_id, source_account_type, source_cash],
    )?;
    let new_id = conn.last_insert_rowid();

    // Copy all positions from source
    conn.execute(
        "INSERT INTO client_scenario_positions (scenario_id, symbol, exchange, quantity, avg_price, side, notes)
         SELECT ?1, symbol, exchange, quantity, avg_price, side, notes
         FROM client_scenario_positions WHERE scenario_id = ?2",
        rusqlite::params![new_id, source_id],
    )?;

    get_scenario_by_id(conn, new_id)
}

/// Create or update baseline scenarios for a client.
///
/// Dispatch:
///   - If `client_holdings` has rows for this client (Schwab dual-doc importer
///     wrote them), use the holdings table directly as the source of truth —
///     one baseline keyed off the client's `account_type`.
///   - Otherwise fall back to the legacy `client_trades` + `import_batches`
///     reconstruction, preserving behavior for clients imported via the older
///     CSV-trades flow.
///
/// `cash` is the account's cash from the latest import (`None` = not recorded);
/// it sets the holdings baseline's cash.
pub fn sync_baseline_scenarios(
    conn: &Connection,
    client_id: i64,
    cash: Option<f64>,
) -> Result<Vec<ClientScenario>> {
    let has_holdings: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM client_holdings WHERE client_id = ?1)",
        [client_id],
        |row| row.get(0),
    )?;
    if has_holdings {
        return sync_baseline_from_holdings(conn, client_id, cash);
    }
    sync_baseline_from_trades(conn, client_id)
}

/// Build a single baseline scenario from `client_holdings` (the new path).
///
/// Stranded CUSIP-only rows are skipped — they're not actionable in the
/// scenario sandbox. The client's `account_type` from the `clients` table
/// determines the scenario name and the baseline's `account_type` field.
fn sync_baseline_from_holdings(
    conn: &Connection,
    client_id: i64,
    cash: Option<f64>,
) -> Result<Vec<ClientScenario>> {
    // 1. Look up account_type on the client record (set by the importer)
    let account_type: Option<String> = conn.query_row(
        "SELECT account_type FROM clients WHERE id = ?1",
        [client_id],
        |row| row.get(0),
    )?;

    // 2. Find or create the baseline matching this account_type
    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM client_scenarios
             WHERE client_id = ?1 AND is_baseline = 1
               AND ((account_type IS NULL AND ?2 IS NULL) OR account_type = ?2)",
            rusqlite::params![client_id, account_type],
            |row| row.get(0),
        )
        .ok();

    let scenario_id = if let Some(id) = existing_id {
        conn.execute(
            "DELETE FROM client_scenario_positions WHERE scenario_id = ?1",
            [id],
        )?;
        conn.execute(
            "UPDATE client_scenarios SET cash = ?2, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![id, cash],
        )?;
        id
    } else {
        let name = match &account_type {
            Some(at) => format!("Current Portfolio — {}", account_type_label(at)),
            None => "Current Portfolio".to_string(),
        };
        let desc = match &account_type {
            Some(at) => format!("Auto-synced from {} holdings snapshot", account_type_label(at)),
            None => "Auto-synced from holdings snapshot".to_string(),
        };
        conn.execute(
            "INSERT INTO client_scenarios (client_id, name, description, is_baseline, account_type, cash)
             VALUES (?1, ?2, ?3, 1, ?4, ?5)",
            rusqlite::params![client_id, name, desc, account_type, cash],
        )?;
        conn.last_insert_rowid()
    };

    // 3. Pull active holdings (skip stranded CUSIPs and zero-qty rows)
    let mut stmt = conn.prepare(
        "SELECT symbol, quantity, avg_cost
         FROM client_holdings
         WHERE client_id = ?1 AND quantity > 0
         ORDER BY symbol ASC",
    )?;
    let rows: Vec<(String, f64, f64)> = stmt
        .query_map([client_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, f64>(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // 4. Insert as scenario positions (long-only — we never persist negative
    //    quantities on no-shorts accounts thanks to the importer clamp)
    for (symbol, quantity, avg_price) in rows {
        if symbol_is_stranded(&symbol) {
            continue;
        }
        conn.execute(
            "INSERT INTO client_scenario_positions (scenario_id, symbol, exchange, quantity, avg_price, side)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![scenario_id, symbol, "GENERIC", quantity, avg_price, "long"],
        )?;
    }

    // 5. Remove any other baseline scenarios that no longer match (e.g. the
    //    client's account_type changed since last sync)
    let mut cleanup_stmt = conn.prepare(
        "SELECT id FROM client_scenarios
         WHERE client_id = ?1 AND is_baseline = 1 AND id != ?2",
    )?;
    let stale_ids: Vec<i64> = cleanup_stmt
        .query_map(rusqlite::params![client_id, scenario_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in stale_ids {
        conn.execute(
            "DELETE FROM client_scenario_positions WHERE scenario_id = ?1",
            [id],
        )?;
        conn.execute("DELETE FROM client_scenarios WHERE id = ?1", [id])?;
    }

    Ok(vec![get_scenario_by_id(conn, scenario_id)?])
}

/// Heuristic: a symbol is "stranded" (CUSIP-only / no ticker mapping) when it
/// matches one of Schwab's untradable patterns. Mirrors the TS detection on
/// ClientDetail's Holdings tab.
fn symbol_is_stranded(symbol: &str) -> bool {
    if symbol == "NO NUMBER" {
        return true;
    }
    // Schwab uses 9-char alphanumeric CUSIPs as a fallback symbol when no
    // ticker is mapped. Real tickers are 1–5 letters. Treat ≥6-char
    // alphanumerics that contain a digit as CUSIPs.
    symbol.len() >= 6
        && symbol.chars().any(|c| c.is_ascii_digit())
        && symbol.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Legacy: build baselines from `client_trades` + `import_batches`.
/// Creates one baseline per account type found in import batches.
/// Trades with no import batch or batches with no account type go into a generic baseline.
fn sync_baseline_from_trades(conn: &Connection, client_id: i64) -> Result<Vec<ClientScenario>> {
    // 1. Get distinct account types from import batches for this client
    let mut at_stmt = conn.prepare(
        "SELECT DISTINCT ib.account_type
         FROM import_batches ib
         WHERE ib.client_id = ?1",
    )?;
    let account_types: Vec<Option<String>> = at_stmt
        .query_map([client_id], |row| row.get::<_, Option<String>>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Check if there are any manually-added trades (no import_batch_id)
    let has_manual_trades: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM client_trades WHERE client_id = ?1 AND import_batch_id IS NULL)",
        [client_id],
        |row| row.get(0),
    )?;

    // Build a list of groups: each group is an Option<String> account_type
    // NULL account_type batches + manual trades → generic group (None)
    let mut groups: Vec<Option<String>> = Vec::new();
    let mut has_generic = false;

    for at in &account_types {
        if at.is_none() {
            has_generic = true;
        } else {
            groups.push(at.clone());
        }
    }
    // Add generic group if there are NULL-type batches or manual trades
    if has_generic || has_manual_trades {
        groups.push(None);
    }

    // If no groups at all (no trades), create one generic baseline with no positions
    if groups.is_empty() {
        groups.push(None);
    }

    // 2. Get existing baseline scenario IDs for this client
    let mut existing_stmt = conn.prepare(
        "SELECT id, account_type FROM client_scenarios WHERE client_id = ?1 AND is_baseline = 1",
    )?;
    let existing_baselines: Vec<(i64, Option<String>)> = existing_stmt
        .query_map([client_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut results: Vec<ClientScenario> = Vec::new();
    let mut used_baseline_ids: Vec<i64> = Vec::new();

    // 3. For each account_type group, find or create a baseline and populate positions
    for group_at in &groups {
        // Find existing baseline matching this account_type
        let existing_id = existing_baselines
            .iter()
            .find(|(_, at)| at == group_at)
            .map(|(id, _)| *id);

        let scenario_id = if let Some(id) = existing_id {
            // Clear existing positions and update
            conn.execute(
                "DELETE FROM client_scenario_positions WHERE scenario_id = ?1",
                [id],
            )?;
            conn.execute(
                "UPDATE client_scenarios SET updated_at = datetime('now') WHERE id = ?1",
                [id],
            )?;
            id
        } else {
            let name = match group_at {
                Some(at) => format!("Current Portfolio — {}", account_type_label(at)),
                None => "Current Portfolio".to_string(),
            };
            let desc = match group_at {
                Some(at) => format!("Auto-synced from {} account trades", account_type_label(at)),
                None => "Auto-synced from trade history".to_string(),
            };
            conn.execute(
                "INSERT INTO client_scenarios (client_id, name, description, is_baseline, account_type)
                 VALUES (?1, ?2, ?3, 1, ?4)",
                rusqlite::params![client_id, name, desc, group_at],
            )?;
            conn.last_insert_rowid()
        };

        used_baseline_ids.push(scenario_id);

        // Query trades for this group
        let rows: Vec<(String, String, String, f64, f64)> = if let Some(at) = group_at {
            // Trades from import batches with this specific account_type
            let mut stmt = conn.prepare(
                "SELECT ct.symbol, ct.exchange, ct.trade_type, ct.quantity, ct.price
                 FROM client_trades ct
                 JOIN import_batches ib ON ct.import_batch_id = ib.id
                 WHERE ct.client_id = ?1 AND ib.account_type = ?2
                 ORDER BY ct.symbol, ct.exchange, ct.trade_date ASC, ct.id ASC",
            )?;
            let result: Vec<(String, String, String, f64, f64)> = stmt
                .query_map(rusqlite::params![client_id, at], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, f64>(3)?,
                        row.get::<_, f64>(4)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        } else {
            // Generic group: trades with no batch OR batches with NULL account_type
            let mut stmt = conn.prepare(
                "SELECT ct.symbol, ct.exchange, ct.trade_type, ct.quantity, ct.price
                 FROM client_trades ct
                 LEFT JOIN import_batches ib ON ct.import_batch_id = ib.id
                 WHERE ct.client_id = ?1 AND (ct.import_batch_id IS NULL OR ib.account_type IS NULL)
                 ORDER BY ct.symbol, ct.exchange, ct.trade_date ASC, ct.id ASC",
            )?;
            let result: Vec<(String, String, String, f64, f64)> = stmt
                .query_map([client_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, f64>(3)?,
                        row.get::<_, f64>(4)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        };

        // Compute positions using weighted-average cost basis with correct
        // handling of position-flipping (long → short → long, etc.).
        let positions = compute_positions_from_trades(&rows);

        // Insert positions with non-zero quantity
        for ((symbol, exchange), (net_qty, avg_price)) in &positions {
            if net_qty.abs() > 1e-10 {
                let side = if *net_qty > 0.0 { "long" } else { "short" };
                conn.execute(
                    "INSERT INTO client_scenario_positions (scenario_id, symbol, exchange, quantity, avg_price, side)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![scenario_id, symbol, exchange, net_qty.abs(), avg_price, side],
                )?;
            }
        }

        results.push(get_scenario_by_id(conn, scenario_id)?);
    }

    // 4. Remove stale baselines that no longer have a matching account_type group
    for (existing_id, _) in &existing_baselines {
        if !used_baseline_ids.contains(existing_id) {
            conn.execute(
                "DELETE FROM client_scenario_positions WHERE scenario_id = ?1",
                [existing_id],
            )?;
            conn.execute(
                "DELETE FROM client_scenarios WHERE id = ?1",
                [existing_id],
            )?;
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Scenario Positions
// ---------------------------------------------------------------------------

/// Get all positions for a scenario
pub fn get_scenario_positions(
    conn: &Connection,
    scenario_id: i64,
) -> Result<Vec<ScenarioPosition>> {
    let mut stmt = conn.prepare(
        "SELECT id, scenario_id, symbol, exchange, quantity, avg_price, side, notes, created_at, updated_at
         FROM client_scenario_positions WHERE scenario_id = ?1 ORDER BY symbol ASC",
    )?;

    let positions = stmt
        .query_map([scenario_id], |row| {
            Ok(ScenarioPosition {
                id: Some(row.get(0)?),
                scenario_id: row.get(1)?,
                symbol: row.get(2)?,
                exchange: row.get(3)?,
                quantity: row.get(4)?,
                avg_price: row.get(5)?,
                side: row.get(6)?,
                notes: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(positions)
}

/// Add a position to a scenario
pub fn add_scenario_position(
    conn: &Connection,
    scenario_id: i64,
    symbol: &str,
    exchange: &str,
    quantity: f64,
    avg_price: f64,
    side: &str,
    notes: Option<&str>,
) -> Result<ScenarioPosition> {
    conn.execute(
        "INSERT INTO client_scenario_positions (scenario_id, symbol, exchange, quantity, avg_price, side, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![scenario_id, symbol, exchange, quantity, avg_price, side, notes],
    )?;
    let id = conn.last_insert_rowid();
    get_scenario_position_by_id(conn, id)
}

/// Update a scenario position
pub fn update_scenario_position(
    conn: &Connection,
    id: i64,
    quantity: Option<f64>,
    avg_price: Option<f64>,
    side: Option<&str>,
    notes: Option<&str>,
) -> Result<ScenarioPosition> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = quantity {
        updates.push("quantity = ?");
        params.push(Box::new(v));
    }
    if let Some(v) = avg_price {
        updates.push("avg_price = ?");
        params.push(Box::new(v));
    }
    if let Some(v) = side {
        updates.push("side = ?");
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = notes {
        updates.push("notes = ?");
        params.push(Box::new(v.to_string()));
    }

    if updates.is_empty() {
        return get_scenario_position_by_id(conn, id);
    }

    updates.push("updated_at = datetime('now')");
    params.push(Box::new(id));

    let sql = format!(
        "UPDATE client_scenario_positions SET {} WHERE id = ?",
        updates.join(", ")
    );
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())?;

    get_scenario_position_by_id(conn, id)
}

/// Delete a scenario position
pub fn delete_scenario_position(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM client_scenario_positions WHERE id = ?1",
        [id],
    )?;
    Ok(rows > 0)
}

/// Apply a simulated trade to a scenario position.
/// If a position for the symbol already exists, updates quantity and recalculates
/// weighted-average cost basis. If not, creates a new position.
pub fn apply_trade_to_scenario(
    conn: &Connection,
    scenario_id: i64,
    symbol: &str,
    exchange: &str,
    side: &str,
    quantity: f64,
    price: f64,
) -> Result<ScenarioPosition> {
    // Callers say buy/sell (Claude's tool) or long/short (the app); positions store long/short.
    let side = match side.to_ascii_lowercase().as_str() {
        "buy" | "long" => "long",
        "sell" | "short" => "short",
        other => return Err(AppError::Validation(format!("Unknown trade side: {other}"))),
    };
    // Every trade moves the scenario's cash by quantity × price, and an opening
    // trade sets the position's cost, so a $0 trade would corrupt both.
    if !(price > 0.0) {
        return Err(AppError::Validation(format!(
            "{symbol}: a trade needs a price above $0"
        )));
    }
    let position = apply_trade_to_position(conn, scenario_id, symbol, exchange, side, quantity, price)?;
    // A buy spends cash and a sell raises it. Unrecorded (NULL) cash stays NULL.
    let cash_change = if side == "long" { -quantity * price } else { quantity * price };
    conn.execute(
        "UPDATE client_scenarios SET cash = cash + ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![cash_change, scenario_id],
    )?;
    Ok(position)
}

/// Apply a priced trade to the position it touches. `side` is "long" or "short".
fn apply_trade_to_position(
    conn: &Connection,
    scenario_id: i64,
    symbol: &str,
    exchange: &str,
    side: &str,
    quantity: f64,
    price: f64,
) -> Result<ScenarioPosition> {
    // Check for existing position
    let existing: Option<(i64, f64, f64, String)> = conn
        .query_row(
            "SELECT id, quantity, avg_price, side FROM client_scenario_positions
             WHERE scenario_id = ?1 AND symbol = ?2 AND exchange = ?3",
            rusqlite::params![scenario_id, symbol, exchange],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok();

    if let Some((pos_id, old_qty, old_avg, old_side)) = existing {
        let is_same_side = old_side == side;

        if is_same_side {
            // Adding to position: weighted average cost basis
            let new_qty = old_qty + quantity;
            let new_avg = if new_qty.abs() > 1e-10 {
                (old_qty * old_avg + quantity * price) / new_qty
            } else {
                0.0
            };
            conn.execute(
                "UPDATE client_scenario_positions SET quantity = ?1, avg_price = ?2, updated_at = datetime('now')
                 WHERE id = ?3",
                rusqlite::params![new_qty, new_avg, pos_id],
            )?;
        } else {
            // Reducing/closing/flipping position
            if quantity >= old_qty {
                let remaining = quantity - old_qty;
                if remaining.abs() < 1e-10 {
                    // Position fully closed — remove it
                    conn.execute(
                        "DELETE FROM client_scenario_positions WHERE id = ?1",
                        [pos_id],
                    )?;
                    // Return a zeroed-out position to signal closure
                    return Ok(ScenarioPosition {
                        id: Some(pos_id),
                        scenario_id,
                        symbol: symbol.to_string(),
                        exchange: exchange.to_string(),
                        quantity: 0.0,
                        avg_price: 0.0,
                        side: old_side,
                        notes: None,
                        created_at: None,
                        updated_at: None,
                    });
                } else {
                    // Flipped: new position on opposite side
                    conn.execute(
                        "UPDATE client_scenario_positions SET quantity = ?1, avg_price = ?2, side = ?3, updated_at = datetime('now')
                         WHERE id = ?4",
                        rusqlite::params![remaining, price, side, pos_id],
                    )?;
                }
            } else {
                // Partial reduction — same side, reduced quantity, avg stays
                let new_qty = old_qty - quantity;
                conn.execute(
                    "UPDATE client_scenario_positions SET quantity = ?1, updated_at = datetime('now')
                     WHERE id = ?2",
                    rusqlite::params![new_qty, pos_id],
                )?;
            }
        }
        get_scenario_position_by_id(conn, pos_id)
    } else {
        // No existing position — create new
        add_scenario_position(conn, scenario_id, symbol, exchange, quantity, price, side, None)
    }
}

/// Get a single scenario position by ID
fn get_scenario_position_by_id(conn: &Connection, id: i64) -> Result<ScenarioPosition> {
    conn.query_row(
        "SELECT id, scenario_id, symbol, exchange, quantity, avg_price, side, notes, created_at, updated_at
         FROM client_scenario_positions WHERE id = ?1",
        [id],
        |row| {
            Ok(ScenarioPosition {
                id: Some(row.get(0)?),
                scenario_id: row.get(1)?,
                symbol: row.get(2)?,
                exchange: row.get(3)?,
                quantity: row.get(4)?,
                avg_price: row.get(5)?,
                side: row.get(6)?,
                notes: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Scenario position {} not found", id))
        }
        other => other.into(),
    })
}

// ---------------------------------------------------------------------------
// Position computation
// ---------------------------------------------------------------------------

/// Compute net positions from a chronologically-ordered trade list using
/// weighted-average cost basis. Correctly handles position flipping
/// (long → short or short → long) by re-anchoring the basis at the
/// flipping trade's price.
///
/// Returns BTreeMap keyed by (symbol, exchange) with values (signed_qty, avg_price).
/// Positive qty = long, negative qty = short. Zero-qty entries are omitted.
///
/// `trades`: ordered list of (symbol, exchange, trade_type, quantity, price)
/// where `trade_type` is "buy" or "sell" (case-insensitive) and `quantity`
/// and `price` are non-negative.
pub fn compute_positions_from_trades(
    trades: &[(String, String, String, f64, f64)],
) -> std::collections::BTreeMap<(String, String), (f64, f64)> {
    let mut positions: std::collections::BTreeMap<(String, String), (f64, f64)> =
        std::collections::BTreeMap::new();

    const EPS: f64 = 1e-10;

    for (symbol, exchange, trade_type, qty, price) in trades {
        let entry = positions
            .entry((symbol.clone(), exchange.clone()))
            .or_insert((0.0, 0.0));
        let net_qty = entry.0;
        let avg_price = entry.1;
        let qty = *qty;
        let price = *price;
        let tt = trade_type.to_lowercase();

        if tt == "buy" {
            if net_qty >= -EPS {
                // Adding to (or starting) a long. Weighted average across long shares.
                let new_qty = net_qty + qty;
                if new_qty.abs() > EPS {
                    entry.1 = (net_qty.max(0.0) * avg_price + qty * price) / new_qty;
                }
                entry.0 = new_qty;
            } else {
                // Covering a short. Cost basis of remaining short stays the same;
                // realized P&L is not tracked here.
                let to_cover = qty.min(-net_qty);
                let new_qty = net_qty + to_cover;
                let remaining = qty - to_cover;
                if remaining > EPS {
                    // Flipped short → long. Re-anchor basis at this trade's price.
                    entry.0 = remaining;
                    entry.1 = price;
                } else {
                    entry.0 = new_qty;
                    if new_qty.abs() <= EPS {
                        entry.1 = 0.0;
                    }
                }
            }
        } else if tt == "sell" {
            if net_qty <= EPS {
                // Adding to (or starting) a short. Weighted avg across short shares
                // (use absolute quantity; avg_price stored as a positive number).
                let prev_short = -net_qty.min(0.0);
                let new_short = prev_short + qty;
                if new_short.abs() > EPS {
                    entry.1 = (prev_short * avg_price + qty * price) / new_short;
                }
                entry.0 = -new_short;
            } else {
                // Closing a long. Avg basis of remaining long is unchanged.
                let to_close = qty.min(net_qty);
                let new_qty = net_qty - to_close;
                let remaining = qty - to_close;
                if remaining > EPS {
                    // Flipped long → short. Re-anchor basis at this trade's price.
                    entry.0 = -remaining;
                    entry.1 = price;
                } else {
                    entry.0 = new_qty;
                    if new_qty.abs() <= EPS {
                        entry.1 = 0.0;
                    }
                }
            }
        }
    }

    // Strip zero-qty entries.
    positions.retain(|_, (qty, _)| qty.abs() > EPS);
    positions
}

#[cfg(test)]
mod tests {
    use super::compute_positions_from_trades;

    fn t(symbol: &str, tt: &str, qty: f64, price: f64) -> (String, String, String, f64, f64) {
        (symbol.to_string(), "NASDAQ".to_string(), tt.to_string(), qty, price)
    }

    fn pos(positions: &std::collections::BTreeMap<(String, String), (f64, f64)>, symbol: &str) -> (f64, f64) {
        *positions
            .get(&(symbol.to_string(), "NASDAQ".to_string()))
            .expect("position not found")
    }

    #[test]
    fn weighted_avg_basic_long() {
        let trades = vec![t("AAPL", "buy", 10.0, 100.0), t("AAPL", "buy", 10.0, 200.0)];
        let p = compute_positions_from_trades(&trades);
        let (qty, avg) = pos(&p, "AAPL");
        assert!((qty - 20.0).abs() < 1e-6);
        assert!((avg - 150.0).abs() < 1e-6, "avg should be 150, got {}", avg);
    }

    #[test]
    fn partial_sell_preserves_long_avg() {
        let trades = vec![
            t("AAPL", "buy", 10.0, 100.0),
            t("AAPL", "buy", 10.0, 200.0), // avg = 150
            t("AAPL", "sell", 5.0, 250.0), // realized P&L not tracked; avg unchanged
        ];
        let p = compute_positions_from_trades(&trades);
        let (qty, avg) = pos(&p, "AAPL");
        assert!((qty - 15.0).abs() < 1e-6);
        assert!((avg - 150.0).abs() < 1e-6, "avg should remain 150, got {}", avg);
    }

    #[test]
    fn long_to_short_flip_reanchors_basis() {
        // Long 10 @ $100, then sell 15 @ $200 → short 5 @ $200 (re-anchored)
        let trades = vec![
            t("AAPL", "buy", 10.0, 100.0),
            t("AAPL", "sell", 15.0, 200.0),
        ];
        let p = compute_positions_from_trades(&trades);
        let (qty, avg) = pos(&p, "AAPL");
        assert!((qty - -5.0).abs() < 1e-6, "qty should be -5, got {}", qty);
        assert!((avg - 200.0).abs() < 1e-6, "avg should be 200, got {}", avg);
    }

    #[test]
    fn short_to_long_flip_reanchors_basis() {
        // Short 10 @ $100, then buy 15 @ $50 → long 5 @ $50 (re-anchored)
        let trades = vec![
            t("AAPL", "sell", 10.0, 100.0),
            t("AAPL", "buy", 15.0, 50.0),
        ];
        let p = compute_positions_from_trades(&trades);
        let (qty, avg) = pos(&p, "AAPL");
        assert!((qty - 5.0).abs() < 1e-6, "qty should be 5, got {}", qty);
        assert!((avg - 50.0).abs() < 1e-6, "avg should be 50, got {}", avg);
    }

    #[test]
    fn weighted_avg_short() {
        // Short 10 @ $100, short another 10 @ $200 → short 20 @ $150
        let trades = vec![
            t("AAPL", "sell", 10.0, 100.0),
            t("AAPL", "sell", 10.0, 200.0),
        ];
        let p = compute_positions_from_trades(&trades);
        let (qty, avg) = pos(&p, "AAPL");
        assert!((qty - -20.0).abs() < 1e-6);
        assert!((avg - 150.0).abs() < 1e-6, "avg should be 150, got {}", avg);
    }

    #[test]
    fn position_round_trip_zero() {
        // Buy 10, sell 10 → no position recorded
        let trades = vec![
            t("AAPL", "buy", 10.0, 100.0),
            t("AAPL", "sell", 10.0, 200.0),
        ];
        let p = compute_positions_from_trades(&trades);
        assert!(p.get(&("AAPL".to_string(), "NASDAQ".to_string())).is_none());
    }

    #[test]
    fn regression_aapl_negative_avg_bug() {
        // Reproduces the bug where interleaved buy/sell flipping past zero
        // produced nonsense (often negative) avg prices for the resulting long.
        // Sequence: long 100 @ $200, oversell 105 @ $300, oversell again 5 @ $100,
        // then buy 110 @ $250 → final position should be long 5 @ $250.
        let trades = vec![
            t("AAPL", "buy", 100.0, 200.0),
            t("AAPL", "sell", 105.0, 300.0), // flips to short 5 @ $300
            t("AAPL", "sell", 5.0, 100.0),   // adds to short, weighted: short 10 @ $200
            t("AAPL", "buy", 110.0, 250.0),  // covers 10 short, leaves long 100 @ $250
        ];
        let p = compute_positions_from_trades(&trades);
        let (qty, avg) = pos(&p, "AAPL");
        assert!((qty - 100.0).abs() < 1e-6, "qty should be 100, got {}", qty);
        assert!(
            avg > 0.0,
            "avg price must be positive (was {} — the original bug produced negative)",
            avg
        );
        assert!((avg - 250.0).abs() < 1e-6, "avg should be 250, got {}", avg);
    }
}

#[cfg(test)]
mod apply_trade_tests {
    use super::*;

    /// A scenario holding 5 VOO @ $679.70 long, as stored by the importer.
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO clients (name) VALUES ('Test')", []).unwrap();
        conn.execute("INSERT INTO client_scenarios (client_id, name) VALUES (1, 'S')", []).unwrap();
        add_scenario_position(&conn, 1, "VOO", "GENERIC", 5.0, 679.7, "long", None).unwrap();
        conn
    }

    fn voo(conn: &Connection) -> Option<(f64, f64, String)> {
        conn.query_row(
            "SELECT quantity, avg_price, side FROM client_scenario_positions WHERE symbol = 'VOO'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()
    }

    #[test]
    fn claudes_buy_adds_to_the_long() {
        let conn = setup();
        apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "buy", 9.0, 696.94).unwrap();
        let (qty, avg, side) = voo(&conn).unwrap();
        assert_eq!(side, "long");
        assert!((qty - 14.0).abs() < 1e-9);
        assert!((avg - (5.0 * 679.7 + 9.0 * 696.94) / 14.0).abs() < 1e-9);
    }

    #[test]
    fn claudes_sell_reduces_the_long() {
        let conn = setup();
        apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "sell", 3.0, 696.94).unwrap();
        let (qty, avg, side) = voo(&conn).unwrap();
        assert_eq!((qty, avg, side.as_str()), (2.0, 679.7, "long"));
    }

    #[test]
    fn long_and_short_still_work() {
        let conn = setup();
        apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "long", 1.0, 700.0).unwrap();
        apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "short", 2.0, 700.0).unwrap();
        assert_eq!(voo(&conn).unwrap().0, 4.0);
    }

    #[test]
    fn a_new_buy_is_stored_as_long() {
        let conn = setup();
        let p = apply_trade_to_scenario(&conn, 1, "SPY", "GENERIC", "buy", 2.0, 758.0).unwrap();
        assert_eq!(p.side, "long");
    }

    #[test]
    fn a_buy_with_no_price_is_refused_and_changes_nothing() {
        let conn = setup();
        assert!(apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "buy", 9.0, 0.0).is_err());
        assert!(apply_trade_to_scenario(&conn, 1, "SPY", "GENERIC", "buy", 2.0, 0.0).is_err());
        assert_eq!(voo(&conn).unwrap(), (5.0, 679.7, "long".to_string()));
    }

    #[test]
    fn a_sell_that_flips_to_short_with_no_price_is_refused() {
        let conn = setup();
        assert!(apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "sell", 8.0, 0.0).is_err());
        assert_eq!(voo(&conn).unwrap(), (5.0, 679.7, "long".to_string()));
    }

    #[test]
    fn a_sell_with_no_price_is_refused_because_it_moves_cash() {
        let conn = setup();
        assert!(apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "sell", 2.0, 0.0).is_err());
        assert_eq!(voo(&conn).unwrap(), (5.0, 679.7, "long".to_string()));
    }

    fn cash(conn: &Connection) -> Option<f64> {
        get_scenario_by_id(conn, 1).unwrap().cash
    }

    #[test]
    fn sells_raise_cash_and_buys_spend_it() {
        let conn = setup();
        conn.execute("UPDATE client_scenarios SET cash = 100.0 WHERE id = 1", []).unwrap();
        apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "sell", 5.0, 700.0).unwrap();
        assert_eq!(voo(&conn), None);
        assert!((cash(&conn).unwrap() - 3600.0).abs() < 1e-9);
        apply_trade_to_scenario(&conn, 1, "SPY", "GENERIC", "buy", 4.0, 750.0).unwrap();
        assert!((cash(&conn).unwrap() - 600.0).abs() < 1e-9);
    }

    #[test]
    fn unrecorded_cash_stays_unrecorded() {
        let conn = setup();
        apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "sell", 5.0, 700.0).unwrap();
        assert_eq!(cash(&conn), None);
    }

    #[test]
    fn a_clone_starts_with_its_sources_cash() {
        let conn = setup();
        conn.execute("UPDATE client_scenarios SET cash = 183.03 WHERE id = 1", []).unwrap();
        let clone = clone_scenario(&conn, 1, 1, "Rebalanced", None).unwrap();
        assert_eq!(clone.cash, Some(183.03));
        assert_eq!(clone.cloned_from_id, Some(1));
    }

    #[test]
    fn only_the_latest_imports_positions_file_counts() {
        let conn = setup();
        let latest = |c: &Connection| super::super::clients::latest_import_positions_document(c, 1).unwrap();
        assert_eq!(latest(&conn), None);
        let add = |c: &Connection, t: &str, body: &str| {
            super::super::clients::add_client_document(c, 1, t, "f.csv", body).unwrap();
        };
        add(&conn, "transactions", "t1");
        add(&conn, "positions", "p1");
        assert_eq!(latest(&conn).as_deref(), Some("p1"));
        // A newer import with no Positions file: the old one no longer describes the account.
        add(&conn, "transactions", "t2");
        assert_eq!(latest(&conn), None);
    }

    #[test]
    fn the_holdings_baseline_takes_the_imports_cash() {
        let conn = setup();
        conn.execute(
            "INSERT INTO client_holdings (client_id, symbol, quantity, avg_cost, total_cost, realized_pnl)
             VALUES (1, 'VOO', 5, 679.7, 3398.5, 0)",
            [],
        )
        .unwrap();
        let baseline = sync_baseline_scenarios(&conn, 1, Some(183.03)).unwrap().remove(0);
        assert!(baseline.is_baseline);
        assert_eq!(baseline.cash, Some(183.03));
        // A later import without a Positions file leaves the cash unrecorded.
        let resynced = sync_baseline_scenarios(&conn, 1, None).unwrap().remove(0);
        assert_eq!(resynced.id, baseline.id);
        assert_eq!(resynced.cash, None);
    }

    #[test]
    fn an_unknown_side_is_refused() {
        let conn = setup();
        assert!(apply_trade_to_scenario(&conn, 1, "VOO", "GENERIC", "hold", 1.0, 700.0).is_err());
    }
}
