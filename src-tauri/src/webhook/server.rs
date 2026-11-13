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

use crate::db::sqlite::WebhookConfig;
use crate::flow::support::dispatch::{InProcessCall, INTERNAL_TOKEN_HEADER};
use crate::state::AppState;
use crate::webhook::handlers::{self, WebhookState};
use crate::webhook::rate_limiter::{rate_limit_middleware, RateLimiterState};
use crate::webhook::types::{ApiResponse, Empty};
use axum::{
    extract::{Request, State},
    http::{header::CONTENT_LENGTH, StatusCode},
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
        .layer(middleware::from_fn_with_state(shared, internal_trust))
        // Rate limiting middleware (applied to all API routes)
        .layer(middleware::from_fn_with_state(rate_limiter.clone(), rate_limit_middleware))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
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
