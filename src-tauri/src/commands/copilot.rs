//! AI Research Copilot IPC commands
//!
//! Tauri commands for the AI Research Copilot feature.
//! Routes frontend requests through the CopilotService for
//! AI-powered financial research with tool-calling support.

use crate::error::AppError;
use crate::providers::anthropic::Message;
use crate::services::copilot_service::{CopilotResponse, CopilotService};
use crate::services::position_service::PositionService;
use crate::state::AppState;
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
) -> Result<CopilotResponse, AppError> {
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
        CopilotService::process_message(&api_key, conversation_history, &message, &state).await?;

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

/// Generate a 10-Minute Briefing
///
/// Autonomously gathers the user's live broker positions, enriches them with
/// market data via tool calls, and produces a structured research briefing.
#[tauri::command]
pub async fn generate_briefing(
    state: State<'_, AppState>,
) -> Result<CopilotResponse, AppError> {
    let api_key = state
        .sqlite
        .get_provider_key("anthropic", &state.security)?
        .ok_or_else(|| {
            AppError::Provider(
                "Anthropic API key not configured. Please add your API key in Settings > Providers."
                    .to_string(),
            )
        })?;

    // Try to get live broker positions
    let portfolio_context = match PositionService::get_positions(&state, None).await {
        Ok(result) if !result.positions.is_empty() => {
            let mut lines = Vec::new();
            lines.push(format!("Portfolio ({} positions, {} mode):", result.positions.len(), result.mode));
            for p in &result.positions {
                lines.push(format!(
                    "  {} | qty: {} | avg: ${:.2} | ltp: ${:.2} | P&L: ${:.2}",
                    p.symbol, p.quantity, p.average_price, p.ltp, p.pnl
                ));
            }
            let symbols: Vec<String> = result.positions.iter()
                .map(|p| p.symbol.clone())
                .collect();
            (lines.join("\n"), symbols)
        }
        _ => {
            // Fall back to manual portfolio positions
            match state.sqlite.get_portfolio_positions() {
                Ok(positions) if !positions.is_empty() => {
                    let mut lines = Vec::new();
                    lines.push(format!("Portfolio ({} positions, manual tracking):", positions.len()));
                    for p in &positions {
                        lines.push(format!(
                            "  {} | qty: {} | avg: ${:.2} | type: {}",
                            p.symbol, p.quantity, p.avg_price, p.position_type
                        ));
                    }
                    let symbols: Vec<String> = positions.iter()
                        .map(|p| p.symbol.clone())
                        .collect();
                    (lines.join("\n"), symbols)
                }
                _ => (
                    "No portfolio positions found. Provide a general market briefing instead.".to_string(),
                    Vec::new(),
                ),
            }
        }
    };

    let symbol_list = if portfolio_context.1.is_empty() {
        "general market".to_string()
    } else {
        portfolio_context.1.join(", ")
    };

    // Build the briefing prompt
    let briefing_prompt = format!(
        r#"Generate a comprehensive 10-Minute Market Briefing. This is an autonomous briefing — gather ALL data yourself using tools, then present a complete analysis.

## Current Portfolio
{portfolio}

## Instructions

You MUST call tools to gather live data. Do NOT rely on training data. Follow these steps:

1. **First**: Call get_stock_quote for all portfolio symbols: [{symbols}]
2. **Then**: Call get_market_overview with data_type "sector_performance" to understand market conditions
3. **Then**: Call get_stock_news for the portfolio symbols to find relevant headlines
4. **Then**: Call screen_stocks to find 3-5 fresh stock ideas NOT already in the portfolio (use criteria like market_cap_more_than=1000000000, volume_more_than=500000, beta_more_than=1, limit=10)
5. **If time permits**: Call get_financial_statements for the 2-3 largest positions (key_metrics, limit 1)
6. **If time permits**: Call get_stock_quote for the top 3-5 New Stock Corner picks to get current prices

After gathering data, write your briefing with these sections. Use markdown headers (##) for each section:

## Market Pulse
Brief overview of market conditions — which sectors are up/down, overall sentiment. 2-3 sentences.

## Portfolio Snapshot
For EACH position: current price, today's change, P&L status. Flag anything moving more than ±3%. Present as a clean table or bullet list.

## Alerts & Watchlist
Stocks that need attention: big movers, earnings approaching, insider activity, news catalysts. Flag risks.

## Opportunities
Stocks showing strength or recovery patterns. Mention any that are near support levels or have positive catalysts.

## Risk Radar
Underperformers in the portfolio. For each: what's going wrong, potential countermeasures (stop loss levels, hedge suggestions, rotation ideas). Be direct — if something should be sold, say so.

## Deep Dive
Pick the 1-2 most important positions (biggest movers or highest risk) and provide a deeper analysis: fundamentals snapshot, recent news impact, insider sentiment if available, and your stance (bullish/bearish/neutral with reasoning).

## New Stock Corner
Present 3-5 stocks that are NOT in the current portfolio but worth watching. For each pick:
- **Ticker & Name** with current price
- **Why it's interesting** — catalyst, sector momentum, technical setup, or fundamental story (1-2 sentences)
- **Risk level** — Low / Medium / High
- **Suggested entry** — price level or condition to watch for

Focus on variety: mix sectors, market caps, and styles (growth, value, momentum). Avoid overlapping with existing portfolio holdings. These should be fresh ideas the user probably isn't already tracking.

Keep the full briefing concise but substantive — this should take about 10 minutes to read and act on.

## IMPORTANT: Actionable Recommendations
At the very end of your briefing, after all sections, include an ACTIONS_JSON block with any specific trade recommendations you made in your analysis. For example, if you recommend buying a New Stock Corner pick, selling a Risk Radar position, or setting a stop loss — include those as structured actions.

Format (place at the very end, after all sections):
<!-- ACTIONS_JSON
[
  {{ "symbol": "TICKER", "exchange": "NASDAQ", "side": "BUY", "quantity": 10, "orderType": "MARKET", "product": "CNC", "price": 0, "rationale": "brief reason" }},
  {{ "symbol": "TICKER2", "exchange": "NYSE", "side": "SELL", "quantity": 5, "orderType": "SL-M", "product": "CNC", "price": 0, "triggerPrice": 135.00, "rationale": "stop loss at support" }}
]
-->

Only include trades you explicitly recommend in your analysis. If you have no concrete buy/sell recommendations, omit this block entirely. The user will review and adjust before confirming.

CRITICAL — Position-aware constraints:
- NEVER recommend SELL for a symbol that is NOT in the portfolio above — you can only sell what the user actually holds
- NEVER recommend selling more shares than the user holds — match or stay under the portfolio quantity shown above
- For stop loss (SL, SL-M) and trailing stop orders on existing positions, the quantity MUST match the position's current quantity exactly
- For BUY recommendations (New Stock Corner picks or adding to existing positions), use reasonable quantities appropriate to the stock price (e.g., 5-10 shares for $100+ stocks, 10-25 for $20-$100 stocks)
- The portfolio data above is the source of truth for what the user owns and how many shares"#,
        portfolio = portfolio_context.0,
        symbols = symbol_list,
    );

    // Process through the copilot service with empty history (fresh briefing)
    let response = CopilotService::process_message(
        &api_key,
        Vec::new(),
        &briefing_prompt,
        &state,
    )
    .await?;

    Ok(response)
}
