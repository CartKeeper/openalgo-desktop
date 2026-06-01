//! Alpaca Markets broker adapter
//!
//! Supports both live and paper trading via API key + secret authentication.
//! Live: https://api.alpaca.markets
//! Paper: https://paper-api.alpaca.markets

#![allow(dead_code)]

use crate::brokers::{AuthResponse, Broker, BrokerCredentials};
use crate::brokers::types::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

const LIVE_BASE_URL: &str = "https://api.alpaca.markets";
const PAPER_BASE_URL: &str = "https://paper-api.alpaca.markets";
const DATA_BASE_URL: &str = "https://data.alpaca.markets";

/// Alpaca Markets broker implementation
pub struct AlpacaBroker {
    client: Client,
}

impl AlpacaBroker {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .pool_idle_timeout(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(10)
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Determine base URL from API key prefix
    /// Paper keys start with "PK", live keys start with "AK" or "CK"
    fn get_base_url(api_key: &str) -> &'static str {
        if api_key.starts_with("PK") {
            PAPER_BASE_URL
        } else {
            LIVE_BASE_URL
        }
    }

    fn get_headers(
        &self,
        api_key: &str,
        api_secret: &str,
    ) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("APCA-API-KEY-ID", api_key.parse().unwrap());
        headers.insert("APCA-API-SECRET-KEY", api_secret.parse().unwrap());
        headers.insert("Accept", "application/json".parse().unwrap());
        headers
    }

    fn map_watchlist(w: AlpacaWatchlist) -> BrokerWatchlist {
        BrokerWatchlist {
            id: w.id,
            name: w.name,
            symbols: w.assets.into_iter().map(|a| a.symbol).collect(),
        }
    }

    /// Build the Alpaca /v2/orders request body. A notional (dollar) order
    /// takes precedence over share quantity; both are sent as strings. The
    /// caller appends limit/stop/trailing/bracket params after this.
    fn build_order_body(order: &OrderRequest) -> serde_json::Map<String, serde_json::Value> {
        let mut body = serde_json::Map::new();
        body.insert(
            "symbol".into(),
            serde_json::json!(order.broker_symbol.as_deref().unwrap_or(&order.symbol)),
        );
        match order.notional {
            Some(n) if n > 0.0 => {
                body.insert("notional".into(), serde_json::json!(n.to_string()));
            }
            _ => {
                body.insert("qty".into(), serde_json::json!(order.quantity.to_string()));
            }
        }
        body.insert("side".into(), serde_json::json!(order.side.to_lowercase()));
        body.insert("type".into(), serde_json::json!(map_order_type(&order.order_type)));
        body.insert(
            "time_in_force".into(),
            serde_json::json!(map_validity(&order.validity)),
        );
        body
    }
}

impl Default for AlpacaBroker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Alpaca API Response Types
// ============================================================================

#[derive(Deserialize)]
struct AlpacaAccount {
    id: String,
    #[serde(default)]
    account_number: String,
    status: String,
    #[serde(default)]
    buying_power: String,
    #[serde(default)]
    cash: String,
    #[serde(default)]
    portfolio_value: String,
    #[serde(default)]
    equity: String,
    #[serde(default)]
    last_equity: String,
    #[serde(default)]
    long_market_value: String,
    #[serde(default)]
    short_market_value: String,
    #[serde(default)]
    initial_margin: String,
    #[serde(default)]
    maintenance_margin: String,
    #[serde(default)]
    daytrading_buying_power: String,
    #[serde(default)]
    regt_buying_power: String,
}

/// Account activity entry. The /v2/account/activities endpoint returns a mixed
/// array of trade (FILL) and non-trade (CSD, CSW, DIV, FEE, TRANS, …) objects,
/// so every type-specific field is optional.
#[derive(Deserialize)]
struct AlpacaActivity {
    id: String,
    activity_type: String,
    /// Trade activities carry `transaction_time`; non-trade carry `date`.
    #[serde(default)]
    transaction_time: Option<String>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    qty: Option<String>,
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    net_amount: Option<String>,
    #[serde(default)]
    per_share_amount: Option<String>,
    #[serde(default)]
    order_id: Option<String>,
    #[serde(default)]
    order_status: Option<String>,
}

/// Portfolio history time series. Alpaca returns aligned arrays; some values
/// (e.g. the first profit_loss_pct) can be null, so element types are optional.
#[derive(Deserialize)]
struct AlpacaPortfolioHistory {
    #[serde(default)]
    timestamp: Vec<i64>,
    #[serde(default)]
    equity: Vec<Option<f64>>,
    #[serde(default)]
    profit_loss: Vec<Option<f64>>,
    #[serde(default)]
    profit_loss_pct: Vec<Option<f64>>,
    #[serde(default)]
    base_value: Option<f64>,
    #[serde(default)]
    timeframe: Option<String>,
}

/// Market clock response.
#[derive(Deserialize)]
struct AlpacaClock {
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    is_open: bool,
    #[serde(default)]
    next_open: String,
    #[serde(default)]
    next_close: String,
}

/// Single calendar day from /v2/calendar.
#[derive(Deserialize)]
struct AlpacaCalendarDay {
    #[serde(default)]
    date: String,
    #[serde(default)]
    open: String,
    #[serde(default)]
    close: String,
    #[serde(default)]
    session_open: Option<String>,
    #[serde(default)]
    session_close: Option<String>,
}

/// Watchlist object. The list endpoint omits `assets`; the detail/create
/// endpoints include them.
#[derive(Deserialize)]
struct AlpacaWatchlist {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    assets: Vec<AlpacaWatchlistAsset>,
}

#[derive(Deserialize)]
struct AlpacaWatchlistAsset {
    #[serde(default)]
    symbol: String,
}

#[derive(Deserialize)]
struct AlpacaOrder {
    id: String,
    #[serde(default)]
    client_order_id: String,
    symbol: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    #[serde(default)]
    qty: Option<String>,
    #[serde(default)]
    filled_qty: Option<String>,
    #[serde(default)]
    limit_price: Option<String>,
    #[serde(default)]
    stop_price: Option<String>,
    #[serde(default)]
    filled_avg_price: Option<String>,
    status: String,
    time_in_force: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    filled_at: Option<String>,
    #[serde(default)]
    asset_class: Option<String>,
}

#[derive(Deserialize)]
struct AlpacaPosition {
    symbol: String,
    #[serde(default)]
    exchange: String,
    #[serde(default)]
    qty: String,
    #[serde(default)]
    qty_available: String,
    side: String,
    #[serde(default)]
    avg_entry_price: String,
    #[serde(default)]
    current_price: String,
    #[serde(default)]
    market_value: String,
    #[serde(default)]
    cost_basis: String,
    #[serde(default)]
    unrealized_pl: String,
    #[serde(default)]
    unrealized_plpc: String,
}

#[derive(Deserialize)]
struct AlpacaAsset {
    id: String,
    symbol: String,
    name: String,
    exchange: String,
    #[serde(default)]
    asset_class: String,
    #[serde(default)]
    tradable: bool,
    #[serde(default)]
    fractionable: bool,
}

#[derive(Deserialize)]
struct AlpacaSnapshot {
    #[serde(default)]
    latest_trade: Option<AlpacaLatestTrade>,
    #[serde(default)]
    latest_quote: Option<AlpacaLatestQuote>,
    #[serde(default)]
    daily_bar: Option<AlpacaBar>,
    #[serde(default)]
    prev_daily_bar: Option<AlpacaBar>,
}

#[derive(Deserialize)]
struct AlpacaLatestTrade {
    #[serde(default, rename = "p")]
    price: f64,
    #[serde(default, rename = "s")]
    size: i64,
    #[serde(default, rename = "t")]
    timestamp: String,
}

#[derive(Deserialize)]
struct AlpacaLatestQuote {
    #[serde(default, rename = "bp")]
    bid_price: f64,
    #[serde(default, rename = "bs")]
    bid_size: i32,
    #[serde(default, rename = "ap")]
    ask_price: f64,
    #[serde(default, rename = "as")]
    ask_size: i32,
}

#[derive(Deserialize)]
struct AlpacaBar {
    #[serde(default, rename = "o")]
    open: f64,
    #[serde(default, rename = "h")]
    high: f64,
    #[serde(default, rename = "l")]
    low: f64,
    #[serde(default, rename = "c")]
    close: f64,
    #[serde(default, rename = "v")]
    volume: i64,
}

#[derive(Deserialize)]
struct AlpacaErrorResponse {
    #[serde(default)]
    message: String,
}

// ============================================================================
// Broker Trait Implementation
// ============================================================================

#[async_trait]
impl Broker for AlpacaBroker {
    fn id(&self) -> &'static str {
        "alpaca"
    }

    fn name(&self) -> &'static str {
        "Alpaca Markets"
    }

    fn logo(&self) -> &'static str {
        "/logos/alpaca.svg"
    }

    fn requires_totp(&self) -> bool {
        false
    }

    async fn authenticate(&self, credentials: BrokerCredentials) -> Result<AuthResponse> {
        let api_secret = credentials
            .api_secret
            .ok_or_else(|| AppError::Validation("API Secret is required for Alpaca".to_string()))?;

        let base_url = Self::get_base_url(&credentials.api_key);

        // Verify credentials by fetching account info
        let response = self
            .client
            .get(format!("{}/v2/account", base_url))
            .headers(self.get_headers(&credentials.api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<AlpacaErrorResponse>(&error_body)
                .map(|e| e.message)
                .unwrap_or_else(|_| format!("HTTP {}", status));
            return Err(AppError::Auth(format!(
                "Alpaca authentication failed: {}",
                error_msg
            )));
        }

        let account: AlpacaAccount = response.json().await?;

        if account.status != "ACTIVE" {
            return Err(AppError::Auth(format!(
                "Alpaca account is not active (status: {})",
                account.status
            )));
        }

        // For Alpaca, the "auth_token" is the API key:secret pair joined
        // We store them together so the broker session can use them
        let auth_token = format!("{}:{}", credentials.api_key, api_secret);

        Ok(AuthResponse {
            auth_token,
            feed_token: None,
            user_id: account.account_number,
            user_name: Some(account.id),
        })
    }

    async fn place_order(&self, auth_token: &str, order: OrderRequest) -> Result<OrderResponse> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let mut body = serde_json::Value::Object(Self::build_order_body(&order));

        if order.order_type == "LIMIT" || order.order_type == "SL" {
            body["limit_price"] = serde_json::json!(order.price.to_string());
        }
        if let Some(trigger) = order.trigger_price {
            if trigger > 0.0 {
                body["stop_price"] = serde_json::json!(trigger.to_string());
            }
        }
        // Trailing stop parameters
        if order.order_type == "TRAILING_STOP" {
            if let Some(tp) = order.trail_price {
                if tp > 0.0 {
                    body["trail_price"] = serde_json::json!(tp.to_string());
                }
            }
            if let Some(tp) = order.trail_percent {
                if tp > 0.0 {
                    body["trail_percent"] = serde_json::json!(tp.to_string());
                }
            }
        }
        // Extended hours for AMO
        if order.amo {
            body["extended_hours"] = serde_json::json!(true);
        }

        let response = self
            .client
            .post(format!("{}/v2/orders", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<AlpacaErrorResponse>(&error_body)
                .map(|e| e.message)
                .unwrap_or(error_body);
            return Err(AppError::Broker(format!("Order failed: {}", error_msg)));
        }

        let alpaca_order: AlpacaOrder = response.json().await?;

        Ok(OrderResponse {
            order_id: alpaca_order.id,
            message: Some(format!("Order {} placed", alpaca_order.status)),
        })
    }

    async fn modify_order(
        &self,
        auth_token: &str,
        order_id: &str,
        order: ModifyOrderRequest,
    ) -> Result<OrderResponse> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let mut body = serde_json::Map::new();
        if let Some(qty) = order.quantity {
            body.insert("qty".to_string(), serde_json::json!(qty.to_string()));
        }
        if let Some(price) = order.price {
            body.insert("limit_price".to_string(), serde_json::json!(price.to_string()));
        }
        if let Some(trigger) = order.trigger_price {
            body.insert("stop_price".to_string(), serde_json::json!(trigger.to_string()));
        }
        if let Some(validity) = &order.validity {
            body.insert("time_in_force".to_string(), serde_json::json!(map_validity(validity)));
        }

        let response = self
            .client
            .patch(format!("{}/v2/orders/{}", base_url, order_id))
            .headers(self.get_headers(&api_key, &api_secret))
            .json(&serde_json::Value::Object(body))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<AlpacaErrorResponse>(&error_body)
                .map(|e| e.message)
                .unwrap_or(error_body);
            return Err(AppError::Broker(format!("Modify order failed: {}", error_msg)));
        }

        let alpaca_order: AlpacaOrder = response.json().await?;

        Ok(OrderResponse {
            order_id: alpaca_order.id,
            message: Some("Order modified".to_string()),
        })
    }

    async fn cancel_order(
        &self,
        auth_token: &str,
        order_id: &str,
        _variety: Option<&str>,
    ) -> Result<()> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .delete(format!("{}/v2/orders/{}", base_url, order_id))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<AlpacaErrorResponse>(&error_body)
                .map(|e| e.message)
                .unwrap_or(error_body);
            return Err(AppError::Broker(format!("Cancel order failed: {}", error_msg)));
        }

        Ok(())
    }

    async fn get_order_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!("{}/v2/orders?status=all&limit=100", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get orders: {}", error_body)));
        }

        let orders: Vec<AlpacaOrder> = response.json().await?;

        Ok(orders.into_iter().map(|o| {
            let qty = parse_f64_or_zero(o.qty.as_deref());
            let filled = parse_f64_or_zero(o.filled_qty.as_deref());
            let pending = qty - filled;

            Order {
                order_id: o.id,
                exchange_order_id: Some(o.client_order_id),
                symbol: o.symbol,
                exchange: "US".to_string(),
                side: o.side.to_uppercase(),
                quantity: qty as f64,
                filled_quantity: filled as f64,
                pending_quantity: pending as f64,
                price: parse_f64_or_zero(o.limit_price.as_deref()),
                trigger_price: parse_f64_or_zero(o.stop_price.as_deref()),
                average_price: parse_f64_or_zero(o.filled_avg_price.as_deref()),
                order_type: map_order_type_back(&o.order_type),
                product: "CNC".to_string(),
                status: map_order_status(&o.status),
                validity: o.time_in_force.to_uppercase(),
                order_timestamp: o.created_at,
                exchange_timestamp: o.filled_at,
                rejection_reason: None,
            }
        }).collect())
    }

    async fn get_trade_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        // Alpaca doesn't have a separate trade book — use filled orders
        let response = self
            .client
            .get(format!("{}/v2/orders?status=closed&limit=100", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get trades: {}", error_body)));
        }

        let orders: Vec<AlpacaOrder> = response.json().await?;

        Ok(orders
            .into_iter()
            .filter(|o| o.status == "filled")
            .map(|o| {
                let qty = parse_f64_or_zero(o.qty.as_deref());
                let filled = parse_f64_or_zero(o.filled_qty.as_deref());

                Order {
                    order_id: o.id,
                    exchange_order_id: Some(o.client_order_id),
                    symbol: o.symbol,
                    exchange: "US".to_string(),
                    side: o.side.to_uppercase(),
                    quantity: qty as f64,
                    filled_quantity: filled as f64,
                    pending_quantity: 0.0,
                    price: parse_f64_or_zero(o.limit_price.as_deref()),
                    trigger_price: parse_f64_or_zero(o.stop_price.as_deref()),
                    average_price: parse_f64_or_zero(o.filled_avg_price.as_deref()),
                    order_type: map_order_type_back(&o.order_type),
                    product: "CNC".to_string(),
                    status: "TRADED".to_string(),
                    validity: o.time_in_force.to_uppercase(),
                    order_timestamp: o.created_at,
                    exchange_timestamp: o.filled_at,
                    rejection_reason: None,
                }
            })
            .collect())
    }

    async fn get_positions(&self, auth_token: &str) -> Result<Vec<Position>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!("{}/v2/positions", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get positions: {}", error_body)));
        }

        let positions: Vec<AlpacaPosition> = response.json().await?;

        Ok(positions.into_iter().map(|p| {
            let qty: f64 = p.qty.parse().unwrap_or(0.0);
            let _qty_i32 = qty as i32;
            let avg_price: f64 = p.avg_entry_price.parse().unwrap_or(0.0);
            let current_price: f64 = p.current_price.parse().unwrap_or(0.0);
            let cost_basis: f64 = p.cost_basis.parse().unwrap_or(0.0);
            let _market_value: f64 = p.market_value.parse().unwrap_or(0.0);
            let unrealized: f64 = p.unrealized_pl.parse().unwrap_or(0.0);

            let (buy_qty, buy_val, sell_qty, sell_val) = if p.side == "long" {
                (qty.abs(), cost_basis, 0.0, 0.0)
            } else {
                (0.0, 0.0, qty.abs(), cost_basis)
            };

            Position {
                symbol: p.symbol,
                exchange: if p.exchange.is_empty() { "US".to_string() } else { p.exchange },
                product: "CNC".to_string(),
                quantity: qty,
                overnight_quantity: qty,
                average_price: avg_price,
                ltp: current_price,
                pnl: unrealized,
                realized_pnl: 0.0,
                unrealized_pnl: unrealized,
                buy_quantity: buy_qty,
                buy_value: buy_val,
                sell_quantity: sell_qty,
                sell_value: sell_val,
            }
        }).collect())
    }

    async fn get_holdings(&self, auth_token: &str) -> Result<Vec<Holding>> {
        // Alpaca positions serve as both positions and holdings
        let positions = self.get_positions(auth_token).await?;

        Ok(positions.into_iter().map(|p| {
            let current_value = p.ltp * (p.quantity as f64);
            let pnl = p.unrealized_pnl;
            let pnl_pct = if p.average_price > 0.0 {
                ((p.ltp - p.average_price) / p.average_price) * 100.0
            } else {
                0.0
            };

            Holding {
                symbol: p.symbol,
                exchange: p.exchange,
                isin: None,
                quantity: p.quantity,
                t1_quantity: 0.0,
                average_price: p.average_price,
                ltp: p.ltp,
                close_price: p.ltp,
                pnl,
                pnl_percentage: pnl_pct,
                current_value,
            }
        }).collect())
    }

    async fn get_funds(&self, auth_token: &str) -> Result<Funds> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!("{}/v2/account", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get account: {}", error_body)));
        }

        let account: AlpacaAccount = response.json().await?;

        let cash: f64 = account.cash.parse().unwrap_or(0.0);
        let _equity: f64 = account.equity.parse().unwrap_or(0.0);
        let buying_power: f64 = account.buying_power.parse().unwrap_or(0.0);
        let initial_margin: f64 = account.initial_margin.parse().unwrap_or(0.0);
        let maintenance_margin: f64 = account.maintenance_margin.parse().unwrap_or(0.0);
        let last_equity: f64 = account.last_equity.parse().unwrap_or(0.0);
        let long_mv: f64 = account.long_market_value.parse().unwrap_or(0.0);
        let short_mv: f64 = account.short_market_value.parse().unwrap_or(0.0);

        Ok(Funds {
            available_cash: cash,
            used_margin: initial_margin,
            total_margin: buying_power,
            opening_balance: last_equity,
            payin: 0.0,
            payout: 0.0,
            span: maintenance_margin,
            exposure: long_mv + short_mv,
            collateral: 0.0,
        })
    }

    async fn get_activities(
        &self,
        auth_token: &str,
        page_size: u32,
    ) -> Result<Vec<AccountActivity>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);
        let ps = page_size.clamp(1, 100);

        let response = self
            .client
            .get(format!(
                "{}/v2/account/activities?page_size={}&direction=desc",
                base_url, ps
            ))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to get account activities: {}",
                error_body
            )));
        }

        let raw: Vec<AlpacaActivity> = response.json().await?;

        Ok(raw
            .into_iter()
            .map(|a| AccountActivity {
                id: a.id,
                activity_type: a.activity_type,
                date: a.transaction_time.or(a.date),
                symbol: a.symbol,
                side: a.side,
                qty: a.qty.as_deref().and_then(|s| s.parse().ok()),
                price: a.price.as_deref().and_then(|s| s.parse().ok()),
                net_amount: a.net_amount.as_deref().and_then(|s| s.parse().ok()),
                per_share_amount: a.per_share_amount.as_deref().and_then(|s| s.parse().ok()),
                order_id: a.order_id,
                status: a.order_status,
            })
            .collect())
    }

    async fn get_portfolio_history(
        &self,
        auth_token: &str,
        period: &str,
        timeframe: &str,
    ) -> Result<PortfolioHistory> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!(
                "{}/v2/account/portfolio/history?period={}&timeframe={}",
                base_url, period, timeframe
            ))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to get portfolio history: {}",
                error_body
            )));
        }

        let h: AlpacaPortfolioHistory = response.json().await?;

        Ok(PortfolioHistory {
            timestamp: h.timestamp,
            equity: h.equity.into_iter().map(|v| v.unwrap_or(0.0)).collect(),
            profit_loss: h.profit_loss.into_iter().map(|v| v.unwrap_or(0.0)).collect(),
            profit_loss_pct: h
                .profit_loss_pct
                .into_iter()
                .map(|v| v.unwrap_or(0.0))
                .collect(),
            base_value: h.base_value.unwrap_or(0.0),
            timeframe: h.timeframe.unwrap_or_default(),
        })
    }

    async fn get_market_clock(&self, auth_token: &str) -> Result<MarketClock> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!("{}/v2/clock", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to get market clock: {}",
                error_body
            )));
        }

        let c: AlpacaClock = response.json().await?;
        Ok(MarketClock {
            timestamp: c.timestamp,
            is_open: c.is_open,
            next_open: c.next_open,
            next_close: c.next_close,
        })
    }

    async fn get_market_calendar(
        &self,
        auth_token: &str,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<Vec<MarketCalendarDay>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let mut url = format!("{}/v2/calendar", base_url);
        let mut params: Vec<String> = Vec::new();
        if let Some(s) = start {
            params.push(format!("start={}", s));
        }
        if let Some(e) = end {
            params.push(format!("end={}", e));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self
            .client
            .get(url)
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to get market calendar: {}",
                error_body
            )));
        }

        let days: Vec<AlpacaCalendarDay> = response.json().await?;
        Ok(days
            .into_iter()
            .map(|d| MarketCalendarDay {
                date: d.date,
                open: d.open,
                close: d.close,
                session_open: d.session_open,
                session_close: d.session_close,
            })
            .collect())
    }

    async fn get_watchlists(&self, auth_token: &str) -> Result<Vec<BrokerWatchlist>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!("{}/v2/watchlists", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to get watchlists: {}",
                error_body
            )));
        }

        let raw: Vec<AlpacaWatchlist> = response.json().await?;
        Ok(raw.into_iter().map(Self::map_watchlist).collect())
    }

    async fn get_watchlist(&self, auth_token: &str, id: &str) -> Result<BrokerWatchlist> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!("{}/v2/watchlists/{}", base_url, id))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to get watchlist: {}",
                error_body
            )));
        }

        let raw: AlpacaWatchlist = response.json().await?;
        Ok(Self::map_watchlist(raw))
    }

    async fn create_watchlist(
        &self,
        auth_token: &str,
        name: &str,
        symbols: Vec<String>,
    ) -> Result<BrokerWatchlist> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let body = serde_json::json!({ "name": name, "symbols": symbols });

        let response = self
            .client
            .post(format!("{}/v2/watchlists", base_url))
            .headers(self.get_headers(&api_key, &api_secret))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to create watchlist: {}",
                error_body
            )));
        }

        let raw: AlpacaWatchlist = response.json().await?;
        Ok(Self::map_watchlist(raw))
    }

    async fn delete_watchlist(&self, auth_token: &str, id: &str) -> Result<()> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .delete(format!("{}/v2/watchlists/{}", base_url, id))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to delete watchlist: {}",
                error_body
            )));
        }

        Ok(())
    }

    async fn get_quote(
        &self,
        auth_token: &str,
        symbols: Vec<(String, String)>,
    ) -> Result<Vec<Quote>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;

        let symbol_list: Vec<&str> = symbols.iter().map(|(s, _)| s.as_str()).collect();

        let mut quotes = Vec::new();

        // Fetch snapshots in batches (Alpaca supports multiple symbols per request)
        for chunk in symbol_list.chunks(50) {
            let symbols_param = chunk.join(",");
            let url = format!(
                "{}/v2/stocks/snapshots?symbols={}&feed=iex",
                DATA_BASE_URL, symbols_param
            );

            let response = self
                .client
                .get(&url)
                .headers(self.get_headers(&api_key, &api_secret))
                .send()
                .await?;

            if !response.status().is_success() {
                tracing::warn!("Alpaca snapshot request failed: {}", response.status());
                continue;
            }

            let snapshots: std::collections::HashMap<String, AlpacaSnapshot> =
                response.json().await?;

            for (symbol, snap) in snapshots {
                let trade = snap.latest_trade.as_ref();
                let quote_data = snap.latest_quote.as_ref();
                let bar = snap.daily_bar.as_ref();
                let prev_bar = snap.prev_daily_bar.as_ref();

                let ltp = trade.map(|t| t.price).unwrap_or(0.0);
                let prev_close = prev_bar.map(|b| b.close).unwrap_or(0.0);
                let change = ltp - prev_close;
                let change_pct = if prev_close > 0.0 {
                    (change / prev_close) * 100.0
                } else {
                    0.0
                };

                quotes.push(Quote {
                    symbol: symbol.clone(),
                    exchange: "US".to_string(),
                    ltp,
                    open: bar.map(|b| b.open).unwrap_or(0.0),
                    high: bar.map(|b| b.high).unwrap_or(0.0),
                    low: bar.map(|b| b.low).unwrap_or(0.0),
                    close: prev_close,
                    volume: bar.map(|b| b.volume).unwrap_or(0),
                    bid: quote_data.map(|q| q.bid_price).unwrap_or(0.0),
                    ask: quote_data.map(|q| q.ask_price).unwrap_or(0.0),
                    bid_qty: quote_data.map(|q| q.bid_size).unwrap_or(0),
                    ask_qty: quote_data.map(|q| q.ask_size).unwrap_or(0),
                    oi: 0,
                    change,
                    change_percent: change_pct,
                    timestamp: trade
                        .map(|t| t.timestamp.clone())
                        .unwrap_or_default(),
                });
            }
        }

        Ok(quotes)
    }

    async fn get_market_depth(
        &self,
        auth_token: &str,
        _exchange: &str,
        symbol: &str,
    ) -> Result<MarketDepth> {
        // Alpaca doesn't provide full market depth on basic plans
        // Return single level from latest quote
        let quotes = self
            .get_quote(auth_token, vec![(symbol.to_string(), "US".to_string())])
            .await?;

        let quote = quotes.first();

        Ok(MarketDepth {
            symbol: symbol.to_string(),
            exchange: "US".to_string(),
            bids: vec![DepthLevel {
                price: quote.map(|q| q.bid).unwrap_or(0.0),
                quantity: quote.map(|q| q.bid_qty).unwrap_or(0),
                orders: 1,
            }],
            asks: vec![DepthLevel {
                price: quote.map(|q| q.ask).unwrap_or(0.0),
                quantity: quote.map(|q| q.ask_qty).unwrap_or(0),
                orders: 1,
            }],
        })
    }

    async fn download_master_contract(&self, auth_token: &str) -> Result<Vec<SymbolData>> {
        let (api_key, api_secret) = parse_auth_token(auth_token)?;
        let base_url = Self::get_base_url(&api_key);

        let response = self
            .client
            .get(format!(
                "{}/v2/assets?status=active&asset_class=us_equity",
                base_url
            ))
            .headers(self.get_headers(&api_key, &api_secret))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!(
                "Failed to download assets: {}",
                error_body
            )));
        }

        let assets: Vec<AlpacaAsset> = response.json().await?;

        Ok(assets
            .into_iter()
            .filter(|a| a.tradable)
            .map(|a| SymbolData {
                symbol: a.symbol.clone(),
                token: a.id,
                exchange: map_alpaca_exchange(&a.exchange),
                name: a.name,
                lot_size: 1,
                tick_size: 0.01,
                instrument_type: if a.asset_class == "crypto" {
                    "CRYPTO".to_string()
                } else {
                    "EQ".to_string()
                },
                expiry: None,
                strike: None,
                option_type: None,
                brsymbol: Some(a.symbol),
                brexchange: Some("US".to_string()),
            })
            .collect())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_auth_token(auth_token: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = auth_token.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::Auth("Invalid Alpaca auth token format".to_string()));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn parse_f64_or_zero(s: Option<&str>) -> f64 {
    s.and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn map_order_type(ot: &str) -> &str {
    match ot {
        "MARKET" => "market",
        "LIMIT" => "limit",
        "SL" => "stop_limit",
        "SL-M" => "stop",
        "TRAILING_STOP" => "trailing_stop",
        _ => "market",
    }
}

fn map_order_type_back(ot: &str) -> String {
    match ot {
        "market" => "MARKET",
        "limit" => "LIMIT",
        "stop_limit" => "SL",
        "stop" => "SL-M",
        "trailing_stop" => "TRAILING_STOP",
        _ => "MARKET",
    }
    .to_string()
}

fn map_validity(v: &str) -> &str {
    match v {
        "DAY" => "day",
        "IOC" => "ioc",
        "GTC" => "gtc",
        "FOK" => "fok",
        _ => "day",
    }
}

fn map_order_status(status: &str) -> String {
    match status {
        "new" | "accepted" | "pending_new" => "OPEN",
        "partially_filled" => "PARTIALLY_FILLED",
        "filled" => "COMPLETE",
        "done_for_day" => "COMPLETE",
        "canceled" | "expired" | "replaced" => "CANCELLED",
        "rejected" => "REJECTED",
        "pending_cancel" | "pending_replace" => "PENDING",
        _ => "UNKNOWN",
    }
    .to_string()
}

fn map_alpaca_exchange(exchange: &str) -> String {
    match exchange {
        "NASDAQ" | "NYSE" | "ARCA" | "BATS" | "OTC" | "AMEX" => exchange.to_string(),
        _ => "US".to_string(),
    }
}

#[cfg(test)]
mod order_body_tests {
    use super::*;

    fn req(notional: Option<f64>, quantity: f64) -> OrderRequest {
        OrderRequest {
            symbol: "AAPL".into(),
            exchange: "NASDAQ".into(),
            side: "BUY".into(),
            quantity,
            price: 0.0,
            order_type: "MARKET".into(),
            product: "CNC".into(),
            validity: "DAY".into(),
            trigger_price: None,
            disclosed_quantity: None,
            amo: false,
            trail_price: None,
            trail_percent: None,
            notional,
            broker_symbol: None,
            symbol_token: None,
        }
    }

    #[test]
    fn notional_order_sends_notional_not_qty() {
        let body = AlpacaBroker::build_order_body(&req(Some(50.0), 0.0));
        assert_eq!(body.get("notional").and_then(|v| v.as_str()), Some("50"));
        assert!(body.get("qty").is_none());
    }

    #[test]
    fn fractional_qty_order_sends_qty() {
        let body = AlpacaBroker::build_order_body(&req(None, 0.5));
        assert_eq!(body.get("qty").and_then(|v| v.as_str()), Some("0.5"));
        assert!(body.get("notional").is_none());
    }
}
