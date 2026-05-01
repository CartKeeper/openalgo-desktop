//! Cross-validate Filled orders (from Order Status) against the Transactions ledger.
//!
//! Per spec, **Transactions are the source of truth**. This module reports diffs
//! so the user can see them, but the holdings reconstruction always uses Transactions.

use crate::providers::types::ReconciliationMismatch;

use super::order_status::OrderStatusRow;
use super::transactions::TransactionRow;

pub fn reconcile(
    filled_orders: &[OrderStatusRow],
    transactions: &[TransactionRow],
) -> Vec<ReconciliationMismatch> {
    let mut mismatches = Vec::new();

    for order in filled_orders {
        // Find a matching trade transaction (buy/sell, same symbol, same direction, same qty±1%)
        let matched = transactions.iter().find(|t| {
            (t.action == "buy" || t.action == "sell")
                && t.symbol.eq_ignore_ascii_case(&order.symbol)
                && t.action.eq_ignore_ascii_case(&order.action)
                && qty_close(t.quantity.abs(), order.quantity.abs())
        });

        match matched {
            None => {
                mismatches.push(ReconciliationMismatch {
                    order_number: order.order_number.clone(),
                    symbol: order.symbol.clone(),
                    action: order.action.clone(),
                    order_quantity: order.quantity,
                    order_fill_price: order.fill_price,
                    transaction_quantity: None,
                    transaction_price: None,
                    mismatch_kind: "missing_transaction".into(),
                    note: format!(
                        "Filled order #{} ({} {} {}) has no matching transaction in the ledger.",
                        order.order_number.as_deref().unwrap_or("?"),
                        order.action,
                        order.quantity,
                        order.symbol,
                    ),
                });
            }
            Some(t) => {
                if let (Some(fill), price) = (order.fill_price, t.price) {
                    if !price_close(fill, price) {
                        mismatches.push(ReconciliationMismatch {
                            order_number: order.order_number.clone(),
                            symbol: order.symbol.clone(),
                            action: order.action.clone(),
                            order_quantity: order.quantity,
                            order_fill_price: order.fill_price,
                            transaction_quantity: Some(t.quantity.abs()),
                            transaction_price: Some(t.price),
                            mismatch_kind: "price_mismatch".into(),
                            note: format!(
                                "{} {}: order fill ${:.4} vs transaction ${:.4} — using transaction.",
                                order.action, order.symbol, fill, price,
                            ),
                        });
                    }
                }
            }
        }
    }

    mismatches
}

fn qty_close(a: f64, b: f64) -> bool {
    if a == b { return true; }
    let denom = a.abs().max(b.abs()).max(1.0);
    ((a - b).abs() / denom) < 0.01
}

fn price_close(a: f64, b: f64) -> bool {
    if a == b { return true; }
    let denom = a.abs().max(b.abs()).max(1.0);
    ((a - b).abs() / denom) < 0.005
}
