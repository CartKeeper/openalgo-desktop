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

use super::transactions::TransactionRow;

pub fn reconstruct(client_id: i64, transactions: &[TransactionRow]) -> Vec<ClientHolding> {
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
            }
        })
        .collect();
    out.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    out
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
