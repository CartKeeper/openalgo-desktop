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
        "SELECT id, client_id, name, description, is_baseline, cloned_from_id, account_type, created_at, updated_at
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
        "SELECT id, client_id, name, description, is_baseline, cloned_from_id, account_type, created_at, updated_at
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
    // Copy account_type from source scenario
    let source_account_type: Option<String> = conn
        .query_row(
            "SELECT account_type FROM client_scenarios WHERE id = ?1",
            [source_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    conn.execute(
        "INSERT INTO client_scenarios (client_id, name, description, is_baseline, cloned_from_id, account_type)
         VALUES (?1, ?2, ?3, 0, ?4, ?5)",
        rusqlite::params![client_id, name, description, source_id, source_account_type],
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

/// Create or update baseline scenarios from the client's trade history.
/// Creates one baseline per account type found in import batches.
/// Trades with no import batch or batches with no account type go into a generic baseline.
pub fn sync_baseline_scenarios(conn: &Connection, client_id: i64) -> Result<Vec<ClientScenario>> {
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
