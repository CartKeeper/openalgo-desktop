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
14. **get_client_portfolio** - Look up a client's portfolio from the local client management system. Returns positions (symbols, quantities, avg price, P&L) and recent trades. Use when the user asks about a specific client's portfolio, holdings, or trades. Search by name (partial match works).

## CRITICAL RULES
- NEVER say "I don't have access to" any tool listed above. You DO have these tools. USE THEM.
- NEVER give a text-only response when you could call a tool instead. If the user asks about a topic and ANY of your tools could provide relevant data, call the tool FIRST.
- When a user asks about Congress, politicians, or government and markets — immediately call get_congressional_trades. Do NOT just describe what you could do — DO IT.
- When a user asks about any stock, sector, or market — immediately call relevant tools (quotes, screener, news, etc.). Do NOT offer to look things up — just look them up.

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
- For economic data, provide context about what the indicator means and its recent trend."#.to_string()
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
            "get_client_portfolio" => Self::tool_get_client_portfolio(tool_input, state).await,
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
            _ => "Tool executed".to_string(),
        }
    }
}
