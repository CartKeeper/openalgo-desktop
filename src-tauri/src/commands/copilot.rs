//! AI Research Copilot IPC commands
//!
//! Tauri commands for the AI Research Copilot feature.
//! Routes frontend requests through the CopilotService for
//! AI-powered financial research with tool-calling support.

use crate::error::AppError;
use crate::providers::anthropic::Message;
use crate::services::copilot_service::{CopilotContext, CopilotResponse, CopilotService};
use crate::services::ride_report::{build_ride_report, render_for_prompt, RideReport};
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

/// Send a message to the AI Research Copilot
///
/// Processes the user message through Anthropic's API with tool-calling support.
/// If the AI needs market data, it will automatically call the appropriate providers
/// (Yahoo Finance, FMP, FRED) and incorporate the results into its response.
#[tauri::command]
pub async fn copilot_send_message(
    state: State<'_, AppState>,
    message: String,
    conversation_history_json: String,
    context: Option<String>,
) -> Result<CopilotResponse, AppError> {
    let context = CopilotContext::from_param(context.as_deref())?;

    // Get the Anthropic API key
    let api_key = state
        .sqlite
        .get_provider_key("anthropic", &state.security)?
        .ok_or_else(|| {
            AppError::Provider(
                "Anthropic API key not configured. Please add your API key in Settings > Providers."
                    .to_string(),
            )
        })?;

    // Parse conversation history from JSON
    let conversation_history: Vec<Message> = if conversation_history_json.is_empty()
        || conversation_history_json == "[]"
    {
        Vec::new()
    } else {
        serde_json::from_str(&conversation_history_json).map_err(|e| {
            AppError::Validation(format!("Invalid conversation history JSON: {}", e))
        })?
    };

    // Process through the copilot service
    let response =
        CopilotService::process_message(&api_key, conversation_history, &message, context, &state)
            .await?;

    Ok(response)
}

/// Check if the AI Research Copilot is configured (Anthropic API key exists)
#[tauri::command]
pub async fn copilot_check_configured(
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    let key = state
        .sqlite
        .get_provider_key("anthropic", &state.security)?;
    Ok(key.is_some())
}

/// The briefing's AI text plus the deterministic Ride Report it was built from.
#[derive(Serialize)]
pub struct BriefingResponse {
    #[serde(flatten)]
    pub copilot: CopilotResponse,
    pub ride_report: RideReport,
}

/// Generate a 10-Minute Briefing
///
/// Builds the Ride Report from the broker, injects it with cash and open orders into
/// the hitchhiker-strategy prompt, lets the model gather market data via tool calls,
/// and returns the model's briefing alongside the Ride Report.
#[tauri::command]
pub async fn generate_briefing(
    state: State<'_, AppState>,
) -> Result<BriefingResponse, AppError> {
    let api_key = state
        .sqlite
        .get_provider_key("anthropic", &state.security)?
        .ok_or_else(|| {
            AppError::Provider(
                "Anthropic API key not configured. Please add your API key in Settings > Providers."
                    .to_string(),
            )
        })?;

    // Deterministic facts per held position; never fails (unknown fields stay unknown).
    let ride_report = build_ride_report(&state).await;

    // Available cash sizes every BUY recommendation. This is the broker's settled,
    // spendable buying power — open-order reservations and unsettled funds are
    // already excluded. Best-effort: if it can't be read we fall back to a generic,
    // cash-aware instruction and the frontend's hard affordability cap still
    // drops/reduces unaffordable buys before display. Routes to sandbox cash in
    // analyze (paper) mode, matching where orders execute.
    let cash_budget = match crate::services::FundsService::get_funds(&state, None).await {
        Ok(result) if result.funds.available_cash > 0.0 => Some(result.funds.available_cash),
        _ => None,
    };
    let cash_line = match cash_budget {
        Some(cash) => format!("Available cash for new buys: ${:.2} (settled funds — money reserved by the open orders below is already excluded)", cash),
        None => "Available cash for new buys: unavailable — size buys conservatively and assume a tight budget.".to_string(),
    };

    // Working orders from the dedicated open-orders query, each with its order id so
    // the model can emit `cancel` legs. Without this the briefing double-spends
    // reserved cash and proposes stops that the broker rejects for held shares.
    let open_orders_block = match crate::services::OrderbookService::get_open_orders(&state, None).await {
        Ok(result) => {
            let open: Vec<String> = result
                .orders
                .iter()
                .map(|o| {
                    let pending_qty = if o.pending_quantity > 0.0 {
                        o.pending_quantity
                    } else {
                        o.quantity - o.filled_quantity
                    };
                    let terms = match o.trail_percent {
                        Some(trail) => format!("trail {:.1}%", trail),
                        None => format!(
                            "@ {}{}",
                            if o.price > 0.0 { format!("${:.2}", o.price) } else { "market".to_string() },
                            if o.trigger_price > 0.0 { format!(" (trigger ${:.2})", o.trigger_price) } else { String::new() },
                        ),
                    };
                    format!(
                        "  [id {}] {} {} x{} {} {} | {} | placed {}",
                        o.order_id,
                        o.side.to_uppercase(),
                        o.symbol,
                        pending_qty,
                        o.order_type,
                        terms,
                        o.validity,
                        o.order_timestamp,
                    )
                })
                .collect();
            if open.is_empty() {
                "No open orders.".to_string()
            } else {
                format!(
                    "{}\n\nThese orders are WORKING at the broker right now. Pending BUY orders keep their cost reserved (already excluded from available cash). Never propose a stop for a position the Ride Report marks protected. When the Ride Report lists \"working sells to cancel first\" for a symbol, emit `cancel` legs for those ids before its `fund` sell or replacement `protect` stop. When its Protection is \"unknown\", propose and cancel nothing for that symbol. If a pending BUY looks stale (placed days ago, far from the market), you may recommend cancelling it to free its reserved cash — mention this in Risk Radar.",
                    open.join("\n")
                )
            }
        }
        Err(_) => "Open orders could not be read. Propose and cancel nothing (no `fund`, `protect` or `cancel` actions) for any held symbol.".to_string(),
    };

    let symbols = if ride_report.positions.is_empty() {
        "general market".to_string()
    } else {
        ride_report
            .positions
            .iter()
            .map(|p| p.symbol.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let briefing_prompt = build_briefing_prompt(
        &render_for_prompt(&ride_report),
        &cash_line,
        &open_orders_block,
        &symbols,
        ride_report.paper_mode,
    );

    // Process through the copilot service with empty history (fresh briefing)
    let copilot = CopilotService::process_message(
        &api_key,
        Vec::new(),
        &briefing_prompt,
        CopilotContext::OwnAccount,
        &state,
    )
    .await?;

    Ok(BriefingResponse { copilot, ride_report })
}

/// Example ACTIONS_JSON shown to the model: a funded rotation, then a cancel-and-replace protection group.
const ACTIONS_EXAMPLE: &str = r#"[
  { "symbol": "PFE", "exchange": "NYSE", "side": "SELL", "quantity": 11, "orderType": "MARKET", "product": "CNC", "price": 0, "validity": "DAY", "group": "rotation-1", "role": "fund", "rationale": "Dormant: +1.2% net over 30 days, analysts see +2% upside. We've made what we can." },
  { "symbol": "BAC", "exchange": "NYSE", "side": "BUY", "quantity": 13, "orderType": "MARKET", "product": "CNC", "price": 0, "validity": "DAY", "group": "rotation-1", "role": "buy", "rationale": "Consensus target $58 (+12%), 14 analysts; +6% over 30 days; funded by PFE proceeds + cash." },
  { "symbol": "BAC", "exchange": "NYSE", "side": "SELL", "quantity": 13, "orderType": "TRAILING_STOP", "product": "CNC", "price": 0, "trailPercent": 4.2, "validity": "GTC", "group": "rotation-1", "role": "protect", "rationale": "Trail 4.2%: BAC moves 2.1% on a normal day." },
  { "symbol": "NVDA", "exchange": "NASDAQ", "side": "SELL", "group": "protect-NVDA", "role": "cancel", "cancelOrderId": "3f9c2b1e-7a4d-4c8e-9b0f-5d6e7a8b9c01", "rationale": "Cancel the DAY stop at $192 so the GTC trailing stop can replace it." },
  { "symbol": "NVDA", "exchange": "NASDAQ", "side": "SELL", "quantity": 4, "orderType": "TRAILING_STOP", "product": "CNC", "price": 0, "trailPercent": 9.9, "validity": "GTC", "group": "protect-NVDA", "role": "protect", "rationale": "Trail 9.9%: NVDA moves 3.3% on a normal day and analysts see 40% upside, so it gets room." }
]"#;

/// The 10-Minute Briefing prompt: the hitchhiker strategy around the injected Ride Report.
///
/// Pure so the section order and rules are unit-testable without the model.
pub fn build_briefing_prompt(
    report_table: &str,
    cash_line: &str,
    open_orders_block: &str,
    symbols: &str,
    paper_mode: bool,
) -> String {
    const LIVE_ONLY: &str = "Write exactly one line: Live account only.";

    let paper_note = if paper_mode {
        "\n\nPAPER MODE (overrides every rule below): this is the paper account. The Scoreboard, Rotation and Protection sections must each be the single line `Live account only.` Emit no `protect`, `fund` or `cancel` actions."
    } else {
        ""
    };

    let scoreboard = if paper_mode {
        LIVE_ONLY.to_string()
    } else {
        "Account value now, one week ago and one month ago, each with the dollar and percent change, from the Ride Report's Scoreboard. One sentence on whether the game is working. If the Scoreboard is not available, say so in one line.".to_string()
    };

    let rotation = if paper_mode {
        LIVE_ONLY.to_string()
    } else {
        r#"Present whenever at least one position is Dormant in the Ride Report.
- No Dormant position: one line saying so and why (every ride is still climbing / too new to judge / falling and handled in Risk Radar).
- Each Dormant position: one line of evidence, e.g. "PFE: net move +1.2% over 30 days, analysts see +2% upside, up 18% from your cost. We've made what we can."
- The single best ride found today (from New Stock Corner) with its evidence: analyst consensus target and upside, 30-day move, catalyst.
- Selection rule: the rotation target must have consensus upside at least 10 percentage points higher than the dormant position being sold, must have `targets_last_90d` ≥ 3, and must not be up more than 10% intraday (the existing spike rule applies). If no candidate qualifies, no rotation: say so and propose nothing.
- Show the money: proceeds from the sells at current prices + available cash = budget; whole-share quantity of the buy; leftover cash.
- The new ride's trailing stop with its trail percent and reason. Its daily swing is the `daily_swing_pct` field returned by get_price_history: use it as returned, never recompute it. Trail = 2 × that value; 3 × when the analyst upside is 15% or more, and say it was widened for its potential; never below 3.0% or above 15.0%; rounded to 0.1. If the swing is unknown, 3.0% and say so."#
            .to_string()
    };

    let protection = if paper_mode {
        LIVE_ONLY.to_string()
    } else {
        r#"One line per position, from the Ride Report's Protection and Proposed trail columns.
- Protected: e.g. "NVDA: trailing stop 8.0% working, GTC". Propose nothing.
- Not protected (and not dismissed, not unknown): propose a GTC trailing stop for the full quantity at the Ride Report's proposed trail, with the reason in plain words, e.g. "Trail 8.0%: NVDA moves 3.3% on a normal day, and analysts see 40% upside, so it gets room."
- Protection dismissed by you: say so; propose nothing while the quantity is unchanged.
- Protection unknown: say the open orders could not be read; propose and cancel nothing."#
            .to_string()
    };

    format!(
        r#"Generate the 10-Minute Briefing. This is an autonomous briefing: gather live data yourself with tools, then write the complete briefing.

Strategy — hitchhiking. Each held stock is a ride: stay on while it climbs; when it goes dormant, get off and move the money into the best ride available. Every ride is protected by a good-till-cancelled (GTC) trailing stop. The goal is account value moving uphill week over week. The Ride Report table below is the single source of truth for every number about held positions: quote it, never recompute or contradict it.{paper_note}

## Computed Ride Report (do not recompute)
{report_table}

## Account Budget
{cash_line}

Available cash is one shared budget counted once across ALL buys in this briefing; each rotation group's buy may additionally spend that group's own `fund` sell proceeds. Whole shares only; walk the buys in order. If not even one whole share fits, emit no BUY for it. SELL and stop orders do not use the budget.

## Open Orders (already working at the broker)
{open_orders_block}

## Instructions
Call tools for live data; do not rely on training data. Positions' own numbers (cost, P&L, 30-day move, daily swing, analyst target, upside, status, protection, trail) are already in the Ride Report: do not re-derive them.
1. get_stock_quote for the portfolio symbols: [{symbols}]
2. get_market_overview with data_type "sector_performance"
3. get_stock_news for the portfolio symbols
4. screen_stocks for 3-5 fresh ideas NOT held (e.g. market_cap_more_than=1000000000, volume_more_than=500000, beta_more_than=1, limit=10)
5. For EVERY New Stock Corner candidate, before ranking: get_stock_quote, get_analyst_targets, and get_price_history with range "3mo"
6. If time permits: get_financial_statements (key_metrics, limit 1) for the 2-3 largest positions

Then write the briefing with exactly these ## headers, in this order:

## Scoreboard
{scoreboard}

## Market Pulse
Market conditions: which sectors are up or down, overall sentiment. 2-3 sentences.

## Ride Report
One row per position, straight from the Ride Report: symbol, got on (date and price), distance traveled (P&L %), 30-day net move, daily swing, analyst target and upside, status, protection. Then one line of commentary per position.

## Rotation
{rotation}

## Protection
{protection}

## Alerts & Watchlist
Stocks that need attention: big movers, earnings approaching, insider activity, news catalysts. Flag risks.

## Risk Radar
Underperformers in the portfolio. For each: what is going wrong and the countermeasure (stop level, hedge, rotation idea). Be direct: if something should be sold, say so. Any stop you recommend is a GTC trailing stop at the Ride Report's proposed trail, never a DAY stop.

## Deep Dive
The 1-2 most important positions (biggest movers or highest risk): fundamentals snapshot, recent news impact, insider sentiment if available, and your stance (bullish / bearish / neutral) with reasoning.

## New Stock Corner
3-5 stocks NOT held, ranked by consensus upside, highest first. Mix sectors, market caps and styles; fresh ideas the user probably is not tracking. For each pick:
- **Ticker & Name** with current price
- **Analyst view**: consensus target, upside %, analyst count and as-of date
- **30-day move**
- **Why it's interesting**: catalyst, sector momentum, setup or fundamental story (1-2 sentences)
- **Risk level**: Low / Medium / High
- **Suggested entry**: price level or condition to watch for
- **Affordability**: whether at least one whole share fits available cash alone, or only with a rotation's proceeds
The top-ranked affordable pick that meets the selection rule is the Rotation target when a rotation exists.

Close with one short risk note, written once, then the one-line AI self-disclosure. The risk note states plainly:
- When you propose a rotation: in a cash account, if the new ride's trailing stop fires before the sale proceeds that funded it settle (T+1), that is a good-faith violation; most GTC trailing stops fire days later, but a same-day fire is possible on a volatile name.
- When you propose any trailing stop: a trailing stop can be triggered by an ordinary pullback and sell a ride on noise; the wider trail on high-upside names reduces this.
- When you propose any MARKET buy: rotation buys are market orders and fill at the live price, not a chosen level.

Keep the briefing concise but substantive: about 10 minutes to read and act on.

## IMPORTANT: Actionable Recommendations
After all sections, end with an ACTIONS_JSON block holding every trade you recommended above, and only those. Omit the block if there are none. The user reviews and approves every action before anything is placed.

Fields: symbol, exchange, side, quantity, orderType, product ("CNC"), price, rationale, plus:
- validity: "DAY" or "GTC" (default "DAY")
- group: e.g. "rotation-1", "protect-NVDA"
- role: "cancel" | "fund" | "buy" | "protect" | "standalone"
- cancelOrderId: required when role is "cancel"
- trailPercent: required for TRAILING_STOP

Example:
<!-- ACTIONS_JSON
{actions_example}
-->

Rules:
- Every TRAILING_STOP has "validity": "GTC" and trailPercent > 0. Never a DAY stop.
- A cancel carries only symbol, exchange, side "SELL", group, role, cancelOrderId and rationale (e.g. "Cancel the DAY stop at $192 so the GTC trailing stop can replace it."). Only use order ids that appear in the Open Orders block.
- A `cancel` action has no quantity.
- A rotation is one group, in this order: cancel legs (one per working sell order on a symbol being sold or re-protected), then the fund sells, then the buy, then the protect trailing stop for the bought symbol. Array order is placement order.
- If the dormant position is protected, emit a cancel leg for every working sell listed in its Protection cell, before its fund sell.
- A protection group for an existing position is cancel (if an old stop exists) then protect.
- Rotation buys are MARKET. A fund sell is the whole dormant position.
- Any other action uses role "standalone".

Position-aware constraints:
- NEVER SELL a symbol that is not in the Ride Report: you can only sell what the user holds, except the `protect` trailing stop for a symbol bought in the same group, which is sized to that group's bought quantity.
- NEVER sell more shares than the Ride Report's Qty, except the `protect` trailing stop for a symbol bought in the same group, which is sized to that group's bought quantity.
- A stop's quantity equals the position's full quantity (for a new ride, the bought quantity)."#,
        paper_note = paper_note,
        report_table = report_table,
        cash_line = cash_line,
        open_orders_block = open_orders_block,
        symbols = symbols,
        scoreboard = scoreboard,
        rotation = rotation,
        protection = protection,
        actions_example = ACTIONS_EXAMPLE,
    )
}

#[cfg(test)]
mod briefing_prompt_tests {
    use super::*;

    #[test]
    fn prompt_has_sections_in_order_and_gtc_example() {
        let p = build_briefing_prompt("TABLE", "cash", "orders", "PFE", false);
        let idx = |s: &str| p.find(s).unwrap();
        assert!(idx("## Scoreboard") < idx("## Market Pulse") && idx("## Market Pulse") < idx("## Ride Report") && idx("## Ride Report") < idx("## Rotation") && idx("## Rotation") < idx("## Protection") && idx("## Protection") < idx("## New Stock Corner"));
        assert!(p.contains(r#""orderType": "TRAILING_STOP""#) && p.contains(r#""validity": "GTC""#) && p.contains(r#""role": "cancel""#));
        assert!(!p.contains("DO NOT include a BUY action for it"));
    }

    #[test]
    fn paper_mode_prompt_says_live_only() {
        assert!(build_briefing_prompt("T", "c", "o", "PFE", true).contains("Live account only."));
    }

    #[test]
    fn prompt_states_proceeds_rule() {
        let p = build_briefing_prompt("T", "c", "o", "PFE", false);
        assert!(p.contains("that group's own `fund` sell proceeds"));
        assert!(!p.contains("watch-only"));
    }

    #[test]
    fn rotation_out_of_a_protected_position_cancels_its_working_sells() {
        let p = build_briefing_prompt("T", "c", "o", "PFE", false);
        let sentence = "If the dormant position is protected, emit a cancel leg for every working sell listed in its Protection cell, before its fund sell.";
        assert!(p.contains(sentence));
        // It sits with the rotation ordering rule.
        assert!(p.find("A rotation is one group, in this order").unwrap() < p.find(sentence).unwrap());
    }

    #[test]
    fn example_trail_matches_its_upside() {
        // The BAC protect trails 2x its 2.1% swing, which the trail rule allows only below 15% upside.
        assert!(ACTIONS_EXAMPLE.contains("Consensus target $58 (+12%)"));
        assert!(ACTIONS_EXAMPLE.contains(r#""trailPercent": 4.2"#));
        assert!(!ACTIONS_EXAMPLE.contains("+15%"));
    }

    #[test]
    fn prompt_allows_protect_on_bought_symbol() {
        assert!(build_briefing_prompt("T", "c", "o", "PFE", false).contains("bought in the same group"));
    }
}
