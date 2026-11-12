//! How a Flow node's `/api/v1` call reaches this app's handlers: in
//! process, through the app's own axum router (`webhook::server::router`)
//! with `tower::ServiceExt::oneshot`, never a socket. The request carries
//! the per-process internal token in the `x-openalgo-internal-token`
//! header; the router's internal-trust layer turns that into the request's
//! `apikey`, so the handlers' API-key check, the rate limiter, analyzer
//! routing and Gate B all apply as for any API client.
//!
//! The types are upstream's `mcp::dispatch::{Raw, Transport}` so the
//! carried client (`flow::contract::client::app`) reads them unchanged.

use crate::state::AppState;
use axum::body::Body;
use axum::http::{Method, Request};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

/// The header the internal-trust layer reads.
pub const INTERNAL_TOKEN_HEADER: &str = "x-openalgo-internal-token";

/// The SDK's request timeout (`BaseAPI(timeout=120.0)`).
pub const SDK_TIMEOUT: Duration = Duration::from_secs(120);

/// `error_type` of a call that reached OpenAlgo and whose effect is
/// unknown: an order placement the broker did not confirm, or a call whose
/// answer was lost. Never safe to retry blindly.
pub const UNKNOWN_OUTCOME: &str = "unknown_outcome";

/// Largest `/api/v1` reply a node reads.
pub const MAX_REPLY_BYTES: usize = 64 * 1024 * 1024;

/// What the handler stack answered.
#[derive(Debug, Clone)]
pub struct Raw {
    pub status: u16,
    pub text: String,
    /// The reply is for an order placement whose outcome at the broker is
    /// unknown: it may have been placed.
    pub outcome_unknown: bool,
}

impl Raw {
    pub fn new(status: u16, text: impl Into<String>) -> Self {
        Self {
            status,
            text: text.into(),
            outcome_unknown: false,
        }
    }
}

/// Why no answer arrived ("never sent" and "sent, no answer" are
/// different facts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// No answer within the SDK's timeout; the call may have taken effect.
    Timeout,
    /// The request was never handed to the handlers (it could not be
    /// built): nothing ran, so nothing changed.
    NotSent,
    /// The handlers ran, but no answer came back (the reply could not be
    /// read, the handler failed, or the call was cut off): it may have
    /// taken effect.
    NoReply,
    /// The caller's budget for this minute is spent (unused here; kept so
    /// the carried client matches every variant).
    Budget,
}

/// The SDK's POST: `payload` as the JSON body, the internal token in its
/// header. The web reads the key from the body; here the router's
/// internal-trust layer writes it there from the header, so the token
/// never has to be in a payload a node built.
pub fn post_request(
    token: &str,
    endpoint: &str,
    payload: Map<String, Value>,
) -> Result<Request<Body>, Transport> {
    Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/{}", endpoint))
        .header("content-type", "application/json")
        .header(INTERNAL_TOKEN_HEADER, token)
        .body(Body::from(Value::Object(payload).to_string()))
        .map_err(|_| Transport::NotSent)
}

/// Hand `req` to the app's `/api/v1` router and read the answer within
/// `timeout`. The call runs as a task the app owns, so a node future that
/// is dropped (the run was stopped) never cuts an order off mid-flight:
/// the handler finishes, and only the answer is lost.
pub async fn run(app: &Arc<AppState>, req: Request<Body>, timeout: Duration) -> Result<Raw, Transport> {
    let router = app.api_router();
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.spawn(async move {
        let answer = match router.oneshot(req).await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                match axum::body::to_bytes(resp.into_body(), MAX_REPLY_BYTES).await {
                    Ok(b) => Some(Raw::new(status, String::from_utf8_lossy(&b).into_owned())),
                    Err(e) => {
                        tracing::error!("Flow could not read an API reply: {}", e);
                        None
                    }
                }
            }
            // `Router`'s error type is `Infallible`.
            Err(never) => match never {},
        };
        let _ = tx.send(answer);
    });
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(Some(raw))) => Ok(raw),
        Ok(_) => Err(Transport::NoReply),
        Err(_) => Err(Transport::Timeout),
    }
}

/// The SDK's `_handle_response` as a bare value (used by tests and the
/// goldens' expectations): non-200 becomes `HTTP <code>: <body>`, a 200
/// error body is reduced to its message, anything else passes through.
pub fn shape(raw: &Raw) -> Value {
    if raw.outcome_unknown {
        let message = serde_json::from_str::<Value>(&raw.text)
            .ok()
            .and_then(|v| v.get("message").cloned())
            .unwrap_or_else(|| json!(raw.text));
        return json!({
            "status": "error",
            "message": message,
            "error_type": UNKNOWN_OUTCOME,
            "code": raw.status,
        });
    }
    match serde_json::from_str::<Value>(&raw.text) {
        Ok(v) if raw.status == 200 => v,
        Ok(v) => json!({"status": "error", "message": format!("HTTP {}: {}", raw.status, v), "code": raw.status}),
        Err(_) => json!({"status": "error", "message": format!("HTTP {}: {}", raw.status, raw.text), "code": raw.status}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_post_carries_the_token_in_its_header_and_the_payload_in_its_body() {
        let mut payload = Map::new();
        payload.insert("symbol".into(), json!("AAPL"));
        let req = post_request("tok", "quotes", payload).unwrap();
        assert_eq!(req.method(), Method::POST);
        assert_eq!(req.uri().path(), "/api/v1/quotes");
        assert_eq!(req.headers()[INTERNAL_TOKEN_HEADER], "tok");
        assert_eq!(req.headers()["content-type"], "application/json");
    }

    #[test]
    fn a_raw_answer_is_shaped_as_the_sdk_shapes_it() {
        let ok = Raw::new(200, r#"{"status":"success","data":{"ltp":1}}"#);
        assert_eq!(shape(&ok)["data"]["ltp"], 1);
        let refused = Raw::new(403, r#"{"status":"error","message":"Invalid openalgo apikey"}"#);
        assert_eq!(shape(&refused)["code"], 403);
        assert!(shape(&refused)["message"].as_str().unwrap().starts_with("HTTP 403"));
    }
}
