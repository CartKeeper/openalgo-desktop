//! Order management commands

use crate::brokers::types::{Order, OrderRequest, ModifyOrderRequest};
use crate::error::Result;
use crate::services::strategy_basket_service;
use crate::services::{LegResult, OrderService, OrderbookService, PlaceOrderResult, SmartOrderService, StrategyLeg};
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub success: bool,
    pub order_id: String,
    pub message: Option<String>,
}

/// Place a new order
///
/// Routes to sandbox in analyze mode.
#[tauri::command]
pub async fn place_order(
    state: State<'_, AppState>,
    order: OrderRequest,
    expected_analyze_mode: Option<bool>,
) -> Result<OrderResponse> {
    tracing::info!("Placing order: {:?}", order);

    let result = OrderService::place_order(&state, order, None, expected_analyze_mode).await?;
    tracing::info!("Order placed in {} mode", result.mode);

    Ok(OrderResponse {
        success: result.success,
        order_id: result.order_id.unwrap_or_default(),
        message: Some(result.message),
    })
}

/// Modify an existing order
///
/// Routes to sandbox in analyze mode.
#[tauri::command]
pub async fn modify_order(
    state: State<'_, AppState>,
    order_id: String,
    order: ModifyOrderRequest,
) -> Result<OrderResponse> {
    tracing::info!("Modifying order {}: {:?}", order_id, order);

    let result = OrderService::modify_order(&state, &order_id, order, None).await?;

    Ok(OrderResponse {
        success: result.success,
        order_id: result.order_id,
        message: Some(result.message),
    })
}

/// Cancel an order
///
/// Routes to sandbox in analyze mode.
#[tauri::command]
pub async fn cancel_order(
    state: State<'_, AppState>,
    order_id: String,
    variety: Option<String>,
) -> Result<OrderResponse> {
    tracing::info!("Cancelling order: {}", order_id);

    let result = OrderService::cancel_order(&state, &order_id, variety.as_deref(), None).await?;

    Ok(OrderResponse {
        success: result.success,
        order_id: result.order_id,
        message: Some(result.message),
    })
}

/// Get order book
///
/// Routes to sandbox orders in analyze mode.
#[tauri::command]
pub async fn get_order_book(state: State<'_, AppState>) -> Result<Vec<Order>> {
    let result = OrderbookService::get_orderbook(&state, None).await?;
    tracing::info!("Order book retrieved in {} mode", result.mode);
    Ok(result.orders)
}

/// Get open (working) orders
///
/// Routes to sandbox open orders in analyze mode. Unlike the order book, this
/// is not limited to the most recent orders, so an old working GTC stop is
/// always listed.
#[tauri::command]
pub async fn get_open_orders(state: State<'_, AppState>) -> Result<Vec<Order>> {
    let result = OrderbookService::get_open_orders(&state, None).await?;
    tracing::info!("Open orders retrieved in {} mode", result.mode);
    Ok(result.orders)
}

/// Get trade book
///
/// Routes to sandbox trades in analyze mode.
#[tauri::command]
pub async fn get_trade_book(state: State<'_, AppState>) -> Result<Vec<Order>> {
    let result = OrderbookService::get_tradebook(&state, None).await?;
    tracing::info!("Trade book retrieved in {} mode", result.mode);
    Ok(result.trades)
}

/// Place a basket of orders
///
/// Routes each order through OrderService (sandbox in analyze mode).
#[tauri::command]
pub async fn place_basket_order(
    state: State<'_, AppState>,
    orders: Vec<OrderRequest>,
    expected_analyze_mode: Option<bool>,
) -> Result<Vec<PlaceOrderResult>> {
    tracing::info!("Placing basket order: {} orders", orders.len());
    SmartOrderService::place_basket_order(&state, orders, None, expected_analyze_mode).await
}

/// Place a strategy basket (briefing rotations and protections)
///
/// Legs run in groups, in cancel -> sell -> buy -> stop order, each step
/// waiting for the previous one to take effect. A failed leg stops its group.
#[tauri::command]
pub async fn place_strategy_basket(
    state: State<'_, AppState>,
    legs: Vec<StrategyLeg>,
    expected_analyze_mode: Option<bool>,
) -> Result<Vec<LegResult>> {
    tracing::info!("Placing strategy basket: {} legs", legs.len());
    strategy_basket_service::place_strategy_basket(&state, legs, expected_analyze_mode).await
}
