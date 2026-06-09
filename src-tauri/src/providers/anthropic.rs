//! Anthropic Messages API client
//!
//! Provides AI-powered research copilot functionality using Claude.
//! API key required. Supports tool use for calling market data providers.

use super::ProviderError;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_MODEL: &str = "claude-sonnet-4-20250514";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 32768;

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// Message content can be a simple string or structured content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// A content block within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Tool definition for the Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Request body for the Anthropic Messages API
#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
}

/// Response from the Anthropic Messages API
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ResponseContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

/// A content block in the API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Token usage information
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Anthropic API error response
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    #[serde(rename = "type")]
    error_type: String,
    error: ApiErrorDetail,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Anthropic API client
pub struct AnthropicClient {
    client: Client,
}

impl AnthropicClient {
    /// Create a new Anthropic client
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Send a message to the Anthropic Messages API
    pub async fn send_message(
        &self,
        api_key: &str,
        messages: Vec<Message>,
        system_prompt: Option<&str>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<MessagesResponse, ProviderError> {
        let request = MessagesRequest {
            model: ANTHROPIC_MODEL.to_string(),
            max_tokens: MAX_TOKENS,
            system: system_prompt.map(String::from),
            messages,
            tools,
        };

        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(ProviderError::RateLimited("Anthropic".to_string()));
            }
            // Try to parse structured error
            if let Ok(api_err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                return Err(ProviderError::ApiError(format!(
                    "Anthropic API error: {}",
                    api_err.error.message
                )));
            }
            return Err(ProviderError::ApiError(format!(
                "Anthropic API returned {}: {}",
                status, body
            )));
        }

        let response: MessagesResponse = resp.json().await.map_err(|e| {
            ProviderError::Parse(format!("Failed to parse Anthropic response: {}", e))
        })?;

        Ok(response)
    }
}

/// Build tool definitions for the copilot's available data functions
pub fn build_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_stock_quote".to_string(),
            description: "Get real-time stock quotes including price, change, volume, market cap, and P/E ratio for one or more symbols. Uses Yahoo Finance data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbols": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of stock ticker symbols (e.g., [\"AAPL\", \"MSFT\", \"GOOGL\"])"
                    }
                },
                "required": ["symbols"]
            }),
        },
        ToolDefinition {
            name: "get_company_profile".to_string(),
            description: "Get detailed company profile including description, sector, industry, market cap, CEO, number of employees, IPO date, and website. Uses Financial Modeling Prep data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Stock ticker symbol (e.g., \"AAPL\")"
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolDefinition {
            name: "get_stock_news".to_string(),
            description: "Get recent news articles for specific stocks or general market news. Uses Financial Modeling Prep data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbols": {
                        "type": "string",
                        "description": "Comma-separated ticker symbols to filter news (e.g., \"AAPL,MSFT\"). Omit for general market news."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of articles to return (default: 10, max: 50)"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "screen_stocks".to_string(),
            description: "Screen and filter stocks based on criteria like market cap, price, sector, industry, beta, volume, and more. Uses Financial Modeling Prep screener.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "market_cap_min": {
                        "type": "number",
                        "description": "Minimum market cap in dollars"
                    },
                    "market_cap_max": {
                        "type": "number",
                        "description": "Maximum market cap in dollars"
                    },
                    "price_min": {
                        "type": "number",
                        "description": "Minimum stock price"
                    },
                    "price_max": {
                        "type": "number",
                        "description": "Maximum stock price"
                    },
                    "beta_min": {
                        "type": "number",
                        "description": "Minimum beta value"
                    },
                    "beta_max": {
                        "type": "number",
                        "description": "Maximum beta value"
                    },
                    "volume_min": {
                        "type": "integer",
                        "description": "Minimum average volume"
                    },
                    "sector": {
                        "type": "string",
                        "description": "Sector filter (e.g., \"Technology\", \"Healthcare\", \"Financial Services\")"
                    },
                    "industry": {
                        "type": "string",
                        "description": "Industry filter (e.g., \"Software\", \"Semiconductors\")"
                    },
                    "country": {
                        "type": "string",
                        "description": "Country filter (e.g., \"US\", \"GB\")"
                    },
                    "exchange": {
                        "type": "string",
                        "description": "Exchange filter (e.g., \"NYSE\", \"NASDAQ\")"
                    },
                    "is_etf": {
                        "type": "boolean",
                        "description": "Filter for ETFs only"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20)"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "get_financial_statements".to_string(),
            description: "Get financial statements (income statement, balance sheet, or cash flow) for a company. Also supports key financial metrics/ratios. Uses Financial Modeling Prep data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Stock ticker symbol (e.g., \"AAPL\")"
                    },
                    "statement_type": {
                        "type": "string",
                        "enum": ["income", "balance_sheet", "cash_flow", "key_metrics"],
                        "description": "Type of financial statement to retrieve"
                    },
                    "period": {
                        "type": "string",
                        "enum": ["annual", "quarter"],
                        "description": "Reporting period (default: \"annual\")"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of periods to return (default: 4)"
                    }
                },
                "required": ["symbol", "statement_type"]
            }),
        },
        ToolDefinition {
            name: "get_economic_data".to_string(),
            description: "Get economic data from FRED (Federal Reserve Economic Data). Supports indicators like GDP, CPI, unemployment rate, federal funds rate, etc. Common series IDs: GDP, CPIAUCSL, UNRATE, FEDFUNDS, DGS10, SP500, DEXUSEU.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "series_id": {
                        "type": "string",
                        "description": "FRED series ID (e.g., \"GDP\", \"CPIAUCSL\", \"UNRATE\", \"FEDFUNDS\", \"DGS10\")"
                    },
                    "observation_start": {
                        "type": "string",
                        "description": "Start date in YYYY-MM-DD format (e.g., \"2020-01-01\")"
                    },
                    "observation_end": {
                        "type": "string",
                        "description": "End date in YYYY-MM-DD format (e.g., \"2024-12-31\")"
                    }
                },
                "required": ["series_id"]
            }),
        },
        ToolDefinition {
            name: "get_congressional_trades".to_string(),
            description: "Get recent stock trades by U.S. Congress members (Senators and House Representatives). Shows what stocks politicians are buying and selling, including transaction amounts, dates, and asset descriptions. Uses Financial Modeling Prep data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "chamber": {
                        "type": "string",
                        "enum": ["senate", "house"],
                        "description": "Which chamber of Congress to query: 'senate' or 'house'"
                    },
                    "name": {
                        "type": "string",
                        "description": "Optional: filter by politician name (e.g., 'Pelosi', 'Tuberville'). If omitted, returns latest trades."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of trades to return (default: 25, max: 25)"
                    }
                },
                "required": ["chamber"]
            }),
        },
        // ---------- New FMP tools ----------
        ToolDefinition {
            name: "get_earnings_transcript".to_string(),
            description: "Get earnings call transcript for a company. Returns the full text of what was said during the earnings call by executives. Great for understanding management's outlook, guidance, and strategic priorities.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Stock ticker symbol (e.g., \"AAPL\")"
                    },
                    "year": {
                        "type": "integer",
                        "description": "Year of the earnings call (e.g., 2024)"
                    },
                    "quarter": {
                        "type": "integer",
                        "description": "Quarter (1-4). If omitted, returns all quarters for the year."
                    }
                },
                "required": ["symbol", "year"]
            }),
        },
        ToolDefinition {
            name: "get_insider_trading".to_string(),
            description: "Get insider trading data (SEC Form 4 filings) for a stock. Shows buys and sells by company executives, directors, and major shareholders. Useful for gauging insider confidence.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Stock ticker symbol (e.g., \"AAPL\")"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of trades to return (default: 25, max: 25)"
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolDefinition {
            name: "get_market_calendar".to_string(),
            description: "Get market calendar events: upcoming earnings reports, IPOs, dividend ex-dates, or stock splits. Useful for finding when companies report earnings or when IPOs are scheduled.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "calendar_type": {
                        "type": "string",
                        "enum": ["earnings", "ipo", "dividends", "splits"],
                        "description": "Type of calendar to retrieve"
                    },
                    "from_date": {
                        "type": "string",
                        "description": "Start date in YYYY-MM-DD format"
                    },
                    "to_date": {
                        "type": "string",
                        "description": "End date in YYYY-MM-DD format"
                    }
                },
                "required": ["calendar_type", "from_date", "to_date"]
            }),
        },
        ToolDefinition {
            name: "get_etf_data".to_string(),
            description: "Get ETF information or holdings. Use data_type 'info' for ETF details (expense ratio, assets, inception date) or 'holdings' to see what stocks the ETF holds and their weights.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "ETF ticker symbol (e.g., \"SPY\", \"QQQ\", \"VTI\")"
                    },
                    "data_type": {
                        "type": "string",
                        "enum": ["info", "holdings"],
                        "description": "Type of ETF data: 'info' for fund details, 'holdings' for portfolio composition"
                    }
                },
                "required": ["symbol", "data_type"]
            }),
        },
        ToolDefinition {
            name: "get_market_overview".to_string(),
            description: "Get market overview data: sector performance, today's biggest gainers, biggest losers, or most actively traded stocks. Great for understanding current market conditions.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "data_type": {
                        "type": "string",
                        "enum": ["sector_performance", "gainers", "losers", "most_active"],
                        "description": "Type of market data to retrieve"
                    }
                },
                "required": ["data_type"]
            }),
        },
        ToolDefinition {
            name: "get_index_constituents".to_string(),
            description: "Get the list of companies in a major stock index (S&P 500, Nasdaq 100, or Dow Jones 30). Returns symbol, name, sector, and other details for each constituent.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "index": {
                        "type": "string",
                        "enum": ["sp500", "nasdaq", "dowjones"],
                        "description": "Which index to retrieve constituents for"
                    }
                },
                "required": ["index"]
            }),
        },
        ToolDefinition {
            name: "get_broker_account".to_string(),
            description: "Get the currently connected broker account information. Returns broker name, account ID, user ID, whether it's a live or paper account, and connection status. Use when the user asks about their account, broker connection, or account details.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "get_my_portfolio".to_string(),
            description: "Get the user's OWN live portfolio on their connected broker account (e.g. Alpaca) — their actual current holdings and available cash. Returns each open position (symbol, quantity, average price, last price, unrealized P&L), any longer-term holdings, the available cash for new buys, and the mode (live/paper/analyze). This is the SOURCE OF TRUTH for what the user personally owns. Use this WHENEVER the user asks about \"my account\", \"my portfolio\", \"my positions\", what they hold, how their account is doing, or wants recommendations about their own holdings. Do NOT confuse this with get_client_portfolio (that is for other people's client accounts).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "list_clients".to_string(),
            description: "List all clients in the client management system. Returns each client's name, ID, broker, account ID, and account type. Use when the user asks about their clients, wants to see who is in the system, or needs to find a specific client.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "get_client_portfolio".to_string(),
            description: "Look up a client's portfolio from the client management system. Returns the client's profile info, computed positions (symbol, quantity, avg price, realized P&L), and recent trades. Search by client name (partial match supported).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "client_name": {
                        "type": "string",
                        "description": "Client name to search for (e.g., \"Casey Borst\"). Partial match supported."
                    }
                },
                "required": ["client_name"]
            }),
        },
        ToolDefinition {
            name: "get_client_scenario".to_string(),
            description: "Retrieve a client's sandbox scenario with its positions and current market prices. Returns scenario info, each position (symbol, quantity, avg price, side), and live pricing from Yahoo Finance (current price, day change). Use when the user asks to analyze or review a specific scenario.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scenario_id": {
                        "type": "integer",
                        "description": "The numeric ID of the scenario to retrieve."
                    }
                },
                "required": ["scenario_id"]
            }),
        },
        ToolDefinition {
            name: "apply_scenario_trades".to_string(),
            description: "Apply a batch of simulated trades to a client's sandbox scenario. Each trade updates or creates a position using weighted-average cost basis. Use this when you have concrete trade recommendations and the user wants to apply them to a scenario. Returns updated positions after all trades are applied.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "scenario_id": {
                        "type": "integer",
                        "description": "The numeric ID of the scenario to apply trades to."
                    },
                    "trades": {
                        "type": "array",
                        "description": "List of trades to apply.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "symbol": {
                                    "type": "string",
                                    "description": "Ticker symbol (e.g., \"AAPL\")"
                                },
                                "exchange": {
                                    "type": "string",
                                    "description": "Exchange identifier. Defaults to GENERIC."
                                },
                                "side": {
                                    "type": "string",
                                    "enum": ["buy", "sell"],
                                    "description": "Trade side: buy adds to long, sell reduces or shorts."
                                },
                                "quantity": {
                                    "type": "number",
                                    "description": "Number of shares."
                                },
                                "price": {
                                    "type": "number",
                                    "description": "Price per share."
                                }
                            },
                            "required": ["symbol", "side", "quantity", "price"]
                        }
                    }
                },
                "required": ["scenario_id", "trades"]
            }),
        },
    ]
}
