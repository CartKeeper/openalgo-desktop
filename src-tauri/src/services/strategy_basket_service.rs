//! Strategy Basket Service
//!
//! Places the groups of orders proposed by the AI briefing, one group at a
//! time and one leg at a time inside each group, waiting for each step to
//! actually happen before the next one is sent:
//!
//! - a rotation is `cancel` (free the shares pledged to an old stop) -> `fund`
//!   (market-sell the dormant position) -> `buy` (market-buy the new ride) ->
//!   `protect` (GTC trailing stop on what was just bought);
//! - a protection group is `cancel`? -> `protect`.
//!
//! If a leg fails, every remaining leg of that group is reported as not placed
//! and the group stops. Groups are independent: one group failing never stops
//! the next one.

use crate::brokers::types::{Order, OrderRequest};
use crate::error::{AppError, Result};
use crate::services::{OrderService, OrderbookService, PlaceOrderResult};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// How often an order's status is re-checked while waiting.
const POLL_SECONDS: u64 = 2;
/// Longest wait for a cancel to take effect.
const CANCEL_CAP_SECONDS: u64 = 30;
/// Longest wait for a market order to fill.
const FILL_CAP_SECONDS: u64 = 60;

const REASON_FAILED: &str = "failed";
const REASON_NOT_FILLED: &str = "did not fill within 60 seconds";
const REASON_NOT_CANCELLED: &str = "was not cancelled within 30 seconds";

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum LegRole {
    Cancel,
    Fund,
    Buy,
    Protect,
    Standalone,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StrategyLeg {
    pub group: Option<String>,
    pub role: LegRole,
    pub symbol: String,
    #[serde(default)]
    pub cancel_order_id: Option<String>,
    #[serde(default)]
    pub order: Option<OrderRequest>,
}

#[derive(Serialize, Clone, Debug)]
pub struct LegResult {
    pub group: Option<String>,
    pub role: LegRole,
    pub symbol: String,
    pub success: bool,
    pub order_id: Option<String>,
    pub filled_quantity: f64,
    pub message: String,
}

// ============================================================================
// Planning (pure)
// ============================================================================

fn role_rank(role: &LegRole) -> u8 {
    match role {
        LegRole::Cancel => 0,
        LegRole::Fund => 1,
        LegRole::Buy => 2,
        LegRole::Protect => 3,
        LegRole::Standalone => 4,
    }
}

/// Plain-words name of a leg's action, used in messages.
fn role_word(role: &LegRole) -> &'static str {
    match role {
        LegRole::Cancel => "cancel",
        LegRole::Fund => "sell",
        LegRole::Buy => "buy",
        LegRole::Protect => "trailing stop",
        LegRole::Standalone => "order",
    }
}

fn validate_leg(leg: &StrategyLeg) -> Result<()> {
    let word = role_word(&leg.role);
    if leg.role == LegRole::Cancel {
        let has_id = leg
            .cancel_order_id
            .as_deref()
            .map(|id| !id.trim().is_empty())
            .unwrap_or(false);
        if !has_id {
            return Err(AppError::Validation(format!(
                "{} leg for {} has no cancel_order_id",
                word, leg.symbol
            )));
        }
        return Ok(());
    }
    match &leg.order {
        None => Err(AppError::Validation(format!(
            "{} leg for {} has no order",
            match leg.role {
                LegRole::Fund => "fund",
                LegRole::Buy => "buy",
                LegRole::Protect => "protect",
                _ => "standalone",
            },
            leg.symbol
        ))),
        Some(order) if !order.symbol.eq_ignore_ascii_case(&leg.symbol) => {
            Err(AppError::Validation(format!(
                "{} leg for {} has an order for a different symbol ({})",
                word, leg.symbol, order.symbol
            )))
        }
        Some(_) => Ok(()),
    }
}

/// Split legs into groups (first-appearance order) and order each group
/// Cancel -> Fund -> Buy -> Protect. Standalone and ungrouped legs are each
/// their own group.
pub fn plan_groups(legs: Vec<StrategyLeg>) -> Result<Vec<Vec<StrategyLeg>>> {
    for leg in &legs {
        validate_leg(leg)?;
    }

    let mut groups: Vec<(Option<String>, Vec<StrategyLeg>)> = Vec::new();
    for leg in legs {
        let key = if leg.role == LegRole::Standalone {
            None
        } else {
            leg.group.clone()
        };
        match key {
            Some(_) => match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, members)) => members.push(leg),
                None => groups.push((key, vec![leg])),
            },
            None => groups.push((None, vec![leg])),
        }
    }

    Ok(groups
        .into_iter()
        .map(|(_, mut members)| {
            members.sort_by_key(|l| role_rank(&l.role)); // stable
            members
        })
        .collect())
}

// ============================================================================
// Message and status helpers (pure)
// ============================================================================

fn is_filled(order: &Order) -> bool {
    order.status == "COMPLETE"
        || (order.quantity > 0.0 && order.filled_quantity >= order.quantity - 1e-9)
}

fn is_cancelled(order: &Order) -> bool {
    order.status == "CANCELLED"
}

fn is_rejected(order: &Order) -> bool {
    order.status == "REJECTED"
}

/// "1 share" / "11 shares" / "2.5 shares".
fn shares(qty: f64) -> String {
    let text = if (qty - qty.round()).abs() < 1e-9 {
        format!("{}", qty.round() as i64)
    } else {
        let t = format!("{:.4}", qty);
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    if (qty - 1.0).abs() < 1e-9 {
        format!("{} share", text)
    } else {
        format!("{} shares", text)
    }
}

/// Shortens a long broker order id for display.
fn short_id(id: &str) -> String {
    if id.chars().count() > 8 {
        format!("{}…", id.chars().take(8).collect::<String>())
    } else {
        id.to_string()
    }
}

/// Message for a leg that was never sent because an earlier leg in its group
/// failed.
fn remaining_failure_message(
    leg_symbol: &str,
    leg_role: &LegRole,
    failed_symbol: &str,
    failed_role: &LegRole,
    reason: &str,
) -> String {
    format!(
        "{} {} not placed because {} {} {}",
        leg_symbol,
        role_word(leg_role),
        failed_symbol,
        role_word(failed_role),
        reason
    )
}

// ============================================================================
// Execution
// ============================================================================

/// The leg that stopped its group, kept so the remaining legs can say why.
struct Halt {
    symbol: String,
    role: LegRole,
    reason: String,
}

/// Ok = leg succeeded. Err = leg failed, with the short reason later legs quote.
type StepResult = std::result::Result<LegResult, (LegResult, &'static str)>;

enum FillOutcome {
    /// Fully filled with this many shares.
    Filled(f64),
    /// Ended without a full fill (cap reached, or cancelled after a partial
    /// fill) with this many shares filled.
    Partial(f64),
    /// Rejected or cancelled with nothing filled.
    Failed(String),
}

enum CancelOutcome {
    Cancelled,
    Failed(String),
    TimedOut,
}

fn leg_result(leg: &StrategyLeg, success: bool, order_id: Option<String>, filled: f64, message: String) -> LegResult {
    LegResult {
        group: leg.group.clone(),
        role: leg.role.clone(),
        symbol: leg.symbol.clone(),
        success,
        order_id,
        filled_quantity: filled,
        message,
    }
}

fn failed(leg: &StrategyLeg, order_id: Option<String>, filled: f64, message: String, reason: &'static str) -> StepResult {
    Err((leg_result(leg, false, order_id, filled, message), reason))
}

/// Polls until the cancel shows up in the order book (or the order is no
/// longer open), or the cap is reached.
async fn wait_for_cancel(state: &AppState, order_id: &str) -> CancelOutcome {
    let start = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(POLL_SECONDS)).await;

        match OrderbookService::get_order_status(state, order_id, None).await {
            Ok(status) => match status.order {
                Some(order) if is_cancelled(&order) => return CancelOutcome::Cancelled,
                Some(order) if is_filled(&order) => {
                    return CancelOutcome::Failed("the order had already filled".to_string())
                }
                Some(order) if is_rejected(&order) => {
                    return CancelOutcome::Failed("the broker rejected the cancel".to_string())
                }
                Some(_) => {}
                None => match OrderbookService::get_open_orders(state, None).await {
                    Ok(open) => {
                        if !open.orders.iter().any(|o| o.order_id == order_id) {
                            return CancelOutcome::Cancelled;
                        }
                    }
                    Err(e) => warn!("strategy basket: open orders lookup failed while waiting for cancel: {}", e),
                },
            },
            Err(e) => warn!("strategy basket: status lookup for {} failed while waiting for cancel: {}", order_id, e),
        }

        if start.elapsed() >= Duration::from_secs(CANCEL_CAP_SECONDS) {
            return CancelOutcome::TimedOut;
        }
    }
}

/// Polls until the order fills, ends, or the cap is reached.
async fn wait_for_fill(state: &AppState, order_id: &str) -> FillOutcome {
    let start = Instant::now();
    let mut last_filled = 0.0_f64;
    loop {
        tokio::time::sleep(Duration::from_secs(POLL_SECONDS)).await;

        match OrderbookService::get_order_status(state, order_id, None).await {
            Ok(status) => {
                if let Some(order) = status.order {
                    last_filled = order.filled_quantity;
                    if is_filled(&order) {
                        return FillOutcome::Filled(order.filled_quantity);
                    }
                    if is_rejected(&order) {
                        let why = order
                            .rejection_reason
                            .clone()
                            .filter(|r| !r.trim().is_empty())
                            .unwrap_or_else(|| "no reason given".to_string());
                        return FillOutcome::Failed(format!("was rejected by the broker ({})", why));
                    }
                    if is_cancelled(&order) {
                        return if order.filled_quantity > 0.0 {
                            FillOutcome::Partial(order.filled_quantity)
                        } else {
                            FillOutcome::Failed("was cancelled before it filled".to_string())
                        };
                    }
                }
            }
            Err(e) => warn!("strategy basket: status lookup for {} failed while waiting for fill: {}", order_id, e),
        }

        if start.elapsed() >= Duration::from_secs(FILL_CAP_SECONDS) {
            return FillOutcome::Partial(last_filled);
        }
    }
}

async fn place(state: &AppState, order: OrderRequest, expected_mode: Option<bool>) -> std::result::Result<PlaceOrderResult, String> {
    match OrderService::place_order(state, order, None, expected_mode).await {
        Ok(r) if r.success => Ok(r),
        Ok(r) => Err(r.message),
        Err(e) => Err(e.to_string()),
    }
}

async fn run_cancel(state: &AppState, leg: &StrategyLeg) -> StepResult {
    let order_id = leg.cancel_order_id.clone().unwrap_or_default();
    let label = short_id(&order_id);

    match OrderService::cancel_order(state, &order_id, None, None).await {
        Ok(r) if r.success => {}
        Ok(r) => {
            return failed(
                leg,
                Some(order_id),
                0.0,
                format!("{} cancel of order {} failed: {}", leg.symbol, label, r.message),
                REASON_FAILED,
            )
        }
        Err(e) => {
            return failed(
                leg,
                Some(order_id),
                0.0,
                format!("{} cancel of order {} failed: {}", leg.symbol, label, e),
                REASON_FAILED,
            )
        }
    }

    match wait_for_cancel(state, &order_id).await {
        CancelOutcome::Cancelled => Ok(leg_result(
            leg,
            true,
            Some(order_id),
            0.0,
            format!("cancelled order {}", label),
        )),
        CancelOutcome::Failed(why) => failed(
            leg,
            Some(order_id),
            0.0,
            format!("{} cancel of order {} failed: {}", leg.symbol, label, why),
            REASON_FAILED,
        ),
        CancelOutcome::TimedOut => failed(
            leg,
            Some(order_id),
            0.0,
            format!("{} order {} was not cancelled within {} seconds", leg.symbol, label, CANCEL_CAP_SECONDS),
            REASON_NOT_CANCELLED,
        ),
    }
}

/// Fund (market sell) and Buy (market buy): place, then wait for the fill.
async fn run_fill(state: &AppState, leg: &StrategyLeg, expected_mode: Option<bool>) -> StepResult {
    let word = role_word(&leg.role);
    let order = leg.order.clone().unwrap_or_else(|| unreachable!("validated by plan_groups"));
    let wanted = order.quantity;

    let placed = match place(state, order, expected_mode).await {
        Ok(r) => r,
        Err(msg) => {
            return failed(leg, None, 0.0, format!("{} {} failed: {}", leg.symbol, word, msg), REASON_FAILED)
        }
    };
    let order_id = placed.order_id.clone();
    let Some(id) = order_id.clone() else {
        return failed(
            leg,
            None,
            0.0,
            format!("{} {} was accepted but the broker returned no order id, so its fill cannot be confirmed", leg.symbol, word),
            REASON_FAILED,
        );
    };

    let outcome = wait_for_fill(state, &id).await;

    // Whatever is left of a buy or sell that stopped short must not stay live.
    if matches!(outcome, FillOutcome::Partial(_)) {
        if let Err(e) = OrderService::cancel_order(state, &id, None, None).await {
            warn!("strategy basket: could not cancel remainder of {}: {}", id, e);
        }
    }

    match outcome {
        FillOutcome::Filled(qty) if qty > 0.0 => Ok(leg_result(
            leg,
            true,
            order_id,
            qty,
            format!("{} {} filled ({})", leg.symbol, word, shares(qty)),
        )),
        FillOutcome::Filled(_) => failed(
            leg,
            order_id,
            0.0,
            format!("{} {} was reported complete but no shares filled", leg.symbol, word),
            REASON_FAILED,
        ),
        FillOutcome::Partial(qty) if qty > 0.0 && leg.role == LegRole::Buy => Ok(leg_result(
            leg,
            true,
            order_id,
            qty,
            format!(
                "{} {} partly filled ({} of {}); the rest was cancelled",
                leg.symbol,
                word,
                shares(qty),
                shares(wanted)
            ),
        )),
        FillOutcome::Partial(qty) if qty > 0.0 => failed(
            leg,
            order_id,
            qty,
            format!(
                "{} {} only partly filled ({} of {}) within {} seconds",
                leg.symbol,
                word,
                shares(qty),
                shares(wanted),
                FILL_CAP_SECONDS
            ),
            REASON_NOT_FILLED,
        ),
        FillOutcome::Partial(_) => failed(
            leg,
            order_id,
            0.0,
            format!("{} {} did not fill within {} seconds", leg.symbol, word, FILL_CAP_SECONDS),
            REASON_NOT_FILLED,
        ),
        FillOutcome::Failed(why) => failed(
            leg,
            order_id,
            0.0,
            format!("{} {} {}", leg.symbol, word, why),
            REASON_FAILED,
        ),
    }
}

/// Protect: place the stop. When a buy precedes it in the group the stop is
/// sized to what actually filled.
async fn run_protect(
    state: &AppState,
    leg: &StrategyLeg,
    bought: Option<f64>,
    expected_mode: Option<bool>,
) -> StepResult {
    let mut order = leg.order.clone().unwrap_or_else(|| unreachable!("validated by plan_groups"));
    if let Some(filled) = bought {
        if filled <= 0.0 {
            return failed(
                leg,
                None,
                0.0,
                format!("{} stop not placed because the buy did not fill", leg.symbol),
                REASON_FAILED,
            );
        }
        order.quantity = filled;
    }
    let qty = order.quantity;

    match place(state, order, expected_mode).await {
        Ok(r) => Ok(leg_result(
            leg,
            true,
            r.order_id,
            qty,
            format!("{} trailing stop placed (GTC)", leg.symbol),
        )),
        Err(msg) => failed(
            leg,
            None,
            0.0,
            format!("{} trailing stop failed: {}", leg.symbol, msg),
            REASON_FAILED,
        ),
    }
}

/// Standalone: a single order with no sequencing.
async fn run_standalone(state: &AppState, leg: &StrategyLeg, expected_mode: Option<bool>) -> StepResult {
    let order = leg.order.clone().unwrap_or_else(|| unreachable!("validated by plan_groups"));
    let qty = order.quantity;
    let side = order.side.to_lowercase();
    match place(state, order, expected_mode).await {
        Ok(r) => Ok(leg_result(
            leg,
            true,
            r.order_id,
            0.0,
            format!("{} {} order placed ({})", leg.symbol, side, shares(qty)),
        )),
        Err(msg) => failed(
            leg,
            None,
            0.0,
            format!("{} {} order failed: {}", leg.symbol, side, msg),
            REASON_FAILED,
        ),
    }
}

async fn run_group(state: &AppState, group: Vec<StrategyLeg>, expected_mode: Option<bool>) -> Vec<LegResult> {
    let mut results = Vec::with_capacity(group.len());
    let mut halt: Option<Halt> = None;
    let mut bought: Option<f64> = None;

    for leg in group {
        if let Some(h) = &halt {
            let message = remaining_failure_message(&leg.symbol, &leg.role, &h.symbol, &h.role, &h.reason);
            results.push(leg_result(&leg, false, None, 0.0, message));
            continue;
        }

        let step = match leg.role {
            LegRole::Cancel => run_cancel(state, &leg).await,
            LegRole::Fund | LegRole::Buy => run_fill(state, &leg, expected_mode).await,
            LegRole::Protect => run_protect(state, &leg, bought, expected_mode).await,
            LegRole::Standalone => run_standalone(state, &leg, expected_mode).await,
        };

        match step {
            Ok(result) => {
                if leg.role == LegRole::Buy {
                    bought = Some(result.filled_quantity);
                }
                results.push(result);
            }
            Err((result, reason)) => {
                warn!("strategy basket: {}", result.message);
                halt = Some(Halt {
                    symbol: leg.symbol.clone(),
                    role: leg.role.clone(),
                    reason: reason.to_string(),
                });
                results.push(result);
            }
        }
    }

    results
}

/// Place a strategy basket. Groups run one after another; legs inside a group
/// run in order, each waiting for the previous step to take effect. `expected_mode`
/// is passed unchanged to every order so Gate B is enforced on every leg.
pub async fn place_strategy_basket(
    state: &AppState,
    legs: Vec<StrategyLeg>,
    expected_mode: Option<bool>,
) -> Result<Vec<LegResult>> {
    let groups = plan_groups(legs)?;
    info!("strategy basket: {} groups", groups.len());

    let mut all = Vec::new();
    for group in groups {
        all.extend(run_group(state, group, expected_mode).await);
    }
    Ok(all)
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    fn order(symbol: &str, side: &str) -> OrderRequest {
        serde_json::from_value(serde_json::json!({
            "symbol": symbol, "exchange": "NYSE", "side": side, "quantity": 10.0,
            "price": 0.0, "order_type": "MARKET", "product": "CNC", "validity": "DAY",
            "trigger_price": null, "disclosed_quantity": null, "amo": false
        }))
        .unwrap()
    }

    fn leg(group: Option<&str>, role: LegRole, symbol: &str) -> StrategyLeg {
        StrategyLeg {
            group: group.map(String::from),
            cancel_order_id: if role == LegRole::Cancel { Some("abc123".to_string()) } else { None },
            order: if role == LegRole::Cancel { None } else { Some(order(symbol, "BUY")) },
            role,
            symbol: symbol.to_string(),
        }
    }

    fn order_row(status: &str, qty: f64, filled: f64) -> Order {
        Order {
            order_id: "1".into(),
            exchange_order_id: None,
            symbol: "PFE".into(),
            exchange: "NYSE".into(),
            side: "SELL".into(),
            quantity: qty,
            filled_quantity: filled,
            pending_quantity: qty - filled,
            price: 0.0,
            trigger_price: 0.0,
            average_price: 0.0,
            order_type: "MARKET".into(),
            product: "CNC".into(),
            status: status.into(),
            validity: "DAY".into(),
            order_timestamp: String::new(),
            exchange_timestamp: None,
            rejection_reason: None,
            trail_percent: None,
            hwm: None,
        }
    }

    #[test]
    fn groups_sorted_cancel_fund_buy_protect() {
        let legs = vec![
            leg(Some("r1"), LegRole::Protect, "BAC"),
            leg(Some("r1"), LegRole::Buy, "BAC"),
            leg(Some("r1"), LegRole::Fund, "PFE"),
            leg(Some("r1"), LegRole::Cancel, "PFE"),
        ];
        let groups = plan_groups(legs).unwrap();
        assert_eq!(groups.len(), 1);
        let roles: Vec<LegRole> = groups[0].iter().map(|l| l.role.clone()).collect();
        assert_eq!(roles, vec![LegRole::Cancel, LegRole::Fund, LegRole::Buy, LegRole::Protect]);
    }

    #[test]
    fn standalone_legs_are_their_own_groups() {
        let legs = vec![
            leg(None, LegRole::Standalone, "AAPL"),
            leg(None, LegRole::Standalone, "MSFT"),
        ];
        assert_eq!(plan_groups(legs).unwrap().len(), 2);
    }

    #[test]
    fn standalone_with_a_group_name_is_still_its_own_group() {
        let legs = vec![
            leg(Some("g"), LegRole::Standalone, "AAPL"),
            leg(Some("g"), LegRole::Standalone, "MSFT"),
        ];
        assert_eq!(plan_groups(legs).unwrap().len(), 2);
    }

    #[test]
    fn groups_keep_first_appearance_order() {
        let legs = vec![
            leg(Some("b"), LegRole::Buy, "BAC"),
            leg(Some("a"), LegRole::Buy, "KO"),
            leg(Some("b"), LegRole::Protect, "BAC"),
        ];
        let groups = plan_groups(legs).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0][0].group.as_deref(), Some("b"));
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1][0].group.as_deref(), Some("a"));
    }

    #[test]
    fn cancel_without_order_id_is_validation_error() {
        let mut bad = leg(Some("r1"), LegRole::Cancel, "PFE");
        bad.cancel_order_id = None;
        match plan_groups(vec![bad]) {
            Err(AppError::Validation(msg)) => assert_eq!(msg, "cancel leg for PFE has no cancel_order_id"),
            other => panic!("expected validation error, got {:?}", other.map(|g| g.len())),
        }
    }

    #[test]
    fn non_cancel_without_order_is_validation_error() {
        let mut bad = leg(Some("r1"), LegRole::Fund, "PFE");
        bad.order = None;
        match plan_groups(vec![bad]) {
            Err(AppError::Validation(msg)) => assert_eq!(msg, "fund leg for PFE has no order"),
            other => panic!("expected validation error, got {:?}", other.map(|g| g.len())),
        }
    }

    #[test]
    fn order_symbol_must_match_leg_symbol() {
        let mut bad = leg(Some("r1"), LegRole::Buy, "BAC");
        bad.order = Some(order("KO", "BUY"));
        assert!(matches!(plan_groups(vec![bad]), Err(AppError::Validation(_))));

        let mut ok = leg(Some("r1"), LegRole::Buy, "BAC");
        ok.order = Some(order("bac", "BUY"));
        assert!(plan_groups(vec![ok]).is_ok());
    }

    #[test]
    fn cancel_for_same_symbol_precedes_fund_even_if_listed_after() {
        let legs = vec![
            leg(Some("r1"), LegRole::Fund, "PFE"),
            leg(Some("r1"), LegRole::Cancel, "PFE"),
        ];
        let groups = plan_groups(legs).unwrap();
        assert_eq!(groups[0][0].role, LegRole::Cancel);
        assert_eq!(groups[0][1].role, LegRole::Fund);
    }

    #[test]
    fn legs_deserialize_from_frontend_json() {
        let leg: StrategyLeg = serde_json::from_value(serde_json::json!({
            "group": "r1", "role": "cancel", "symbol": "PFE", "cancel_order_id": "abc"
        }))
        .unwrap();
        assert_eq!(leg.role, LegRole::Cancel);
        assert_eq!(serde_json::to_value(&LegRole::Protect).unwrap(), "protect");
    }

    #[test]
    fn filled_when_complete_or_fully_filled() {
        assert!(is_filled(&order_row("COMPLETE", 11.0, 11.0)));
        assert!(is_filled(&order_row("OPEN", 11.0, 11.0)));
        assert!(!is_filled(&order_row("PARTIALLY_FILLED", 11.0, 5.0)));
        assert!(!is_filled(&order_row("OPEN", 11.0, 0.0)));
        assert!(!is_filled(&order_row("PENDING", 0.0, 0.0)));
    }

    #[test]
    fn cancelled_and_rejected_statuses() {
        assert!(is_cancelled(&order_row("CANCELLED", 1.0, 0.0)));
        assert!(!is_cancelled(&order_row("OPEN", 1.0, 0.0)));
        assert!(is_rejected(&order_row("REJECTED", 1.0, 0.0)));
        assert!(!is_rejected(&order_row("COMPLETE", 1.0, 1.0)));
    }

    #[test]
    fn remaining_leg_message_names_the_failed_step() {
        assert_eq!(
            remaining_failure_message("BAC", &LegRole::Buy, "PFE", &LegRole::Fund, REASON_NOT_FILLED),
            "BAC buy not placed because PFE sell did not fill within 60 seconds"
        );
        assert_eq!(
            remaining_failure_message("PFE", &LegRole::Fund, "PFE", &LegRole::Cancel, REASON_NOT_CANCELLED),
            "PFE sell not placed because PFE cancel was not cancelled within 30 seconds"
        );
        assert_eq!(
            remaining_failure_message("BAC", &LegRole::Protect, "BAC", &LegRole::Buy, REASON_FAILED),
            "BAC trailing stop not placed because BAC buy failed"
        );
    }

    #[test]
    fn share_and_id_formatting() {
        assert_eq!(shares(11.0), "11 shares");
        assert_eq!(shares(1.0), "1 share");
        assert_eq!(shares(2.5), "2.5 shares");
        assert_eq!(short_id("3f9c1a2b-77aa-4c10"), "3f9c1a2b…");
        assert_eq!(short_id("abc123"), "abc123");
    }
}
