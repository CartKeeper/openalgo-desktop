//! Copilot Service
//!
//! Orchestrates AI Research Copilot interactions. Manages the conversation loop
//! between the Anthropic Messages API and local data provider tool calls.
//! Called by Tauri commands.

use crate::error::{AppError, Result};
use crate::providers::anthropic::{
    build_tool_definitions, AnthropicClient, ContentBlock, Message, MessageContent,
    MessagesResponse, ResponseContentBlock,
};
use crate::providers::fmp::FmpClient;
use crate::providers::fred::FredClient;
use crate::providers::yahoo::YahooClient;
use crate::providers::types::ScreenerFilters;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Result of a tool call executed during copilot processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub tool_name: String,
    pub tool_use_id: String,
    pub success: bool,
    pub summary: String,
}

/// Full copilot response returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotResponse {
    pub response_text: String,
    pub tool_calls_made: Vec<ToolCallResult>,
    pub conversation: Vec<Message>,
}

/// Copilot service for orchestrating AI research interactions
pub struct CopilotService;

impl CopilotService {
    /// Build the system prompt for the financial research copilot
    pub fn build_system_prompt() -> String {
        r#"You are an AI Research Copilot for OpenAlgo Desktop, a professional trading and market analysis platform. You are a knowledgeable financial analyst assistant.

## RESPONSIBLE TRADING ADVICE — HIGHEST PRIORITY (overrides any conflicting guidance below)

Your job is to help the user make informed decisions — NOT to maximize their willingness to trade. You are not a financial advisor and you know nothing about the user's finances, goals, or risk tolerance beyond what they tell you this session.

CORE RULE: Never sound more certain than the data justifies. Present the distribution of outcomes, not a target. Treating variance as opportunity is a failure, not a feature.

LEAD WITH THE ANSWER. Do not preface responses with a risk lecture or with qualifying questions. A recommendation response is:
1. The pick(s) — each with inline source + timestamp, a momentum flag, and a realistic dollar downside.
2. One short, factual risk note (what the chosen risk level means statistically: higher variance, NOT higher expected return). Brief — not a gate, not a wall of warnings.
3. A plain one-line AI self-disclosure.

CONTENT RULES:
- Do not invent rules that sound systematic (e.g. "exit if volume drops below 10M") unless grounded and sourced. Do not dress guesses as analysis.
- Replace price-target language ("room to run to $X") with range + base rate ("traded $A–$B over 52 weeks; a same-day +X% move reverses roughly as often as it continues"). No single price target stated as an expectation.
- If an instrument is already up 10% or more intraday, you MUST state plainly, ADJACENT to that pick, that buying after the move is structurally different from buying before it.
- Express realistic (not worst-case) downside in the user's ACTUAL dollars (e.g. "a 25% drop is normal here = $500 of your $2,000").
- NEVER endorse increasing risk as a way to grow capital faster. If the user proposes it, correct the premise: variance is not expected return, and a 50% loss requires a 100% gain to recover.

REFERENCES:
- Attach a source and as-of timestamp (including data delay when known, e.g. "as of 15:42 ET, 15-min delayed") to every price, volume, and range you cite. Your quote tools return timestamps — use them.
- Any interpretive claim ("strong momentum," "room to run") must be backed by a stated base rate or omitted. Interpretation is not data.
- Every recommendation includes a plain note that this output is AI-generated, can be confidently wrong, and is not based on the user's full financial picture.

NO INTERROGATION:
- The user owns this account and has already made their own suitability decisions. NEVER ask affordability, time-horizon, or maximum-loss questions as a precondition to analyzing a symbol or producing a pick, and NEVER withhold a pick waiting for those answers. Answer the request directly. Only ask about the user's situation if THEY ask you to tailor advice to it.
- The app still shows a final dollar-downside acknowledgment before any order is actually placed (Gate B). That is the safety step — reinforce it rather than front-loading questions.

TONE: Calm, plain, specific about uncertainty. No hype words. You may flag when a setup looks risky and state the downside, but still give the user the pick they asked for — do not refuse to surface it or replace it with questions.

HONEST LIMITATION: Do not imply that any framing makes intraday-spike day-trading a reliable way to grow a small account. Make the risk and your own uncertainty visible — do not make a stacked bet look favorable.

## EVIDENCE BASE FOR GROWTH & "AGGRESSIVE" ADVICE (factual foundation — use whenever the user asks about growing the account, raising risk, or being aggressive)

This is sourced educational information, not financial advice. You are not licensed; tell the user to verify tax/account specifics with a fee-only fiduciary or CPA. All figures are historical; past returns do not guarantee future results. The user makes every final call — your job is to give them the facts and the trade-offs, not to decide for them.

GOVERNING PRINCIPLE — compensated vs uncompensated risk:
- Compensated (systematic) risk = risk you cannot diversify away (broad market exposure, documented factor exposures). The market pays a premium for it. This is the ONLY place "aggressive" reliably raises expected return.
- Uncompensated (idiosyncratic) risk = risk you could diversify away but don't (a few concentrated stocks, single bets). It adds variance with NO higher expected return — the extra risk is unpaid.
- Almost every popular way to "be aggressive" (concentration, day-trading, options, leverage held too long) loads UNCOMPENSATED risk. When a user asks to be aggressive, lead with this split before naming anything.

FACTUAL RETURN MENU (US, 1928–2024, nominal, dividends/coupons included; source: Damodaran/NYU Stern):
- US small-cap (bottom decile): ~12% long-run (highest; severe, long drawdowns)
- S&P 500 large-cap: ~9.9% (the benchmark almost nothing beats long-term)
- Baa corporate bonds: ~7% | 10-yr Treasuries: ~4.6% | 3-mo T-bills (cash): ~3.3%
Equities have paid ~2–3x bonds long-run, which is why a growth-max book leans heavily/entirely to equities. The small-cap premium is real historically but shows up only over long horizons and with deeper drawdowns.

LEVERS, RANKED BY STRENGTH OF EVIDENCE:
- TIER 1 — compensated, evidence-backed (actually raise expected return): (1) Maximize equity allocation (90–100%) — the single biggest, best-supported lever; cost is bigger drawdowns (US equities fell ~50% twice since 2000); the real question is whether the user can hold through a 50% drop without selling. (2) Factor tilts to small-cap/value (Fama-French) — historically add return; can underperform 10+ years at a stretch. (3) Full global + emerging-markets exposure. Implement all via low-cost index/factor funds, NOT stock-picking.
- TIER 2 — amplifiers (raise return AND loss; mechanics matter): Leverage. Margin carries interest + margin-call risk (a drawdown can force liquidation at the worst time, turning a paper loss permanent). Leveraged ETFs (2x/3x) reset daily and suffer volatility decay — they can lose value even if the index ends flat; issuers and the SEC warn they are NOT for holding beyond ~a day. They are short-term tools, not buy-and-hold growth vehicles.
- TIER 3 — mostly UNCOMPENSATED (high variance, no reliable edge; negative expected return for most): Concentration in a few stocks raises dispersion of outcomes, not expected value (you might 5x; you might go to zero). Active/day-trading/options buying: over 15 years 89.5% of large-cap active funds underperformed the S&P 500, and 0 of 22 US equity categories had a majority beat their benchmark (SPIVA YE2024); 80–95% of day traders lose money and fewer than ~5% stay profitable past five years; retail options buyers lose on average (UF/Warrington). State these facts plainly when a user proposes these; they are "entertainment with money you'll lose," never the critical path.

TWO SILENT DRAGS (guaranteed costs): (1) Fees — every 1% expense ratio is ~1% off compound return every year; broad index funds are the cheapest way to hold the compensated premium. (2) Taxes — frequent trading realizes short-term gains at ordinary income rates; holds over 1 year are taxed lower; tax-advantaged accounts (IRA/Roth/employer plans) can shelter growth entirely. For most people the tax savings of buy-and-hold in a sheltered account beat any edge they would chase trading.

THE REAL LEVERS are ALLOCATION and SAVINGS/CONTRIBUTION RATE — not trade selection. The master dial is the maximum drawdown the user can hold through without selling; if 50% would make them sell, 100% equities is too aggressive FOR THEM regardless of expected return (selling at the bottom is what actually destroys returns).

HONEST BOTTOM LINE (state when asked for the most aggressive evidence-backed portfolio): roughly 100% equities, tilted small-cap/value, globally diversified, in low-cost index funds, inside tax-advantaged accounts, held through every drawdown — optionally amplified with carefully-capped leverage if they accept the added ruin risk. Expected long-run return is high-single to low-double digits, with stomach-churning drops along the way. Everything beyond that (concentration, day-trading, options, leveraged ETFs held long) adds variance without expected return and, per the data, makes most people poorer. The aggression that compounds is in the allocation and the savings rate, not the trade.

SPECULATION SLEEVE: if the user still wants concentration/options/active trading, ring-fence a fixed small slice (commonly <=5–10%) treated as fully expendable, and benchmark it honestly against just buying the index (the app's backtester reports a buy-and-hold benchmark for exactly this). Treat the rest as the real portfolio.

SOURCES (cite when relevant): Damodaran/NYU Stern historical returns 1928–2024 (pages.stern.nyu.edu/~adamodar); S&P SPIVA US Year-End 2024 (spglobal.com/spdji); SEC/FINRA leveraged & inverse ETF guidance (sec.gov/investor/pubs/leveragedetfs-alert.htm); University of Florida / Warrington on retail options losses (warrington.ufl.edu).

## Your Role
- Help users research stocks, analyze companies, understand market trends, and interpret economic data.
- ALWAYS use your tools to fetch real-time data FIRST before responding. Do not give a text-only answer when a tool call would provide better data. Act first, analyze second.
- Provide clear, concise analysis based on the data you retrieve.
- When presenting financial data, format numbers clearly (e.g., use "$1.2B" for billions, percentages with 2 decimal places).

## Available Tools
You have access to these data retrieval tools:
1. **get_stock_quote** - Real-time quotes (price, change, volume, market cap, P/E) from Yahoo Finance
2. **get_company_profile** - Company details (description, sector, employees, CEO) from FMP
3. **get_stock_news** - Recent news articles for stocks or general market from FMP
4. **screen_stocks** - Filter stocks by market cap, sector, price, beta, volume, etc. from FMP
5. **get_financial_statements** - Income statements, balance sheets, cash flow, key metrics from FMP
6. **get_economic_data** - FRED economic indicators (GDP, CPI, unemployment, interest rates, etc.)
7. **get_congressional_trades** - Recent stock trades (buys/sells) by U.S. Senators and House Representatives from FMP. Use this when users ask about what Congress members are buying/selling, political stock trades, congressional investing, or anything related to Congress and stocks/markets.
8. **get_earnings_transcript** - Full text of earnings call transcripts. Use when users ask what a company said on their earnings call, management guidance, or CEO commentary.
9. **get_insider_trading** - SEC Form 4 insider trading data. Shows buys/sells by executives, directors, and major shareholders. Use when users ask about insider buys, insider confidence, or who's buying/selling inside a company.
10. **get_market_calendar** - Upcoming earnings dates, IPOs, dividend ex-dates, stock splits. Use when users ask when a company reports earnings, upcoming IPOs, or dividend dates.
11. **get_etf_data** - ETF fund info (expense ratio, assets, company) and holdings (what stocks the ETF holds). Use when users ask about an ETF's composition or details.
12. **get_market_overview** - Sector performance, today's gainers, losers, and most active stocks. Use when users ask about market conditions, what's up/down today, or sector trends.
13. **get_index_constituents** - Companies in the S&P 500, Nasdaq 100, or Dow Jones 30. Use when users ask about what's in an index or want a list of companies in a specific index.
14. **get_broker_account** - Get the currently connected broker account info (broker name, account ID, user ID, live/paper mode). Use ONLY when the user asks about their broker account, connection status, or account details.
15. **list_clients** - List all clients in the client management system with their names, IDs, and account info. Use ONLY when the user asks about clients or needs to find a specific client.
16. **get_client_portfolio** - Look up a client's portfolio from the local client management system. Returns positions (symbols, quantities, avg price, P&L) and recent trades. Use when the user asks about a specific client's portfolio, holdings, or trades. Search by name (partial match works).
17. **get_client_scenario** - Retrieve a sandbox what-if scenario with positions and live market prices. Use when the user asks you to analyze a scenario portfolio.
18. **apply_scenario_trades** - Apply a batch of simulated trades to a sandbox scenario. Use when you have specific trade recommendations and the user wants them applied. Each trade adjusts position quantity and recalculates weighted-average cost basis.

## CRITICAL RULES
- NEVER say "I don't have access to" any tool listed above. You DO have these tools. USE THEM.
- NEVER give a text-only response when you could call a tool instead. If the user asks about a topic and ANY of your tools could provide relevant data, call the tool FIRST.
- When a user asks about Congress, politicians, or government and markets — immediately call get_congressional_trades. Do NOT just describe what you could do — DO IT.
- When a user asks about any stock, sector, or market — immediately call relevant tools (quotes, screener, news, etc.). Do NOT offer to look things up — just look them up.

## Account Type Restrictions
- When analyzing a client scenario, check the `account_type` and `short_selling_restricted` fields in the response from get_client_scenario.
- If `short_selling_restricted` is true (e.g., 401(k), IRA, 529, custodial accounts), NEVER recommend short selling or SELL trades that would create a short position. Only recommend selling to reduce or close existing long positions.
- Always mention the account type restriction in your analysis if it limits available strategies.

## Response Strategy
- Plan your tool calls efficiently. Batch related lookups (e.g., screen first, then profile the top 3-5 results). Avoid making more than 5 tool calls per question.
- After gathering data, write your final analysis immediately. Do NOT repeat raw JSON from tool results — summarize key figures in a readable format.
- Keep your final response focused: lead with the answer, then supporting evidence. Aim for 300-600 words of analysis, not thousands.

## Guidelines
- Always fetch current data using tools rather than relying on training data for prices, financials, or economic figures.
- If a tool call fails (e.g., API key not configured), explain the issue clearly and suggest what the user can do.
- When comparing stocks, fetch data for all of them to give an informed comparison.
- For financial analysis, consider multiple data points: valuation ratios, growth trends, profitability, and debt levels.
- Be direct and analytical. Avoid unnecessary hedging, but do note significant risks or caveats.
- When presenting screener results, summarize the key findings rather than just listing raw data.
- For economic data, provide context about what the indicator means and its recent trend.

## Actionable Recommendations
When your analysis leads to specific trade recommendations (buy, sell, set stop loss, take profit), you MUST include a structured action block at the END of your response. This is in addition to your prose analysis.

Format (place at the very end of your response):
<!-- ACTIONS_JSON
[
  { "symbol": "AAPL", "exchange": "NASDAQ", "side": "BUY", "quantity": 10, "orderType": "LIMIT", "product": "CNC", "price": 185.50, "rationale": "Near support at $184, strong fundamentals" },
  { "symbol": "TSLA", "exchange": "NASDAQ", "side": "SELL", "quantity": 5, "orderType": "TRAILING_STOP", "product": "CNC", "price": 0, "trailPercent": 5, "rationale": "Extended above resistance, lock in gains with trailing stop" }
]
-->

Rules for ACTIONS_JSON:
- Only include when you have concrete, actionable recommendations (not vague suggestions like "consider watching")
- "side" must be "BUY" or "SELL"
- "orderType" must be one of: "MARKET", "LIMIT", "SL", "SL-M", "TRAILING_STOP"
- "product" should be "CNC" for delivery/investment positions
- "price" should be 0 for MARKET orders, otherwise the target limit price
- "triggerPrice" is optional, for SL/SL-M stop loss orders
- "trailPrice" or "trailPercent" is for TRAILING_STOP orders (dollar amount or percentage)
- "exchange" should be the stock's primary exchange (e.g., "NASDAQ", "NYSE")
- "rationale" is a brief 1-sentence reason for the trade
- Include ALL recommended trades in a single ACTIONS_JSON block
- This block is invisible to the user — it is parsed programmatically by the app
- ALWAYS write your full prose analysis FIRST, then add the ACTIONS_JSON block at the very end
- When the user asks for trade ideas, EMIT the pick(s) with an ACTIONS_JSON block. Do NOT respond with qualifying questions instead of picks. Each pick still carries a momentum flag, a realistic dollar downside, and a one-line AI self-disclosure — but lead with the pick, not a risk lecture or a questionnaire.
- If you have NO concrete trade recommendations, do NOT include an ACTIONS_JSON block

CRITICAL — Position-aware constraints:
- NEVER recommend SELL for a symbol the user does not currently hold in their portfolio
- NEVER recommend selling more shares than the user actually holds — match or stay under the portfolio quantity
- For stop loss (SL, SL-M) and trailing stop orders on existing positions, the quantity MUST equal the position's current quantity, not an arbitrary number
- For BUY recommendations (new positions or adding to existing), use reasonable quantities appropriate to the stock price (e.g., 5-10 shares for $100+ stocks, 10-25 for $20-$100 stocks)
- If the user's portfolio data is provided in the conversation, treat it as the source of truth for what they own"#.to_string()
    }

    /// Execute a tool call by routing to the appropriate provider
    pub async fn execute_tool_call(
        tool_name: &str,
        tool_input: &serde_json::Value,
        state: &AppState,
    ) -> serde_json::Value {
        let result = match tool_name {
            "get_stock_quote" => Self::tool_get_stock_quote(tool_input, state).await,
            "get_company_profile" => Self::tool_get_company_profile(tool_input, state).await,
            "get_stock_news" => Self::tool_get_stock_news(tool_input, state).await,
            "screen_stocks" => Self::tool_screen_stocks(tool_input, state).await,
            "get_financial_statements" => Self::tool_get_financial_statements(tool_input, state).await,
            "get_economic_data" => Self::tool_get_economic_data(tool_input, state).await,
            "get_congressional_trades" => Self::tool_get_congressional_trades(tool_input, state).await,
            "get_earnings_transcript" => Self::tool_get_earnings_transcript(tool_input, state).await,
            "get_insider_trading" => Self::tool_get_insider_trading(tool_input, state).await,
            "get_market_calendar" => Self::tool_get_market_calendar(tool_input, state).await,
            "get_etf_data" => Self::tool_get_etf_data(tool_input, state).await,
            "get_market_overview" => Self::tool_get_market_overview(tool_input, state).await,
            "get_index_constituents" => Self::tool_get_index_constituents(tool_input, state).await,
            "get_broker_account" => Self::tool_get_broker_account(tool_input, state).await,
            "list_clients" => Self::tool_list_clients(tool_input, state).await,
            "get_client_portfolio" => Self::tool_get_client_portfolio(tool_input, state).await,
            "get_client_scenario" => Self::tool_get_client_scenario(tool_input, state).await,
            "apply_scenario_trades" => Self::tool_apply_scenario_trades(tool_input, state).await,
            _ => Err(AppError::Provider(format!("Unknown tool: {}", tool_name))),
        };

        match result {
            Ok(data) => data,
            Err(e) => serde_json::json!({
                "error": true,
                "message": e.to_string()
            }),
        }
    }

    /// Process a user message through the full copilot loop
    pub async fn process_message(
        api_key: &str,
        conversation_history: Vec<Message>,
        user_message: &str,
        state: &AppState,
    ) -> Result<CopilotResponse> {
        let client = AnthropicClient::new((*state.http_client).clone());
        let tools = build_tool_definitions();
        let system_prompt = Self::build_system_prompt();

        // Build conversation with the new user message
        let mut conversation = conversation_history;
        conversation.push(Message {
            role: "user".to_string(),
            content: MessageContent::Text(user_message.to_string()),
        });

        let mut tool_calls_made: Vec<ToolCallResult> = Vec::new();

        // Loop to handle tool use - Claude may request multiple rounds of tools
        let mut iterations = 0;
        let max_iterations = 10; // Safety limit — enough for briefing tool calls + final response

        loop {
            iterations += 1;
            if iterations > max_iterations {
                warn!("Copilot hit max iterations ({}) - sending final request without tools", max_iterations);
                // Send one final request WITHOUT tools so Claude writes its analysis
                let final_response: MessagesResponse = client
                    .send_message(api_key, conversation.clone(), Some(&system_prompt), None)
                    .await
                    .map_err(|e| AppError::Provider(format!("Anthropic API error (final): {}", e)))?;
                let final_blocks: Vec<ContentBlock> = final_response
                    .content
                    .iter()
                    .map(|block| match block {
                        ResponseContentBlock::Text { text } => ContentBlock::Text { text: text.clone() },
                        ResponseContentBlock::ToolUse { id, name, input } => ContentBlock::ToolUse { id: id.clone(), name: name.clone(), input: input.clone() },
                    })
                    .collect();
                conversation.push(Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Blocks(final_blocks),
                });
                break;
            }

            // Send to Anthropic
            let response: MessagesResponse = client
                .send_message(api_key, conversation.clone(), Some(&system_prompt), Some(tools.clone()))
                .await
                .map_err(|e| AppError::Provider(format!("Anthropic API error: {}", e)))?;

            let stop_reason = response.stop_reason.clone().unwrap_or_default();

            // Build the assistant message content blocks from the response
            let assistant_blocks: Vec<ContentBlock> = response
                .content
                .iter()
                .map(|block| match block {
                    ResponseContentBlock::Text { text } => ContentBlock::Text {
                        text: text.clone(),
                    },
                    ResponseContentBlock::ToolUse { id, name, input } => ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    },
                })
                .collect();

            // Add the assistant's response to the conversation
            conversation.push(Message {
                role: "assistant".to_string(),
                content: MessageContent::Blocks(assistant_blocks.clone()),
            });

            // If the model wants to use tools, execute them and continue the loop
            if stop_reason == "tool_use" {
                let mut tool_result_blocks: Vec<ContentBlock> = Vec::new();

                for block in &assistant_blocks {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        info!("Copilot executing tool: {} with input: {}", name, input);

                        let result = Self::execute_tool_call(name, input, state).await;
                        let is_error = result.get("error").and_then(|v| v.as_bool()).unwrap_or(false);

                        // Build a summary for the frontend
                        let summary = if is_error {
                            result
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Tool call failed")
                                .to_string()
                        } else {
                            Self::summarize_tool_result(name, &result)
                        };

                        tool_calls_made.push(ToolCallResult {
                            tool_name: name.clone(),
                            tool_use_id: id.clone(),
                            success: !is_error,
                            summary,
                        });

                        // Serialize the result, truncating large payloads to preserve token budget.
                        // For arrays: keep first N items. For strings: byte-level truncation.
                        let result_str = Self::truncate_tool_result(&result);

                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: result_str,
                            is_error: if is_error { Some(true) } else { None },
                        });
                    }
                }

                // Add tool results as a user message
                if !tool_result_blocks.is_empty() {
                    conversation.push(Message {
                        role: "user".to_string(),
                        content: MessageContent::Blocks(tool_result_blocks),
                    });
                }

                // Continue the loop so Claude can process tool results
                continue;
            }

            // If stop_reason is "end_turn" or anything else, we're done
            break;
        }

        // Extract the final text response from the last assistant message
        let response_text = Self::extract_final_text(&conversation);

        Ok(CopilotResponse {
            response_text,
            tool_calls_made,
            conversation,
        })
    }

    // ========== Tool Implementations ==========

    async fn tool_get_stock_quote(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbols: Vec<String> = input["symbols"]
            .as_array()
            .ok_or_else(|| AppError::Validation("symbols must be an array".to_string()))?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if symbols.is_empty() {
            return Err(AppError::Validation("No symbols provided".to_string()));
        }

        let client = YahooClient::new((*state.http_client).clone());
        let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
        let quotes = client.get_quotes(&symbol_refs).await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        Ok(serde_json::to_value(&quotes)
            .map_err(|e| AppError::Serialization(e))?)
    }

    async fn tool_get_company_profile(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbol = input["symbol"]
            .as_str()
            .ok_or_else(|| AppError::Validation("symbol is required".to_string()))?;

        let fmp = Self::get_fmp_client(state)?;
        let profile = fmp.get_company_profile(symbol).await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        Ok(serde_json::to_value(&profile)
            .map_err(|e| AppError::Serialization(e))?)
    }

    async fn tool_get_stock_news(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbols = input["symbols"].as_str();
        let limit = input["limit"].as_i64().unwrap_or(10) as i32;
        let limit = limit.min(50);

        let fmp = Self::get_fmp_client(state)?;
        let news = fmp.get_stock_news(symbols, limit).await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        Ok(serde_json::to_value(&news)
            .map_err(|e| AppError::Serialization(e))?)
    }

    async fn tool_screen_stocks(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let filters = ScreenerFilters {
            market_cap_min: input["market_cap_min"].as_f64(),
            market_cap_max: input["market_cap_max"].as_f64(),
            price_min: input["price_min"].as_f64(),
            price_max: input["price_max"].as_f64(),
            pe_min: None,
            pe_max: None,
            beta_min: input["beta_min"].as_f64(),
            beta_max: input["beta_max"].as_f64(),
            volume_min: input["volume_min"].as_i64(),
            dividend_yield_min: None,
            sector: input["sector"].as_str().map(String::from),
            industry: input["industry"].as_str().map(String::from),
            country: input["country"].as_str().map(String::from),
            exchange: input["exchange"].as_str().map(String::from),
            is_etf: input["is_etf"].as_bool(),
            limit: input["limit"].as_i64().map(|v| v as i32).or(Some(20)),
        };

        let fmp = Self::get_fmp_client(state)?;
        let results = fmp.screen_stocks(&filters).await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        Ok(serde_json::to_value(&results)
            .map_err(|e| AppError::Serialization(e))?)
    }

    async fn tool_get_financial_statements(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbol = input["symbol"]
            .as_str()
            .ok_or_else(|| AppError::Validation("symbol is required".to_string()))?;
        let statement_type = input["statement_type"]
            .as_str()
            .ok_or_else(|| AppError::Validation("statement_type is required".to_string()))?;
        let period = input["period"].as_str().unwrap_or("annual");
        let limit = input["limit"].as_i64().unwrap_or(4) as i32;

        let fmp = Self::get_fmp_client(state)?;

        match statement_type {
            "income" => {
                let data = fmp.get_income_statement(symbol, period, limit).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data)
                    .map_err(|e| AppError::Serialization(e))?)
            }
            "balance_sheet" => {
                let data = fmp.get_balance_sheet(symbol, period, limit).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data)
                    .map_err(|e| AppError::Serialization(e))?)
            }
            "cash_flow" => {
                let data = fmp.get_cash_flow(symbol, period, limit).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data)
                    .map_err(|e| AppError::Serialization(e))?)
            }
            "key_metrics" => {
                let data = fmp.get_key_metrics(symbol, period, limit).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data)
                    .map_err(|e| AppError::Serialization(e))?)
            }
            _ => Err(AppError::Validation(format!(
                "Invalid statement_type: {}. Must be one of: income, balance_sheet, cash_flow, key_metrics",
                statement_type
            ))),
        }
    }

    async fn tool_get_economic_data(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let series_id = input["series_id"]
            .as_str()
            .ok_or_else(|| AppError::Validation("series_id is required".to_string()))?;
        let observation_start = input["observation_start"].as_str();
        let observation_end = input["observation_end"].as_str();

        let fred = Self::get_fred_client(state)?;
        let data = fred
            .get_series(series_id, observation_start, observation_end)
            .await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        Ok(serde_json::to_value(&data)
            .map_err(|e| AppError::Serialization(e))?)
    }

    async fn tool_get_congressional_trades(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let chamber = input["chamber"]
            .as_str()
            .ok_or_else(|| AppError::Validation("chamber is required ('senate' or 'house')".to_string()))?;
        let name = input["name"].as_str();
        let limit = (input["limit"].as_i64().unwrap_or(25) as i32).min(25);

        let fmp = Self::get_fmp_client(state)?;

        let trades = match (chamber, name) {
            ("senate", Some(n)) => fmp.get_senate_trades_by_name(n).await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            ("senate", None) => fmp.get_senate_trades(0, limit).await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            ("house", Some(n)) => fmp.get_house_trades_by_name(n).await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            ("house", None) => fmp.get_house_trades(0, limit).await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            _ => return Err(AppError::Validation(format!(
                "Invalid chamber: {}. Must be 'senate' or 'house'", chamber
            ))),
        };

        Ok(serde_json::to_value(&trades)
            .map_err(|e| AppError::Serialization(e))?)
    }

    // ---------- New FMP tool implementations ----------

    async fn tool_get_earnings_transcript(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbol = input["symbol"].as_str()
            .ok_or_else(|| AppError::Validation("symbol is required".to_string()))?;
        let year = input["year"].as_i64()
            .ok_or_else(|| AppError::Validation("year is required".to_string()))? as i32;
        let quarter = input["quarter"].as_i64().map(|v| v as i32);

        let fmp = Self::get_fmp_client(state)?;
        let transcripts = fmp.get_earnings_call_transcript(symbol, year, quarter).await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        // Truncate content to avoid overwhelming the context
        let truncated: Vec<serde_json::Value> = transcripts.iter().map(|t| {
            let content = t.content.as_deref().unwrap_or("");
            let truncated_content = if content.len() > 3000 {
                format!("{}... [truncated, {} total chars]", &content[..3000], content.len())
            } else {
                content.to_string()
            };
            serde_json::json!({
                "symbol": t.symbol,
                "quarter": t.quarter,
                "year": t.year,
                "date": t.date,
                "content": truncated_content
            })
        }).collect();

        Ok(serde_json::Value::Array(truncated))
    }

    async fn tool_get_insider_trading(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbol = input["symbol"].as_str()
            .ok_or_else(|| AppError::Validation("symbol is required".to_string()))?;
        let limit = input["limit"].as_i64().unwrap_or(30) as i32;

        let fmp = Self::get_fmp_client(state)?;
        let trades = fmp.get_insider_trading(symbol, limit).await
            .map_err(|e| AppError::Provider(e.to_string()))?;

        Ok(serde_json::to_value(&trades).map_err(|e| AppError::Serialization(e))?)
    }

    async fn tool_get_market_calendar(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let calendar_type = input["calendar_type"].as_str()
            .ok_or_else(|| AppError::Validation("calendar_type is required".to_string()))?;
        let from_date = input["from_date"].as_str()
            .ok_or_else(|| AppError::Validation("from_date is required".to_string()))?;
        let to_date = input["to_date"].as_str()
            .ok_or_else(|| AppError::Validation("to_date is required".to_string()))?;

        let fmp = Self::get_fmp_client(state)?;

        match calendar_type {
            "earnings" => {
                let data = fmp.get_earnings_calendar(from_date, to_date).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "ipo" => {
                let data = fmp.get_ipo_calendar(from_date, to_date).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "dividends" => {
                let data = fmp.get_dividend_calendar(from_date, to_date).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "splits" => {
                let data = fmp.get_stock_split_calendar(from_date, to_date).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            _ => Err(AppError::Validation(format!(
                "Invalid calendar_type: {}. Must be 'earnings', 'ipo', 'dividends', or 'splits'", calendar_type
            ))),
        }
    }

    async fn tool_get_etf_data(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let symbol = input["symbol"].as_str()
            .ok_or_else(|| AppError::Validation("symbol is required".to_string()))?;
        let data_type = input["data_type"].as_str()
            .ok_or_else(|| AppError::Validation("data_type is required".to_string()))?;

        let fmp = Self::get_fmp_client(state)?;

        match data_type {
            "info" => {
                let data = fmp.get_etf_info(symbol).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "holdings" => {
                let data = fmp.get_etf_holdings(symbol).await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            _ => Err(AppError::Validation(format!(
                "Invalid data_type: {}. Must be 'info' or 'holdings'", data_type
            ))),
        }
    }

    async fn tool_get_market_overview(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let data_type = input["data_type"].as_str()
            .ok_or_else(|| AppError::Validation("data_type is required".to_string()))?;

        let fmp = Self::get_fmp_client(state)?;

        match data_type {
            "sector_performance" => {
                let data = fmp.get_sector_performance().await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "gainers" => {
                let data = fmp.get_market_gainers().await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "losers" => {
                let data = fmp.get_market_losers().await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            "most_active" => {
                let data = fmp.get_market_most_active().await
                    .map_err(|e| AppError::Provider(e.to_string()))?;
                Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
            }
            _ => Err(AppError::Validation(format!(
                "Invalid data_type: {}. Must be 'sector_performance', 'gainers', 'losers', or 'most_active'", data_type
            ))),
        }
    }

    async fn tool_get_index_constituents(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let index = input["index"].as_str()
            .ok_or_else(|| AppError::Validation("index is required".to_string()))?;

        let fmp = Self::get_fmp_client(state)?;

        let data = match index {
            "sp500" => fmp.get_sp500_constituents().await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            "nasdaq" => fmp.get_nasdaq_constituents().await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            "dowjones" => fmp.get_dowjones_constituents().await
                .map_err(|e| AppError::Provider(e.to_string()))?,
            _ => return Err(AppError::Validation(format!(
                "Invalid index: {}. Must be 'sp500', 'nasdaq', or 'dowjones'", index
            ))),
        };

        Ok(serde_json::to_value(&data).map_err(|e| AppError::Serialization(e))?)
    }

    // ========== Broker Account & Clients ==========

    async fn tool_get_broker_account(
        _input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        match state.get_broker_session() {
            Some(session) => {
                // Determine live vs paper for Alpaca (paper keys start with "PK")
                let mode = if session.broker_id == "alpaca" {
                    // The auth_token is "api_key:api_secret"
                    if session.auth_token.starts_with("PK") {
                        "paper"
                    } else {
                        "live"
                    }
                } else {
                    "live"
                };

                // Get the broker display name from the registry
                let broker_name = state.brokers.get(&session.broker_id)
                    .map(|b| b.name().to_string())
                    .unwrap_or_else(|| session.broker_id.clone());

                Ok(serde_json::json!({
                    "connected": true,
                    "broker_id": session.broker_id,
                    "broker_name": broker_name,
                    "user_id": session.user_id,
                    "user_name": session.user_name,
                    "mode": mode,
                    "authenticated_at": session.authenticated_at.to_rfc3339(),
                }))
            }
            None => {
                Ok(serde_json::json!({
                    "connected": false,
                    "message": "No broker is currently connected. The user can connect a broker from the Broker login page."
                }))
            }
        }
    }

    async fn tool_list_clients(
        _input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let clients = state.sqlite.get_clients()?;

        if clients.is_empty() {
            return Ok(serde_json::json!({
                "clients": [],
                "count": 0,
                "message": "No clients in the system yet."
            }));
        }

        let client_list: Vec<serde_json::Value> = clients.iter().map(|c| {
            serde_json::json!({
                "id": c.id,
                "name": c.name,
                "broker": c.broker,
                "account_id": c.account_id,
                "account_type": c.account_type,
                "email": c.email,
            })
        }).collect();

        Ok(serde_json::json!({
            "clients": client_list,
            "count": clients.len(),
        }))
    }

    // ========== Client Portfolio ==========

    async fn tool_get_client_portfolio(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let search_name = input["client_name"].as_str()
            .ok_or_else(|| AppError::Validation("client_name is required".to_string()))?
            .to_lowercase();

        // Find matching client
        let clients = state.sqlite.get_clients()?;
        let client = clients.iter().find(|c| c.name.to_lowercase().contains(&search_name));

        let client = match client {
            Some(c) => c,
            None => {
                let names: Vec<&str> = clients.iter().map(|c| c.name.as_str()).collect();
                return Ok(serde_json::json!({
                    "error": false,
                    "found": false,
                    "message": format!("No client found matching '{}'. Available clients: {}", search_name, if names.is_empty() { "none".to_string() } else { names.join(", ") }),
                }));
            }
        };

        let client_id = client.id.unwrap();
        let positions = state.sqlite.get_client_positions(client_id)?;
        let trades = state.sqlite.get_client_trades(client_id)?;

        // Build summary
        let position_data: Vec<serde_json::Value> = positions.iter().map(|p| {
            serde_json::json!({
                "symbol": p.symbol,
                "exchange": p.exchange,
                "net_quantity": p.net_quantity,
                "avg_price": p.avg_price,
                "total_fees": p.total_fees,
                "trade_count": p.trade_count,
                "realized_pnl": p.realized_pnl,
            })
        }).collect();

        // Include last 20 trades for context
        let recent_trades: Vec<serde_json::Value> = trades.iter().take(20).map(|t| {
            serde_json::json!({
                "symbol": t.symbol,
                "trade_date": t.trade_date,
                "trade_type": t.trade_type,
                "quantity": t.quantity,
                "price": t.price,
                "fees": t.fees,
                "notes": t.notes,
            })
        }).collect();

        Ok(serde_json::json!({
            "found": true,
            "client": {
                "name": client.name,
                "broker": client.broker,
                "account_id": client.account_id,
                "email": client.email,
                "notes": client.notes,
            },
            "positions": position_data,
            "position_count": positions.len(),
            "recent_trades": recent_trades,
            "total_trades": trades.len(),
        }))
    }

    async fn tool_get_client_scenario(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let scenario_id = input["scenario_id"]
            .as_i64()
            .ok_or_else(|| AppError::Validation("scenario_id is required".to_string()))?;

        let scenario = state.sqlite.get_client_scenario_by_id(scenario_id)?;
        let positions = state.sqlite.get_scenario_positions(scenario_id)?;

        // Fetch live pricing from Yahoo
        let symbols: Vec<String> = positions
            .iter()
            .map(|p| p.symbol.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut price_map: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();

        if !symbols.is_empty() {
            let yahoo = YahooClient::new((*state.http_client).clone());
            let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
            if let Ok(quotes) = yahoo.get_quotes(&symbol_refs).await {
                for q in quotes {
                    price_map.insert(
                        q.symbol.to_uppercase(),
                        serde_json::json!({
                            "price": q.price,
                            "change": q.change,
                            "change_percent": q.change_percent,
                            "name": q.name,
                        }),
                    );
                }
            }
        }

        // Build enriched position data
        let mut total_value = 0.0f64;
        let mut total_cost = 0.0f64;
        let position_data: Vec<serde_json::Value> = positions
            .iter()
            .map(|p| {
                let cost = p.quantity * p.avg_price;
                total_cost += cost;
                let pricing = price_map.get(&p.symbol.to_uppercase());
                let current_price = pricing
                    .and_then(|pv| pv["price"].as_f64());
                let market_value = current_price.map(|cp| p.quantity * cp);
                if let Some(mv) = market_value {
                    total_value += mv;
                }
                let pnl = market_value.map(|mv| mv - cost);
                let pnl_pct = if cost > 0.0 { pnl.map(|pl| pl / cost * 100.0) } else { None };

                serde_json::json!({
                    "symbol": p.symbol,
                    "exchange": p.exchange,
                    "quantity": p.quantity,
                    "avg_price": p.avg_price,
                    "side": p.side,
                    "current_price": current_price,
                    "market_value": market_value,
                    "cost_basis": cost,
                    "unrealized_pnl": pnl,
                    "pnl_pct": pnl_pct,
                    "day_change_pct": pricing.and_then(|pv| pv["change_percent"].as_f64()),
                    "company_name": pricing.and_then(|pv| pv["name"].as_str()),
                    "notes": p.notes,
                })
            })
            .collect();

        // Get client name for context
        let client = state.sqlite.get_client_by_id(scenario.client_id).ok();

        // Prefer scenario-level account_type (set on per-account baselines), fall back to client-level
        let account_type = scenario.account_type.as_deref()
            .or_else(|| client.as_ref().and_then(|c| c.account_type.as_deref()));
        let short_selling_restricted = matches!(
            account_type,
            Some("401k") | Some("roth_401k") | Some("traditional_ira") | Some("roth_ira")
                | Some("sep_ira") | Some("simple_ira") | Some("529") | Some("custodial")
        );

        Ok(serde_json::json!({
            "scenario": {
                "id": scenario.id,
                "name": scenario.name,
                "description": scenario.description,
                "is_baseline": scenario.is_baseline,
                "client_name": client.as_ref().map(|c| c.name.as_str()),
                "account_type": account_type,
                "short_selling_restricted": short_selling_restricted,
            },
            "positions": position_data,
            "position_count": positions.len(),
            "summary": {
                "total_market_value": total_value,
                "total_cost_basis": total_cost,
                "total_unrealized_pnl": total_value - total_cost,
                "return_pct": if total_cost > 0.0 { (total_value - total_cost) / total_cost * 100.0 } else { 0.0 },
            }
        }))
    }

    async fn tool_apply_scenario_trades(
        input: &serde_json::Value,
        state: &AppState,
    ) -> Result<serde_json::Value> {
        let scenario_id = input["scenario_id"]
            .as_i64()
            .ok_or_else(|| AppError::Validation("scenario_id is required".to_string()))?;

        let trades = input["trades"]
            .as_array()
            .ok_or_else(|| AppError::Validation("trades must be an array".to_string()))?;

        if trades.is_empty() {
            return Err(AppError::Validation("No trades provided".to_string()));
        }

        let mut applied: Vec<serde_json::Value> = Vec::new();

        for trade in trades {
            let symbol = trade["symbol"]
                .as_str()
                .ok_or_else(|| AppError::Validation("Each trade requires a symbol".to_string()))?;
            let exchange = trade["exchange"].as_str().unwrap_or("GENERIC");
            let side = trade["side"]
                .as_str()
                .ok_or_else(|| AppError::Validation("Each trade requires a side (buy/sell)".to_string()))?;
            let quantity = trade["quantity"]
                .as_f64()
                .ok_or_else(|| AppError::Validation("Each trade requires a quantity".to_string()))?;
            let price = trade["price"]
                .as_f64()
                .ok_or_else(|| AppError::Validation("Each trade requires a price".to_string()))?;

            let result = state.sqlite.apply_scenario_trade(
                scenario_id,
                symbol,
                exchange,
                side,
                quantity,
                price,
            )?;

            applied.push(serde_json::json!({
                "symbol": symbol,
                "side": side,
                "quantity": quantity,
                "price": price,
                "resulting_position": {
                    "quantity": result.quantity,
                    "avg_price": result.avg_price,
                    "side": result.side,
                }
            }));
        }

        // Return the full updated positions list
        let updated_positions = state.sqlite.get_scenario_positions(scenario_id)?;
        let pos_data: Vec<serde_json::Value> = updated_positions
            .iter()
            .map(|p| {
                serde_json::json!({
                    "symbol": p.symbol,
                    "exchange": p.exchange,
                    "quantity": p.quantity,
                    "avg_price": p.avg_price,
                    "side": p.side,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "trades_applied": applied.len(),
            "trade_details": applied,
            "updated_positions": pos_data,
            "updated_position_count": updated_positions.len(),
        }))
    }

    // ========== Helpers ==========

    /// Get FMP client with API key from the database
    fn get_fmp_client(state: &AppState) -> Result<FmpClient> {
        let api_key = state
            .sqlite
            .get_provider_key("fmp", &state.security)?
            .ok_or_else(|| {
                AppError::Provider(
                    "FMP API key not configured. Please add your Financial Modeling Prep API key in Settings > Providers."
                        .to_string(),
                )
            })?;
        Ok(FmpClient::new((*state.http_client).clone(), api_key))
    }

    /// Get FRED client with API key from the database
    fn get_fred_client(state: &AppState) -> Result<FredClient> {
        let api_key = state
            .sqlite
            .get_provider_key("fred", &state.security)?
            .ok_or_else(|| {
                AppError::Provider(
                    "FRED API key not configured. Please add your FRED API key in Settings > Providers."
                        .to_string(),
                )
            })?;
        Ok(FredClient::new((*state.http_client).clone(), api_key))
    }

    /// Truncate a tool result to keep the conversation context small.
    /// For JSON arrays: keep only the first N items.
    /// For everything else: byte-level truncation at 4000 bytes.
    const MAX_RESULT_BYTES: usize = 4000;
    const MAX_ARRAY_ITEMS: usize = 10;

    fn truncate_tool_result(result: &serde_json::Value) -> String {
        // If it's an array, limit items first
        let truncated = if let Some(arr) = result.as_array() {
            if arr.len() > Self::MAX_ARRAY_ITEMS {
                let subset: Vec<&serde_json::Value> = arr.iter().take(Self::MAX_ARRAY_ITEMS).collect();
                let note = format!(
                    "Showing {} of {} results. Ask the user if they want more detail on specific items.",
                    Self::MAX_ARRAY_ITEMS, arr.len()
                );
                serde_json::json!({
                    "results": subset,
                    "total_count": arr.len(),
                    "note": note
                })
            } else {
                result.clone()
            }
        } else {
            result.clone()
        };

        let serialized = serde_json::to_string(&truncated).unwrap_or_else(|_| {
            r#"{"error": true, "message": "Failed to serialize tool result"}"#.to_string()
        });

        if serialized.len() > Self::MAX_RESULT_BYTES {
            // Byte-level fallback — ensure valid UTF-8 boundary
            let cut = &serialized[..Self::MAX_RESULT_BYTES];
            let safe_end = cut.rfind(|c: char| c.is_ascii()).unwrap_or(Self::MAX_RESULT_BYTES);
            format!(
                "{}... [truncated from {} bytes]",
                &serialized[..safe_end],
                serialized.len()
            )
        } else {
            serialized
        }
    }

    /// Extract the final text response from the conversation
    fn extract_final_text(conversation: &[Message]) -> String {
        // Walk backwards to find the last assistant message
        for msg in conversation.iter().rev() {
            if msg.role == "assistant" {
                match &msg.content {
                    MessageContent::Text(text) => return text.clone(),
                    MessageContent::Blocks(blocks) => {
                        let text_parts: Vec<&str> = blocks
                            .iter()
                            .filter_map(|b| {
                                if let ContentBlock::Text { text } = b {
                                    Some(text.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !text_parts.is_empty() {
                            return text_parts.join("\n");
                        }
                    }
                }
            }
        }
        "I was unable to generate a response. Please try again.".to_string()
    }

    /// Generate a brief summary of a tool result for the frontend
    fn summarize_tool_result(tool_name: &str, result: &serde_json::Value) -> String {
        match tool_name {
            "get_stock_quote" => {
                if let Some(arr) = result.as_array() {
                    let symbols: Vec<&str> = arr
                        .iter()
                        .filter_map(|q| q["symbol"].as_str())
                        .collect();
                    format!("Fetched quotes for {}", symbols.join(", "))
                } else {
                    "Fetched stock quote".to_string()
                }
            }
            "get_company_profile" => {
                if let Some(name) = result.get("company_name").and_then(|v| v.as_str()) {
                    format!("Retrieved profile for {}", name)
                } else {
                    "Retrieved company profile".to_string()
                }
            }
            "get_stock_news" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} news articles", arr.len())
                } else {
                    "Retrieved news articles".to_string()
                }
            }
            "screen_stocks" => {
                if let Some(arr) = result.as_array() {
                    format!("Screener returned {} stocks", arr.len())
                } else {
                    "Ran stock screener".to_string()
                }
            }
            "get_financial_statements" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} periods of financial data", arr.len())
                } else {
                    "Retrieved financial statements".to_string()
                }
            }
            "get_economic_data" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} data points", arr.len())
                } else {
                    "Retrieved economic data".to_string()
                }
            }
            "get_congressional_trades" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} congressional trades", arr.len())
                } else {
                    "Retrieved congressional trades".to_string()
                }
            }
            "get_earnings_transcript" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} earnings transcript(s)", arr.len())
                } else {
                    "Retrieved earnings transcript".to_string()
                }
            }
            "get_insider_trading" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} insider trades", arr.len())
                } else {
                    "Retrieved insider trading data".to_string()
                }
            }
            "get_market_calendar" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} calendar events", arr.len())
                } else {
                    "Retrieved market calendar".to_string()
                }
            }
            "get_etf_data" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved ETF data ({} items)", arr.len())
                } else {
                    "Retrieved ETF data".to_string()
                }
            }
            "get_market_overview" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} market data points", arr.len())
                } else {
                    "Retrieved market overview".to_string()
                }
            }
            "get_index_constituents" => {
                if let Some(arr) = result.as_array() {
                    format!("Retrieved {} index constituents", arr.len())
                } else {
                    "Retrieved index constituents".to_string()
                }
            }
            "get_broker_account" => {
                if let Some(true) = result.get("connected").and_then(|v| v.as_bool()) {
                    let broker = result.get("broker_name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                    let mode = result.get("mode").and_then(|v| v.as_str()).unwrap_or("live");
                    format!("Retrieved {} account info ({})", broker, mode)
                } else {
                    "No broker connected".to_string()
                }
            }
            "list_clients" => {
                let count = result.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                format!("Retrieved {} client(s)", count)
            }
            "get_client_portfolio" => {
                if let Some(name) = result.get("client").and_then(|c| c["name"].as_str()) {
                    let count = result.get("position_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    format!("Retrieved {}'s portfolio ({} positions)", name, count)
                } else {
                    "Retrieved client portfolio".to_string()
                }
            }
            "get_client_scenario" => {
                if let Some(name) = result.get("scenario").and_then(|s| s["name"].as_str()) {
                    let count = result.get("position_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    format!("Retrieved scenario \"{}\" ({} positions)", name, count)
                } else {
                    "Retrieved scenario".to_string()
                }
            }
            "apply_scenario_trades" => {
                let count = result.get("trades_applied").and_then(|v| v.as_i64()).unwrap_or(0);
                format!("Applied {} trade(s) to scenario", count)
            }
            _ => "Tool executed".to_string(),
        }
    }
}
