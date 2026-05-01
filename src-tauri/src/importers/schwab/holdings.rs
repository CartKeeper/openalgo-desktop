//! Reconstruct current holdings from a list of normalized transactions.
//!
//! Cost basis uses the **weighted-average** method:
//!   - Buy adds qty × price to total_cost and qty to total_qty
//!   - Sell reduces qty proportionally; realized P&L = (sell_price − avg_cost) × sell_qty;
//!     total_cost is reduced by sell_qty × avg_cost so avg_cost stays stable.
//!   - Splits / spinoffs / liquidations adjust quantity by the signed amount with no
//!     direct cost change (cost basis just dilutes/concentrates with the share count).
//!
//! Dividends, fees, and interest don't move share counts — they're skipped here.

use crate::providers::types::ClientHolding;
use std::collections::HashMap;

use super::positions::{ParsedPositions, PositionRow};
use super::transactions::TransactionRow;

/// Result of reconstruct() so callers can surface symbols where transaction
/// history doesn't go back far enough to fully account for current position.
pub struct ReconstructResult {
    pub holdings: Vec<ClientHolding>,
    /// Symbols whose net-from-transactions came out negative on a no-shorts
    /// account — i.e. a sell with no matching buy in the import window.
    /// These are NOT real shorts; they're pre-existing positions sold within
    /// the imported period. The frontend surfaces them as info, not violations.
    pub incomplete_history_symbols: Vec<String>,
}

/// Use a Schwab Positions snapshot as the authoritative current state.
/// Cost basis comes directly from the file (no inference). Realized P/L is
/// summed across in-window sells in the transactions ledger using the
/// position's avg cost as a proxy.
pub fn reconstruct_from_positions(
    client_id: i64,
    parsed: &ParsedPositions,
    transactions: &[TransactionRow],
) -> Vec<ClientHolding> {
    // Realized P/L per symbol: sum of (sell_proceeds - sell_qty * avg_cost) - fees
    let mut realized: HashMap<String, f64> = HashMap::new();
    for t in transactions {
        if t.action != "sell" {
            continue;
        }
        let key = t.symbol.to_uppercase();
        let pos = parsed.positions.iter().find(|p| p.symbol.eq_ignore_ascii_case(&t.symbol));
        let avg = pos
            .and_then(|p| {
                if p.quantity.abs() > 1e-9 {
                    p.cost_basis.map(|c| c / p.quantity)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);
        let qty = t.quantity.abs();
        let pnl = (t.price - avg) * qty - t.fees;
        *realized.entry(key).or_insert(0.0) += pnl;
    }

    // Last activity date per symbol (max date across transactions)
    let mut last_activity: HashMap<String, String> = HashMap::new();
    for t in transactions {
        if t.symbol.is_empty() {
            continue;
        }
        let key = t.symbol.to_uppercase();
        last_activity
            .entry(key)
            .and_modify(|d| {
                if t.date > *d {
                    *d = t.date.clone();
                }
            })
            .or_insert_with(|| t.date.clone());
    }

    // Schwab sometimes splits one position into multiple rows (different reinvest
    // settings, mixed lots, etc.). Merge by uppercase symbol so we don't violate
    // the (client_id, symbol) UNIQUE index in client_holdings.
    let mut merged: HashMap<String, MergedPos> = HashMap::new();
    for p in &parsed.positions {
        if p.quantity.abs() < 1e-9 && !p.is_stranded {
            continue;
        }
        let key = p.symbol.to_uppercase();
        let entry = merged.entry(key).or_insert_with(|| MergedPos {
            symbol: p.symbol.clone(),
            description: p.description.clone(),
            quantity: 0.0,
            cost_basis: 0.0,
            market_value: 0.0,
            has_market_value: false,
            current_price: None,
        });
        if entry.description.is_empty() && !p.description.is_empty() {
            entry.description = p.description.clone();
        }
        entry.quantity += p.quantity;
        entry.cost_basis += p.cost_basis.unwrap_or(0.0);
        if let Some(mv) = p.market_value {
            entry.market_value += mv;
            entry.has_market_value = true;
        }
        // Last non-None price across merged rows. Schwab reports the same
        // price across split rows so this is safe.
        if entry.current_price.is_none() {
            entry.current_price = p.price;
        }
    }

    let mut out: Vec<ClientHolding> = merged
        .into_iter()
        .map(|(key, m)| {
            let avg = if m.quantity.abs() > 1e-9 { m.cost_basis / m.quantity } else { 0.0 };
            let market_value = if m.has_market_value { Some(round2(m.market_value)) } else { None };
            let gain_percent = match (market_value, m.cost_basis.abs() > 1e-9) {
                (Some(mv), true) => Some(round2((mv - m.cost_basis) / m.cost_basis * 100.0)),
                _ => None,
            };
            ClientHolding {
                id: None,
                client_id,
                symbol: m.symbol,
                description: if m.description.is_empty() { None } else { Some(m.description) },
                quantity: round2(m.quantity),
                avg_cost: round4(avg),
                total_cost: round2(m.cost_basis),
                realized_pnl: round2(realized.get(&key).copied().unwrap_or(0.0)),
                last_activity_date: last_activity.get(&key).cloned(),
                updated_at: None,
                current_price: m.current_price.map(round4),
                market_value,
                gain_percent,
            }
        })
        .collect();
    out.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    out
}

struct MergedPos {
    symbol: String,
    description: String,
    quantity: f64,
    cost_basis: f64,
    market_value: f64,
    has_market_value: bool,
    current_price: Option<f64>,
}

/// Stranded positions (revoked, restricted, escrow, CUSIP-only) extracted from
/// a Positions snapshot. Surfaced separately so they don't pollute the main
/// holdings table or trigger compliance checks.
pub fn extract_stranded(parsed: &ParsedPositions) -> Vec<PositionRow> {
    parsed.positions.iter().filter(|p| p.is_stranded).cloned().collect()
}

pub fn reconstruct(
    client_id: i64,
    transactions: &[TransactionRow],
    account_disallows_shorts: bool,
) -> ReconstructResult {
    // Sort ascending by date so cost basis evolves correctly. (Original may be desc.)
    let mut sorted: Vec<&TransactionRow> = transactions.iter().collect();
    sorted.sort_by(|a, b| a.date.cmp(&b.date));

    let mut by_symbol: HashMap<String, Acc> = HashMap::new();

    for t in sorted {
        if t.symbol.is_empty() {
            continue;
        }
        let entry = by_symbol.entry(t.symbol.clone()).or_insert_with(|| Acc {
            description: t.description.clone(),
            quantity: 0.0,
            total_cost: 0.0,
            realized_pnl: 0.0,
            last_activity: String::new(),
        });

        if entry.description.is_empty() && !t.description.is_empty() {
            entry.description = t.description.clone();
        }
        if t.date > entry.last_activity {
            entry.last_activity = t.date.clone();
        }

        match t.action.as_str() {
            "buy" => {
                if t.quantity > 0.0 && t.price > 0.0 {
                    entry.quantity += t.quantity;
                    entry.total_cost += t.quantity * t.price + t.fees;
                }
            }
            "sell" => {
                let sell_qty = t.quantity.abs();
                if sell_qty > 0.0 {
                    let avg = if entry.quantity.abs() > 1e-9 {
                        entry.total_cost / entry.quantity
                    } else {
                        0.0
                    };
                    let realized = (t.price - avg) * sell_qty - t.fees;
                    entry.realized_pnl += realized;
                    entry.quantity -= sell_qty;
                    entry.total_cost -= sell_qty * avg;
                    if entry.quantity.abs() < 1e-9 {
                        entry.quantity = 0.0;
                        entry.total_cost = 0.0;
                    }
                }
            }
            "split" | "spinoff" | "liquidation" | "other" => {
                // Apply signed quantity adjustment when one is reported.
                if t.quantity != 0.0 {
                    entry.quantity += t.quantity;
                    if entry.quantity.abs() < 1e-9 {
                        entry.quantity = 0.0;
                        entry.total_cost = 0.0;
                    }
                }
            }
            _ => {} // dividend / fee / interest don't affect share count
        }
    }

    // Collect symbols whose net came out negative — these are "incomplete history"
    // (pre-existing position sold during window), NOT real shorts.
    // On no-shorts accounts we clamp them to zero so the rules engine doesn't
    // fire `short_position` flags against false positives.
    let mut incomplete: Vec<String> = Vec::new();
    if account_disallows_shorts {
        for (sym, acc) in by_symbol.iter_mut() {
            if acc.quantity < -1e-6 {
                incomplete.push(sym.clone());
                acc.quantity = 0.0;
                acc.total_cost = 0.0;
                // Realized P/L is unreliable here too (cost basis was unknown
                // because the buy was outside the window) — zero it out so we
                // don't surface phantom gains.
                acc.realized_pnl = 0.0;
            }
        }
    }

    let mut out: Vec<ClientHolding> = by_symbol
        .into_iter()
        .filter(|(_, acc)| acc.quantity.abs() > 1e-9 || acc.realized_pnl.abs() > 0.005)
        .map(|(symbol, acc)| {
            let avg_cost = if acc.quantity.abs() > 1e-9 {
                acc.total_cost / acc.quantity
            } else {
                0.0
            };
            ClientHolding {
                id: None,
                client_id,
                symbol,
                description: if acc.description.is_empty() { None } else { Some(acc.description) },
                quantity: round2(acc.quantity),
                avg_cost: round4(avg_cost),
                total_cost: round2(acc.total_cost),
                realized_pnl: round2(acc.realized_pnl),
                last_activity_date: if acc.last_activity.is_empty() { None } else { Some(acc.last_activity) },
                updated_at: None,
                // No market value when reconstructing from transactions only —
                // we don't have prices in the transactions ledger. Brief
                // generators must handle None gracefully.
                current_price: None,
                market_value: None,
                gain_percent: None,
            }
        })
        .collect();
    out.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    incomplete.sort();
    incomplete.dedup();
    ReconstructResult {
        holdings: out,
        incomplete_history_symbols: incomplete,
    }
}

struct Acc {
    description: String,
    quantity: f64,
    total_cost: f64,
    realized_pnl: f64,
    last_activity: String,
}

fn round2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }
fn round4(v: f64) -> f64 { (v * 10000.0).round() / 10000.0 }
