//! The in-app [`FlowClient`]: this app's `/api/v1` handler stack in
//! process (`flow::support::dispatch` over the app's axum router), with the
//! app's per-process internal token, so a node gets the same key check,
//! rate limit, analyzer routing, Gate B and reply shapes as any API client,
//! and never a socket.
//!
//! Upstream reaches its handlers through `mcp::dispatch::run_as` with the
//! trader's API key, and sends its alerts through its Telegram and WhatsApp
//! services. Here the alerts are answered by WP8 Phase C (Telegram) and
//! refused with one sentence (WhatsApp, not available in this app).

use super::{handle_response, ApiCall, FlowClient, UNKNOWN_OUTCOME};
use crate::flow::support::dispatch::{self, Raw, Transport};
use crate::security::Secret;
use crate::state::AppState;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Weak;
use std::time::Duration;

/// How long a node waits for one call. A slow broker never cancels an
/// order mid-flight: the call finishes in a task the app owns, and the node
/// is told the outcome is unknown.
pub const CALL_TIMEOUT: Duration = dispatch::SDK_TIMEOUT;

/// What a Telegram alert answers until the Telegram settings exist (WP8
/// Phase C, Task C5 replaces this with the real send).
pub const TELEGRAM_NOT_SET_UP: &str =
    "Telegram is not set up: add the bot token and chat id on the Telegram page";

/// What a WhatsApp alert answers in this app.
pub const WHATSAPP_NOT_AVAILABLE: &str = "WhatsApp alerts are not available in this app";

/// The real client for one run: the app and the run's credential (the
/// internal token), and the workflow whose run it serves, if any.
pub struct AppFlowClient {
    app: Weak<AppState>,
    api_key: Secret,
    workflow_id: Option<i64>,
}

impl AppFlowClient {
    pub fn new(app: Weak<AppState>, api_key: Secret) -> Self {
        Self {
            app,
            api_key,
            workflow_id: None,
        }
    }

    /// The client of a run of `workflow_id`: the orders it places are
    /// remembered as that workflow's own, which its order-update watch
    /// skips (WP8 Phase D, decision 6d).
    pub fn for_workflow(mut self, workflow_id: i64) -> Self {
        self.workflow_id = Some(workflow_id);
        self
    }

    /// Remember the orders a placing call's reply names as the run's
    /// workflow's own.
    fn remember_orders(&self, app: &AppState, endpoint: &str, reply: &Value) {
        let Some(workflow_id) = self.workflow_id else {
            return;
        };
        if !crate::flow::triggers::own_orders::PLACING.contains(&endpoint) {
            return;
        }
        let ids = crate::flow::triggers::own_orders::order_ids(reply);
        if !ids.is_empty() {
            app.flow.own_orders.record(workflow_id, ids);
        }
    }
}

/// What a node reads when its call got no answer: it may have taken
/// effect, so an order is never sent again blindly (parity CL-26, D-04).
pub const NO_ANSWER: &str = "OpenAlgo did not answer in time. If this was an order, it may have been placed: check the order book before running the workflow again.";

/// What a node reads when its call could not be sent at all.
pub const NOT_SENT: &str = "The request could not be sent to OpenAlgo, so nothing was changed.";

/// A call that never got an answer, as a client dict. Anything handed to
/// the handlers may have taken effect, so it is marked unknown.
fn transport_failure(t: Transport) -> Value {
    match t {
        Transport::NotSent => json!({"status": "error", "error": NOT_SENT, "code": 500}),
        Transport::Timeout => {
            json!({"status": "error", "error": NO_ANSWER, "code": 504, "error_type": UNKNOWN_OUTCOME})
        }
        Transport::NoReply | Transport::Budget => {
            json!({"status": "error", "error": NO_ANSWER, "code": 500, "error_type": UNKNOWN_OUTCOME})
        }
    }
}

/// The client dict for what the handlers answered.
fn shaped(raw: Raw) -> Value {
    let body: Value = match serde_json::from_str(&raw.text) {
        Ok(v @ Value::Object(_)) => v,
        _ => {
            tracing::error!("Flow could not read an API reply (HTTP {})", raw.status);
            return json!({
                "status": "error",
                "error": "OpenAlgo answered in a form Flow could not read.",
                "code": raw.status,
            });
        }
    };
    let mut out = handle_response(raw.status, &body);
    if raw.outcome_unknown {
        if let Some(m) = out.as_object_mut() {
            m.insert("status".into(), json!("error"));
            m.entry("error")
                .or_insert_with(|| body.get("message").cloned().unwrap_or(Value::Null));
            m.insert("code".into(), json!(raw.status));
            m.insert("error_type".into(), json!(UNKNOWN_OUTCOME));
        }
    }
    out
}

/// A broker call answered while no broker is logged in: this app's
/// handlers refuse with the key sentence (403) or the service's own
/// "Broker not connected" (500), either of which would send a trader to
/// check the wrong thing. A run states the real cause at the node instead
/// (upstream lead decision 16): the no-broker-session sentence.
fn with_broker_cause(app: &AppState, mut out: Value) -> Value {
    let error = out.get("error").and_then(Value::as_str).unwrap_or_default();
    let refused = error.starts_with(crate::flow::support::reply::INVALID_API_KEY)
        || error.contains("Broker not connected");
    if refused && app.get_broker_session().is_none() {
        out["error"] = json!(crate::flow::support::NO_BROKER_SESSION);
    }
    out
}

/// The brokers whose contracts and margin service the web's Indian
/// lookups read (this app's Indian adapters).
const INDIAN_BROKERS: &[&str] = &["zerodha", "angel", "fyers"];

/// The calls that read an Indian broker's option contracts when their
/// exchange is an Indian one (an index's options, expiries, chain).
const CONTRACT_LOOKUPS: &[&str] = &[
    "optionsorder",
    "optionsmultiorder",
    "optionsymbol",
    "optionchain",
    "syntheticfuture",
    "expiry",
];

/// The node's error for a call that needs an Indian broker while another
/// broker is logged in (WP8 Global Constraints: margin and index lookups
/// that need an Indian broker answer a clear node error, never an empty
/// list or a margin of zero). `None` lets the call through: an Indian
/// broker, no broker at all (the handlers name that cause), or a US or
/// crypto exchange.
fn needs_an_indian_broker(app: &AppState, call: &ApiCall) -> Option<Value> {
    let session = app.get_broker_session()?;
    if INDIAN_BROKERS.contains(&session.broker_id.as_str()) {
        return None;
    }
    let broker = app
        .brokers
        .get(&session.broker_id)
        .map(|b| b.name().to_string())
        .unwrap_or(session.broker_id);
    let tail = format!(
        "log in to Zerodha, Angel One or Fyers. The broker logged in now ({}) trades US markets in this app.",
        broker
    );
    let refusal = |message: String| json!({"status": "error", "error": message, "code": 400});
    if call.endpoint == "margin" {
        return Some(refusal(format!(
            "Margin needs an Indian broker's margin calculator: {}",
            tail
        )));
    }
    if !CONTRACT_LOOKUPS.contains(&call.endpoint) {
        return None;
    }
    let field = |k: &str| {
        call.payload
            .get(k)
            .and_then(Value::as_str)
            .map(|v| v.trim().to_uppercase())
            .unwrap_or_default()
    };
    let exchange = field("exchange");
    if exchange.is_empty() || exchange == "CRYPTO" || crate::flow::support::is_us_venue(&exchange) {
        return None;
    }
    let mut what = field("underlying");
    if what.is_empty() {
        what = field("symbol");
    }
    Some(refusal(format!(
        "{} on {} needs an Indian broker: {}",
        what, exchange, tail
    )))
}

#[async_trait]
impl FlowClient for AppFlowClient {
    async fn api(&self, call: ApiCall) -> Value {
        let Some(app) = self.app.upgrade() else {
            return json!({"status": "error", "error": "OpenAlgo is shutting down.", "code": 503});
        };
        if let Some(refusal) = needs_an_indian_broker(&app, &call) {
            tracing::info!("Flow {}: {}", call.method, refusal["error"]);
            return refusal;
        }
        // Strike-mode legs and a scoped close: the public schemas cannot
        // carry them, so they take Flow's internal entry (nodes-orders).
        let endpoint = call.endpoint;
        if let Some(which) = crate::flow::nodes::internal::route(&call) {
            let key = self.api_key.expose();
            let out = match crate::flow::nodes::internal::run(&app, key, which, call.payload).await
            {
                Ok(raw) => with_broker_cause(&app, shaped(raw)),
                Err(t) => transport_failure(t),
            };
            // A scoped close places its order through the same path.
            self.remember_orders(&app, "placeorder", &out);
            return out;
        }
        let req = match dispatch::post_request(self.api_key.expose(), call.endpoint, call.payload) {
            Ok(r) => r,
            Err(t) => return transport_failure(t),
        };
        let out = match dispatch::run(&app, req, CALL_TIMEOUT).await {
            Ok(raw) => with_broker_cause(&app, shaped(raw)),
            Err(t) => {
                tracing::warn!("Flow {} got no answer: {:?}", call.method, t);
                transport_failure(t)
            }
        };
        self.remember_orders(&app, endpoint, &out);
        out
    }

    async fn telegram(&self, _message: &str) -> Value {
        json!({"status": "error", "error": TELEGRAM_NOT_SET_UP})
    }

    async fn whatsapp(&self, _message: &str, _to: Option<&str>) -> Value {
        json!({"status": "error", "error": WHATSAPP_NOT_AVAILABLE})
    }
}

/// The web's Telegram alert text: a 21-character rule under "Flow Alert",
/// stamped in market time on any machine (the web wrote the host's local
/// time). Phase C's send uses it.
pub fn telegram_text(now: chrono::DateTime<chrono::Utc>, message: &str) -> String {
    let time = now
        .with_timezone(&crate::flow::contract::time::market_tz())
        .format("%H:%M:%S");
    let rule = "\u{2500}".repeat(21);
    format!("*Flow Alert*\n{}\n{}\n\nTime: {}", rule, message, time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_answer_is_marked_unknown_and_an_unsent_one_is_not() {
        let lost = transport_failure(Transport::Timeout);
        assert_eq!(lost["error_type"], UNKNOWN_OUTCOME);
        assert_eq!(lost["code"], 504);
        let unsent = transport_failure(Transport::NotSent);
        assert!(unsent.get("error_type").is_none());
        assert_eq!(unsent["error"], NOT_SENT);
    }

    #[test]
    fn an_unreadable_reply_is_an_error_the_node_can_show() {
        let out = shaped(Raw::new(200, "<html>"));
        assert_eq!(out["status"], "error");
        assert_eq!(out["code"], 200);
    }

    use crate::brokers::BrokerRegistry;
    use crate::flow::contract::client::ApiCall;
    use crate::state::testing::{build, TestCtx};
    use std::sync::Arc;

    /// The harness with analyzer mode set, and the run's client over it.
    fn harness(analyzer: bool) -> (TestCtx, AppFlowClient) {
        let t = build(BrokerRegistry::new(), chrono::Utc::now());
        t.ctx.sqlite.set_analyze_mode(analyzer).unwrap();
        let client = AppFlowClient::new(
            Arc::downgrade(&t.ctx),
            Secret::new(t.ctx.internal_token.clone()),
        );
        (t, client)
    }

    /// Task A4: a node's call reaches this app's own `/api/v1` handlers in
    /// process and reads the web client's dict.
    #[tokio::test]
    async fn funds_are_read_through_the_apps_own_handlers() {
        let (_t, client) = harness(true);
        let out = client.api(ApiCall::new("funds", "funds", json!({}))).await;
        assert_eq!(out["status"], "success", "{}", out);
        assert!(out["data"]["availablecash"].is_number(), "{}", out);
    }

    /// An order goes through `/api/v1/placeorder`: analyzer mode routes it
    /// to the sandbox and the node reads its order id.
    #[tokio::test]
    async fn an_order_placed_through_the_handlers_lands_in_the_sandbox() {
        let (t, client) = harness(true);
        let payload = json!({
            "strategy": "flow_workflow", "symbol": "AAPL", "exchange": "NASDAQ",
            "action": "BUY", "quantity": 1, "pricetype": "MARKET", "product": "CNC",
            "price": 0, "trigger_price": 0, "disclosed_quantity": 0
        });
        let out = client
            .api(ApiCall::new("place_order", "placeorder", payload))
            .await;
        assert_eq!(out["status"], "success", "{}", out);
        assert!(
            out["orderid"].as_str().is_some_and(|id| !id.is_empty()),
            "{}",
            out
        );
        let orders = t.ctx.sqlite.get_sandbox_orders().unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].symbol, "AAPL");
    }

    /// The bypass is gated on the per-process token: a request with
    /// neither the header nor a key is refused by the router, and a client
    /// with the wrong token is refused (403), so nothing but the app's own
    /// runs use it.
    #[tokio::test]
    async fn the_internal_trust_is_gated_on_the_token() {
        let (t, _client) = harness(true);
        let bare = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/funds")
            .header("content-type", "application/json")
            .body(axum::body::Body::from("{}"))
            .unwrap();
        let resp = tower::ServiceExt::oneshot(t.ctx.api_router(), bare)
            .await
            .unwrap();
        assert!(resp.status().is_client_error(), "{}", resp.status());
        let wrong = AppFlowClient::new(Arc::downgrade(&t.ctx), Secret::new("not-the-token"));
        let out = wrong.api(ApiCall::new("funds", "funds", json!({}))).await;
        assert_eq!(out["status"], "error", "{}", out);
        assert_eq!(out["code"], 403, "{}", out);
    }

    /// Review Focus 5: with no broker logged in and analyzer mode off, a
    /// broker-backed call fails with the no-broker sentence, never a key
    /// complaint.
    #[tokio::test]
    async fn a_broker_call_without_a_broker_names_the_cause() {
        let (_t, client) = harness(false);
        let out = client
            .api(ApiCall::new(
                "quotes",
                "quotes",
                json!({"symbol": "AAPL", "exchange": "NASDAQ"}),
            ))
            .await;
        assert_eq!(out["status"], "error", "{}", out);
        assert_eq!(out["error"], crate::flow::support::NO_BROKER_SESSION, "{}", out);
    }

    /// The harness with `broker` logged in (no call reaches the broker in
    /// these tests: the handlers they reach read only the app).
    fn logged_in(broker: &str) -> (TestCtx, AppFlowClient) {
        let (t, client) = harness(false);
        t.ctx.set_broker_session(Some(crate::state::BrokerSession {
            broker_id: broker.into(),
            auth_token: "key:secret".into(),
            feed_token: None,
            user_id: "ACCOUNT1".into(),
            user_name: None,
            authenticated_at: chrono::Utc::now(),
        }));
        (t, client)
    }

    /// Task D2 (Global Constraints): margin needs an Indian broker's margin
    /// calculator. With a US broker logged in the node reads one sentence
    /// naming what to log in to, never a margin of zero.
    #[tokio::test]
    async fn margin_with_a_us_broker_names_the_indian_broker_it_needs() {
        let (_t, client) = logged_in("alpaca");
        let out = client
            .api(ApiCall::new(
                "margin",
                "margin",
                json!({"positions": [{"symbol": "AAPL", "exchange": "NASDAQ", "action": "BUY",
                       "quantity": 1, "product": "CNC", "pricetype": "MARKET"}]}),
            ))
            .await;
        assert_eq!(out["status"], "error", "{}", out);
        assert_eq!(
            out["error"],
            "Margin needs an Indian broker's margin calculator: log in to Zerodha, Angel One or Fyers. The broker logged in now (Alpaca Markets) trades US markets in this app.",
            "{}",
            out
        );
    }

    /// Task D2: an index's option lookup (here its expiries on NFO) needs an
    /// Indian broker's contracts; with a US broker logged in the node says
    /// so instead of answering an empty list.
    #[tokio::test]
    async fn an_index_lookup_with_a_us_broker_names_the_indian_broker_it_needs() {
        let (_t, client) = logged_in("alpaca");
        let out = client
            .api(ApiCall::new(
                "expiry",
                "expiry",
                json!({"symbol": "NIFTY", "exchange": "NFO", "instrumenttype": "options"}),
            ))
            .await;
        assert_eq!(out["status"], "error", "{}", out);
        assert_eq!(
            out["error"],
            "NIFTY on NFO needs an Indian broker: log in to Zerodha, Angel One or Fyers. The broker logged in now (Alpaca Markets) trades US markets in this app.",
            "{}",
            out
        );
    }

    /// Every index contract call with a non-Indian broker logged in reads
    /// the same sentence (a broker this registry does not have, so no call
    /// could ever reach a broker here).
    #[tokio::test]
    async fn every_index_contract_call_names_the_indian_broker_it_needs() {
        let (_t, client) = logged_in("examplebroker");
        let calls = [
            (
                "optionsorder",
                json!({"underlying": "NIFTY", "exchange": "NSE_INDEX"}),
            ),
            (
                "optionsmultiorder",
                json!({"underlying": "NIFTY", "exchange": "NSE_INDEX"}),
            ),
            (
                "optionsymbol",
                json!({"underlying": "BANKNIFTY", "exchange": "NFO"}),
            ),
            (
                "optionchain",
                json!({"underlying": "SENSEX", "exchange": "BSE_INDEX"}),
            ),
            (
                "syntheticfuture",
                json!({"underlying": "GOLDM", "exchange": "MCX"}),
            ),
        ];
        for (endpoint, payload) in calls {
            let underlying = payload["underlying"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let exchange = payload["exchange"].as_str().unwrap_or_default().to_string();
            let out = client.api(ApiCall::new(endpoint, endpoint, payload)).await;
            assert_eq!(out["status"], "error", "{endpoint}: {out}");
            assert_eq!(
                out["error"],
                format!(
                    "{underlying} on {exchange} needs an Indian broker: log in to Zerodha, Angel One or Fyers. The broker logged in now (examplebroker) trades US markets in this app."
                ),
                "{endpoint}"
            );
        }
    }

    /// The gate stops only what needs an Indian broker: an Indian broker's
    /// margin reaches the handler, and so does a US expiry lookup with a US
    /// broker (both answered by the app, no broker call).
    #[tokio::test]
    async fn the_gate_lets_through_what_does_not_need_an_indian_broker() {
        let (_t, client) = logged_in("zerodha");
        let margin = client
            .api(ApiCall::new(
                "margin",
                "margin",
                json!({"positions": [{"symbol": "NIFTY28AUG26FUT", "exchange": "NFO",
                       "action": "BUY", "quantity": 75, "product": "NRML", "pricetype": "MARKET"}]}),
            ))
            .await;
        assert_eq!(margin["status"], "success", "{}", margin);
        let (_t, client) = logged_in("alpaca");
        let expiry = client
            .api(ApiCall::new(
                "expiry",
                "expiry",
                json!({"symbol": "AAPL", "exchange": "NASDAQ", "instrumenttype": "options"}),
            ))
            .await;
        assert_eq!(expiry["status"], "success", "{}", expiry);
    }

    #[test]
    fn the_alert_text_is_stamped_in_market_time() {
        // 2026-10-30 13:30:05Z is 09:30:05 in New York.
        let text = telegram_text("2026-10-30T13:30:05Z".parse().unwrap(), "hello");
        assert!(text.starts_with("*Flow Alert*\n"));
        assert!(text.ends_with("hello\n\nTime: 09:30:05"), "{}", text);
    }
}
