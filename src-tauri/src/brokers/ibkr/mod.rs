//! Interactive Brokers adapter via Client Portal API
//!
//! Uses the IBKR Client Portal Gateway REST API.
//! Requires the IB Gateway or TWS running locally.
//! Default gateway URL: https://localhost:5000 (IB Gateway)
//!
//! Auth flow: User starts IB Gateway → authenticates in browser → API becomes available.
//! The "api_key" field stores the gateway URL (e.g., "https://localhost:5000").

#![allow(dead_code, non_snake_case)]

use crate::brokers::{AuthResponse, Broker, BrokerCredentials};
use crate::brokers::types::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

const DEFAULT_GATEWAY_URL: &str = "https://localhost:5000";

/// Interactive Brokers broker implementation (Client Portal API)
pub struct IbkrBroker {
    client: Client,
}

impl IbkrBroker {
    pub fn new() -> Self {
        Self {
            // IBKR Gateway uses a self-signed cert — accept invalid certs for localhost
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .danger_accept_invalid_certs(true)
                .pool_idle_timeout(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(10)
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    fn get_base_url(credentials_api_key: &str) -> String {
        if credentials_api_key.is_empty() || credentials_api_key == "default" {
            DEFAULT_GATEWAY_URL.to_string()
        } else {
            credentials_api_key.trim_end_matches('/').to_string()
        }
    }
}

impl Default for IbkrBroker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// IBKR Client Portal API Response Types
// ============================================================================

#[derive(Deserialize)]
struct IbkrAuthStatus {
    #[serde(default)]
    authenticated: bool,
    #[serde(default)]
    connected: bool,
    #[serde(default)]
    competing: bool,
}

#[derive(Deserialize)]
struct IbkrAccount {
    #[serde(default)]
    id: String,
    #[serde(default, rename = "accountId")]
    account_id: String,
    #[serde(rename = "type", default)]
    account_type: String,
    #[serde(default)]
    desc: String,
}

#[derive(Deserialize)]
struct IbkrAccountsResponse {
    #[serde(default)]
    accounts: Vec<String>,
    #[serde(default)]
    selectedAccount: String,
}

#[derive(Deserialize)]
struct IbkrOrderReply {
    #[serde(default)]
    order_id: String,
    #[serde(default)]
    order_status: String,
    #[serde(default)]
    encrypt_message: Option<String>,
    // Confirmation fields
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    message: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct IbkrLiveOrder {
    #[serde(default, rename = "orderId")]
    order_id: i64,
    #[serde(default)]
    conid: i64,
    #[serde(default)]
    ticker: String,
    #[serde(default)]
    side: String,
    #[serde(default, rename = "totalSize")]
    total_size: f64,
    #[serde(default, rename = "filledQuantity")]
    filled_quantity: f64,
    #[serde(default, rename = "remainingQuantity")]
    remaining_quantity: f64,
    #[serde(default, rename = "orderType")]
    order_type: String,
    #[serde(default)]
    price: f64,
    #[serde(default, rename = "auxPrice")]
    aux_price: f64,
    #[serde(default, rename = "avgPrice")]
    avg_price: f64,
    #[serde(default)]
    status: String,
    #[serde(default)]
    tif: String,
    #[serde(default, rename = "lastExecutionTime")]
    last_execution_time: String,
    #[serde(default, rename = "orderDesc")]
    order_desc: Option<String>,
}

#[derive(Deserialize)]
struct IbkrLiveOrdersResponse {
    #[serde(default)]
    orders: Vec<IbkrLiveOrder>,
}

#[derive(Deserialize)]
struct IbkrPosition {
    #[serde(default)]
    conid: i64,
    #[serde(default, rename = "contractDesc")]
    contract_desc: String,
    #[serde(default)]
    position: f64,
    #[serde(default, rename = "mktPrice")]
    mkt_price: f64,
    #[serde(default, rename = "mktValue")]
    mkt_value: f64,
    #[serde(default, rename = "avgCost")]
    avg_cost: f64,
    #[serde(default, rename = "avgPrice")]
    avg_price: f64,
    #[serde(default, rename = "unrealizedPnl")]
    unrealized_pnl: f64,
    #[serde(default, rename = "realizedPnl")]
    realized_pnl: f64,
    #[serde(default)]
    currency: String,
    #[serde(default)]
    ticker: Option<String>,
    #[serde(default, rename = "assetClass")]
    asset_class: Option<String>,
    #[serde(default, rename = "listingExchange")]
    listing_exchange: Option<String>,
}

#[derive(Deserialize)]
struct IbkrAccountSummary {
    #[serde(default, rename = "availablefunds")]
    available_funds: Option<IbkrSummaryValue>,
    #[serde(default, rename = "buyingpower")]
    buying_power: Option<IbkrSummaryValue>,
    #[serde(default, rename = "netliquidation")]
    net_liquidation: Option<IbkrSummaryValue>,
    #[serde(default, rename = "totalcashvalue")]
    total_cash: Option<IbkrSummaryValue>,
    #[serde(default, rename = "maintenancemargin")]
    maintenance_margin: Option<IbkrSummaryValue>,
    #[serde(default, rename = "excessliquidity")]
    excess_liquidity: Option<IbkrSummaryValue>,
    #[serde(default, rename = "grosspositionvalue")]
    gross_position_value: Option<IbkrSummaryValue>,
}

#[derive(Deserialize, Clone, Copy)]
struct IbkrSummaryValue {
    #[serde(default)]
    amount: f64,
}

#[derive(Deserialize)]
struct IbkrSnapshot {
    #[serde(default)]
    conid: i64,
    // Market data fields use string keys like "31", "84", etc.
    // 31 = last, 84 = bid, 85 = ask, 86 = ask size
    // 7295 = open, 7296 = high, 7297 = low, 7291 = close
    // 87 = bid size, 7282 = change, 7283 = change_pct
    #[serde(default, rename = "31")]
    last: Option<String>,
    #[serde(default, rename = "84")]
    bid: Option<String>,
    #[serde(default, rename = "85")]
    ask: Option<String>,
    #[serde(default, rename = "86")]
    ask_size: Option<String>,
    #[serde(default, rename = "87")]
    bid_size: Option<String>,
    #[serde(default, rename = "7295")]
    open: Option<String>,
    #[serde(default, rename = "7296")]
    high: Option<String>,
    #[serde(default, rename = "7297")]
    low: Option<String>,
    #[serde(default, rename = "7291")]
    close: Option<String>,
    #[serde(default, rename = "7282")]
    change: Option<String>,
    #[serde(default, rename = "7283")]
    change_pct: Option<String>,
    #[serde(default, rename = "7762")]
    volume: Option<String>,
    #[serde(default, rename = "55")]
    symbol: Option<String>,
}

#[derive(Deserialize)]
struct IbkrSearchResult {
    #[serde(default)]
    conid: i64,
    #[serde(default)]
    companyName: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    secType: String,
    #[serde(default)]
    listingExchange: String,
}

// ============================================================================
// Broker Trait Implementation
// ============================================================================

#[async_trait]
impl Broker for IbkrBroker {
    fn id(&self) -> &'static str {
        "ibkr"
    }

    fn name(&self) -> &'static str {
        "Interactive Brokers"
    }

    fn logo(&self) -> &'static str {
        "/logos/ibkr.svg"
    }

    fn requires_totp(&self) -> bool {
        false
    }

    async fn authenticate(&self, credentials: BrokerCredentials) -> Result<AuthResponse> {
        let base_url = Self::get_base_url(&credentials.api_key);

        // Check if the IB Gateway is running and authenticated
        let response = self
            .client
            .get(format!("{}/v1/api/iserver/auth/status", base_url))
            .send()
            .await
            .map_err(|e| {
                AppError::Auth(format!(
                    "Cannot connect to IB Gateway at {}. Make sure IB Gateway or TWS is running. Error: {}",
                    base_url, e
                ))
            })?;

        if !response.status().is_success() {
            return Err(AppError::Auth(
                "IB Gateway returned an error. Please authenticate in the gateway first."
                    .to_string(),
            ));
        }

        let auth_status: IbkrAuthStatus = response.json().await?;

        if !auth_status.authenticated {
            return Err(AppError::Auth(
                "Not authenticated with IB Gateway. Please log in through the IB Gateway web interface first."
                    .to_string(),
            ));
        }

        if auth_status.competing {
            tracing::warn!("IB Gateway session is competing with another session");
        }

        // Get accounts
        let accounts_resp = self
            .client
            .get(format!("{}/v1/api/portfolio/accounts", base_url))
            .send()
            .await?;

        if !accounts_resp.status().is_success() {
            return Err(AppError::Auth("Failed to fetch IB accounts".to_string()));
        }

        let accounts: Vec<IbkrAccount> = accounts_resp.json().await.unwrap_or_default();
        let account = accounts
            .first()
            .ok_or_else(|| AppError::Auth("No IB accounts found".to_string()))?;

        // Auth token stores the gateway URL for subsequent requests
        Ok(AuthResponse {
            auth_token: format!("{}|{}", base_url, account.account_id),
            feed_token: None,
            user_id: account.account_id.clone(),
            user_name: Some(if account.desc.is_empty() {
                account.account_type.clone()
            } else {
                account.desc.clone()
            }),
        })
    }

    async fn place_order(&self, auth_token: &str, order: OrderRequest) -> Result<OrderResponse> {
        let (base_url, account_id) = parse_ibkr_auth(auth_token)?;

        let symbol = order
            .broker_symbol
            .as_deref()
            .unwrap_or(&order.symbol);

        // IBKR requires conid (contract ID). If we have a symbol_token, use it as conid.
        let conid = order
            .symbol_token
            .as_deref()
            .and_then(|t| t.parse::<i64>().ok())
            .unwrap_or(0);

        if conid == 0 {
            return Err(AppError::Validation(format!(
                "Contract ID (conid) required for IBKR orders. Symbol: {}",
                symbol
            )));
        }

        let mut order_obj = serde_json::json!({
            "conid": conid,
            "orderType": map_order_type(&order.order_type),
            "side": order.side.to_uppercase(),
            "quantity": order.quantity,
            "tif": map_validity(&order.validity),
            "price": order.price,
            "auxPrice": order.trigger_price.unwrap_or(0.0),
            "outsideRTH": order.amo,
        });
        // Trailing stop parameters
        if order.order_type == "TRAILING_STOP" {
            if let Some(tp) = order.trail_price {
                if tp > 0.0 {
                    order_obj["trailingType"] = serde_json::json!("amt");
                    order_obj["trailingAmt"] = serde_json::json!(tp);
                }
            } else if let Some(tp) = order.trail_percent {
                if tp > 0.0 {
                    order_obj["trailingType"] = serde_json::json!("%");
                    order_obj["trailingAmt"] = serde_json::json!(tp);
                }
            }
        }
        let order_body = serde_json::json!({
            "orders": [order_obj]
        });

        let response = self
            .client
            .post(format!(
                "{}/v1/api/iserver/account/{}/orders",
                base_url, account_id
            ))
            .json(&order_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Order failed: {}", error_body)));
        }

        let replies: Vec<IbkrOrderReply> = response.json().await?;
        let reply = replies
            .first()
            .ok_or_else(|| AppError::Broker("No order reply".to_string()))?;

        // IBKR may return a confirmation request — auto-confirm
        if reply.id.is_some() && reply.message.is_some() {
            // Need to confirm the order
            let confirm_resp = self
                .client
                .post(format!(
                    "{}/v1/api/iserver/reply/{}",
                    base_url,
                    reply.id.as_deref().unwrap_or("")
                ))
                .json(&serde_json::json!({"confirmed": true}))
                .send()
                .await?;

            if confirm_resp.status().is_success() {
                let confirm_replies: Vec<IbkrOrderReply> =
                    confirm_resp.json().await.unwrap_or_default();
                if let Some(confirmed) = confirm_replies.first() {
                    return Ok(OrderResponse {
                        order_id: confirmed.order_id.clone(),
                        message: Some(
                            confirmed
                                .order_status
                                .clone(),
                        ),
                    });
                }
            }
        }

        Ok(OrderResponse {
            order_id: reply.order_id.clone(),
            message: Some(reply.order_status.clone()),
        })
    }

    async fn modify_order(
        &self,
        auth_token: &str,
        order_id: &str,
        order: ModifyOrderRequest,
    ) -> Result<OrderResponse> {
        let (base_url, account_id) = parse_ibkr_auth(auth_token)?;

        let mut body = serde_json::Map::new();
        if let Some(qty) = order.quantity {
            body.insert("quantity".to_string(), serde_json::json!(qty));
        }
        if let Some(price) = order.price {
            body.insert("price".to_string(), serde_json::json!(price));
        }
        if let Some(trigger) = order.trigger_price {
            body.insert("auxPrice".to_string(), serde_json::json!(trigger));
        }
        if let Some(ref ot) = order.order_type {
            body.insert(
                "orderType".to_string(),
                serde_json::json!(map_order_type(ot)),
            );
        }

        let response = self
            .client
            .post(format!(
                "{}/v1/api/iserver/account/{}/order/{}",
                base_url, account_id, order_id
            ))
            .json(&serde_json::Value::Object(body))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Modify failed: {}", error_body)));
        }

        Ok(OrderResponse {
            order_id: order_id.to_string(),
            message: Some("Order modified".to_string()),
        })
    }

    async fn cancel_order(
        &self,
        auth_token: &str,
        order_id: &str,
        _variety: Option<&str>,
    ) -> Result<()> {
        let (base_url, account_id) = parse_ibkr_auth(auth_token)?;

        let response = self
            .client
            .delete(format!(
                "{}/v1/api/iserver/account/{}/order/{}",
                base_url, account_id, order_id
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Cancel failed: {}", error_body)));
        }

        Ok(())
    }

    async fn get_order_book(&self, auth_token: &str) -> Result<Vec<Order>> {
        let (base_url, _) = parse_ibkr_auth(auth_token)?;

        let response = self
            .client
            .get(format!("{}/v1/api/iserver/account/orders", base_url))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get orders: {}", error_body)));
        }

        let orders_resp: IbkrLiveOrdersResponse = response.json().await?;

        Ok(orders_resp
            .orders
            .into_iter()
            .map(|o| {
                Order {
                    order_id: o.order_id.to_string(),
                    exchange_order_id: None,
                    symbol: o.ticker,
                    exchange: "US".to_string(),
                    side: map_ibkr_side(&o.side),
                    quantity: o.total_size as f64,
                    filled_quantity: o.filled_quantity as f64,
                    pending_quantity: o.remaining_quantity as f64,
                    price: o.price,
                    trigger_price: o.aux_price,
                    average_price: o.avg_price,
                    order_type: map_order_type_back(&o.order_type),
                    product: "CNC".to_string(),
                    status: map_ibkr_order_status(&o.status),
                    validity: o.tif.to_uppercase(),
                    order_timestamp: o.last_execution_time.clone(),
                    exchange_timestamp: if o.last_execution_time.is_empty() {
                        None
                    } else {
                        Some(o.last_execution_time)
                    },
                    rejection_reason: o.order_desc,
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
        let (base_url, account_id) = parse_ibkr_auth(auth_token)?;

        let response = self
            .client
            .get(format!(
                "{}/v1/api/portfolio/{}/positions/0",
                base_url, account_id
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get positions: {}", error_body)));
        }

        let positions: Vec<IbkrPosition> = response.json().await?;

        Ok(positions
            .into_iter()
            .map(|p| {
                let qty = p.position as f64;
                let (buy_qty, buy_val, sell_qty, sell_val) = if qty > 0.0 {
                    (qty, p.avg_cost * qty, 0.0, 0.0)
                } else {
                    (0.0, 0.0, qty.abs(), p.avg_cost * qty.abs())
                };

                Position {
                    symbol: p.ticker.unwrap_or(p.contract_desc),
                    exchange: p.listing_exchange.unwrap_or_else(|| "US".to_string()),
                    product: "CNC".to_string(),
                    quantity: qty,
                    overnight_quantity: qty,
                    average_price: p.avg_price,
                    ltp: p.mkt_price,
                    pnl: p.unrealized_pnl + p.realized_pnl,
                    realized_pnl: p.realized_pnl,
                    unrealized_pnl: p.unrealized_pnl,
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
        let (base_url, account_id) = parse_ibkr_auth(auth_token)?;

        let response = self
            .client
            .get(format!(
                "{}/v1/api/portfolio/{}/summary",
                base_url, account_id
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Failed to get account: {}", error_body)));
        }

        let summary: IbkrAccountSummary = response.json().await?;

        Ok(Funds {
            available_cash: summary.total_cash.map(|v| v.amount).unwrap_or(0.0),
            used_margin: summary
                .maintenance_margin
                .map(|v| v.amount)
                .unwrap_or(0.0),
            total_margin: summary.buying_power.map(|v| v.amount).unwrap_or(0.0),
            opening_balance: summary.net_liquidation.map(|v| v.amount).unwrap_or(0.0),
            payin: 0.0,
            payout: 0.0,
            span: summary
                .maintenance_margin
                .map(|v| v.amount)
                .unwrap_or(0.0),
            exposure: summary
                .gross_position_value
                .map(|v| v.amount)
                .unwrap_or(0.0),
            collateral: summary
                .excess_liquidity
                .map(|v| v.amount)
                .unwrap_or(0.0),
        })
    }

    async fn get_quote(
        &self,
        auth_token: &str,
        symbols: Vec<(String, String)>,
    ) -> Result<Vec<Quote>> {
        let (base_url, _) = parse_ibkr_auth(auth_token)?;

        // IBKR requires conids for snapshots — need to search first
        let mut quotes = Vec::new();

        for (symbol, _) in &symbols {
            // Search for conid
            let search_resp = self
                .client
                .get(format!(
                    "{}/v1/api/iserver/secdef/search?symbol={}&secType=STK",
                    base_url, symbol
                ))
                .send()
                .await;

            let conid = match search_resp {
                Ok(resp) if resp.status().is_success() => {
                    let results: Vec<IbkrSearchResult> = resp.json().await.unwrap_or_default();
                    results.first().map(|r| r.conid).unwrap_or(0)
                }
                _ => 0,
            };

            if conid == 0 {
                tracing::warn!("Could not find conid for {}", symbol);
                continue;
            }

            // Request snapshot
            let snap_resp = self
                .client
                .get(format!(
                    "{}/v1/api/iserver/marketdata/snapshot?conids={}&fields=31,84,85,86,87,7295,7296,7297,7291,7282,7283,7762,55",
                    base_url, conid
                ))
                .send()
                .await;

            match snap_resp {
                Ok(resp) if resp.status().is_success() => {
                    let snapshots: Vec<IbkrSnapshot> = resp.json().await.unwrap_or_default();
                    if let Some(snap) = snapshots.first() {
                        let ltp = parse_ibkr_num(snap.last.as_deref());
                        let close = parse_ibkr_num(snap.close.as_deref());
                        let change = parse_ibkr_num(snap.change.as_deref());
                        let change_pct = parse_ibkr_num(snap.change_pct.as_deref());

                        quotes.push(Quote {
                            symbol: snap
                                .symbol
                                .clone()
                                .unwrap_or_else(|| symbol.clone()),
                            exchange: "US".to_string(),
                            ltp,
                            open: parse_ibkr_num(snap.open.as_deref()),
                            high: parse_ibkr_num(snap.high.as_deref()),
                            low: parse_ibkr_num(snap.low.as_deref()),
                            close,
                            volume: parse_ibkr_num(snap.volume.as_deref()) as i64,
                            bid: parse_ibkr_num(snap.bid.as_deref()),
                            ask: parse_ibkr_num(snap.ask.as_deref()),
                            bid_qty: parse_ibkr_num(snap.bid_size.as_deref()) as i32,
                            ask_qty: parse_ibkr_num(snap.ask_size.as_deref()) as i32,
                            oi: 0,
                            change,
                            change_percent: change_pct,
                            timestamp: String::new(),
                        });
                    }
                }
                _ => {
                    tracing::warn!("Failed to get snapshot for conid {}", conid);
                }
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

    async fn download_master_contract(&self, _auth_token: &str) -> Result<Vec<SymbolData>> {
        // IBKR doesn't support bulk symbol download
        // Symbols are searched on-demand via iserver/secdef/search
        tracing::info!("IBKR master contract: symbols are searched on-demand");
        Ok(Vec::new())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_ibkr_auth(auth_token: &str) -> Result<(String, String)> {
    // Format: "base_url|account_id"
    let parts: Vec<&str> = auth_token.splitn(2, '|').collect();
    if parts.len() != 2 {
        return Err(AppError::Auth("Invalid IBKR auth token format".to_string()));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn parse_ibkr_num(s: Option<&str>) -> f64 {
    s.and_then(|v| {
        // IBKR sometimes returns "C123.45" or "H123.45" with prefixes
        let cleaned = v.trim_start_matches(|c: char| !c.is_ascii_digit() && c != '-' && c != '.');
        cleaned.parse().ok()
    })
    .unwrap_or(0.0)
}

fn map_order_type(ot: &str) -> &str {
    match ot {
        "MARKET" => "MKT",
        "LIMIT" => "LMT",
        "SL" => "STP LMT",
        "SL-M" => "STP",
        "TRAILING_STOP" => "TRAIL",
        _ => "MKT",
    }
}

fn map_order_type_back(ot: &str) -> String {
    match ot {
        "MKT" => "MARKET",
        "LMT" => "LIMIT",
        "STP LMT" | "STP_LMT" => "SL",
        "STP" => "SL-M",
        "TRAIL" => "TRAILING_STOP",
        _ => "MARKET",
    }
    .to_string()
}

fn map_validity(v: &str) -> &str {
    match v {
        "DAY" => "DAY",
        "IOC" => "IOC",
        "GTC" => "GTC",
        _ => "DAY",
    }
}

fn map_ibkr_side(side: &str) -> String {
    match side.to_uppercase().as_str() {
        "BUY" | "BOT" | "B" => "BUY",
        "SELL" | "SLD" | "S" => "SELL",
        _ => side,
    }
    .to_string()
}

fn map_ibkr_order_status(status: &str) -> String {
    match status {
        "Submitted" | "PreSubmitted" => "OPEN",
        "Filled" => "COMPLETE",
        "Cancelled" | "Inactive" | "ApiCancelled" => "CANCELLED",
        "PendingSubmit" | "PendingCancel" => "PENDING",
        _ => "UNKNOWN",
    }
    .to_string()
}
