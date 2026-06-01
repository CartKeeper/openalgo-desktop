//! Charles Schwab (formerly TD Ameritrade) broker adapter
//!
//! Uses the Schwab Individual Developer API.
//! OAuth 2.0 flow: browser-based authorization → exchange code for tokens.
//! Base URL: https://api.schwabapi.com

#![allow(dead_code)]

use crate::brokers::{AuthResponse, Broker, BrokerCredentials};
use crate::brokers::types::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;

const BASE_URL: &str = "https://api.schwabapi.com";
const TOKEN_URL: &str = "https://api.schwabapi.com/v1/oauth/token";

/// Charles Schwab broker implementation
pub struct SchwabBroker {
    client: Client,
}

impl SchwabBroker {
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

    fn get_headers(access_token: &str) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", access_token).parse().unwrap(),
        );
        headers.insert("Accept", "application/json".parse().unwrap());
        headers
    }
}

impl Default for SchwabBroker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Schwab API Response Types
// ============================================================================

#[derive(Deserialize)]
struct SchwabTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    token_type: String,
}

#[derive(Deserialize)]
struct SchwabAccountsResponse {
    #[serde(default)]
    accounts: Vec<SchwabAccountWrapper>,
}

#[derive(Deserialize)]
struct SchwabAccountWrapper {
    #[serde(default, rename = "securitiesAccount")]
    securities_account: Option<SchwabSecuritiesAccount>,
    #[serde(default, rename = "hashValue")]
    hash_value: Option<String>,
}

// Schwab returns accounts as an array of objects with securitiesAccount
// Try both formats
#[derive(Deserialize)]
struct SchwabSecuritiesAccount {
    #[serde(default, rename = "accountNumber")]
    account_number: String,
    #[serde(rename = "type", default)]
    account_type: String,
    #[serde(default, rename = "currentBalances")]
    current_balances: Option<SchwabBalances>,
    #[serde(default)]
    positions: Option<Vec<SchwabPosition>>,
}

// Alternative account format (direct response)
#[derive(Deserialize)]
struct SchwabAccountDirect {
    #[serde(default, rename = "accountNumber")]
    account_number: String,
    #[serde(default, rename = "accountHash")]
    account_hash: String,
}

#[derive(Deserialize)]
struct SchwabBalances {
    #[serde(default, rename = "availableFunds")]
    available_funds: f64,
    #[serde(default, rename = "buyingPower")]
    buying_power: f64,
    #[serde(default, rename = "cashBalance")]
    cash_balance: f64,
    #[serde(default, rename = "longMarketValue")]
    long_market_value: f64,
    #[serde(default, rename = "shortMarketValue")]
    short_market_value: f64,
    #[serde(default, rename = "liquidationValue")]
    liquidation_value: f64,
    #[serde(default, rename = "maintenanceRequirement")]
    maintenance_requirement: f64,
    #[serde(default, rename = "totalCash")]
    total_cash: f64,
}

#[derive(Deserialize)]
struct SchwabPosition {
    #[serde(default, rename = "shortQuantity")]
    short_quantity: f64,
    #[serde(default, rename = "averagePrice")]
    average_price: f64,
    #[serde(default, rename = "currentDayProfitLoss")]
    current_day_pl: f64,
    #[serde(default, rename = "currentDayProfitLossPercentage")]
    current_day_pl_pct: f64,
    #[serde(default, rename = "longQuantity")]
    long_quantity: f64,
    #[serde(default, rename = "marketValue")]
    market_value: f64,
    #[serde(default)]
    instrument: Option<SchwabInstrument>,
}

#[derive(Deserialize)]
struct SchwabInstrument {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "assetType")]
    asset_type: String,
    #[serde(default)]
    cusip: Option<String>,
}

#[derive(Deserialize)]
struct SchwabOrderResponse {
    #[serde(default, rename = "orderId")]
    order_id: Option<i64>,
}

#[derive(Deserialize)]
struct SchwabOrder {
    #[serde(default, rename = "orderId")]
    order_id: i64,
    #[serde(default)]
    status: String,
    #[serde(default, rename = "orderType")]
    order_type: String,
    #[serde(default)]
    session: String,
    #[serde(default)]
    duration: String,
    #[serde(default)]
    price: f64,
    #[serde(default, rename = "stopPrice")]
    stop_price: f64,
    #[serde(default)]
    quantity: f64,
    #[serde(default, rename = "filledQuantity")]
    filled_quantity: f64,
    #[serde(default, rename = "remainingQuantity")]
    remaining_quantity: f64,
    #[serde(default, rename = "enteredTime")]
    entered_time: String,
    #[serde(default, rename = "closeTime")]
    close_time: Option<String>,
    #[serde(default, rename = "orderLegCollection")]
    order_legs: Option<Vec<SchwabOrderLeg>>,
    #[serde(default, rename = "statusDescription")]
    status_description: Option<String>,
}

#[derive(Deserialize)]
struct SchwabOrderLeg {
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    quantity: f64,
    #[serde(default)]
    instrument: Option<SchwabInstrument>,
}

#[derive(Deserialize)]
struct SchwabQuoteResponse {
    #[serde(flatten)]
    quotes: std::collections::HashMap<String, SchwabQuoteData>,
}

#[derive(Deserialize)]
struct SchwabQuoteData {
    #[serde(default)]
    quote: Option<SchwabQuoteInner>,
    #[serde(default)]
    reference: Option<SchwabReference>,
}

#[derive(Deserialize)]
struct SchwabQuoteInner {
    #[serde(default, rename = "lastPrice")]
    last_price: f64,
    #[serde(default, rename = "openPrice")]
    open_price: f64,
    #[serde(default, rename = "highPrice")]
    high_price: f64,
    #[serde(default, rename = "lowPrice")]
    low_price: f64,
    #[serde(default, rename = "closePrice")]
    close_price: f64,
    #[serde(default, rename = "totalVolume")]
    total_volume: i64,
    #[serde(default, rename = "bidPrice")]
    bid_price: f64,
    #[serde(default, rename = "askPrice")]
    ask_price: f64,
    #[serde(default, rename = "bidSize")]
    bid_size: i32,
    #[serde(default, rename = "askSize")]
    ask_size: i32,
    #[serde(default, rename = "netChange")]
    net_change: f64,
    #[serde(default, rename = "netPercentChange")]
    net_percent_change: f64,
}

#[derive(Deserialize)]
struct SchwabReference {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    exchange: String,
    #[serde(default, rename = "exchangeName")]
    exchange_name: String,
}

// ============================================================================
// Broker Trait Implementation
// ============================================================================

#[async_trait]
impl Broker for SchwabBroker {
    fn id(&self) -> &'static str {
        "schwab"
    }

    fn name(&self) -> &'static str {
        "Charles Schwab"
    }

    fn logo(&self) -> &'static str {
        "/logos/schwab.svg"
    }

    fn requires_totp(&self) -> bool {
        false
    }

    async fn authenticate(&self, credentials: BrokerCredentials) -> Result<AuthResponse> {
        // Schwab uses OAuth 2.0. The auth_code comes from the OAuth callback.
        let auth_code = credentials
            .auth_code
            .ok_or_else(|| {
                AppError::Validation("Authorization code is required for Schwab OAuth".to_string())
            })?;

        let api_secret = credentials.api_secret.ok_or_else(|| {
            AppError::Validation("App Secret is required for Schwab".to_string())
        })?;

        // Exchange authorization code for access token
        let basic_auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", credentials.api_key, api_secret));

        // Build redirect URI from credentials or use a default
        let redirect_uri = credentials
            .client_id
            .as_deref()
            .unwrap_or("http://127.0.0.1:5000/schwab/callback");

        let response = self
            .client
            .post(TOKEN_URL)
            .header("Authorization", format!("Basic {}", basic_auth))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", &auth_code),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "Schwab token exchange failed (HTTP {}): {}",
                status, error_body
            )));
        }

        let token_resp: SchwabTokenResponse = response.json().await?;

        // Store access_token + refresh_token together
        let auth_token = if let Some(ref refresh) = token_resp.refresh_token {
            format!("{}|{}", token_resp.access_token, refresh)
        } else {
            token_resp.access_token.clone()
        };

        // Fetch account numbers to get account hash
        let accounts_resp = self
            .client
            .get(format!("{}/trader/v1/accounts/accountNumbers", BASE_URL))
            .headers(Self::get_headers(&token_resp.access_token))
            .send()
            .await?;

        let mut account_number = String::new();
        let mut account_hash = String::new();

        if accounts_resp.status().is_success() {
            let accounts: Vec<SchwabAccountDirect> = accounts_resp.json().await.unwrap_or_default();
            if let Some(first) = accounts.first() {
                account_number = first.account_number.clone();
                account_hash = first.account_hash.clone();
            }
        }

        // Append account hash to auth token for later use
        let full_auth = format!("{}|ACCT:{}", auth_token, account_hash);

        Ok(AuthResponse {
            auth_token: full_auth,
            feed_token: token_resp.refresh_token,
            user_id: account_number,
            user_name: None,
        })
    }

    async fn place_order(&self, auth_token: &str, order: OrderRequest) -> Result<OrderResponse> {
        let (access_token, account_hash) = parse_schwab_auth(auth_token)?;

        let symbol = order
            .broker_symbol
            .as_deref()
            .unwrap_or(&order.symbol);

        let mut order_body = serde_json::json!({
            "orderType": map_order_type(&order.order_type),
            "session": if order.amo { "SEAMLESS" } else { "NORMAL" },
            "duration": map_validity(&order.validity),
            "orderStrategyType": "SINGLE",
            "orderLegCollection": [{
                "instruction": order.side.to_uppercase(),
                "quantity": order.quantity,
                "instrument": {
                    "symbol": symbol,
                    "assetType": "EQUITY",
                }
            }]
        });

        if order.order_type == "LIMIT" || order.order_type == "SL" {
            order_body["price"] = serde_json::json!(format!("{:.2}", order.price));
        }
        if let Some(trigger) = order.trigger_price {
            if trigger > 0.0 {
                order_body["stopPrice"] = serde_json::json!(format!("{:.2}", trigger));
            }
        }
        // Trailing stop parameters
        if order.order_type == "TRAILING_STOP" {
            order_body["stopPriceLinkBasis"] = serde_json::json!("LAST");
            if let Some(tp) = order.trail_price {
                if tp > 0.0 {
                    order_body["stopPriceLinkType"] = serde_json::json!("VALUE");
                    order_body["stopPriceOffset"] = serde_json::json!(tp);
                }
            } else if let Some(tp) = order.trail_percent {
                if tp > 0.0 {
                    order_body["stopPriceLinkType"] = serde_json::json!("PERCENT");
                    order_body["stopPriceOffset"] = serde_json::json!(tp);
                }
            }
        }

        let response = self
            .client
            .post(format!(
                "{}/trader/v1/accounts/{}/orders",
                BASE_URL, account_hash
            ))
            .headers(Self::get_headers(&access_token))
            .header("Content-Type", "application/json")
            .json(&order_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Order failed: {}", error_body)));
        }

        // Schwab returns order ID in Location header for 201 Created
        let order_id = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|loc| loc.rsplit('/').next())
            .unwrap_or("unknown")
            .to_string();

        Ok(OrderResponse {
            order_id,
            message: Some("Order placed".to_string()),
        })
    }

    async fn modify_order(
        &self,
        auth_token: &str,
        order_id: &str,
        order: ModifyOrderRequest,
    ) -> Result<OrderResponse> {
        let (access_token, account_hash) = parse_schwab_auth(auth_token)?;

        // Schwab requires the full order to be resubmitted for replacement
        // We need to fetch the original order first
        let original_resp = self
            .client
            .get(format!(
                "{}/trader/v1/accounts/{}/orders/{}",
                BASE_URL, account_hash, order_id
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !original_resp.status().is_success() {
            return Err(AppError::Broker("Failed to fetch original order".to_string()));
        }

        let mut original: serde_json::Value = original_resp.json().await?;

        // Update fields
        if let Some(qty) = order.quantity {
            if let Some(legs) = original.get_mut("orderLegCollection") {
                if let Some(first_leg) = legs.get_mut(0) {
                    first_leg["quantity"] = serde_json::json!(qty);
                }
            }
        }
        if let Some(price) = order.price {
            original["price"] = serde_json::json!(format!("{:.2}", price));
        }
        if let Some(trigger) = order.trigger_price {
            original["stopPrice"] = serde_json::json!(format!("{:.2}", trigger));
        }
        if let Some(ref ot) = order.order_type {
            original["orderType"] = serde_json::json!(map_order_type(ot));
        }

        let response = self
            .client
            .put(format!(
                "{}/trader/v1/accounts/{}/orders/{}",
                BASE_URL, account_hash, order_id
            ))
            .headers(Self::get_headers(&access_token))
            .header("Content-Type", "application/json")
            .json(&original)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Modify failed: {}", error_body)));
        }

        Ok(OrderResponse {
            order_id: order_id.to_string(),
            message: Some("Order replaced".to_string()),
        })
    }

    async fn cancel_order(
        &self,
        auth_token: &str,
        order_id: &str,
        _variety: Option<&str>,
    ) -> Result<()> {
        let (access_token, account_hash) = parse_schwab_auth(auth_token)?;

        let response = self
            .client
            .delete(format!(
                "{}/trader/v1/accounts/{}/orders/{}",
                BASE_URL, account_hash, order_id
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Cancel failed: {}", error_body)));
        }

        Ok(())
    }

    async fn get_order_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        let (access_token, account_hash) = parse_schwab_auth(auth_token)?;

        let response = self
            .client
            .get(format!(
                "{}/trader/v1/accounts/{}/orders",
                BASE_URL, account_hash
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get orders: {}", error_body)));
        }

        let orders: Vec<SchwabOrder> = response.json().await.unwrap_or_default();

        Ok(orders
            .into_iter()
            .map(|o| {
                let first_leg = o.order_legs.as_ref().and_then(|legs| legs.first());
                let symbol = first_leg
                    .and_then(|l| l.instrument.as_ref())
                    .map(|i| i.symbol.clone())
                    .unwrap_or_default();
                let side = first_leg
                    .map(|l| l.instruction.clone())
                    .unwrap_or_default();

                Order {
                    order_id: o.order_id.to_string(),
                    exchange_order_id: None,
                    symbol,
                    exchange: "US".to_string(),
                    side: side.to_uppercase(),
                    quantity: o.quantity as f64,
                    filled_quantity: o.filled_quantity as f64,
                    pending_quantity: o.remaining_quantity as f64,
                    price: o.price,
                    trigger_price: o.stop_price,
                    average_price: if o.filled_quantity > 0.0 {
                        o.price
                    } else {
                        0.0
                    },
                    order_type: map_order_type_back(&o.order_type),
                    product: "CNC".to_string(),
                    status: map_order_status(&o.status),
                    validity: map_duration_back(&o.duration),
                    order_timestamp: o.entered_time,
                    exchange_timestamp: o.close_time,
                    rejection_reason: o.status_description,
                }
            })
            .collect())
    }

    async fn get_trade_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        let all_orders = self.get_order_book(auth_token).await?;
        Ok(all_orders
            .into_iter()
            .filter(|o| o.status == "COMPLETE" || o.status == "TRADED")
            .collect())
    }

    async fn get_positions(&self, auth_token: &str) -> Result<Vec<Position>> {
        let (access_token, account_hash) = parse_schwab_auth(auth_token)?;

        let response = self
            .client
            .get(format!(
                "{}/trader/v1/accounts/{}?fields=positions",
                BASE_URL, account_hash
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get positions: {}", error_body)));
        }

        let account: SchwabAccountWrapper = response.json().await?;

        let positions = account
            .securities_account
            .and_then(|sa| sa.positions)
            .unwrap_or_default();

        Ok(positions
            .into_iter()
            .filter_map(|p| {
                let instrument = p.instrument?;
                let long_qty = p.long_quantity as f64;
                let short_qty = p.short_quantity as f64;
                let qty = long_qty - short_qty;
                let avg_price = p.average_price;
                let ltp = if qty != 0.0 {
                    p.market_value / qty
                } else {
                    avg_price
                };

                Some(Position {
                    symbol: instrument.symbol,
                    exchange: "US".to_string(),
                    product: "CNC".to_string(),
                    quantity: qty,
                    overnight_quantity: qty,
                    average_price: avg_price,
                    ltp,
                    pnl: p.current_day_pl,
                    realized_pnl: 0.0,
                    unrealized_pnl: p.current_day_pl,
                    buy_quantity: long_qty,
                    buy_value: avg_price * (long_qty as f64),
                    sell_quantity: short_qty,
                    sell_value: avg_price * (short_qty as f64),
                })
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
                    t1_quantity: 0.0,
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
        let (access_token, account_hash) = parse_schwab_auth(auth_token)?;

        let response = self
            .client
            .get(format!(
                "{}/trader/v1/accounts/{}",
                BASE_URL, account_hash
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get account: {}", error_body)));
        }

        let account: SchwabAccountWrapper = response.json().await?;
        let balances = account
            .securities_account
            .and_then(|sa| sa.current_balances)
            .unwrap_or(SchwabBalances {
                available_funds: 0.0,
                buying_power: 0.0,
                cash_balance: 0.0,
                long_market_value: 0.0,
                short_market_value: 0.0,
                liquidation_value: 0.0,
                maintenance_requirement: 0.0,
                total_cash: 0.0,
            });

        Ok(Funds {
            available_cash: balances.available_funds,
            used_margin: balances.maintenance_requirement,
            total_margin: balances.buying_power,
            opening_balance: balances.liquidation_value,
            payin: 0.0,
            payout: 0.0,
            span: balances.maintenance_requirement,
            exposure: balances.long_market_value + balances.short_market_value,
            collateral: 0.0,
        })
    }

    async fn get_quote(
        &self,
        auth_token: &str,
        symbols: Vec<(String, String)>,
    ) -> Result<Vec<Quote>> {
        let (access_token, _) = parse_schwab_auth(auth_token)?;

        let symbol_list: String = symbols
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let response = self
            .client
            .get(format!(
                "{}/marketdata/v1/quotes?symbols={}&indicative=false",
                BASE_URL, symbol_list
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get quotes: {}", error_body)));
        }

        let quote_map: std::collections::HashMap<String, SchwabQuoteData> =
            response.json().await?;

        Ok(quote_map
            .into_iter()
            .filter_map(|(symbol, data)| {
                let q = data.quote?;
                Some(Quote {
                    symbol: symbol.clone(),
                    exchange: data
                        .reference
                        .map(|r| r.exchange_name)
                        .unwrap_or_else(|| "US".to_string()),
                    ltp: q.last_price,
                    open: q.open_price,
                    high: q.high_price,
                    low: q.low_price,
                    close: q.close_price,
                    volume: q.total_volume,
                    bid: q.bid_price,
                    ask: q.ask_price,
                    bid_qty: q.bid_size,
                    ask_qty: q.ask_size,
                    oi: 0,
                    change: q.net_change,
                    change_percent: q.net_percent_change,
                    timestamp: String::new(),
                })
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
        let (access_token, _) = parse_schwab_auth(auth_token)?;

        // Schwab doesn't have a bulk download — use instrument search
        // Search for actively traded equities
        let response = self
            .client
            .get(format!(
                "{}/marketdata/v1/instruments?symbol=.*&projection=symbol-regex",
                BASE_URL
            ))
            .headers(Self::get_headers(&access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            // Fall back to empty — symbols can be searched on demand
            tracing::warn!("Schwab master contract download not available via bulk API");
            return Ok(Vec::new());
        }

        let instruments: std::collections::HashMap<String, SchwabInstrumentData> =
            response.json().await.unwrap_or_default();

        Ok(instruments
            .into_iter()
            .map(|(_, inst)| SymbolData {
                symbol: inst.symbol.clone(),
                token: inst.cusip.unwrap_or_else(|| inst.symbol.clone()),
                exchange: inst.exchange,
                name: inst.description,
                lot_size: 1,
                tick_size: 0.01,
                instrument_type: map_schwab_asset_type(&inst.asset_type),
                expiry: None,
                strike: None,
                option_type: None,
                brsymbol: Some(inst.symbol),
                brexchange: Some("US".to_string()),
            })
            .collect())
    }
}

#[derive(Deserialize)]
struct SchwabInstrumentData {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    exchange: String,
    #[serde(default, rename = "assetType")]
    asset_type: String,
    #[serde(default)]
    cusip: Option<String>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_schwab_auth(auth_token: &str) -> Result<(String, String)> {
    // Format: "access_token|refresh_token|ACCT:account_hash"
    // or: "access_token|ACCT:account_hash"
    let acct_marker = "|ACCT:";
    if let Some(acct_pos) = auth_token.find(acct_marker) {
        let account_hash = auth_token[acct_pos + acct_marker.len()..].to_string();
        let token_part = &auth_token[..acct_pos];
        // Access token is everything before first | (or the whole thing if no refresh)
        let access_token = token_part
            .split('|')
            .next()
            .unwrap_or(token_part)
            .to_string();
        Ok((access_token, account_hash))
    } else {
        Err(AppError::Auth(
            "Invalid Schwab auth token format (missing account hash)".to_string(),
        ))
    }
}

fn map_order_type(ot: &str) -> &str {
    match ot {
        "MARKET" => "MARKET",
        "LIMIT" => "LIMIT",
        "SL" => "STOP_LIMIT",
        "SL-M" => "STOP",
        "TRAILING_STOP" => "TRAILING_STOP",
        _ => "MARKET",
    }
}

fn map_order_type_back(ot: &str) -> String {
    match ot {
        "MARKET" => "MARKET",
        "LIMIT" => "LIMIT",
        "STOP_LIMIT" => "SL",
        "STOP" => "SL-M",
        "TRAILING_STOP" => "TRAILING_STOP",
        _ => "MARKET",
    }
    .to_string()
}

fn map_validity(v: &str) -> &str {
    match v {
        "DAY" => "DAY",
        "IOC" => "IMMEDIATE_OR_CANCEL",
        "FOK" => "FILL_OR_KILL",
        "GTC" => "GOOD_TILL_CANCEL",
        _ => "DAY",
    }
}

fn map_duration_back(d: &str) -> String {
    match d {
        "DAY" => "DAY",
        "GOOD_TILL_CANCEL" => "GTC",
        "FILL_OR_KILL" => "IOC",
        _ => "DAY",
    }
    .to_string()
}

fn map_order_status(status: &str) -> String {
    match status {
        "AWAITING_PARENT_ORDER" | "AWAITING_CONDITION" | "PENDING_ACTIVATION" | "QUEUED"
        | "WORKING" => "OPEN",
        "ACCEPTED" => "OPEN",
        "FILLED" => "COMPLETE",
        "EXPIRED" | "CANCELED" | "REPLACED" => "CANCELLED",
        "REJECTED" => "REJECTED",
        "PENDING_CANCEL" | "PENDING_REPLACE" => "PENDING",
        _ => "UNKNOWN",
    }
    .to_string()
}

fn map_schwab_asset_type(at: &str) -> String {
    match at {
        "EQUITY" => "EQ",
        "OPTION" => "OPT",
        "MUTUAL_FUND" => "MF",
        "FIXED_INCOME" => "BOND",
        "INDEX" => "INDEX",
        "ETF" => "ETF",
        _ => "EQ",
    }
    .to_string()
}
