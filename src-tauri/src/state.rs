//! Application state management

use crate::brokers::BrokerRegistry;
use crate::clock::{Clock, SystemClock};
use crate::db::duckdb::DuckDb;
use crate::db::sqlite::SqliteDb;
use crate::error::{AppError, Result};
use crate::events::{self, OrderUpdates};
use crate::flow::support::symbols::SymbolStore;
use crate::flow::FlowModule;
use crate::security::SecurityManager;
use crate::websocket::WebSocketManager;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use reqwest::Client;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// User session information
#[derive(Debug, Clone)]
pub struct UserSession {
    pub user_id: i64,
    pub username: String,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
}

/// Broker session information
#[derive(Debug, Clone)]
pub struct BrokerSession {
    pub broker_id: String,
    pub auth_token: String,
    pub feed_token: Option<String>,
    pub user_id: String,
    pub user_name: Option<String>,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
}

/// Symbol cache entry
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol: String,
    pub token: String,
    pub exchange: String,
    pub name: String,
    pub lot_size: i32,
    pub tick_size: f64,
    pub instrument_type: String,
    /// Broker's original symbol format (e.g., "NSE:RELIANCE-EQ" for Fyers)
    pub brsymbol: Option<String>,
    /// Broker's exchange code
    pub brexchange: Option<String>,
    /// Whether the symbol supports fractional/notional orders (Alpaca).
    pub fractionable: bool,
}

/// Application state shared across all commands
pub struct AppState {
    /// SQLite database connection
    pub sqlite: Arc<SqliteDb>,

    /// DuckDB connection for historical data
    pub duckdb: Arc<DuckDb>,

    /// Security manager for encryption/keychain
    pub security: Arc<SecurityManager>,

    /// Broker registry
    pub brokers: Arc<BrokerRegistry>,

    /// WebSocket manager for real-time market data
    pub websocket: Arc<WebSocketManager>,

    /// Current user session
    pub user_session: RwLock<Option<UserSession>>,

    /// Current broker session
    pub broker_session: RwLock<Option<BrokerSession>>,

    /// Symbol cache (token -> symbol info)
    pub symbol_cache: DashMap<String, SymbolInfo>,

    /// Reverse symbol cache (symbol -> token)
    pub symbol_reverse_cache: DashMap<String, String>,

    /// Shared HTTP client for provider API calls
    pub http_client: Arc<Client>,

    /// Application data directory
    pub data_dir: PathBuf,

    /// The clock everything that depends on wall time reads (a manual
    /// clock in tests).
    pub clock: Arc<dyn Clock>,

    /// Flow: the workflows' runs, triggers and node services.
    pub flow: Arc<FlowModule>,

    /// The master contract as Flow reads it (filled with the symbol cache).
    pub symbols: SymbolStore,

    /// The per-process credential Flow's in-process `/api/v1` calls carry
    /// (32 random bytes, hex); never logged, never stored.
    pub internal_token: String,

    /// Order updates for Flow's order-update triggers.
    pub order_updates: OrderUpdates,
}

impl AppState {
    /// Create new application state
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Get app data directory
        let data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Config(format!("Failed to get app data directory: {}", e)))?;

        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&data_dir)?;

        tracing::info!("Data directory: {:?}", data_dir);

        // Initialize SQLite database
        let sqlite_path = data_dir.join("openalgo.db");
        let sqlite = Arc::new(SqliteDb::new(&sqlite_path)?);

        // Initialize DuckDB for historical data
        let duckdb_path = data_dir.join("historify.duckdb");
        let duckdb = Arc::new(DuckDb::new(&duckdb_path)?);

        // Initialize security manager with file-based storage (no keychain prompts)
        let security = Arc::new(SecurityManager::new(data_dir.clone())?);

        // Initialize broker registry
        let brokers = Arc::new(BrokerRegistry::new());

        // Initialize WebSocket manager
        let websocket = Arc::new(WebSocketManager::new(app_handle.clone()));

        // Initialize shared HTTP client for provider API calls
        let http_client = Arc::new(Client::new());

        Ok(Self {
            sqlite,
            duckdb,
            security,
            brokers,
            websocket,
            user_session: RwLock::new(None),
            broker_session: RwLock::new(None),
            symbol_cache: DashMap::new(),
            symbol_reverse_cache: DashMap::new(),
            http_client,
            data_dir,
            clock: Arc::new(SystemClock),
            flow: Arc::new(FlowModule::new()),
            symbols: SymbolStore::new(),
            internal_token: new_internal_token(),
            order_updates: events::channel(),
        })
    }

    /// Now, by the app's clock.
    pub fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// Run synchronous database work on the blocking pool, so a slow query
    /// never holds an async worker.
    pub async fn blocking<T, F>(self: &Arc<Self>, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&AppState) -> Result<T> + Send + 'static,
    {
        let ctx = self.clone();
        tokio::task::spawn_blocking(move || f(&ctx))
            .await
            .map_err(|e| AppError::Internal(format!("Blocking task failed: {}", e)))?
    }

    /// Spawn a background task on the app's async runtime.
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(fut);
        } else {
            tauri::async_runtime::spawn(fut);
        }
    }

    /// When the trading session turns (hour, minute in market time): the
    /// auto-logout time, 03:00 when it cannot be read.
    pub fn session_boundary(&self) -> (u32, u32) {
        match self.sqlite.get_auto_logout_config() {
            Ok(c) => (c.hour.min(23), c.minute.min(59)),
            Err(_) => (3, 0),
        }
    }

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.user_session.read().is_some()
    }

    /// Check if broker is connected
    pub fn is_broker_connected(&self) -> bool {
        self.broker_session.read().is_some()
    }

    /// Get current user session
    pub fn get_user_session(&self) -> Option<UserSession> {
        self.user_session.read().clone()
    }

    /// Set user session
    pub fn set_user_session(&self, session: Option<UserSession>) {
        *self.user_session.write() = session;
    }

    /// Get current broker session
    pub fn get_broker_session(&self) -> Option<BrokerSession> {
        self.broker_session.read().clone()
    }

    /// Set broker session
    pub fn set_broker_session(&self, session: Option<BrokerSession>) {
        *self.broker_session.write() = session;
    }

    /// Get symbol info by exchange:token (O(1) lookup)
    pub fn get_symbol_by_token(&self, exchange: &str, token: &str) -> Option<SymbolInfo> {
        let key = format!("{}:{}", exchange, token);
        self.symbol_cache.get(&key).map(|r| r.clone())
    }

    /// Get symbol info by exchange:symbol (O(1) lookup)
    pub fn get_symbol_by_name(&self, exchange: &str, symbol: &str) -> Option<SymbolInfo> {
        let reverse_key = format!("{}:{}", exchange, symbol);
        self.symbol_reverse_cache
            .get(&reverse_key)
            .and_then(|token_ref| {
                let token = token_ref.value();
                let cache_key = format!("{}:{}", exchange, token);
                self.symbol_cache.get(&cache_key).map(|r| r.clone())
            })
    }

    /// Get token by exchange:symbol (O(1) lookup)
    pub fn get_token_by_symbol(&self, exchange: &str, symbol: &str) -> Option<String> {
        let key = format!("{}:{}", exchange, symbol);
        self.symbol_reverse_cache.get(&key).map(|r| r.clone())
    }

    /// Check if symbol exists (O(1) lookup)
    pub fn symbol_exists(&self, exchange: &str, symbol: &str) -> bool {
        let key = format!("{}:{}", exchange, symbol);
        self.symbol_reverse_cache.contains_key(&key)
    }

    /// Get total number of symbols in cache
    pub fn symbol_count(&self) -> usize {
        self.symbol_cache.len()
    }

    /// Load symbols into cache
    pub fn load_symbol_cache(&self, symbols: Vec<SymbolInfo>) {
        self.symbol_cache.clear();
        self.symbol_reverse_cache.clear();
        self.symbols.load_infos(symbols.iter());

        for symbol in symbols {
            let cache_key = format!("{}:{}", symbol.exchange, symbol.token);
            let reverse_key = format!("{}:{}", symbol.exchange, symbol.symbol);
            self.symbol_reverse_cache.insert(reverse_key, symbol.token.clone());
            self.symbol_cache.insert(cache_key, symbol);
        }

        tracing::info!("Loaded {} symbols into cache", self.symbol_cache.len());
    }

    /// Get all symbols for a specific exchange
    pub fn get_symbols_by_exchange(&self, exchange: &str) -> Vec<SymbolInfo> {
        self.symbol_cache
            .iter()
            .filter(|entry| entry.value().exchange.eq_ignore_ascii_case(exchange))
            .map(|entry| entry.value().clone())
            .collect()
    }
}

/// A fresh internal token: 32 random bytes as hex.
fn new_internal_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}
