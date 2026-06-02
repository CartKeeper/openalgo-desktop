//! Broker adapters module

pub mod types;
pub mod angel;
pub mod zerodha;
pub mod fyers;

// US brokers
pub mod alpaca;
pub mod tradier;
pub mod schwab;
pub mod ibkr;

use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use types::*;

/// Broker trait that all broker implementations must implement
#[async_trait]
pub trait Broker: Send + Sync {
    /// Broker ID (e.g., "angel", "zerodha", "fyers")
    fn id(&self) -> &'static str;

    /// Broker display name
    fn name(&self) -> &'static str;

    /// Broker logo path
    fn logo(&self) -> &'static str;

    /// Whether this broker requires TOTP for login
    fn requires_totp(&self) -> bool;

    /// Authenticate with broker
    async fn authenticate(&self, credentials: BrokerCredentials) -> Result<AuthResponse>;

    /// Place a new order
    async fn place_order(&self, auth_token: &str, order: OrderRequest) -> Result<OrderResponse>;

    /// Modify an existing order
    async fn modify_order(
        &self,
        auth_token: &str,
        order_id: &str,
        order: ModifyOrderRequest,
    ) -> Result<OrderResponse>;

    /// Cancel an order
    async fn cancel_order(
        &self,
        auth_token: &str,
        order_id: &str,
        variety: Option<&str>,
    ) -> Result<()>;

    /// Get order book
    async fn get_order_book(&self, auth_token: &str) -> Result<Vec<Order>>;

    /// Get trade book
    async fn get_trade_book(&self, auth_token: &str) -> Result<Vec<Order>>;

    /// Get positions
    async fn get_positions(&self, auth_token: &str) -> Result<Vec<Position>>;

    /// Get holdings
    async fn get_holdings(&self, auth_token: &str) -> Result<Vec<Holding>>;

    /// Get funds/margin
    async fn get_funds(&self, auth_token: &str) -> Result<Funds>;

    /// Get quote for symbols
    async fn get_quote(
        &self,
        auth_token: &str,
        symbols: Vec<(String, String)>,
    ) -> Result<Vec<Quote>>;

    /// Get market depth
    async fn get_market_depth(
        &self,
        auth_token: &str,
        exchange: &str,
        symbol: &str,
    ) -> Result<MarketDepth>;

    /// Download master contract
    async fn download_master_contract(&self, auth_token: &str) -> Result<Vec<SymbolData>>;

    /// Get account activity — trade fills and non-trade events such as cash
    /// deposits/withdrawals, ACH transfers, dividends, and fees.
    ///
    /// Default: not supported. Brokers that expose an activities endpoint
    /// (e.g. Alpaca) override this.
    async fn get_activities(
        &self,
        _auth_token: &str,
        _page_size: u32,
    ) -> Result<Vec<AccountActivity>> {
        Err(crate::error::AppError::Broker(
            "Account activities are not supported by this broker".to_string(),
        ))
    }

    /// Get account equity history (time series) for the given window.
    ///
    /// `period` e.g. "1M", "1A"; `timeframe` e.g. "1D", "1H".
    ///
    /// Default: not supported. Brokers that expose a portfolio-history
    /// endpoint (e.g. Alpaca) override this.
    async fn get_portfolio_history(
        &self,
        _auth_token: &str,
        _period: &str,
        _timeframe: &str,
    ) -> Result<PortfolioHistory> {
        Err(crate::error::AppError::Broker(
            "Portfolio history is not supported by this broker".to_string(),
        ))
    }

    /// Get the real-time market clock (open/closed + next session times).
    ///
    /// Default: not supported. Overridden by brokers with a clock endpoint.
    async fn get_market_clock(&self, _auth_token: &str) -> Result<MarketClock> {
        Err(crate::error::AppError::Broker(
            "Market clock is not supported by this broker".to_string(),
        ))
    }

    /// Get the trading calendar between optional `start`/`end` dates
    /// ("YYYY-MM-DD"). When omitted, the broker's default window is used.
    ///
    /// Default: not supported. Overridden by brokers with a calendar endpoint.
    async fn get_market_calendar(
        &self,
        _auth_token: &str,
        _start: Option<&str>,
        _end: Option<&str>,
    ) -> Result<Vec<MarketCalendarDay>> {
        Err(crate::error::AppError::Broker(
            "Market calendar is not supported by this broker".to_string(),
        ))
    }

    /// List broker-hosted (cloud) watchlists. Symbols may be omitted in the
    /// list view; use `get_watchlist` for the full contents.
    ///
    /// Default: not supported.
    async fn get_watchlists(&self, _auth_token: &str) -> Result<Vec<BrokerWatchlist>> {
        Err(crate::error::AppError::Broker(
            "Watchlists are not supported by this broker".to_string(),
        ))
    }

    /// Get a single broker-hosted watchlist (including its symbols).
    ///
    /// Default: not supported.
    async fn get_watchlist(&self, _auth_token: &str, _id: &str) -> Result<BrokerWatchlist> {
        Err(crate::error::AppError::Broker(
            "Watchlists are not supported by this broker".to_string(),
        ))
    }

    /// Create a broker-hosted watchlist with the given name and symbols.
    ///
    /// Default: not supported.
    async fn create_watchlist(
        &self,
        _auth_token: &str,
        _name: &str,
        _symbols: Vec<String>,
    ) -> Result<BrokerWatchlist> {
        Err(crate::error::AppError::Broker(
            "Watchlists are not supported by this broker".to_string(),
        ))
    }

    /// Delete a broker-hosted watchlist by id.
    ///
    /// Default: not supported.
    async fn delete_watchlist(&self, _auth_token: &str, _id: &str) -> Result<()> {
        Err(crate::error::AppError::Broker(
            "Watchlists are not supported by this broker".to_string(),
        ))
    }

    /// Get historical OHLCV bars. `interval` is the app interval string
    /// (e.g. "D", "1h", "5m"); the broker maps it to its own timeframe.
    /// Default: not supported.
    async fn get_history(
        &self,
        _auth_token: &str,
        _symbol: &str,
        _exchange: &str,
        _interval: &str,
        _from_date: &str,
        _to_date: &str,
    ) -> Result<Vec<crate::brokers::types::HistoricalBar>> {
        Err(crate::error::AppError::Broker(
            "Historical data is not supported by this broker".to_string(),
        ))
    }
}

/// Broker credentials for authentication
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BrokerCredentials {
    pub api_key: String,
    pub api_secret: Option<String>,
    pub client_id: Option<String>,
    pub password: Option<String>,
    pub totp: Option<String>,
    pub request_token: Option<String>,
    pub auth_code: Option<String>,
}

/// Authentication response from broker
#[derive(Debug, Clone)]
pub struct AuthResponse {
    pub auth_token: String,
    pub feed_token: Option<String>,
    pub user_id: String,
    pub user_name: Option<String>,
}

/// Broker registry for managing multiple brokers
pub struct BrokerRegistry {
    brokers: HashMap<String, Arc<dyn Broker>>,
}

impl BrokerRegistry {
    /// Create new broker registry with all supported brokers
    pub fn new() -> Self {
        let mut brokers: HashMap<String, Arc<dyn Broker>> = HashMap::new();

        // Register Indian brokers
        brokers.insert("angel".to_string(), Arc::new(angel::AngelBroker::new()));
        brokers.insert("zerodha".to_string(), Arc::new(zerodha::ZerodhaBroker::new()));
        brokers.insert("fyers".to_string(), Arc::new(fyers::FyersBroker::new()));

        // Register US brokers
        brokers.insert("alpaca".to_string(), Arc::new(alpaca::AlpacaBroker::new()));
        brokers.insert("tradier".to_string(), Arc::new(tradier::TradierBroker::new()));
        brokers.insert("schwab".to_string(), Arc::new(schwab::SchwabBroker::new()));
        brokers.insert("ibkr".to_string(), Arc::new(ibkr::IbkrBroker::new()));

        Self { brokers }
    }

    /// Get broker by ID
    pub fn get(&self, id: &str) -> Option<Arc<dyn Broker>> {
        self.brokers.get(id).cloned()
    }

    /// List all available brokers
    pub fn list(&self) -> Vec<Arc<dyn Broker>> {
        self.brokers.values().cloned().collect()
    }
}

impl Default for BrokerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
