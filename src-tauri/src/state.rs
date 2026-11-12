//! Application state management
//!
//! [`AppState`] is a cheap handle: an `Arc` around [`AppStateInner`], which
//! holds every field. Tauri manages one handle and the 250-odd commands
//! read it through `State<'_, AppState>` exactly as before (field access
//! goes through `Deref`); Flow, which was written against upstream's
//! `Arc<AppState>` with `Weak` back-references from its long-lived tasks,
//! takes the one root `Arc` made by [`AppState::root`] and holds the
//! handle weakly from there.

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
use parking_lot::{Mutex, RwLock};
use reqwest::Client;
use std::future::Future;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
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

/// Everything the application state holds. Reached through [`AppState`].
pub struct AppStateInner {
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

    /// The Tauri app, when the state belongs to a running app (`None` for
    /// a headless state in tests): what the in-process API router emits
    /// its events through.
    pub app_handle: Option<AppHandle>,

    /// The root `Arc` ([`AppState::root`]), held weakly so it can be found
    /// again from any handle while something keeps it alive.
    me: Mutex<Weak<AppState>>,
}

/// Application state shared across all commands: a handle, cheap to clone.
#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

impl Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &AppStateInner {
        &self.0
    }
}

/// What opens with the state: the clock, the broker adapters and the app.
pub struct OpenOptions {
    pub clock: Arc<dyn Clock>,
    pub brokers: Arc<BrokerRegistry>,
    pub app_handle: Option<AppHandle>,
}

impl OpenOptions {
    /// The running app's: the system clock, every broker adapter, the app.
    pub fn production(app_handle: AppHandle) -> Self {
        Self {
            clock: Arc::new(SystemClock),
            brokers: Arc::new(BrokerRegistry::new()),
            app_handle: Some(app_handle),
        }
    }
}

impl AppState {
    /// Create new application state
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Get app data directory
        let data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Config(format!("Failed to get app data directory: {}", e)))?;
        Self::build(&data_dir, OpenOptions::production(app_handle.clone()))
    }

    /// Open the state on `data_dir` (made when missing) with `opts` and
    /// start Flow on it, as the app does at start-up: the root `Arc` Flow's
    /// long-lived tasks hold weakly. Tests open a temporary folder this way
    /// (inside a tokio runtime: Flow's trigger side spawns its drivers);
    /// the app goes through [`AppState::new`] and starts Flow from
    /// `lib.rs`.
    pub fn open(data_dir: &Path, opts: OpenOptions) -> Result<Arc<Self>> {
        let root = Self::build(data_dir, opts)?.root();
        crate::flow::start(&root);
        Ok(root)
    }

    fn build(data_dir: &Path, opts: OpenOptions) -> Result<Self> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(data_dir)?;

        tracing::info!("Data directory: {:?}", data_dir);

        // Initialize SQLite database
        let sqlite_path = data_dir.join("openalgo.db");
        let sqlite = Arc::new(SqliteDb::new(&sqlite_path)?);

        // Initialize DuckDB for historical data
        let duckdb_path = data_dir.join("historify.duckdb");
        let duckdb = Arc::new(DuckDb::new(&duckdb_path)?);

        // Initialize security manager with file-based storage (no keychain prompts)
        let security = Arc::new(SecurityManager::new(data_dir.to_path_buf())?);

        // Initialize WebSocket manager
        let websocket = Arc::new(match &opts.app_handle {
            Some(handle) => WebSocketManager::new(handle.clone()),
            None => WebSocketManager::headless(),
        });

        // Initialize shared HTTP client for provider API calls
        let http_client = Arc::new(Client::new());

        Ok(AppState(Arc::new(AppStateInner {
            sqlite,
            duckdb,
            security,
            brokers: opts.brokers,
            websocket,
            user_session: RwLock::new(None),
            broker_session: RwLock::new(None),
            symbol_cache: DashMap::new(),
            symbol_reverse_cache: DashMap::new(),
            http_client,
            data_dir: data_dir.to_path_buf(),
            clock: opts.clock,
            flow: Arc::new(FlowModule::new()),
            symbols: SymbolStore::new(),
            internal_token: new_internal_token(),
            order_updates: events::channel(),
            app_handle: opts.app_handle,
            me: Mutex::new(Weak::new()),
        })))
    }

    /// The root `Arc` of this state: the one Flow's start-time installs
    /// hold weakly, so it must be kept alive by whoever starts Flow (the
    /// app manages it; a test holds it in its `TestCtx`). Made on first
    /// call, found again while it lives, made anew once it was dropped.
    pub fn root(&self) -> Arc<AppState> {
        let mut me = self.me.lock();
        if let Some(root) = me.upgrade() {
            return root;
        }
        let root = Arc::new(self.clone());
        *me = Arc::downgrade(&root);
        root
    }

    /// Now, by the app's clock.
    pub fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// Run synchronous database work on the blocking pool, so a slow query
    /// never holds an async worker.
    pub async fn blocking<T, F>(&self, f: F) -> Result<T>
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

    /// Stop Flow's background work, as the app does when it quits: the
    /// trigger side's drivers and monitors, then every run in flight (each
    /// gets `flow::STOP_GRACE` to write its record). A run between two
    /// order legs ends here and is recorded, never resumed blindly.
    pub async fn shutdown(&self) {
        self.flow.triggers().stop().await;
        self.flow.stop_all(crate::flow::STOP_GRACE).await;
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

#[cfg(test)]
pub mod testing {
    //! A headless state for unit tests: a temporary data folder with its
    //! own SQLite and DuckDB files, random keys (no OS keyring), a manual
    //! clock, no window, no user or broker session, a fresh database (so
    //! analyzer mode is off until a test turns it on).

    use super::*;
    use crate::clock::ManualClock;

    pub struct TestCtx {
        /// The root `Arc` (drop the `TestCtx` and Flow's weak references
        /// read the app as shutting down).
        pub ctx: Arc<AppState>,
        pub clock: Arc<ManualClock>,
        pub dir: tempfile::TempDir,
    }

    /// A state in a temporary folder with `brokers` and a clock at `now`.
    pub fn build(brokers: BrokerRegistry, now: DateTime<Utc>) -> TestCtx {
        let dir = tempfile::tempdir().expect("tempdir");
        let clock = ManualClock::new(now);
        let ctx = AppState::open(
            dir.path(),
            OpenOptions {
                clock: clock.clone(),
                brokers: Arc::new(brokers),
                app_handle: None,
            },
        )
        .expect("open state");
        TestCtx { ctx, clock, dir }
    }

    /// The state's database.
    pub fn db(state: &AppState) -> &SqliteDb {
        &state.sqlite
    }

    /// A state handle by value, for code that takes `AppState` as Tauri's
    /// `State` hands it. Its temporary folder outlives the test (the
    /// handle has no owner to remove it); use [`build`] where a `TestCtx`
    /// fits.
    pub fn object() -> AppState {
        let t = build(BrokerRegistry::new(), Utc::now());
        let _kept = t.dir.keep();
        (*t.ctx).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_headless_state_opens_with_empty_tables_and_no_runs() {
        let t = testing::build(BrokerRegistry::new(), Utc::now());
        assert!(testing::db(&t.ctx).get_clients().unwrap().is_empty());
        assert_eq!(t.ctx.flow.running(), 0);
        assert!(!t.ctx.is_authenticated());
        assert!(!t.ctx.is_broker_connected());
        assert_eq!(t.ctx.sqlite.get_analyze_mode().unwrap_or(false), false);
        assert_eq!(t.ctx.internal_token.len(), 64);
        assert!(t.ctx.app_handle.is_none());
    }

    #[tokio::test]
    async fn the_root_is_found_again_while_it_lives_and_made_anew_after() {
        let t = testing::build(BrokerRegistry::new(), Utc::now());
        let handle: AppState = (*t.ctx).clone();
        assert!(Arc::ptr_eq(&handle.root(), &t.ctx), "the same root");
        let weak = Arc::downgrade(&t.ctx);
        drop(t);
        assert!(weak.upgrade().is_none(), "dropped with the TestCtx");
        let again = handle.root();
        assert!(Arc::ptr_eq(&again, &handle.root()), "a new root, then found again");
    }

    #[tokio::test]
    async fn a_state_object_is_a_handle_over_a_fresh_folder() {
        let state = testing::object();
        assert_eq!(state.symbol_count(), 0);
        assert!(state.data_dir.join("openalgo.db").exists());
    }
}
