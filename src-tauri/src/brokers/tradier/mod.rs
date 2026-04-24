//! Tradier broker adapter
//!
//! Supports live and sandbox trading via API access token.
//! Live: https://api.tradier.com
//! Sandbox: https://sandbox.tradier.com

#![allow(dead_code)]

use crate::brokers::{AuthResponse, Broker, BrokerCredentials};
use crate::brokers::types::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

const LIVE_BASE_URL: &str = "https://api.tradier.com";
const SANDBOX_BASE_URL: &str = "https://sandbox.tradier.com";

/// Tradier broker implementation
pub struct TradierBroker {
    client: Client,
}

impl TradierBroker {
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

    /// Determine base URL — sandbox tokens typically start differently
    /// Users can also prefix their token with "sandbox:" to force sandbox mode
    fn parse_token(api_key: &str) -> (&str, &str) {
        if let Some(stripped) = api_key.strip_prefix("sandbox:") {
            (SANDBOX_BASE_URL, stripped)
        } else {
            (LIVE_BASE_URL, api_key)
        }
    }

    fn get_headers(token: &str) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        headers.insert("Accept", "application/json".parse().unwrap());
        headers
    }
}

impl Default for TradierBroker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tradier API Response Types
// ============================================================================

#[derive(Deserialize)]
struct TradierProfileResponse {
    profile: Option<TradierProfile>,
}

#[derive(Deserialize)]
struct TradierProfile {
    account: Option<TradierAccountOrVec>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierAccountOrVec {
    Single(TradierAccount),
    Multiple(Vec<TradierAccount>),
}

#[derive(Deserialize, Clone)]
struct TradierAccount {
    #[serde(default)]
    account_number: String,
    #[serde(default)]
    status: String,
    #[serde(rename = "type", default)]
    account_type: String,
}

#[derive(Deserialize)]
struct TradierBalancesResponse {
    balances: Option<TradierBalances>,
}

#[derive(Deserialize)]
struct TradierBalances {
    #[serde(default)]
    total_cash: f64,
    #[serde(default)]
    total_equity: f64,
    #[serde(default)]
    market_value: f64,
    #[serde(default)]
    option_buying_power: f64,
    #[serde(default)]
    stock_buying_power: f64,
    #[serde(default)]
    open_pl: f64,
    #[serde(default)]
    close_pl: f64,
    #[serde(default)]
    pending_cash: f64,
    margin: Option<TradierMarginBalances>,
}

#[derive(Deserialize)]
struct TradierMarginBalances {
    #[serde(default)]
    stock_buying_power: f64,
    #[serde(default)]
    option_buying_power: f64,
}

#[derive(Deserialize)]
struct TradierOrdersResponse {
    orders: Option<TradierOrdersWrapper>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierOrdersWrapper {
    Null,
    Orders { order: TradierOrderOrVec },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierOrderOrVec {
    Single(Box<TradierOrder>),
    Multiple(Vec<TradierOrder>),
}

#[derive(Deserialize)]
struct TradierOrder {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    status: String,
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    quantity: f64,
    #[serde(default)]
    exec_quantity: f64,
    #[serde(default)]
    remaining_quantity: f64,
    #[serde(rename = "type", default)]
    order_type: String,
    #[serde(default)]
    duration: String,
    #[serde(default)]
    price: Option<f64>,
    #[serde(default)]
    stop_price: Option<f64>,
    #[serde(default)]
    avg_fill_price: f64,
    #[serde(default)]
    create_date: String,
    #[serde(default)]
    transaction_date: Option<String>,
    #[serde(default)]
    reason_description: Option<String>,
    #[serde(default, rename = "class")]
    order_class: String,
}

#[derive(Deserialize)]
struct TradierPositionsResponse {
    positions: Option<TradierPositionsWrapper>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierPositionsWrapper {
    Null,
    Positions { position: TradierPositionOrVec },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierPositionOrVec {
    Single(Box<TradierPosition>),
    Multiple(Vec<TradierPosition>),
}

#[derive(Deserialize)]
struct TradierPosition {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    quantity: f64,
    #[serde(default)]
    cost_basis: f64,
    #[serde(default)]
    date_acquired: String,
}

#[derive(Deserialize)]
struct TradierQuotesResponse {
    quotes: Option<TradierQuotesWrapper>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierQuotesWrapper {
    Null,
    Quotes { quote: TradierQuoteOrVec },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierQuoteOrVec {
    Single(Box<TradierQuote>),
    Multiple(Vec<TradierQuote>),
}

#[derive(Deserialize)]
struct TradierQuote {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    last: Option<f64>,
    #[serde(default)]
    open: Option<f64>,
    #[serde(default)]
    high: Option<f64>,
    #[serde(default)]
    low: Option<f64>,
    #[serde(default)]
    close: Option<f64>,
    #[serde(default)]
    prevclose: Option<f64>,
    #[serde(default)]
    volume: Option<i64>,
    #[serde(default)]
    bid: Option<f64>,
    #[serde(default)]
    ask: Option<f64>,
    #[serde(default)]
    bidsize: Option<i32>,
    #[serde(default)]
    asksize: Option<i32>,
    #[serde(default)]
    change: Option<f64>,
    #[serde(default)]
    change_percentage: Option<f64>,
    #[serde(default)]
    exchange: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct TradierOrderResponse {
    order: Option<TradierOrderResult>,
}

#[derive(Deserialize)]
struct TradierOrderResult {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    status: String,
}

#[derive(Deserialize)]
struct TradierErrorResponse {
    #[serde(default)]
    fault: Option<TradierFault>,
}

#[derive(Deserialize)]
struct TradierFault {
    #[serde(default)]
    faultstring: String,
}

#[derive(Deserialize)]
struct TradierSearchResponse {
    securities: Option<TradierSecuritiesWrapper>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierSecuritiesWrapper {
    Null,
    Securities { security: TradierSecurityOrVec },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TradierSecurityOrVec {
    Single(Box<TradierSecurity>),
    Multiple(Vec<TradierSecurity>),
}

#[derive(Deserialize)]
struct TradierSecurity {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    exchange: String,
    #[serde(rename = "type", default)]
    security_type: String,
    #[serde(default)]
    description: String,
}

// ============================================================================
// Broker Trait Implementation
// ============================================================================

#[async_trait]
impl Broker for TradierBroker {
    fn id(&self) -> &'static str {
        "tradier"
    }

    fn name(&self) -> &'static str {
        "Tradier"
    }

    fn logo(&self) -> &'static str {
        "/logos/tradier.svg"
    }

    fn requires_totp(&self) -> bool {
        false
    }

    async fn authenticate(&self, credentials: BrokerCredentials) -> Result<AuthResponse> {
        let (base_url, token) = Self::parse_token(&credentials.api_key);

        // Verify by fetching user profile
        let response = self
            .client
            .get(format!("{}/v1/user/profile", base_url))
            .headers(Self::get_headers(token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<TradierErrorResponse>(&error_body)
                .ok()
                .and_then(|e| e.fault)
                .map(|f| f.faultstring)
                .unwrap_or_else(|| format!("HTTP {}", status));
            return Err(AppError::Auth(format!(
                "Tradier authentication failed: {}",
                error_msg
            )));
        }

        let profile_resp: TradierProfileResponse = response.json().await?;
        let profile = profile_resp
            .profile
            .ok_or_else(|| AppError::Auth("No profile in response".to_string()))?;

        let accounts = match profile.account {
            Some(TradierAccountOrVec::Single(a)) => vec![a],
            Some(TradierAccountOrVec::Multiple(v)) => v,
            None => {
                return Err(AppError::Auth("No accounts found".to_string()));
            }
        };

        let account = accounts
            .first()
            .ok_or_else(|| AppError::Auth("No accounts found".to_string()))?;

        // Store token and account number together
        let auth_token = format!("{}:{}", token, account.account_number);

        Ok(AuthResponse {
            auth_token,
            feed_token: None,
            user_id: account.account_number.clone(),
            user_name: Some(format!("{} ({})", account.account_type, account.status)),
        })
    }

    async fn place_order(&self, auth_token: &str, order: OrderRequest) -> Result<OrderResponse> {
        let (token, account_id) = parse_tradier_auth(auth_token)?;
        let token_for_parse = format!("{}:dummy", token);
        let (base_url, _) = Self::parse_token(&token_for_parse);
        let actual_token = if token.starts_with("sandbox:") {
            &token[8..]
        } else {
            &token
        };

        let symbol = order
            .broker_symbol
            .as_deref()
            .unwrap_or(&order.symbol);

        let mut params = vec![
            ("class", "equity".to_string()),
            ("symbol", symbol.to_string()),
            ("side", order.side.to_lowercase()),
            ("quantity", order.quantity.to_string()),
            ("type", map_order_type(&order.order_type).to_string()),
            ("duration", map_validity(&order.validity).to_string()),
        ];

        if order.order_type == "LIMIT" || order.order_type == "SL" {
            params.push(("price", format!("{:.2}", order.price)));
        }
        if let Some(trigger) = order.trigger_price {
            if trigger > 0.0 {
                params.push(("stop", format!("{:.2}", trigger)));
            }
        }
        // Trailing stop parameters
        if order.order_type == "TRAILING_STOP" {
            if let Some(tp) = order.trail_price {
                if tp > 0.0 {
                    params.push(("stop", format!("{:.2}", tp)));
                }
            } else if let Some(tp) = order.trail_percent {
                if tp > 0.0 {
                    params.push(("stop", format!("{:.2}", tp)));
                }
            }
        }

        let response = self
            .client
            .post(format!(
                "{}/v1/accounts/{}/orders",
                base_url, account_id
            ))
            .headers(Self::get_headers(actual_token))
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Order failed: {}", error_body)));
        }

        let order_resp: TradierOrderResponse = response.json().await?;
        let result = order_resp
            .order
            .ok_or_else(|| AppError::Broker("No order in response".to_string()))?;

        Ok(OrderResponse {
            order_id: result.id.to_string(),
            message: Some(format!("Order {}", result.status)),
        })
    }

    async fn modify_order(
        &self,
        auth_token: &str,
        order_id: &str,
        order: ModifyOrderRequest,
    ) -> Result<OrderResponse> {
        let (token, account_id) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        let mut params = Vec::new();
        if let Some(qty) = order.quantity {
            params.push(("quantity", qty.to_string()));
        }
        if let Some(price) = order.price {
            params.push(("price", format!("{:.2}", price)));
        }
        if let Some(trigger) = order.trigger_price {
            params.push(("stop", format!("{:.2}", trigger)));
        }
        if let Some(ref validity) = order.validity {
            params.push(("duration", map_validity(validity).to_string()));
        }
        if let Some(ref ot) = order.order_type {
            params.push(("type", map_order_type(ot).to_string()));
        }

        let response = self
            .client
            .put(format!(
                "{}/v1/accounts/{}/orders/{}",
                base_url, account_id, order_id
            ))
            .headers(Self::get_headers(actual_token))
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Modify failed: {}", error_body)));
        }

        let order_resp: TradierOrderResponse = response.json().await?;
        let result = order_resp
            .order
            .ok_or_else(|| AppError::Broker("No order in response".to_string()))?;

        Ok(OrderResponse {
            order_id: result.id.to_string(),
            message: Some("Order modified".to_string()),
        })
    }

    async fn cancel_order(
        &self,
        auth_token: &str,
        order_id: &str,
        _variety: Option<&str>,
    ) -> Result<()> {
        let (token, account_id) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        let response = self
            .client
            .delete(format!(
                "{}/v1/accounts/{}/orders/{}",
                base_url, account_id, order_id
            ))
            .headers(Self::get_headers(actual_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Cancel failed: {}", error_body)));
        }

        Ok(())
    }

    async fn get_order_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        let (token, account_id) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        let response = self
            .client
            .get(format!(
                "{}/v1/accounts/{}/orders",
                base_url, account_id
            ))
            .headers(Self::get_headers(actual_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get orders: {}", error_body)));
        }

        let orders_resp: TradierOrdersResponse = response.json().await?;

        let orders = match orders_resp.orders {
            Some(TradierOrdersWrapper::Orders { order }) => match order {
                TradierOrderOrVec::Single(o) => vec![*o],
                TradierOrderOrVec::Multiple(v) => v,
            },
            _ => Vec::new(),
        };

        Ok(orders
            .into_iter()
            .map(|o| {
                Order {
                    order_id: o.id.to_string(),
                    exchange_order_id: None,
                    symbol: o.symbol,
                    exchange: "US".to_string(),
                    side: o.side.to_uppercase(),
                    quantity: o.quantity as i32,
                    filled_quantity: o.exec_quantity as i32,
                    pending_quantity: o.remaining_quantity as i32,
                    price: o.price.unwrap_or(0.0),
                    trigger_price: o.stop_price.unwrap_or(0.0),
                    average_price: o.avg_fill_price,
                    order_type: map_order_type_back(&o.order_type),
                    product: "CNC".to_string(),
                    status: map_order_status(&o.status),
                    validity: o.duration.to_uppercase(),
                    order_timestamp: o.create_date,
                    exchange_timestamp: o.transaction_date,
                    rejection_reason: o.reason_description,
                }
            })
            .collect())
    }

    async fn get_trade_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        // Tradier doesn't have a separate trade book — filter filled orders
        let all_orders = self.get_order_book(auth_token).await?;
        Ok(all_orders
            .into_iter()
            .filter(|o| o.status == "COMPLETE" || o.status == "TRADED")
            .collect())
    }

    async fn get_positions(&self, auth_token: &str) -> Result<Vec<Position>> {
        let (token, account_id) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        let response = self
            .client
            .get(format!(
                "{}/v1/accounts/{}/positions",
                base_url, account_id
            ))
            .headers(Self::get_headers(actual_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get positions: {}", error_body)));
        }

        let pos_resp: TradierPositionsResponse = response.json().await?;

        let positions = match pos_resp.positions {
            Some(TradierPositionsWrapper::Positions { position }) => match position {
                TradierPositionOrVec::Single(p) => vec![*p],
                TradierPositionOrVec::Multiple(v) => v,
            },
            _ => Vec::new(),
        };

        // Fetch current prices for P&L calculation
        if positions.is_empty() {
            return Ok(Vec::new());
        }

        let symbols: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();
        let quotes = self
            .get_quote(
                auth_token,
                symbols.iter().map(|s| (s.clone(), "US".to_string())).collect(),
            )
            .await
            .unwrap_or_default();

        let quote_map: std::collections::HashMap<String, &Quote> =
            quotes.iter().map(|q| (q.symbol.clone(), q)).collect();

        Ok(positions
            .into_iter()
            .map(|p| {
                let qty = p.quantity as i32;
                let cost_basis = p.cost_basis;
                let avg_price = if qty != 0 {
                    cost_basis / (qty as f64)
                } else {
                    0.0
                };
                let ltp = quote_map
                    .get(&p.symbol)
                    .map(|q| q.ltp)
                    .unwrap_or(avg_price);
                let current_value = ltp * (qty as f64);
                let unrealized = current_value - cost_basis;

                let (buy_qty, buy_val, sell_qty, sell_val) = if qty > 0 {
                    (qty, cost_basis, 0, 0.0)
                } else {
                    (0, 0.0, qty.unsigned_abs() as i32, cost_basis.abs())
                };

                Position {
                    symbol: p.symbol,
                    exchange: "US".to_string(),
                    product: "CNC".to_string(),
                    quantity: qty,
                    overnight_quantity: qty,
                    average_price: avg_price,
                    ltp,
                    pnl: unrealized,
                    realized_pnl: 0.0,
                    unrealized_pnl: unrealized,
                    buy_quantity: buy_qty,
                    buy_value: buy_val,
                    sell_quantity: sell_qty,
                    sell_value: sell_val,
                }
            })
            .collect())
    }

    async fn get_holdings(&self, auth_token: &str) -> Result<Vec<Holding>> {
        let positions = self.get_positions(auth_token).await?;

        Ok(positions
            .into_iter()
            .map(|p| {
                let current_value = p.ltp * (p.quantity as f64);
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
                    t1_quantity: 0,
                    average_price: p.average_price,
                    ltp: p.ltp,
                    close_price: p.ltp,
                    pnl: p.unrealized_pnl,
                    pnl_percentage: pnl_pct,
                    current_value,
                }
            })
            .collect())
    }

    async fn get_funds(&self, auth_token: &str) -> Result<Funds> {
        let (token, account_id) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        let response = self
            .client
            .get(format!(
                "{}/v1/accounts/{}/balances",
                base_url, account_id
            ))
            .headers(Self::get_headers(actual_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get balances: {}", error_body)));
        }

        let bal_resp: TradierBalancesResponse = response.json().await?;
        let balances = bal_resp
            .balances
            .ok_or_else(|| AppError::Broker("No balances in response".to_string()))?;

        Ok(Funds {
            available_cash: balances.total_cash,
            used_margin: balances.total_equity - balances.total_cash,
            total_margin: balances
                .margin
                .as_ref()
                .map(|m| m.stock_buying_power)
                .unwrap_or(balances.stock_buying_power),
            opening_balance: balances.total_equity,
            payin: balances.pending_cash,
            payout: 0.0,
            span: 0.0,
            exposure: balances.market_value,
            collateral: 0.0,
        })
    }

    async fn get_quote(
        &self,
        auth_token: &str,
        symbols: Vec<(String, String)>,
    ) -> Result<Vec<Quote>> {
        let (token, _) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        let symbol_list: String = symbols
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let response = self
            .client
            .get(format!(
                "{}/v1/markets/quotes?symbols={}",
                base_url, symbol_list
            ))
            .headers(Self::get_headers(actual_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get quotes: {}", error_body)));
        }

        let quotes_resp: TradierQuotesResponse = response.json().await?;

        let tradier_quotes = match quotes_resp.quotes {
            Some(TradierQuotesWrapper::Quotes { quote }) => match quote {
                TradierQuoteOrVec::Single(q) => vec![*q],
                TradierQuoteOrVec::Multiple(v) => v,
            },
            _ => Vec::new(),
        };

        Ok(tradier_quotes
            .into_iter()
            .map(|q| {
                let ltp = q.last.unwrap_or(0.0);
                let prev_close = q.prevclose.unwrap_or(0.0);
                let change = q.change.unwrap_or(ltp - prev_close);
                let change_pct = q.change_percentage.unwrap_or_else(|| {
                    if prev_close > 0.0 {
                        (change / prev_close) * 100.0
                    } else {
                        0.0
                    }
                });

                Quote {
                    symbol: q.symbol,
                    exchange: q.exchange.unwrap_or_else(|| "US".to_string()),
                    ltp,
                    open: q.open.unwrap_or(0.0),
                    high: q.high.unwrap_or(0.0),
                    low: q.low.unwrap_or(0.0),
                    close: prev_close,
                    volume: q.volume.unwrap_or(0),
                    bid: q.bid.unwrap_or(0.0),
                    ask: q.ask.unwrap_or(0.0),
                    bid_qty: q.bidsize.unwrap_or(0),
                    ask_qty: q.asksize.unwrap_or(0),
                    oi: 0,
                    change,
                    change_percent: change_pct,
                    timestamp: String::new(),
                }
            })
            .collect())
    }

    async fn get_market_depth(
        &self,
        auth_token: &str,
        _exchange: &str,
        symbol: &str,
    ) -> Result<MarketDepth> {
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
        let (token, _) = parse_tradier_auth(auth_token)?;
        let (base_url, actual_token) = Self::parse_token(&token);

        // Tradier doesn't have a bulk symbol download — search common exchanges
        let exchanges = ["Q", "N", "A"]; // NASDAQ, NYSE, AMEX
        let mut all_symbols = Vec::new();

        for exchange in &exchanges {
            let response = self
                .client
                .get(format!(
                    "{}/v1/markets/search?q=&exchanges={}",
                    base_url, exchange
                ))
                .headers(Self::get_headers(actual_token))
                .send()
                .await?;

            if !response.status().is_success() {
                tracing::warn!("Failed to search exchange {}", exchange);
                continue;
            }

            let search_resp: TradierSearchResponse = response.json().await?;

            let securities = match search_resp.securities {
                Some(TradierSecuritiesWrapper::Securities { security }) => match security {
                    TradierSecurityOrVec::Single(s) => vec![*s],
                    TradierSecurityOrVec::Multiple(v) => v,
                },
                _ => Vec::new(),
            };

            for sec in securities {
                all_symbols.push(SymbolData {
                    symbol: sec.symbol.clone(),
                    token: sec.symbol.clone(),
                    exchange: map_tradier_exchange(&sec.exchange),
                    name: sec.description,
                    lot_size: 1,
                    tick_size: 0.01,
                    instrument_type: map_tradier_security_type(&sec.security_type),
                    expiry: None,
                    strike: None,
                    option_type: None,
                    brsymbol: Some(sec.symbol),
                    brexchange: Some(sec.exchange),
                });
            }
        }

        Ok(all_symbols)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_tradier_auth(auth_token: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = auth_token.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::Auth("Invalid Tradier auth token format".to_string()));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
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
        _ => "day",
    }
}

fn map_order_status(status: &str) -> String {
    match status {
        "pending" | "open" | "partially_filled" => {
            if status == "partially_filled" {
                "PARTIALLY_FILLED"
            } else {
                "OPEN"
            }
        }
        "filled" => "COMPLETE",
        "expired" | "canceled" => "CANCELLED",
        "rejected" => "REJECTED",
        _ => "UNKNOWN",
    }
    .to_string()
}

fn map_tradier_exchange(exchange: &str) -> String {
    match exchange {
        "Q" => "NASDAQ".to_string(),
        "N" => "NYSE".to_string(),
        "A" => "AMEX".to_string(),
        "V" => "ARCA".to_string(),
        _ => exchange.to_string(),
    }
}

fn map_tradier_security_type(st: &str) -> String {
    match st {
        "stock" => "EQ".to_string(),
        "option" => "OPT".to_string(),
        "etf" => "ETF".to_string(),
        "index" => "INDEX".to_string(),
        _ => "EQ".to_string(),
    }
}
