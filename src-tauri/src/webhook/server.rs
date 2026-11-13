//! HTTP server for webhooks and REST API
//!
//! Provides:
//! - Dynamic strategy-based webhooks (/webhook/{webhook_id})
//! - OpenAlgo SDK compatible REST API (/api/v1/*)
//! - Rate limiting to prevent hitting broker API limits
//!
//! The router is built by [`router`], so Flow's nodes can call the same
//! handlers in process (`flow::support::dispatch`, `tower::ServiceExt::
//! oneshot`) with the app's per-process internal token: the same key
//! check, rate limiter, analyzer routing and Gate B as any API client.
//!
//! - Flow's public webhook (`/flow/webhook/{token}`, WP8 Phase C): the token
//!   names a workflow and is its credential, so the request log shows it as
//!   `[redacted]` ([`redacted_uri`]), as it does a `secret` in the query.

use crate::db::sqlite::WebhookConfig;
use crate::flow::service::webhooks;
use crate::flow::support::dispatch::{InProcessCall, INTERNAL_TOKEN_HEADER};
use crate::state::AppState;
use crate::webhook::handlers::{self, WebhookState};
use crate::webhook::rate_limiter::{rate_limit_middleware, RateLimiterState};
use crate::webhook::types::{ApiResponse, Empty};
use axum::{
    extract::{Path, Request, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        StatusCode, Uri,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

/// Largest body the internal-trust layer rewrites (an order basket is a
/// few kilobytes; a multi-quote list well under this).
const INTERNAL_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// The `/api/v1` and webhook routes over `state`, with their rate limiter
/// (read from the settings) and the internal-trust layer. Each router has
/// its own rate limiter, so Flow's in-process calls have their own share
/// of the limit and never spend an SDK client's.
pub fn router(state: AppState) -> Router {
    // Get rate limit config from database
    let (api_rate, order_rate, smart_order_rate, smart_order_delay) =
        match state.sqlite.get_rate_limit_config() {
            Ok(config) => (
                config.api_rate_limit,
                config.order_rate_limit,
                config.smart_order_rate_limit,
                config.smart_order_delay,
            ),
            Err(e) => {
                error!("Failed to get rate limit config, using defaults: {}", e);
                (100, 10, 2, 0.5) // Default values
            }
        };

    info!("Rate limits: API={}/s, Order={}/s, SmartOrder={}/s, Delay={}s",
          api_rate, order_rate, smart_order_rate, smart_order_delay);

    // Create rate limiter state
    let rate_limiter = Arc::new(RateLimiterState::new(
        api_rate,
        order_rate,
        smart_order_rate,
        smart_order_delay,
    ));

    // Create shared state
    let shared = Arc::new(WebhookState::new(&state));

    // Build CORS layer (allow all for local development)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router with all routes
    Router::new()
        // ================================================================
        // Health check
        // ================================================================
        .route("/health", get(handlers::health_check))
        .route("/", get(handlers::health_check))

        // ================================================================
        // Dynamic webhook endpoint (strategy-based)
        // POST /webhook/{webhook_id}
        // ================================================================
        .route("/webhook/:webhook_id", post(handlers::webhook_handler))

        // Legacy: Support /strategy/webhook/{webhook_id} for compatibility
        .route("/strategy/webhook/:webhook_id", post(handlers::webhook_handler))

        // ================================================================
        // OAuth Callback (for Fyers, Zerodha, etc.)
        // GET /{broker}/callback?code=xxx&state=xxx
        // ================================================================
        .route("/:broker/callback", get(handlers::oauth_callback))

        // ================================================================
        // REST API v1 (OpenAlgo SDK Compatible)
        // ================================================================

        // Order placement
        .route("/api/v1/placeorder", post(handlers::place_order))
        .route("/api/v1/placesmartorder", post(handlers::place_smart_order))
        .route("/api/v1/modifyorder", post(handlers::modify_order))
        .route("/api/v1/cancelorder", post(handlers::cancel_order))
        .route("/api/v1/cancelallorder", post(handlers::cancel_all_orders))
        .route("/api/v1/closeposition", post(handlers::close_position))
        .route("/api/v1/basketorder", post(handlers::place_basket_order))
        .route("/api/v1/splitorder", post(handlers::place_split_order))

        // Order/Position status
        .route("/api/v1/orderstatus", post(handlers::get_order_status))
        .route("/api/v1/openposition", post(handlers::get_open_position))

        // Data retrieval
        .route("/api/v1/orderbook", post(handlers::get_orderbook))
        .route("/api/v1/tradebook", post(handlers::get_tradebook))
        .route("/api/v1/positionbook", post(handlers::get_positionbook))
        .route("/api/v1/holdings", post(handlers::get_holdings))
        .route("/api/v1/funds", post(handlers::get_funds))
        .route("/api/v1/quotes", post(handlers::get_quotes))

        // Market data
        .route("/api/v1/depth", post(handlers::get_depth))
        .route("/api/v1/symbol", post(handlers::get_symbol))
        .route("/api/v1/history", post(handlers::get_history))
        .route("/api/v1/intervals", post(handlers::get_intervals))
        .route("/api/v1/multiquotes", post(handlers::get_multiquotes))
        .route("/api/v1/search", post(handlers::search_symbols))
        .route("/api/v1/expiry", post(handlers::get_expiry))
        .route("/api/v1/instruments", get(handlers::get_instruments))
        .route("/api/v1/syntheticfuture", post(handlers::get_synthetic_future))

        // Account/Analyzer
        .route("/api/v1/analyzer", post(handlers::get_analyzer_status))
        .route("/api/v1/analyzer/toggle", post(handlers::toggle_analyzer))
        .route("/api/v1/margin", post(handlers::get_margin))

        // Options API
        .route("/api/v1/optionchain", post(handlers::get_option_chain))
        .route("/api/v1/optiongreeks", post(handlers::get_option_greeks))
        .route("/api/v1/optionsorder", post(handlers::place_options_order))
        .route("/api/v1/optionsymbol", post(handlers::get_option_symbol))
        .route("/api/v1/optionsmultiorder", post(handlers::place_options_multi_order))

        // ================================================================
        // Add state and middleware
        // ================================================================
        .with_state(shared.clone())
        // The app's own runs (Flow) identify themselves by header
        .layer(middleware::from_fn_with_state(shared.clone(), internal_trust))
        // Rate limiting middleware (applied to all API routes)
        .layer(middleware::from_fn_with_state(rate_limiter.clone(), rate_limit_middleware))
        // Flow's public webhook, added after the API's rate limit so it is
        // outside it: that limiter logs the path it refuses (the token is
        // the workflow's credential), and an internet sender would spend
        // the SDK's budget. Its own guards: the secret, the lockout after
        // wrong secrets, one run at a time per workflow.
        .merge(flow_webhook_routes(shared))
        .layer(cors)
        .layer(TraceLayer::new_for_http().make_span_with(request_span))
}

/// The prefix of Flow's public webhook routes.
const FLOW_WEBHOOK_PREFIX: &str = "/flow/webhook/";

/// What the request log shows in place of a credential.
const REDACTED: &str = "[redacted]";

/// The largest alert body read (TradingView's and Chartink's are a few
/// hundred bytes).
const FLOW_WEBHOOK_BODY_LIMIT: usize = INTERNAL_BODY_LIMIT;

/// The request log's span (tower-http's own, with the URI redacted).
fn request_span(req: &Request) -> tracing::Span {
    tracing::debug_span!(
        "request",
        method = %req.method(),
        uri = %redacted_uri(req.uri()),
        version = ?req.version(),
    )
}

/// `uri` as the request log shows it: a Flow webhook's token as
/// `[redacted]` (the symbol after it kept), and the value of any `secret`
/// in the query as `[redacted]`, its key read as the webhook reads it
/// (percent-decoded).
pub(crate) fn redacted_uri(uri: &Uri) -> String {
    let path = uri.path();
    let mut shown = match path.strip_prefix(FLOW_WEBHOOK_PREFIX) {
        Some(rest) => match rest.split_once('/') {
            Some((_, symbol)) => format!("{}{}/{}", FLOW_WEBHOOK_PREFIX, REDACTED, symbol),
            None => format!("{}{}", FLOW_WEBHOOK_PREFIX, REDACTED),
        },
        None => path.to_string(),
    };
    if let Some(query) = uri.query() {
        let pairs: Vec<String> = query
            .split('&')
            .map(|pair| {
                let key = url::form_urlencoded::parse(pair.as_bytes())
                    .next()
                    .map(|(k, _)| k.into_owned())
                    .unwrap_or_default();
                if key == "secret" {
                    format!("secret={}", REDACTED)
                } else {
                    pair.to_string()
                }
            })
            .collect();
        shown.push('?');
        shown.push_str(&pairs.join("&"));
    }
    shown
}

/// Flow's two public webhook routes, upstream's `POST /flow/webhook/{token}`
/// and `POST /flow/webhook/{token}/{symbol}` (`flow::service::webhooks`
/// checks the token, the secret, the lockout and the workflow, then runs
/// it).
fn flow_webhook_routes(shared: Arc<WebhookState>) -> Router {
    Router::new()
        .route("/flow/webhook/:token", post(flow_webhook))
        .route("/flow/webhook/:token/:symbol", post(flow_webhook_with_symbol))
        .with_state(shared)
}

/// `POST /flow/webhook/{token}`: public, with its own credential.
async fn flow_webhook(
    State(shared): State<Arc<WebhookState>>,
    Path(token): Path<String>,
    req: Request,
) -> Response {
    flow_webhook_call(&shared, token, None, req).await
}

/// `POST /flow/webhook/{token}/{symbol}`: the path's symbol replaces any in
/// the body.
async fn flow_webhook_with_symbol(
    State(shared): State<Arc<WebhookState>>,
    Path((token, symbol)): Path<(String, String)>,
    req: Request,
) -> Response {
    flow_webhook_call(&shared, token, Some(symbol), req).await
}

/// Read the alert as upstream's route did (the body as JSON, text or an
/// urlencoded form, `?secret=` for URL auth) and answer the service's reply.
async fn flow_webhook_call(
    shared: &WebhookState,
    token: String,
    symbol: Option<String>,
    req: Request,
) -> Response {
    let Some(app) = shared.get_app_state() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "OpenAlgo is shutting down."})),
        )
            .into_response();
    };
    let app = app.root();
    let declared = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let url_secret = req.uri().query().and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .find(|(k, _)| k == "secret")
            .map(|(_, v)| v.into_owned())
    });
    let Ok(body) = axum::body::to_bytes(req.into_body(), FLOW_WEBHOOK_BODY_LIMIT).await else {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({"error": "The alert is too large."})),
        )
            .into_response();
    };
    // The web's `request.form`: a body the sender declared urlencoded. A
    // multipart body is read as text (this app's axum has no multipart).
    let form = if declared.starts_with("application/x-www-form-urlencoded") {
        webhooks::urlencoded_fields(&body)
    } else {
        serde_json::Map::new()
    };
    let mut payload = webhooks::read_payload(&body, &form);
    if let Some(s) = symbol {
        payload.insert("symbol".into(), serde_json::Value::String(s));
    }
    let answer = webhooks::trigger(
        &app,
        webhooks::WebhookCall {
            token,
            payload,
            url_secret,
        },
    )
    .await;
    let status = StatusCode::from_u16(answer.reply.status)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(answer.reply.body)).into_response()
}

tokio::task_local! {
    /// Set while the handlers serve a request `flow::support::dispatch::run`
    /// sent in process with the right token: the one place the handlers
    /// honor the internal token (`WebhookState::is_internal`).
    static IN_PROCESS_CALL: ();
}

/// Whether the handler running now serves an in-process Flow call.
pub(crate) fn in_process_call() -> bool {
    IN_PROCESS_CALL.try_with(|_| ()).is_ok()
}

/// Flow's in-process calls carry the app's per-process token in
/// `x-openalgo-internal-token`, and the `InProcessCall` mark only
/// `flow::support::dispatch::run` puts on a request (a request from the
/// network listener, or a public URL in front of it, cannot carry one).
/// A marked request with the right token has the token written into its
/// JSON body as `apikey`, where every `/api/v1` schema reads the key, and
/// runs inside `IN_PROCESS_CALL`, so the handlers' key check, the rate
/// limiter, analyzer routing and Gate B apply as for any API client. The
/// header without the mark, or with a wrong token, is refused here as a
/// wrong key. A request without the header passes untouched, and the token
/// in its body is checked like any other key, and refused.
async fn internal_trust(
    State(shared): State<Arc<WebhookState>>,
    req: Request,
    next: Next,
) -> Response {
    let Some(given) = req
        .headers()
        .get(INTERNAL_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
    else {
        return next.run(req).await;
    };
    if req.extensions().get::<InProcessCall>().is_none() {
        return refused(StatusCode::FORBIDDEN, "Invalid openalgo apikey");
    }
    let Some(app) = shared.get_app_state() else {
        return refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error: AppState not available",
        );
    };
    if !shared.is_internal_token(&given) {
        return refused(StatusCode::FORBIDDEN, "Invalid openalgo apikey");
    }
    let (mut parts, body) = req.into_parts();
    let bytes = match axum::body::to_bytes(body, INTERNAL_BODY_LIMIT).await {
        Ok(b) => b,
        Err(e) => {
            return refused(
                StatusCode::BAD_REQUEST,
                &format!("Could not read the request body: {}", e),
            )
        }
    };
    let mut json: serde_json::Value = if bytes.iter().all(u8::is_ascii_whitespace) {
        serde_json::json!({})
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    &format!("The request body is not JSON: {}", e),
                )
            }
        }
    };
    match json.as_object_mut() {
        Some(m) => {
            m.insert(
                "apikey".into(),
                serde_json::Value::String(app.internal_token.clone()),
            );
        }
        None => return refused(StatusCode::BAD_REQUEST, "The request body must be a JSON object"),
    }
    parts.headers.remove(CONTENT_LENGTH);
    IN_PROCESS_CALL
        .scope(
            (),
            next.run(Request::from_parts(parts, axum::body::Body::from(json.to_string()))),
        )
        .await
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(ApiResponse::<Empty>::error(message))).into_response()
}

/// Webhook/API server manager
pub struct WebhookServer {
    app_handle: AppHandle,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl WebhookServer {
    /// Create a new server
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            shutdown_tx: None,
        }
    }

    /// Start the server
    pub async fn start(&mut self, config: WebhookConfig) -> Result<(), String> {
        if !config.enabled {
            info!("Webhook/API server is disabled");
            return Ok(());
        }

        let host = config.host.clone();
        let port = config.port;

        // Parse address
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?;

        // Build the router over the app's state
        let app_state = self.app_handle.state::<AppState>().inner().clone();
        let app = router(app_state);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        // Start server
        info!("Starting OpenAlgo Desktop API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

        // Spawn server task
        tokio::spawn(async move {
            let server = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                    info!("API server shutting down");
                });

            if let Err(e) = server.await {
                error!("API server error: {}", e);
            }
        });

        info!("OpenAlgo Desktop API server started successfully");
        info!("");
        info!("=== Endpoints ===");
        info!("");
        info!("Health Check:");
        info!("  GET  http://{}:{}/health", host, port);
        info!("");
        info!("Dynamic Webhook (strategy-based):");
        info!("  POST http://{}:{}/webhook/{{webhook_id}}", host, port);
        info!("");
        info!("Flow Webhook (a workflow's own token and secret):");
        info!("  POST http://{}:{}/flow/webhook/{{token}}", host, port);
        info!("");
        info!("REST API (OpenAlgo SDK compatible):");
        info!("  Order Placement:");
        info!("    POST http://{}:{}/api/v1/placeorder", host, port);
        info!("    POST http://{}:{}/api/v1/placesmartorder", host, port);
        info!("    POST http://{}:{}/api/v1/modifyorder", host, port);
        info!("    POST http://{}:{}/api/v1/cancelorder", host, port);
        info!("    POST http://{}:{}/api/v1/cancelallorder", host, port);
        info!("    POST http://{}:{}/api/v1/closeposition", host, port);
        info!("    POST http://{}:{}/api/v1/basketorder", host, port);
        info!("    POST http://{}:{}/api/v1/splitorder", host, port);
        info!("  Order/Position Status:");
        info!("    POST http://{}:{}/api/v1/orderstatus", host, port);
        info!("    POST http://{}:{}/api/v1/openposition", host, port);
        info!("  Data Retrieval:");
        info!("    POST http://{}:{}/api/v1/orderbook", host, port);
        info!("    POST http://{}:{}/api/v1/tradebook", host, port);
        info!("    POST http://{}:{}/api/v1/positionbook", host, port);
        info!("    POST http://{}:{}/api/v1/holdings", host, port);
        info!("    POST http://{}:{}/api/v1/funds", host, port);
        info!("    POST http://{}:{}/api/v1/quotes", host, port);
        info!("  Market Data:");
        info!("    POST http://{}:{}/api/v1/depth", host, port);
        info!("    POST http://{}:{}/api/v1/symbol", host, port);
        info!("    POST http://{}:{}/api/v1/history", host, port);
        info!("    POST http://{}:{}/api/v1/intervals", host, port);

        if let Some(ngrok_url) = config.ngrok_url {
            info!("");
            info!("=== Ngrok URL ===");
            info!("Base URL: {}", ngrok_url);
            info!("");
            info!("Webhook URL for strategies:");
            info!("  {}/webhook/{{your_webhook_id}}", ngrok_url);
            info!("");
            info!("REST API Base URL:");
            info!("  {}/api/v1/", ngrok_url);
        } else {
            info!("");
            info!("=== Ngrok Setup ===");
            info!("Run: ngrok http {}", port);
            info!("Then configure the ngrok URL in settings.");
        }

        Ok(())
    }

    /// Stop the server
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            info!("API server stop signal sent");
        }
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
    }
}

impl Drop for WebhookServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod key_tests {
    //! The `/api/v1` key (WP8 Phase B follow-up): never in a log line or a
    //! frontend event, and the app's per-process internal token honored
    //! only for in-process calls (`flow::support::dispatch::run`), never
    //! for a request over the network listener or a public URL in front of
    //! it.

    use super::*;
    use crate::brokers::BrokerRegistry;
    use crate::flow::support::dispatch;
    use crate::flow::support::testkit::capture_logs;
    use crate::state::testing::{build, TestCtx};
    use crate::webhook::handlers::emitted;
    use axum::body::Body;
    use serde_json::{json, Value};
    use std::time::Duration;
    use tower::ServiceExt;

    /// A headless app in analyzer mode, so an order lands in the sandbox.
    fn harness() -> TestCtx {
        let t = build(BrokerRegistry::new(), chrono::Utc::now());
        t.ctx.sqlite.set_analyze_mode(true).unwrap();
        t
    }

    /// The router the network listener serves (`WebhookServer::start`
    /// builds the same `router`; a public URL forwards to it).
    fn listener(t: &TestCtx) -> Router {
        router((*t.ctx).clone())
    }

    fn post(uri: &str, body: Value) -> Request {
        axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn answer(router: Router, req: Request) -> (u16, Value) {
        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    fn order(apikey: &str) -> Value {
        json!({
            "apikey": apikey, "strategy": "test", "exchange": "NASDAQ", "symbol": "AAPL",
            "action": "BUY", "quantity": 1, "pricetype": "MARKET", "product": "CNC"
        })
    }

    fn with(mut body: Value, key: &str, value: Value) -> Value {
        body[key] = value;
        body
    }

    fn refused_as_a_wrong_key(status: u16, body: &Value) {
        assert_eq!(status, 403, "{body}");
        assert_eq!(body["status"], "error", "{body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap_or_default()
                .starts_with("Invalid openalgo apikey"),
            "{body}"
        );
    }

    /// Each order route logs its request before checking the key; the key
    /// in that line is masked, whoever sent it.
    #[tokio::test]
    async fn a_logged_request_shows_no_apikey() {
        let t = harness();
        let logs = capture_logs();
        let requests = [
            ("/api/v1/placeorder", order("k-123")),
            ("/api/v1/placesmartorder", with(order("k-123"), "position_size", json!(1))),
            ("/api/v1/modifyorder", with(order("k-123"), "orderid", json!("1"))),
            ("/api/v1/cancelorder", json!({"apikey": "k-123", "strategy": "test", "orderid": "1"})),
            ("/api/v1/cancelallorder", json!({"apikey": "k-123", "strategy": "test"})),
            ("/api/v1/closeposition", json!({"apikey": "k-123", "strategy": "test"})),
        ];
        for (uri, body) in requests {
            let (status, body) = answer(listener(&t), post(uri, body)).await;
            refused_as_a_wrong_key(status, &body);
        }
        let text = logs.text();
        for line in [
            "Place order request",
            "Place smart order request",
            "Modify order request",
            "Cancel order request",
            "Cancel all orders request",
            "Close position request",
        ] {
            assert!(text.contains(line), "{line} is not logged:\n{text}");
        }
        assert!(!text.contains("k-123"), "the key is in the log:\n{text}");
        assert!(text.contains("[redacted]"), "{text}");
    }

    /// A placed order's frontend event carries the request without its key:
    /// the user's own API key here.
    #[tokio::test]
    async fn an_emitted_order_shows_no_apikey() {
        let t = harness();
        let (_, key) = t
            .ctx
            .sqlite
            .create_api_key("sdk", "read,write", &t.ctx.security)
            .unwrap();
        emitted::take();
        let (status, body) = answer(listener(&t), post("/api/v1/placeorder", order(&key))).await;
        assert_eq!(status, 200, "{body}");
        let events = emitted::take();
        let (_, payload) = events
            .iter()
            .find(|(event, _)| event == "api_order")
            .expect("the order's event");
        assert_eq!(payload["apikey"], "[redacted]", "{payload}");
        assert_eq!(payload["symbol"], "AAPL", "{payload}");
        assert!(!payload.to_string().contains(&key), "{payload}");
    }

    /// The internal token in the header of a request over the listener is
    /// refused, as a wrong key is.
    #[tokio::test]
    async fn the_listener_refuses_the_internal_token_in_a_header() {
        let t = harness();
        let mut req = post("/api/v1/placeorder", order(""));
        req.headers_mut().insert(
            dispatch::INTERNAL_TOKEN_HEADER,
            t.ctx.internal_token.parse().unwrap(),
        );
        let (status, body) = answer(listener(&t), req).await;
        refused_as_a_wrong_key(status, &body);
        assert!(t.ctx.sqlite.get_sandbox_orders().unwrap().is_empty());
    }

    /// The internal token as the body's key over the listener is refused
    /// with exactly what a wrong key gets.
    #[tokio::test]
    async fn the_listener_refuses_the_internal_token_in_a_body() {
        let t = harness();
        let wrong = answer(listener(&t), post("/api/v1/placeorder", order("not-a-key"))).await;
        let token = answer(
            listener(&t),
            post("/api/v1/placeorder", order(&t.ctx.internal_token)),
        )
        .await;
        refused_as_a_wrong_key(wrong.0, &wrong.1);
        refused_as_a_wrong_key(token.0, &token.1);
        assert!(t.ctx.sqlite.get_sandbox_orders().unwrap().is_empty());
    }

    /// Flow's own path keeps working: the order goes in process through
    /// `dispatch::run` with the token, lands in the sandbox, and neither
    /// the log nor the event carries the token.
    #[tokio::test]
    async fn the_in_process_order_path_takes_the_token_and_hides_it() {
        let t = harness();
        let logs = capture_logs();
        emitted::take();
        let mut payload = order("").as_object().unwrap().clone();
        payload.remove("apikey");
        let req = dispatch::post_request(&t.ctx.internal_token, "placeorder", payload).unwrap();
        let raw = dispatch::run(&t.ctx, req, Duration::from_secs(30)).await.unwrap();
        let out: Value = serde_json::from_str(&raw.text).unwrap();
        assert_eq!(raw.status, 200, "{out}");
        assert_eq!(out["status"], "success", "{out}");
        assert!(out["orderid"].as_str().is_some_and(|id| !id.is_empty()), "{out}");
        assert_eq!(t.ctx.sqlite.get_sandbox_orders().unwrap().len(), 1);
        let token = t.ctx.internal_token.clone();
        assert!(!logs.text().contains(&token), "the token is in the log");
        let events = emitted::take();
        let (_, event) = events
            .iter()
            .find(|(event, _)| event == "api_order")
            .expect("the order's event");
        assert!(!event.to_string().contains(&token), "{event}");
    }
}

#[cfg(test)]
mod flow_webhook_tests {
    //! WP8 Phase C, Task C3: Flow's public webhook on the app's listener,
    //! `POST /flow/webhook/<token>` and `POST /flow/webhook/<token>/<symbol>`
    //! (upstream's two public routes): the token names the workflow, the
    //! secret proves the caller, wrong secrets lock the webhook, a replay
    //! while the first run holds the workflow is refused, and neither the
    //! token nor a secret reaches the request log.

    use super::*;
    use crate::brokers::BrokerRegistry;
    use crate::flow::store::workflows::{self, NewWorkflow};
    use crate::flow::support::testkit::{capture_logs, sign_in};
    use crate::state::testing::{build, TestCtx};
    use axum::body::Body;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    /// A test process-wide subscriber that wants every line and prints
    /// none, so a log capture here sees every line whatever other tests'
    /// threads hold (see `flow::triggers::tests::support`).
    fn every_line_on() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry());
        });
    }

    /// The app signed in, analyzer mode off, no broker.
    fn harness() -> TestCtx {
        every_line_on();
        let t = build(BrokerRegistry::new(), chrono::Utc::now());
        sign_in(&t.ctx);
        t
    }

    /// An active workflow with its webhook on: Webhook Trigger, then a Delay
    /// of `wait` seconds when given, then a Log of `{{webhook.symbol}}`.
    /// Its id, token and secret.
    fn hooked(t: &TestCtx, wait: Option<u64>) -> (i64, String, String) {
        let mut nodes = vec![json!({"id": "w", "type": "webhookTrigger", "position": {"x": 0, "y": 0}, "data": {}})];
        let mut edges = Vec::new();
        let mut last = "w";
        if let Some(seconds) = wait {
            nodes.push(json!({"id": "d", "type": "delay", "position": {"x": 0, "y": 100}, "data": {"delayValue": seconds}}));
            edges.push(json!({"id": "e0", "source": "w", "target": "d"}));
            last = "d";
        }
        nodes.push(json!({"id": "l", "type": "log", "position": {"x": 0, "y": 200}, "data": {"message": "Alert for {{webhook.symbol}}"}}));
        edges.push(json!({"id": "e1", "source": last, "target": "l"}));
        let conn = t.ctx.sqlite.conn().unwrap();
        let w = workflows::create(
            &conn,
            &t.ctx.security,
            &NewWorkflow {
                name: "Hook",
                description: None,
                nodes: &json!(nodes),
                edges: &json!(edges),
            },
            t.ctx.now(),
        )
        .unwrap();
        workflows::set_active(&conn, w.id, true, t.ctx.now()).unwrap();
        workflows::set_webhook_enabled(&conn, w.id, true, t.ctx.now()).unwrap();
        (
            w.id,
            w.webhook_token.unwrap(),
            w.webhook_secret.unwrap().expose().to_string(),
        )
    }

    fn post(uri: &str, content_type: &str, body: String) -> Request {
        axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap()
    }

    fn post_json(uri: &str, body: Value) -> Request {
        post(uri, "application/json", body.to_string())
    }

    /// The answer of the app's listener (`WebhookServer::start` serves this
    /// same router).
    async fn answer(t: &TestCtx, req: Request) -> (u16, Value) {
        let resp = router((*t.ctx).clone()).oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    fn runs(t: &TestCtx, id: i64) -> Vec<crate::flow::contract::model::Execution> {
        crate::flow::store::executions::list(&t.ctx.sqlite.conn().unwrap(), id, 20).unwrap()
    }

    fn logged_messages(run: &crate::flow::contract::model::Execution) -> Vec<String> {
        run.logs
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l["message"].as_str().map(str::to_string))
            .collect()
    }

    /// The address the webhook settings show is on 127.0.0.1 (the app's
    /// listener is IPv4 only; `localhost` can resolve to ::1); a POST of
    /// it with the secret in the JSON runs the workflow and answers
    /// upstream's JSON; the address with a symbol puts that symbol in the
    /// run's `{{webhook.symbol}}`; an unknown token answers 404.
    #[tokio::test]
    async fn the_right_secret_runs_the_workflow_and_answers_upstreams_json() {
        let t = harness();
        let (id, token, secret) = hooked(&t, None);
        let info = crate::flow::service::webhooks::info(&t.ctx, id).await;
        let url = info.body["webhook_url"].as_str().unwrap().to_string();
        assert_eq!(url, format!("http://127.0.0.1:5000/flow/webhook/{}", token));
        let path = url.trim_start_matches("http://127.0.0.1:5000");
        let (status, body) =
            answer(&t, post_json(path, json!({"secret": secret, "symbol": "AAPL"}))).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["status"], "success", "{body}");
        assert_eq!(body["message"], "Workflow 'Hook' triggered", "{body}");
        assert_eq!(body["workflow_id"], id, "{body}");
        assert_eq!(body["errors"], Value::Null, "{body}");
        assert!(body["execution_id"].as_i64().is_some(), "{body}");
        let first = runs(&t, id);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].status, "completed", "{:?}", first[0].error);
        assert!(logged_messages(&first[0]).contains(&"[LOG] Alert for AAPL".to_string()));

        let (status, body) = answer(
            &t,
            post_json(&format!("{}/MSFT", path), json!({"secret": secret, "symbol": "AAPL"})),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let newest = &runs(&t, id)[0];
        assert!(
            logged_messages(newest).contains(&"[LOG] Alert for MSFT".to_string()),
            "the path's symbol replaces the body's: {:?}",
            logged_messages(newest)
        );

        let (status, body) =
            answer(&t, post_json("/flow/webhook/not-a-token", json!({"secret": secret}))).await;
        assert_eq!((status, body), (404, json!({"error": "Invalid webhook token"})));
    }

    /// A wrong secret is refused 401 and runs nothing; the tenth inside the
    /// guard's window locks the webhook, so even the right secret is then
    /// refused 403 with the lock's sentence.
    #[tokio::test]
    async fn wrong_secrets_are_refused_and_then_lock_the_webhook() {
        let t = harness();
        let (id, token, secret) = hooked(&t, None);
        let path = format!("/flow/webhook/{}", token);
        for n in 1..=crate::flow::service::webhook_guard::LOCKOUT_FAILURES {
            let (status, body) =
                answer(&t, post_json(&path, json!({"secret": format!("wrong-{n}")}))).await;
            assert_eq!(
                (status, body),
                (401, json!({"error": "Invalid webhook secret"})),
                "wrong secret {n}"
            );
        }
        let (status, body) = answer(&t, post_json(&path, json!({"secret": secret}))).await;
        assert_eq!(status, 403, "{body}");
        assert_eq!(
            body["error"],
            crate::flow::service::webhook_guard::LOCKED,
            "{body}"
        );
        assert!(runs(&t, id).is_empty(), "nothing ran");
    }

    /// The same alert twice within a second: the first starts the run
    /// (which waits a minute, so it answers 202 and goes on in the
    /// background holding the workflow); the second is refused 409
    /// `already running` and starts nothing.
    #[tokio::test]
    async fn the_same_alert_twice_while_the_first_runs_is_already_running() {
        let t = harness();
        let (id, token, secret) = hooked(&t, Some(60));
        let path = format!("/flow/webhook/{}", token);
        let alert = json!({"secret": secret, "symbol": "AAPL"});
        let first = answer(&t, post_json(&path, alert.clone())).await;
        let second = answer(&t, post_json(&path, alert)).await;
        assert_eq!(first.0, 202, "{}", first.1);
        assert_eq!(first.1["status"], "accepted", "{}", first.1);
        assert_eq!(second.0, 409, "{}", second.1);
        assert_eq!(second.1["status"], "error", "{}", second.1);
        assert_eq!(second.1["message"], "Workflow is already running", "{}", second.1);
        assert_eq!(runs(&t, id).len(), 1, "one run, still waiting");
        t.ctx.flow.stop_all(std::time::Duration::from_secs(5)).await;
    }

    /// Every way a caller can put a credential in the request (the token in
    /// the path, with and without a symbol, a `?secret=` and a form body)
    /// leaves the request log with `[redacted]` in its place.
    #[tokio::test]
    async fn the_token_and_the_secret_never_reach_the_request_log() {
        let t = harness();
        let (_, token, secret) = hooked(&t, None);
        let logs = capture_logs();
        let path = format!("/flow/webhook/{}", token);
        answer(&t, post_json(&path, json!({"secret": secret}))).await;
        answer(&t, post_json(&format!("{}/AAPL", path), json!({"secret": "wrong"}))).await;
        answer(&t, post_json(&format!("{}?secret={}", path, secret), json!({}))).await;
        answer(
            &t,
            post(
                &path,
                "application/x-www-form-urlencoded",
                format!("secret={}&symbol=AAPL", secret),
            ),
        )
        .await;
        let text = logs.text();
        assert!(!text.contains(&token), "the token is in the log:\n{text}");
        assert!(!text.contains(&secret), "the secret is in the log:\n{text}");
        assert!(
            text.contains("uri=/flow/webhook/[redacted]"),
            "no request line:\n{text}"
        );
        assert!(text.contains("uri=/flow/webhook/[redacted]/AAPL"), "{text}");
        assert!(text.contains("secret=[redacted]"), "{text}");
    }
}
