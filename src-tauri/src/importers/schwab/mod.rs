//! Schwab document importer.
//!
//! Two file types are accepted:
//! 1. **Transactions** — JSON or CSV ledger of fills, dividends, fees, corporate actions.
//!    This is the source of truth for current holdings and cost basis.
//! 2. **Order Status** (optional) — CSV of open / canceled / filled orders. Used to
//!    surface working GTC stops/limits and to cross-check filled orders against
//!    the transaction ledger.
//!
//! On mismatch between the two files, **Transactions always win** (per spec).
//!
//! All output flows through `run_import` which returns an `ImportReport` containing
//! reconstructed holdings, open orders, 401k violations, and reconciliation diffs.

pub mod holdings;
pub mod order_status;
pub mod positions;
pub mod reconciler;
pub mod rules_401k;
pub mod transactions;

use crate::error::AppError;
use crate::providers::types::{
    ClientHolding, ClientOpenOrder, ComplianceViolation, ImportReport, ImportReportSummary,
    StrandedPosition,
};

/// Run a Schwab document import end to end.
///
/// Parameters:
/// - `transactions_content` — required, JSON or CSV string
/// - `order_status_csv` — optional, enables open-order detection + reconciliation
/// - `positions_csv` — optional but **strongly preferred**: when supplied it
///   becomes the authoritative current-state baseline so we don't have to
///   reconstruct from a (typically partial) transaction history. Without it,
///   no-shorts accounts get a clamp-to-zero fallback to suppress false-short
///   flags from incomplete history.
/// - `account_disallows_shorts` — drives the clamp behavior in the fallback path
pub fn run_import(
    client_id: i64,
    transactions_content: &str,
    order_status_csv: Option<&str>,
    positions_csv: Option<&str>,
    account_disallows_shorts: bool,
) -> Result<ImportReport, AppError> {
    // 1. Parse transactions (auto-detect JSON vs CSV)
    let transactions = transactions::parse(transactions_content)?;
    let transactions_processed = transactions.len() as i64;

    // 2. Reconstruct holdings — prefer Positions snapshot when present
    let parsed_positions = match positions_csv {
        Some(content) => Some(positions::parse(content)?),
        None => None,
    };

    let (holdings_raw, incomplete_history_symbols, stranded_positions) = match &parsed_positions {
        Some(parsed) => {
            let stranded: Vec<StrandedPosition> = holdings::extract_stranded(parsed)
                .into_iter()
                .map(|p| StrandedPosition {
                    symbol: p.symbol,
                    description: p.description,
                    quantity: p.quantity,
                    cost_basis: p.cost_basis,
                    asset_type: p.asset_type,
                    reason: stranded_reason(&p.is_stranded, &p.price, &p.market_value),
                })
                .collect();
            let h = holdings::reconstruct_from_positions(client_id, parsed, &transactions);
            (h, Vec::new(), stranded)
        }
        None => {
            let res = holdings::reconstruct(client_id, &transactions, account_disallows_shorts);
            (res.holdings, res.incomplete_history_symbols, Vec::new())
        }
    };

    // 3. Parse order status (if provided)
    let (open_orders, filled_orders, order_status_processed) = match order_status_csv {
        Some(csv) => {
            let parsed = order_status::parse(csv)?;
            let processed = parsed.len() as i64;
            let (open, filled) = order_status::split_open_and_filled(client_id, parsed);
            (open, filled, processed)
        }
        None => (Vec::new(), Vec::new(), 0),
    };

    // 4. Reconcile filled orders ↔ transactions (transactions win on mismatch)
    let reconciliation_mismatches = reconciler::reconcile(&filled_orders, &transactions);

    // 5. Run strict 401k rules against holdings + open orders
    let violations =
        rules_401k::evaluate(client_id, &holdings_raw, &open_orders, &transactions);

    // 6. Summary
    let summary = build_summary(
        transactions_processed,
        order_status_processed,
        &holdings_raw,
        &open_orders,
        &violations,
    );

    Ok(ImportReport {
        client_id,
        summary,
        holdings: holdings_raw,
        open_orders,
        violations,
        reconciliation_mismatches,
        incomplete_history_symbols,
        stranded_positions,
    })
}

fn stranded_reason(is_stranded: &bool, price: &Option<f64>, market_value: &Option<f64>) -> String {
    if !is_stranded {
        return String::new();
    }
    if price.is_none() && market_value.is_none() {
        "No live price (delisted, revoked, restricted, or escrow)".to_string()
    } else {
        "CUSIP-only entry without ticker mapping".to_string()
    }
}

fn build_summary(
    transactions_processed: i64,
    order_status_processed: i64,
    holdings: &[ClientHolding],
    open_orders: &[ClientOpenOrder],
    violations: &[ComplianceViolation],
) -> ImportReportSummary {
    let total_cost_basis: f64 = holdings.iter().map(|h| h.total_cost).sum();
    let open_buy_orders = open_orders
        .iter()
        .filter(|o| o.action.eq_ignore_ascii_case("buy"))
        .count() as i64;
    let open_sell_orders = open_orders
        .iter()
        .filter(|o| o.action.eq_ignore_ascii_case("sell"))
        .count() as i64;
    let blocking = violations
        .iter()
        .filter(|v| v.severity == "block")
        .count() as i64;
    ImportReportSummary {
        transactions_processed,
        order_status_processed,
        total_holdings: holdings.iter().filter(|h| h.quantity.abs() > 1e-9).count() as i64,
        total_cost_basis,
        open_buy_orders,
        open_sell_orders,
        violation_count: violations.len() as i64,
        is_compliant: blocking == 0,
    }
}

// Helpers shared across the submodules ------------------------------------

/// Parse a money-formatted string (`$1,234.56`, `-$200.00`, `($100)`, `""`).
pub(crate) fn parse_money(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "-" {
        return None;
    }
    let cleaned = s
        .replace('$', "")
        .replace(',', "")
        .replace('(', "-")
        .replace(')', "");
    cleaned.trim().parse::<f64>().ok()
}

/// Parse a Schwab quantity string. Supports plain numbers and `"5 Shares"` style.
pub(crate) fn parse_quantity(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let first_token = s
        .split_whitespace()
        .next()
        .unwrap_or("")
        .replace(',', "");
    first_token.parse::<f64>().ok()
}

/// Convert Schwab-style `MM/DD/YYYY` (with optional `" as of MM/DD/YYYY"`) to ISO `YYYY-MM-DD`.
/// Falls back to the original string if parse fails so we never lose data.
pub(crate) fn normalize_date(raw: &str) -> String {
    let trimmed = raw.trim();
    let primary = match trimmed.find(" as of ") {
        Some(pos) => &trimmed[..pos],
        None => trimmed,
    }
    .trim();

    // Strip any trailing time portion ("1:27 PM 04/30/2026" → "04/30/2026")
    let date_part = primary
        .split_whitespace()
        .last()
        .unwrap_or(primary);

    let parts: Vec<&str> = date_part.split('/').collect();
    if parts.len() == 3 {
        if let (Ok(m), Ok(d), Ok(y)) = (
            parts[0].parse::<u32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<i32>(),
        ) {
            return format!("{:04}-{:02}-{:02}", y, m, d);
        }
    }
    primary.to_string()
}
