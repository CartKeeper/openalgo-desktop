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
//! the next one. A group with a cancel, fund or buy leg runs only while the
//! market clock says open; otherwise none of its legs is sent.

use crate::brokers::types::{Order, OrderRequest};
use crate::error::{AppError, Result};
use crate::services::market_status_service::MarketStatusService;
use crate::services::{OrderService, OrderbookService, PlaceOrderResult};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// How often an order's status is re-checked while waiting.
const POLL_SECONDS: u64 = 2;
/// Longest wait for a cancel to take effect.
const CANCEL_CAP_SECONDS: u64 = 30;
/// Longest wait for a market order to fill.
const FILL_CAP_SECONDS: u64 = 60;
/// Longest wait for a single order-book lookup; a timeout counts as a failed lookup.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Same copy as the review modal's market-closed line.
const MARKET_CLOSED_MESSAGE: &str = "Market opens at 9:30 AM ET. Place this then.";
const CLOCK_UNREADABLE_SUFFIX: &str = " (could not read the market clock)";

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
        Some(order) => {
            let required_side = match leg.role {
                LegRole::Fund => Some("SELL"),
                LegRole::Buy => Some("BUY"),
                _ => None,
            };
            match required_side {
                Some(side) if !order.side.eq_ignore_ascii_case(side) => Err(AppError::Validation(format!(
                    "{} leg for {} must be a {} order, not {}",
                    match leg.role {
                        LegRole::Fund => "fund",
                        _ => "buy",
                    },
                    leg.symbol,
                    side,
                    order.side
                ))),
                _ => Ok(()),
            }
        }
    }
}

/// Group-level checks: at most one Buy, a Protect must protect that Buy's
/// symbol, and a Protect with no Buy needs its own quantity.
fn validate_group(name: &Option<String>, members: &[StrategyLeg]) -> Result<()> {
    let label = name.as_deref().unwrap_or("(ungrouped)");
    let buys: Vec<&StrategyLeg> = members.iter().filter(|l| l.role == LegRole::Buy).collect();
    if buys.len() > 1 {
        return Err(AppError::Validation(format!("group {} has more than one buy leg", label)));
    }
    for protect in members.iter().filter(|l| l.role == LegRole::Protect) {
        match buys.first() {
            Some(buy) if !buy.symbol.eq_ignore_ascii_case(&protect.symbol) => {
                return Err(AppError::Validation(format!(
                    "protect leg for {} does not match the buy leg for {} in group {}",
                    protect.symbol, buy.symbol, label
                )));
            }
            Some(_) => {}
            None => {
                let qty = protect.order.as_ref().map(|o| o.quantity).unwrap_or(0.0);
                if qty <= 0.0 {
                    return Err(AppError::Validation(format!(
                        "protect leg for {} in group {} has no quantity and no buy to size it from",
                        protect.symbol, label
                    )));
                }
            }
        }
    }
    Ok(())
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

    for (name, members) in &groups {
        validate_group(name, members)?;
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

/// What to tell the user about the unfilled remainder of an order we tried to cancel.
fn remainder_note(confirmed: bool) -> &'static str {
    if confirmed {
        "remainder confirmed cancelled"
    } else {
        "remainder may still be working at the broker \u{2014} check the Order Book"
    }
}

/// Appends a note for each symbol whose old stop was cancelled earlier in the
/// group, so a halted group says plainly that the position is unprotected.
fn with_old_stop_note(message: String, cancelled_symbols: &[String]) -> String {
    let mut out = message;
    for symbol in cancelled_symbols {
        out.push_str(&format!("; the old stop on {} is no longer active", symbol));
    }
    out
}

/// Success message for a stop leg, from the order actually placed.
fn protect_success_message(symbol: &str, order_type: &str, validity: &str) -> String {
    if order_type == "TRAILING_STOP" {
        format!("{} trailing stop placed ({})", symbol, validity)
    } else {
        format!("{} stop placed ({})", symbol, validity)
    }
}

/// Gate B wording, identical to `OrderService::place_order`. `None` = modes agree
/// (or no acknowledgment was supplied).
fn mode_mismatch_message(expected: Option<bool>, analyze_mode: bool) -> Option<String> {
    let expected = expected?;
    if expected == analyze_mode {
        return None;
    }
    let ack = if expected { "paper (sandbox)" } else { "LIVE (real money)" };
    let actual = if analyze_mode { "paper (sandbox)" } else { "LIVE (real money)" };
    Some(format!(
        "Order rejected: you confirmed {ack} mode, but the app is now in {actual} mode. \
         Re-confirm before placing."
    ))
}

/// Market-hours gate, the same predicate as the review modal: a group with a
/// cancel, fund or buy leg only runs while the market is open. Protect-only
/// groups (GTC stops) and standalone legs are not gated.
fn group_needs_open_market(group: &[StrategyLeg]) -> bool {
    group
        .iter()
        .any(|l| matches!(l.role, LegRole::Cancel | LegRole::Fund | LegRole::Buy))
}

/// Results for a gated group refused because the market is closed (or its
/// clock could not be read): nothing in the group was placed or cancelled.
fn closed_market_results(group: &[StrategyLeg], read_failed: bool) -> Vec<LegResult> {
    let message = if read_failed {
        format!("{}{}", MARKET_CLOSED_MESSAGE, CLOCK_UNREADABLE_SUFFIX)
    } else {
        MARKET_CLOSED_MESSAGE.to_string()
    };
    group
        .iter()
        .map(|leg| leg_result(leg, false, None, 0.0, message.clone()))
        .collect()
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
    /// Stopped short of a full fill (cap reached, or cancelled after a partial
    /// fill). Carries the last fill count seen, which may be stale.
    Partial(f64),
    /// Rejected or cancelled with nothing filled.
    Failed(String),
}

enum CancelOutcome {
    Cancelled,
    Failed(String),
    TimedOut,
}

/// State of an order's unfilled remainder after a cancel attempt.
struct Settled {
    /// Fill count from a fresh read after the cancel (falls back to the last
    /// seen count when that read failed).
    filled: f64,
    /// True only when the remainder is known to be gone.
    confirmed: bool,
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

/// One order-status lookup. Outer `None` = the lookup failed or timed out
/// (logged); `Some(None)` = the order is not in the order book.
async fn lookup_status(state: &AppState, order_id: &str) -> Option<Option<Order>> {
    match tokio::time::timeout(LOOKUP_TIMEOUT, OrderbookService::get_order_status(state, order_id, None)).await {
        Ok(Ok(r)) => Some(r.order),
        Ok(Err(e)) => {
            warn!("strategy basket: status lookup for {} failed: {}", order_id, e);
            None
        }
        Err(_) => {
            warn!("strategy basket: status lookup for {} timed out", order_id);
            None
        }
    }
}

/// Whether the order is still listed among open orders. `None` = lookup failed.
async fn lookup_is_open(state: &AppState, order_id: &str) -> Option<bool> {
    match tokio::time::timeout(LOOKUP_TIMEOUT, OrderbookService::get_open_orders(state, None)).await {
        Ok(Ok(open)) => Some(open.orders.iter().any(|o| o.order_id == order_id)),
        Ok(Err(e)) => {
            warn!("strategy basket: open orders lookup failed: {}", e);
            None
        }
        Err(_) => {
            warn!("strategy basket: open orders lookup timed out");
            None
        }
    }
}

/// Polls until the cancel shows up in the order book (or the order is no
/// longer open), or the cap is reached.
async fn wait_for_cancel(state: &AppState, order_id: &str) -> CancelOutcome {
    let start = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(POLL_SECONDS)).await;

        match lookup_status(state, order_id).await {
            Some(Some(order)) if is_cancelled(&order) => return CancelOutcome::Cancelled,
            Some(Some(order)) if is_filled(&order) => {
                return CancelOutcome::Failed("the order had already filled".to_string())
            }
            Some(Some(order)) if is_rejected(&order) => {
                return CancelOutcome::Failed("the broker rejected the cancel".to_string())
            }
            Some(Some(_)) => {}
            Some(None) => {
                if lookup_is_open(state, order_id).await == Some(false) {
                    return CancelOutcome::Cancelled;
                }
            }
            None => {}
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

        if let Some(Some(order)) = lookup_status(state, order_id).await {
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

        if start.elapsed() >= Duration::from_secs(FILL_CAP_SECONDS) {
            return FillOutcome::Partial(last_filled);
        }
    }
}

/// Cancels whatever is left of an order that stopped short, waits for the
/// cancel to take effect, then re-reads the order so the final fill count is
/// what the broker actually shows after the race.
async fn settle_remainder(state: &AppState, order_id: &str, last_filled: f64) -> Settled {
    let mut wait_confirmed = false;
    match OrderService::cancel_order(state, order_id, None, None).await {
        Ok(r) if r.success => {
            wait_confirmed = matches!(wait_for_cancel(state, order_id).await, CancelOutcome::Cancelled);
        }
        Ok(r) => warn!("strategy basket: cancel of remainder of {} was refused: {}", order_id, r.message),
        Err(e) => warn!("strategy basket: could not cancel remainder of {}: {}", order_id, e),
    }

    match lookup_status(state, order_id).await {
        Some(Some(order)) => Settled {
            filled: order.filled_quantity,
            confirmed: wait_confirmed || is_cancelled(&order) || is_filled(&order),
        },
        // Final read failed: the fill count is stale, so the remainder cannot be vouched for.
        _ => Settled { filled: last_filled, confirmed: false },
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

    match wait_for_fill(state, &id).await {
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
        FillOutcome::Failed(why) => failed(
            leg,
            order_id,
            0.0,
            format!("{} {} {}", leg.symbol, word, why),
            REASON_FAILED,
        ),
        FillOutcome::Partial(last_filled) => {
            // Stopped short: cancel the rest, then trust only a fresh read.
            let settled = settle_remainder(state, &id, last_filled).await;
            let note = remainder_note(settled.confirmed);
            let qty = settled.filled;
            if wanted > 0.0 && qty >= wanted - 1e-9 {
                // The rest filled while we were cancelling.
                Ok(leg_result(
                    leg,
                    true,
                    order_id,
                    qty,
                    format!("{} {} filled ({})", leg.symbol, word, shares(qty)),
                ))
            } else if qty > 0.0 && leg.role == LegRole::Buy {
                Ok(leg_result(
                    leg,
                    true,
                    order_id,
                    qty,
                    format!(
                        "{} {} partly filled ({} of {}); {}",
                        leg.symbol,
                        word,
                        shares(qty),
                        shares(wanted),
                        note
                    ),
                ))
            } else if qty > 0.0 {
                failed(
                    leg,
                    order_id,
                    qty,
                    format!(
                        "{} {} only partly filled ({} of {}); {}",
                        leg.symbol,
                        word,
                        shares(qty),
                        shares(wanted),
                        note
                    ),
                    REASON_NOT_FILLED,
                )
            } else {
                failed(
                    leg,
                    order_id,
                    0.0,
                    format!("{} {} did not fill within {} seconds; {}", leg.symbol, word, FILL_CAP_SECONDS, note),
                    REASON_NOT_FILLED,
                )
            }
        }
    }
}

/// Protect: place the stop. When a buy precedes it in the group the stop is
/// sized to what actually filled (`bought`, taken from the same-symbol buy).
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
    let success_message = protect_success_message(&leg.symbol, &order.order_type, &order.validity);

    match place(state, order, expected_mode).await {
        Ok(r) => Ok(leg_result(leg, true, r.order_id, qty, success_message)),
        Err(msg) => failed(
            leg,
            None,
            0.0,
            format!("{} stop failed: {}", leg.symbol, msg),
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

/// Whether the market is open now. `None` = the clock could not be read (a
/// timeout counts as a failed read).
async fn read_market_open(state: &AppState) -> Option<bool> {
    match tokio::time::timeout(LOOKUP_TIMEOUT, MarketStatusService::get_clock(state, None)).await {
        Ok(Ok(clock)) => Some(clock.is_open),
        Ok(Err(e)) => {
            warn!("strategy basket: market clock read failed: {}", e);
            None
        }
        Err(_) => {
            warn!("strategy basket: market clock read timed out");
            None
        }
    }
}

async fn run_group(state: &AppState, group: Vec<StrategyLeg>, expected_mode: Option<bool>) -> Vec<LegResult> {
    if group_needs_open_market(&group) {
        match read_market_open(state).await {
            Some(true) => {}
            Some(false) => return closed_market_results(&group, false),
            None => return closed_market_results(&group, true),
        }
    }

    let mut results = Vec::with_capacity(group.len());
    let mut halt: Option<Halt> = None;
    // Fill of each successful buy, keyed by uppercased symbol.
    let mut bought: HashMap<String, f64> = HashMap::new();
    // Symbols whose old stop this group has already cancelled.
    let mut cancelled: Vec<String> = Vec::new();

    for leg in group {
        if let Some(h) = &halt {
            let message = with_old_stop_note(
                remaining_failure_message(&leg.symbol, &leg.role, &h.symbol, &h.role, &h.reason),
                &cancelled,
            );
            results.push(leg_result(&leg, false, None, 0.0, message));
            continue;
        }

        let step = match leg.role {
            LegRole::Cancel => run_cancel(state, &leg).await,
            LegRole::Fund | LegRole::Buy => run_fill(state, &leg, expected_mode).await,
            LegRole::Protect => {
                let filled = bought.get(&leg.symbol.to_uppercase()).copied();
                run_protect(state, &leg, filled, expected_mode).await
            }
            LegRole::Standalone => run_standalone(state, &leg, expected_mode).await,
        };

        match step {
            Ok(result) => {
                match leg.role {
                    LegRole::Buy => {
                        bought.insert(leg.symbol.to_uppercase(), result.filled_quantity);
                    }
                    LegRole::Cancel if !cancelled.contains(&leg.symbol) => cancelled.push(leg.symbol.clone()),
                    _ => {}
                }
                results.push(result);
            }
            Err((mut result, reason)) => {
                result.message = with_old_stop_note(result.message, &cancelled);
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

    // Cancel legs run before any order is placed, so Gate B must be checked up
    // front or a mode mismatch would cancel a live stop and only then reject.
    // (place_order still re-checks every order.)
    let analyze_mode = state.sqlite.get_analyze_mode().unwrap_or(false);
    if let Some(message) = mode_mismatch_message(expected_mode, analyze_mode) {
        warn!("strategy basket rejected before any cancel: {}", message);
        return Err(AppError::Validation(message));
    }

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
            order: if role == LegRole::Cancel {
                None
            } else {
                Some(order(symbol, if role == LegRole::Fund { "SELL" } else { "BUY" }))
            },
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

    #[test]
    fn unknown_and_partial_statuses_are_not_filled_or_cancelled() {
        assert!(!is_filled(&order_row("UNKNOWN", 11.0, 0.0)));
        assert!(!is_cancelled(&order_row("UNKNOWN", 11.0, 0.0)));
        assert!(!is_cancelled(&order_row("PARTIALLY_FILLED", 11.0, 5.0)));
    }

    #[test]
    fn two_buys_in_a_group_is_validation_error() {
        let legs = vec![
            leg(Some("r1"), LegRole::Buy, "BAC"),
            leg(Some("r1"), LegRole::Buy, "KO"),
        ];
        match plan_groups(legs) {
            Err(AppError::Validation(msg)) => assert!(msg.contains("r1") && msg.contains("more than one buy")),
            other => panic!("expected validation error, got {:?}", other.map(|g| g.len())),
        }
    }

    #[test]
    fn protect_symbol_must_match_the_groups_buy() {
        let legs = vec![
            leg(Some("r1"), LegRole::Buy, "BAC"),
            leg(Some("r1"), LegRole::Protect, "KO"),
        ];
        match plan_groups(legs) {
            Err(AppError::Validation(msg)) => assert!(msg.contains("KO") && msg.contains("BAC") && msg.contains("r1")),
            other => panic!("expected validation error, got {:?}", other.map(|g| g.len())),
        }
        let ok = vec![
            leg(Some("r1"), LegRole::Buy, "BAC"),
            leg(Some("r1"), LegRole::Protect, "bac"),
        ];
        assert!(plan_groups(ok).is_ok());
    }

    #[test]
    fn fund_must_sell_and_buy_must_buy() {
        let mut fund = leg(Some("r1"), LegRole::Fund, "PFE");
        fund.order = Some(order("PFE", "BUY"));
        assert!(matches!(plan_groups(vec![fund]), Err(AppError::Validation(_))));

        let mut buy = leg(Some("r1"), LegRole::Buy, "BAC");
        buy.order = Some(order("BAC", "SELL"));
        assert!(matches!(plan_groups(vec![buy]), Err(AppError::Validation(_))));
    }

    #[test]
    fn protect_without_a_buy_needs_a_quantity() {
        let mut zero = leg(Some("p1"), LegRole::Protect, "PFE");
        let mut o = order("PFE", "SELL");
        o.quantity = 0.0;
        zero.order = Some(o);
        assert!(matches!(plan_groups(vec![zero]), Err(AppError::Validation(_))));

        let with_qty = leg(Some("p1"), LegRole::Protect, "PFE");
        assert!(plan_groups(vec![with_qty]).is_ok());

        // With a buy present, the protect's own quantity is not required.
        let mut sized_by_buy = leg(Some("r1"), LegRole::Protect, "BAC");
        let mut o = order("BAC", "SELL");
        o.quantity = 0.0;
        sized_by_buy.order = Some(o);
        let legs = vec![leg(Some("r1"), LegRole::Buy, "BAC"), sized_by_buy];
        assert!(plan_groups(legs).is_ok());
    }

    #[test]
    fn halt_message_says_the_old_stop_is_gone_only_when_a_cancel_succeeded() {
        assert_eq!(
            with_old_stop_note("PFE sell failed: rejected".to_string(), &["PFE".to_string()]),
            "PFE sell failed: rejected; the old stop on PFE is no longer active"
        );
        assert_eq!(
            with_old_stop_note(
                "BAC buy not placed because PFE sell failed".to_string(),
                &["PFE".to_string(), "KO".to_string()]
            ),
            "BAC buy not placed because PFE sell failed; the old stop on PFE is no longer active; the old stop on KO is no longer active"
        );
        assert_eq!(with_old_stop_note("PFE sell failed".to_string(), &[]), "PFE sell failed");
    }

    #[test]
    fn remainder_note_distinguishes_confirmed_from_unconfirmed() {
        assert_eq!(remainder_note(true), "remainder confirmed cancelled");
        assert!(remainder_note(false).starts_with("remainder may still be working at the broker"));
        assert!(remainder_note(false).ends_with("check the Order Book"));
    }

    #[test]
    fn protect_message_reflects_the_placed_order() {
        assert_eq!(protect_success_message("BAC", "TRAILING_STOP", "GTC"), "BAC trailing stop placed (GTC)");
        assert_eq!(protect_success_message("BAC", "SL-M", "DAY"), "BAC stop placed (DAY)");
    }

    #[test]
    fn mode_gate_matches_place_order_wording() {
        assert_eq!(mode_mismatch_message(None, true), None);
        assert_eq!(mode_mismatch_message(Some(false), false), None);
        assert_eq!(
            mode_mismatch_message(Some(false), true).unwrap(),
            "Order rejected: you confirmed LIVE (real money) mode, but the app is now in paper (sandbox) mode. Re-confirm before placing."
        );
        assert_eq!(
            mode_mismatch_message(Some(true), false).unwrap(),
            "Order rejected: you confirmed paper (sandbox) mode, but the app is now in LIVE (real money) mode. Re-confirm before placing."
        );
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    fn leg(group: Option<&str>, role: LegRole, symbol: &str) -> StrategyLeg {
        StrategyLeg {
            group: group.map(String::from),
            role,
            symbol: symbol.to_string(),
            cancel_order_id: None,
            order: None,
        }
    }

    #[test]
    fn cancel_fund_and_buy_groups_need_an_open_market() {
        assert!(group_needs_open_market(&[leg(Some("r1"), LegRole::Cancel, "PFE"), leg(Some("r1"), LegRole::Protect, "PFE")]));
        assert!(group_needs_open_market(&[leg(Some("r1"), LegRole::Fund, "PFE")]));
        assert!(group_needs_open_market(&[leg(Some("r1"), LegRole::Buy, "BAC"), leg(Some("r1"), LegRole::Protect, "BAC")]));
        assert!(!group_needs_open_market(&[leg(Some("p1"), LegRole::Protect, "NVDA")]));
        assert!(!group_needs_open_market(&[leg(None, LegRole::Standalone, "AAPL")]));
    }

    #[test]
    fn closed_market_fails_every_leg_of_the_group_with_the_modal_copy() {
        let group = vec![leg(Some("r1"), LegRole::Cancel, "PFE"), leg(Some("r1"), LegRole::Fund, "PFE")];
        let closed = closed_market_results(&group, false);
        assert_eq!(closed.len(), 2);
        for r in &closed {
            assert!(!r.success);
            assert_eq!(r.order_id, None);
            assert_eq!(r.filled_quantity, 0.0);
            assert_eq!(r.message, "Market opens at 9:30 AM ET. Place this then.");
            assert_eq!(r.group.as_deref(), Some("r1"));
        }
        assert_eq!(closed[1].role, LegRole::Fund);

        let unread = closed_market_results(&group, true);
        assert!(unread
            .iter()
            .all(|r| r.message == "Market opens at 9:30 AM ET. Place this then. (could not read the market clock)"));
    }
}
