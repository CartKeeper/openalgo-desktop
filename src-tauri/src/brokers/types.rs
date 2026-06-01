//! Common broker types

use serde::{Deserialize, Serialize};

/// Order request for placing new orders
#[derive(Debug, Clone, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub exchange: String,
    pub side: String,         // BUY or SELL
    pub quantity: f64,
    pub price: f64,
    pub order_type: String,   // MARKET, LIMIT, SL, SL-M
    pub product: String,      // CNC, MIS, NRML
    pub validity: String,     // DAY, IOC
    pub trigger_price: Option<f64>,
    pub disclosed_quantity: Option<i32>,
    pub amo: bool,
    #[serde(default)]
    pub trail_price: Option<f64>,
    #[serde(default)]
    pub trail_percent: Option<f64>,
    /// Dollar-amount (notional) order. When set (and > 0), the broker places a
    /// market-day notional order instead of a share-quantity order.
    #[serde(default)]
    pub notional: Option<f64>,
    /// Broker-specific symbol format (e.g., "NSE:RELIANCE-EQ" for Fyers)
    /// Set by OrderService after looking up from symbol cache
    #[serde(skip_deserializing)]
    pub broker_symbol: Option<String>,
    /// Exchange token (required for Angel One)
    /// Set by OrderService after looking up from symbol cache
    #[serde(skip_deserializing)]
    pub symbol_token: Option<String>,
}

/// Modify order request
#[derive(Debug, Clone, Deserialize)]
pub struct ModifyOrderRequest {
    pub quantity: Option<i32>,
    pub price: Option<f64>,
    pub order_type: Option<String>,
    pub trigger_price: Option<f64>,
    pub validity: Option<String>,
}

/// Order response from broker
#[derive(Debug, Clone, Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub message: Option<String>,
}

/// Order from order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub exchange_order_id: Option<String>,
    pub symbol: String,
    pub exchange: String,
    pub side: String,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub pending_quantity: f64,
    pub price: f64,
    pub trigger_price: f64,
    pub average_price: f64,
    pub order_type: String,
    pub product: String,
    pub status: String,
    pub validity: String,
    pub order_timestamp: String,
    pub exchange_timestamp: Option<String>,
    pub rejection_reason: Option<String>,
}

/// Position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub exchange: String,
    pub product: String,
    pub quantity: f64,
    pub overnight_quantity: f64,
    pub average_price: f64,
    pub ltp: f64,
    pub pnl: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub buy_quantity: f64,
    pub buy_value: f64,
    pub sell_quantity: f64,
    pub sell_value: f64,
}

/// Holding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holding {
    pub symbol: String,
    pub exchange: String,
    pub isin: Option<String>,
    pub quantity: f64,
    pub t1_quantity: f64,
    pub average_price: f64,
    pub ltp: f64,
    pub close_price: f64,
    pub pnl: f64,
    pub pnl_percentage: f64,
    pub current_value: f64,
}

/// Funds/Margin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Funds {
    pub available_cash: f64,
    pub used_margin: f64,
    pub total_margin: f64,
    pub opening_balance: f64,
    pub payin: f64,
    pub payout: f64,
    pub span: f64,
    pub exposure: f64,
    pub collateral: f64,
}

/// Account activity — trade fills and non-trade events (cash deposits/
/// withdrawals, ACH transfers, dividends, fees).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountActivity {
    pub id: String,
    /// Activity type code (e.g. FILL, CSD, CSW, DIV, FEE, TRANS, JNLC)
    pub activity_type: String,
    /// ISO timestamp (trade `transaction_time`) or date (non-trade `date`)
    pub date: Option<String>,
    pub symbol: Option<String>,
    pub side: Option<String>,
    pub qty: Option<f64>,
    pub price: Option<f64>,
    /// Net cash amount for non-trade activities (deposits, dividends, fees)
    pub net_amount: Option<f64>,
    pub per_share_amount: Option<f64>,
    pub order_id: Option<String>,
    pub status: Option<String>,
}

/// Account portfolio equity history (aligned time series of equity and P&L).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioHistory {
    /// Unix epoch seconds for each data point.
    pub timestamp: Vec<i64>,
    /// Account equity at each point.
    pub equity: Vec<f64>,
    /// Profit/loss vs base value at each point.
    pub profit_loss: Vec<f64>,
    /// Profit/loss percent (as a fraction) at each point.
    pub profit_loss_pct: Vec<f64>,
    /// Equity at the start of the window.
    pub base_value: f64,
    /// Bar resolution (e.g. "1D", "1H").
    pub timeframe: String,
}

/// Real-time market clock (open/closed status and next session boundaries).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketClock {
    /// Current server timestamp (ISO 8601).
    pub timestamp: String,
    pub is_open: bool,
    /// Next market open (ISO 8601).
    pub next_open: String,
    /// Next market close (ISO 8601).
    pub next_close: String,
}

/// A single trading day from the market calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCalendarDay {
    /// Calendar date "YYYY-MM-DD".
    pub date: String,
    /// Regular session open "HH:MM".
    pub open: String,
    /// Regular session close "HH:MM".
    pub close: String,
    /// Extended-hours session open, if provided.
    pub session_open: Option<String>,
    /// Extended-hours session close, if provided.
    pub session_close: Option<String>,
}

/// A broker-hosted (cloud) watchlist. `symbols` is populated when fetching a
/// single watchlist; the list endpoint may return it empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerWatchlist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub symbols: Vec<String>,
}

/// Quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub exchange: String,
    pub ltp: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub bid: f64,
    pub ask: f64,
    pub bid_qty: i32,
    pub ask_qty: i32,
    pub oi: i64,
    pub change: f64,
    pub change_percent: f64,
    pub timestamp: String,
}

/// Market depth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDepth {
    pub symbol: String,
    pub exchange: String,
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
}

/// Depth level (bid/ask)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthLevel {
    pub price: f64,
    pub quantity: i32,
    pub orders: i32,
}

/// Symbol data from master contract
#[derive(Debug, Clone)]
pub struct SymbolData {
    pub symbol: String,
    pub token: String,
    pub exchange: String,
    pub name: String,
    pub lot_size: i32,
    pub tick_size: f64,
    pub instrument_type: String,
    pub expiry: Option<String>,
    pub strike: Option<f64>,
    pub option_type: Option<String>,
    /// Broker's original symbol format (e.g., "NSE:RELIANCE-EQ" for Fyers, "RELIANCE" for Angel)
    pub brsymbol: Option<String>,
    /// Broker's exchange code (e.g., "NSE" for the broker's API)
    pub brexchange: Option<String>,
}
