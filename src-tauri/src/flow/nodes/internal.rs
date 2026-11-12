//! The two order calls Flow makes that the public `/api/v1` schemas do not
//! carry (upstream sends them to its services through an internal entry):
//!
//! - a multi-leg order with a leg that names an absolute `strike` (the leg
//!   schema requires `offset`), parity N-optionsMultiOrder-12;
//! - a Close Positions node naming one position (`/api/v1/closeposition`
//!   takes only a strategy and closes everything).
//!
//! In this app a scoped close is served through the same `/api/v1`
//! handlers every other call takes (`openposition`, then the opposite
//! `placeorder`), so the rate limiter, analyzer routing and Gate B apply;
//! absolute-strike legs have no service here and are refused with one
//! sentence.

use crate::flow::contract::client::app::CALL_TIMEOUT;
use crate::flow::contract::client::{ApiCall, DEFAULT_STRATEGY};
use crate::flow::contract::pyfmt;
use crate::flow::support::dispatch::{self, Raw, Transport};
use crate::state::AppState;
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// Which internal entry a call takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Internal {
    /// `optionsmultiorder` with at least one leg naming a `strike`.
    StrikeLegs,
    /// `closeposition` naming a `symbol`.
    ScopedClose,
}

/// What a strike leg answers in this app.
pub const STRIKE_LEGS_NOT_AVAILABLE: &str = "Legs with an absolute strike are not available in this app: use an offset (ATM, ITM1, OTM1, ...) instead.";

/// The internal entry `call` needs, if any.
pub fn route(call: &ApiCall) -> Option<Internal> {
    match call.endpoint {
        "optionsmultiorder" => call
            .payload
            .get("legs")
            .and_then(Value::as_array)
            .is_some_and(|legs| legs.iter().any(has_strike))
            .then_some(Internal::StrikeLegs),
        "closeposition" => call
            .payload
            .get("symbol")
            .is_some_and(|s| !s.is_null())
            .then_some(Internal::ScopedClose),
        _ => None,
    }
}

fn has_strike(leg: &Value) -> bool {
    leg.get("strike").is_some_and(|s| !s.is_null())
}

/// Send `payload` through `which`, with the run's credential.
pub async fn run(
    app: &Arc<AppState>,
    api_key: &str,
    which: Internal,
    payload: Map<String, Value>,
) -> Result<Raw, Transport> {
    match which {
        Internal::StrikeLegs => Ok(Raw::new(
            400,
            json!({"status": "error", "message": STRIKE_LEGS_NOT_AVAILABLE}).to_string(),
        )),
        Internal::ScopedClose => scoped_close(app, api_key, payload).await,
    }
}

/// A close of one position: `/api/v1/openposition` for its quantity, then
/// the opposite market order through `/api/v1/placeorder`; a flat symbol
/// places nothing and answers as the position service does.
async fn scoped_close(
    app: &Arc<AppState>,
    api_key: &str,
    payload: Map<String, Value>,
) -> Result<Raw, Transport> {
    let text = |k: &str, default: &str| {
        payload
            .get(k)
            .filter(|v| pyfmt::truthy(v))
            .map(pyfmt::str)
            .unwrap_or_else(|| default.to_string())
    };
    let strategy = text("strategy", DEFAULT_STRATEGY);
    let symbol = text("symbol", "");
    let exchange = text("exchange", "");
    let product = text("product", "CNC");
    let mut ask = Map::new();
    ask.insert("strategy".into(), json!(strategy));
    ask.insert("symbol".into(), json!(symbol));
    ask.insert("exchange".into(), json!(exchange));
    ask.insert("product".into(), json!(product));
    let open = dispatch::run(app, dispatch::post_request(api_key, "openposition", ask)?, CALL_TIMEOUT).await?;
    if open.status != 200 {
        return Ok(open);
    }
    let body: Value = serde_json::from_str(&open.text).unwrap_or(Value::Null);
    let quantity = body["data"]["quantity"].as_i64().unwrap_or(0);
    if quantity == 0 {
        return Ok(Raw::new(
            200,
            json!({"status": "success", "message": "Position already closed"}).to_string(),
        ));
    }
    let (action, quantity) = if quantity > 0 {
        ("SELL", quantity)
    } else {
        ("BUY", -quantity)
    };
    let mut order = Map::new();
    for (k, v) in [
        ("strategy", json!(strategy)),
        ("symbol", json!(symbol)),
        ("exchange", json!(exchange)),
        ("action", json!(action)),
        ("quantity", json!(quantity)),
        ("pricetype", json!("MARKET")),
        ("product", json!(product)),
        ("price", json!(0)),
        ("trigger_price", json!(0)),
        ("disclosed_quantity", json!(0)),
    ] {
        order.insert(k.into(), v);
    }
    dispatch::run(app, dispatch::post_request(api_key, "placeorder", order)?, CALL_TIMEOUT).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_strike_legs_and_a_scoped_close_take_the_internal_entry() {
        let strike = ApiCall::new(
            "place_options_multi_order",
            "optionsmultiorder",
            json!({"legs": [{"offset": "ATM"}, {"strike": 24000}]}),
        );
        assert_eq!(route(&strike), Some(Internal::StrikeLegs));
        let offsets = ApiCall::new(
            "place_options_multi_order",
            "optionsmultiorder",
            json!({"legs": [{"offset": "ATM"}, {"strike": null}]}),
        );
        assert_eq!(route(&offsets), None);
        let scoped = ApiCall::new("close_position", "closeposition", json!({"symbol": "AAPL"}));
        assert_eq!(route(&scoped), Some(Internal::ScopedClose));
        let all = ApiCall::new("close_position", "closeposition", json!({"strategy": "x"}));
        assert_eq!(route(&all), None);
        let quote = ApiCall::new("quotes", "quotes", json!({"symbol": "AAPL"}));
        assert_eq!(route(&quote), None);
    }

    use crate::brokers::BrokerRegistry;
    use crate::state::testing::{build, TestCtx};

    fn sandbox() -> TestCtx {
        let t = build(BrokerRegistry::new(), chrono::Utc::now());
        t.ctx.sqlite.set_analyze_mode(true).unwrap();
        t
    }

    fn body(raw: &Raw) -> Value {
        serde_json::from_str(&raw.text).unwrap_or_else(|e| panic!("{}: {}", e, raw.text))
    }

    /// Absolute-strike legs have no service in this app: one sentence, 400.
    #[tokio::test]
    async fn strike_legs_are_refused_with_one_sentence() {
        let t = sandbox();
        let token = t.ctx.internal_token.clone();
        let raw = run(&t.ctx, &token, Internal::StrikeLegs, Map::new())
            .await
            .unwrap();
        assert_eq!(raw.status, 400);
        assert_eq!(body(&raw)["message"], STRIKE_LEGS_NOT_AVAILABLE);
    }

    /// A scoped close of a symbol with no position answers as the position
    /// service does, placing nothing.
    #[tokio::test]
    async fn a_scoped_close_of_a_flat_symbol_places_nothing() {
        let t = sandbox();
        let token = t.ctx.internal_token.clone();
        let payload = json!({"strategy": "flow_workflow", "symbol": "AAPL", "exchange": "NASDAQ", "product": "CNC"});
        let raw = run(&t.ctx, &token, Internal::ScopedClose, payload.as_object().unwrap().clone())
            .await
            .unwrap();
        assert_eq!(raw.status, 200, "{}", raw.text);
        assert_eq!(body(&raw)["status"], "success");
        assert_eq!(body(&raw)["message"], "Position already closed");
        assert!(t.ctx.sqlite.get_sandbox_orders().unwrap().is_empty());
    }

    /// A scoped close of a held position places the opposite market order
    /// through `/api/v1/placeorder`, in the sandbox under analyzer mode.
    #[tokio::test]
    async fn a_scoped_close_places_the_opposite_order_through_the_handlers() {
        let t = sandbox();
        t.ctx
            .sqlite
            .place_sandbox_order("AAPL", "NASDAQ", "BUY", 2, 190.0, "MARKET", "CNC")
            .unwrap();
        let token = t.ctx.internal_token.clone();
        let payload = json!({"strategy": "flow_workflow", "symbol": "AAPL", "exchange": "NASDAQ", "product": "CNC"});
        let raw = run(&t.ctx, &token, Internal::ScopedClose, payload.as_object().unwrap().clone())
            .await
            .unwrap();
        assert_eq!(raw.status, 200, "{}", raw.text);
        assert_eq!(body(&raw)["status"], "success");
        let orders = t.ctx.sqlite.get_sandbox_orders().unwrap();
        assert_eq!(orders.len(), 2, "{:?}", orders);
        let close = orders.iter().find(|o| o.side == "SELL").expect("the closing sell");
        assert_eq!(close.quantity, 2);
    }
}
