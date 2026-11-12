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
use crate::flow::support::dispatch::INTERNAL_TOKEN_HEADER;
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

/// Flow's in-process calls carry the app's per-process token in
/// `x-openalgo-internal-token`. A request with the right token has it
/// written into its JSON body as `apikey`, where every `/api/v1` schema
/// reads the key, so the handlers' key check, the rate limiter, analyzer
/// routing and Gate B apply as for any API client; a wrong token is refused
/// here. A request without the header passes untouched.
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
    let Some(app) = shared.get_app_state() else {
        return refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error: AppState not available",
        );
    };
    if !shared.is_internal(&given) {
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
    next.run(Request::from_parts(parts, axum::body::Body::from(json.to_string())))
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
