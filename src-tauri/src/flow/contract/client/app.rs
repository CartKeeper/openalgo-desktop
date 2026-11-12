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
/// internal token).
pub struct AppFlowClient {
    app: Weak<AppState>,
    api_key: Secret,
}

impl AppFlowClient {
    pub fn new(app: Weak<AppState>, api_key: Secret) -> Self {
        Self { app, api_key }
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

#[async_trait]
impl FlowClient for AppFlowClient {
    async fn api(&self, call: ApiCall) -> Value {
        let Some(app) = self.app.upgrade() else {
            return json!({"status": "error", "error": "OpenAlgo is shutting down.", "code": 503});
        };
        // Strike-mode legs and a scoped close: the public schemas cannot
        // carry them, so they take Flow's internal entry (nodes-orders).
        if let Some(which) = crate::flow::nodes::internal::route(&call) {
            let key = self.api_key.expose();
            return match crate::flow::nodes::internal::run(&app, key, which, call.payload).await {
                Ok(raw) => with_broker_cause(&app, shaped(raw)),
                Err(t) => transport_failure(t),
            };
        }
        let req = match dispatch::post_request(self.api_key.expose(), call.endpoint, call.payload) {
            Ok(r) => r,
            Err(t) => return transport_failure(t),
        };
        match dispatch::run(&app, req, CALL_TIMEOUT).await {
            Ok(raw) => with_broker_cause(&app, shaped(raw)),
            Err(t) => {
                tracing::warn!("Flow {} got no answer: {:?}", call.method, t);
                transport_failure(t)
            }
        }
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

    #[test]
    fn the_alert_text_is_stamped_in_market_time() {
        // 2026-10-30 13:30:05Z is 09:30:05 in New York.
        let text = telegram_text("2026-10-30T13:30:05Z".parse().unwrap(), "hello");
        assert!(text.starts_with("*Flow Alert*\n"));
        assert!(text.ends_with("hello\n\nTime: 09:30:05"), "{}", text);
    }
}
