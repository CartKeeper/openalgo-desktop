//! SQLite database module

pub mod models;
mod connection;
mod migrations;
mod auth;
mod user;
mod api_keys;
mod symbol;
mod strategy;
mod settings;
pub mod sandbox;
mod order_logs;
mod market;
mod analyzer_logs;
mod latency_logs;
mod traffic_logs;
mod provider_keys;
mod portfolio;
mod reports;
mod watchlist;
pub mod alerts;
mod clients;
mod client_scenarios;

pub use reports::{ResearchReport, ResearchReportSummary, ReportNote};
pub use watchlist::WatchlistItem;
pub use alerts::{AlertConfig, AlertHistoryEntry, AlertGlobalSettings};
pub use clients::ClientAccount;

use crate::error::Result;
use crate::security::SecurityManager;
use crate::state::SymbolInfo;
pub use models::{AutoLogoutConfig, WebhookConfig, ApiKey, ApiKeyInfo, SandboxFunds, SandboxHolding};
pub use order_logs::{OrderLog, LogStats};
pub use market::{MarketHoliday, MarketTiming, CreateHolidayRequest, UpdateTimingRequest};
pub use analyzer_logs::{AnalyzerLog, AnalyzerLogStats};
pub use latency_logs::{LatencyLog, LatencyStats, BrokerLatencyStats};
pub use traffic_logs::{TrafficLog, TrafficStats, IPBan};
use models::*;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::Path;

/// SQLite database wrapper
pub struct SqliteDb {
    conn: Mutex<Connection>,
}

impl SqliteDb {
    /// Create new SQLite database connection
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        // Run migrations
        db.run_migrations()?;

        Ok(db)
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock();
        migrations::run_migrations(&conn)
    }

    // ========== User Methods ==========

    /// Verify user credentials
    pub fn verify_user(
        &self,
        username: &str,
        password: &str,
        security: &SecurityManager,
    ) -> Result<Option<User>> {
        let conn = self.conn.lock();
        user::verify_user(&conn, username, password, security)
    }

    /// Create a new user
    pub fn create_user(
        &self,
        username: &str,
        password: &str,
        security: &SecurityManager,
    ) -> Result<User> {
        let conn = self.conn.lock();
        user::create_user(&conn, username, password, security)
    }

    /// Check if any user exists
    pub fn has_user(&self) -> Result<bool> {
        let conn = self.conn.lock();
        user::has_user(&conn)
    }

    /// Delete all users (for password reset)
    pub fn delete_all_users(&self) -> Result<()> {
        let conn = self.conn.lock();
        user::delete_all_users(&conn)
    }

    // ========== Auth Token Methods ==========

    /// Store encrypted auth token
    pub fn store_auth_token(
        &self,
        broker_id: &str,
        auth_token: &str,
        feed_token: Option<&str>,
        security: &SecurityManager,
    ) -> Result<()> {
        let conn = self.conn.lock();
        auth::store_auth_token(&conn, broker_id, auth_token, feed_token, security)
    }

    /// Get decrypted auth token
    pub fn get_auth_token(
        &self,
        broker_id: &str,
        security: &SecurityManager,
    ) -> Result<Option<(String, Option<String>)>> {
        let conn = self.conn.lock();
        auth::get_auth_token(&conn, broker_id, security)
    }

    /// Delete auth token
    pub fn delete_auth_token(&self, broker_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        auth::delete_auth_token(&conn, broker_id)
    }

    /// Clear all auth tokens (used by auto-logout)
    pub fn clear_all_auth_tokens(&self) -> Result<()> {
        let conn = self.conn.lock();
        auth::clear_all_auth_tokens(&conn)
    }

    // ========== Symbol Methods ==========

    /// Store symbols in database
    pub fn store_symbols(&self, symbols: &[SymbolInfo]) -> Result<()> {
        let mut conn = self.conn.lock();
        symbol::store_symbols(&mut conn, symbols)
    }

    /// Load all symbols from database
    pub fn load_symbols(&self) -> Result<Vec<SymbolInfo>> {
        let conn = self.conn.lock();
        symbol::load_symbols(&conn)
    }

    // ========== Strategy Methods ==========

    /// Get all strategies
    pub fn get_strategies(&self) -> Result<Vec<Strategy>> {
        let conn = self.conn.lock();
        strategy::get_strategies(&conn)
    }

    /// Create a new strategy
    pub fn create_strategy(&self, strategy: &Strategy) -> Result<Strategy> {
        let conn = self.conn.lock();
        strategy::create_strategy(&conn, strategy)
    }

    /// Update a strategy
    pub fn update_strategy(
        &self,
        id: i64,
        name: Option<String>,
        exchange: Option<String>,
        symbol: Option<String>,
        product: Option<String>,
        quantity: Option<i32>,
        enabled: Option<bool>,
    ) -> Result<Strategy> {
        let conn = self.conn.lock();
        strategy::update_strategy(&conn, id, name, exchange, symbol, product, quantity, enabled)
    }

    /// Delete a strategy
    pub fn delete_strategy(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock();
        strategy::delete_strategy(&conn, id)
    }

    /// Get strategy by webhook_id (for webhook handler)
    pub fn get_strategy_by_webhook_id(
        &self,
        webhook_id: &str,
    ) -> Result<Option<crate::webhook::handlers::Strategy>> {
        let conn = self.conn.lock();
        strategy::get_strategy_by_webhook_id(&conn, webhook_id)
    }

    /// Get symbol mapping for a strategy (for webhook handler)
    pub fn get_symbol_mapping(
        &self,
        strategy_id: &i64,
        symbol: &str,
    ) -> Result<Option<crate::webhook::handlers::SymbolMapping>> {
        let conn = self.conn.lock();
        strategy::get_symbol_mapping(&conn, *strategy_id, symbol)
    }

    // ========== API Key Methods ==========

    /// Create a new API key
    ///
    /// Returns (id, plaintext_key) - the plaintext key is only shown once
    pub fn create_api_key(
        &self,
        name: &str,
        permissions: &str,
        security: &SecurityManager,
    ) -> Result<(i64, String)> {
        let conn = self.conn.lock();
        api_keys::create_api_key(&conn, name, permissions, security)
    }

    /// Validate API key and return the ApiKey if valid
    pub fn validate_api_key(
        &self,
        apikey: &str,
        security: &SecurityManager,
    ) -> Result<ApiKey> {
        let conn = self.conn.lock();
        api_keys::validate_api_key(&conn, apikey, security)
    }

    /// List all API keys (with masked key values)
    pub fn list_api_keys(&self, security: &SecurityManager) -> Result<Vec<ApiKeyInfo>> {
        let conn = self.conn.lock();
        api_keys::list_api_keys(&conn, security)
    }

    /// Get API key by name
    pub fn get_api_key_by_name(&self, name: &str) -> Result<Option<ApiKey>> {
        let conn = self.conn.lock();
        api_keys::get_api_key_by_name(&conn, name)
    }

    /// Delete API key by name
    pub fn delete_api_key(&self, name: &str) -> Result<bool> {
        let conn = self.conn.lock();
        api_keys::delete_api_key(&conn, name)
    }

    /// Delete API key by ID
    pub fn delete_api_key_by_id(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        api_keys::delete_api_key_by_id(&conn, id)
    }

    /// Count total API keys
    pub fn count_api_keys(&self) -> Result<i64> {
        let conn = self.conn.lock();
        api_keys::count_api_keys(&conn)
    }

    /// Get the user's API key (decrypted) - for single-user desktop app
    pub fn get_user_api_key(&self, security: &SecurityManager) -> Result<Option<String>> {
        let conn = self.conn.lock();
        api_keys::get_first_api_key_decrypted(&conn, security)
    }

    /// Check if any API key exists
    pub fn has_api_key(&self) -> Result<bool> {
        let conn = self.conn.lock();
        api_keys::has_api_key(&conn)
    }

    // ========== Settings Methods ==========

    /// Get settings
    pub fn get_settings(&self) -> Result<Settings> {
        let conn = self.conn.lock();
        settings::get_settings(&conn)
    }

    /// Update settings
    pub fn update_settings(
        &self,
        theme: Option<String>,
        default_broker: Option<String>,
        default_exchange: Option<String>,
        default_product: Option<String>,
        order_confirm: Option<bool>,
        sound_enabled: Option<bool>,
    ) -> Result<Settings> {
        let conn = self.conn.lock();
        settings::update_settings(
            &conn,
            theme,
            default_broker,
            default_exchange,
            default_product,
            order_confirm,
            sound_enabled,
        )
    }

    /// Get auto-logout configuration
    pub fn get_auto_logout_config(&self) -> Result<AutoLogoutConfig> {
        let conn = self.conn.lock();
        settings::get_auto_logout_config(&conn)
    }

    /// Update auto-logout configuration
    pub fn update_auto_logout_config(
        &self,
        enabled: Option<bool>,
        hour: Option<u32>,
        minute: Option<u32>,
        warnings: Option<Vec<u32>>,
    ) -> Result<AutoLogoutConfig> {
        let conn = self.conn.lock();
        settings::update_auto_logout_config(&conn, enabled, hour, minute, warnings)
    }

    /// Get webhook configuration
    pub fn get_webhook_config(&self) -> Result<WebhookConfig> {
        let conn = self.conn.lock();
        settings::get_webhook_config(&conn)
    }

    /// Update webhook configuration
    pub fn update_webhook_config(
        &self,
        enabled: Option<bool>,
        port: Option<u16>,
        host: Option<String>,
        ngrok_url: Option<String>,
        webhook_secret: Option<String>,
    ) -> Result<WebhookConfig> {
        let conn = self.conn.lock();
        settings::update_webhook_config(&conn, enabled, port, host, ngrok_url, webhook_secret)
    }

    /// Get rate limit configuration
    pub fn get_rate_limit_config(&self) -> Result<models::RateLimitConfig> {
        let conn = self.conn.lock();
        settings::get_rate_limit_config(&conn)
    }

    /// Update rate limit configuration
    pub fn update_rate_limit_config(
        &self,
        api_rate_limit: Option<u32>,
        order_rate_limit: Option<u32>,
        smart_order_rate_limit: Option<u32>,
        smart_order_delay: Option<f64>,
    ) -> Result<models::RateLimitConfig> {
        let conn = self.conn.lock();
        settings::update_rate_limit_config(&conn, api_rate_limit, order_rate_limit, smart_order_rate_limit, smart_order_delay)
    }

    // ========== Sandbox Methods ==========

    /// Get sandbox positions
    pub fn get_sandbox_positions(&self) -> Result<Vec<SandboxPosition>> {
        let conn = self.conn.lock();
        sandbox::get_positions(&conn)
    }

    /// Get sandbox orders
    pub fn get_sandbox_orders(&self) -> Result<Vec<SandboxOrder>> {
        let conn = self.conn.lock();
        sandbox::get_orders(&conn)
    }

    /// Place sandbox order
    pub fn place_sandbox_order(
        &self,
        symbol: &str,
        exchange: &str,
        side: &str,
        quantity: i32,
        price: f64,
        order_type: &str,
        product: &str,
    ) -> Result<SandboxOrder> {
        let conn = self.conn.lock();
        sandbox::place_order(&conn, symbol, exchange, side, quantity, price, order_type, product)
    }

    /// Reset sandbox
    pub fn reset_sandbox(&self) -> Result<()> {
        let conn = self.conn.lock();
        sandbox::reset(&conn)
    }

    /// Get sandbox holdings
    pub fn get_sandbox_holdings(&self) -> Result<Vec<SandboxHolding>> {
        let conn = self.conn.lock();
        sandbox::get_holdings(&conn)
    }

    /// Get sandbox funds
    pub fn get_sandbox_funds(&self) -> Result<SandboxFunds> {
        let conn = self.conn.lock();
        sandbox::get_funds(&conn)
    }

    /// Update sandbox LTP and recalculate P&L
    pub fn update_sandbox_ltp(&self, exchange: &str, symbol: &str, ltp: f64) -> Result<()> {
        let conn = self.conn.lock();
        sandbox::update_position_ltp(&conn, exchange, symbol, ltp)
    }

    /// Cancel sandbox order
    pub fn cancel_sandbox_order(&self, order_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        sandbox::cancel_order(&conn, order_id)
    }

    /// Get sandbox configuration
    pub fn get_sandbox_config(&self) -> Result<sandbox::SandboxConfig> {
        let conn = self.conn.lock();
        sandbox::get_config(&conn)
    }

    /// Update sandbox configuration
    pub fn update_sandbox_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        sandbox::update_config(&conn, key, value)
    }

    /// Get sandbox trades
    pub fn get_sandbox_trades(&self) -> Result<Vec<sandbox::SandboxTrade>> {
        let conn = self.conn.lock();
        sandbox::get_trades(&conn)
    }

    /// Get sandbox daily P&L history
    pub fn get_sandbox_daily_pnl(&self) -> Result<Vec<sandbox::SandboxDailyPnl>> {
        let conn = self.conn.lock();
        sandbox::get_daily_pnl(&conn)
    }

    /// Get consolidated sandbox P&L data
    pub fn get_sandbox_pnl(&self) -> Result<sandbox::SandboxPnlData> {
        let conn = self.conn.lock();
        sandbox::get_pnl_data(&conn)
    }

    // ========== Order Logs Methods ==========

    /// Create an order log entry
    #[allow(clippy::too_many_arguments)]
    pub fn create_order_log(
        &self,
        order_id: Option<&str>,
        broker: &str,
        symbol: &str,
        exchange: &str,
        side: &str,
        quantity: i32,
        price: Option<f64>,
        order_type: &str,
        product: &str,
        status: &str,
        message: Option<&str>,
        source: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        order_logs::create_log(
            &conn, order_id, broker, symbol, exchange, side, quantity, price, order_type, product, status, message, source,
        )
    }

    /// Get order logs with pagination and filters
    pub fn get_order_logs(
        &self,
        limit: usize,
        offset: usize,
        broker: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<OrderLog>> {
        let conn = self.conn.lock();
        order_logs::get_logs(&conn, limit, offset, broker, status)
    }

    /// Get logs for a specific order
    pub fn get_order_logs_by_order_id(&self, order_id: &str) -> Result<Vec<OrderLog>> {
        let conn = self.conn.lock();
        order_logs::get_logs_by_order_id(&conn, order_id)
    }

    /// Get recent order logs
    pub fn get_recent_order_logs(&self, limit: usize) -> Result<Vec<OrderLog>> {
        let conn = self.conn.lock();
        order_logs::get_recent_logs(&conn, limit)
    }

    /// Count order logs
    pub fn count_order_logs(&self, broker: Option<&str>, status: Option<&str>) -> Result<i64> {
        let conn = self.conn.lock();
        order_logs::count_logs(&conn, broker, status)
    }

    /// Clear old order logs
    pub fn clear_old_order_logs(&self, days: i32) -> Result<usize> {
        let conn = self.conn.lock();
        order_logs::clear_old_logs(&conn, days)
    }

    /// Get order log statistics
    pub fn get_order_log_stats(&self) -> Result<LogStats> {
        let conn = self.conn.lock();
        order_logs::get_stats(&conn)
    }

    // ========== Market Holiday Methods ==========

    /// Create a market holiday
    pub fn create_market_holiday(&self, req: &CreateHolidayRequest) -> Result<MarketHoliday> {
        let conn = self.conn.lock();
        market::create_holiday(&conn, req)
    }

    /// Get holidays by year
    pub fn get_market_holidays_by_year(&self, year: i32) -> Result<Vec<MarketHoliday>> {
        let conn = self.conn.lock();
        market::get_holidays_by_year(&conn, year)
    }

    /// Get holidays by exchange
    pub fn get_market_holidays_by_exchange(&self, exchange: &str, year: Option<i32>) -> Result<Vec<MarketHoliday>> {
        let conn = self.conn.lock();
        market::get_holidays_by_exchange(&conn, exchange, year)
    }

    /// Check if a date is a holiday
    pub fn is_market_holiday(&self, exchange: &str, date: &str) -> Result<bool> {
        let conn = self.conn.lock();
        market::is_holiday(&conn, exchange, date)
    }

    /// Delete a market holiday
    pub fn delete_market_holiday(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        market::delete_holiday(&conn, id)
    }

    // ========== Market Timing Methods ==========

    /// Get all market timings
    pub fn get_all_market_timings(&self) -> Result<Vec<MarketTiming>> {
        let conn = self.conn.lock();
        market::get_all_timings(&conn)
    }

    /// Get timing for an exchange
    pub fn get_market_timing(&self, exchange: &str) -> Result<Option<MarketTiming>> {
        let conn = self.conn.lock();
        market::get_timing_by_exchange(&conn, exchange)
    }

    /// Update market timing
    pub fn update_market_timing(&self, exchange: &str, req: &UpdateTimingRequest) -> Result<MarketTiming> {
        let conn = self.conn.lock();
        market::update_timing(&conn, exchange, req)
    }

    /// Create market timing
    pub fn create_market_timing(&self, timing: &MarketTiming) -> Result<MarketTiming> {
        let conn = self.conn.lock();
        market::create_timing(&conn, timing)
    }

    /// Check if market is open
    pub fn is_market_open(&self, exchange: &str) -> Result<bool> {
        let conn = self.conn.lock();
        market::is_market_open(&conn, exchange)
    }

    // ========== Analyze Mode Methods ==========

    /// Get analyze mode (sandbox/paper trading mode)
    pub fn get_analyze_mode(&self) -> Result<bool> {
        let settings = self.get_settings()?;
        Ok(settings.analyze_mode.unwrap_or(false))
    }

    /// Set analyze mode
    pub fn set_analyze_mode(&self, enabled: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE settings SET analyze_mode = ?1",
            [enabled],
        )?;
        Ok(())
    }

    // ========== Order Logging Helper ==========

    /// Log an order (convenience wrapper for order_logs::create_log)
    #[allow(clippy::too_many_arguments)]
    pub fn log_order(
        &self,
        order_id: &str,
        action: &str,
        symbol: &str,
        exchange: &str,
        side: &str,
        quantity: i32,
        price: Option<f64>,
        order_type: &str,
        product: &str,
        status: &str,
        message: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<i64> {
        // Get broker from current session (we'll use "api" as placeholder if from API)
        let broker = api_key.map(|_| "api").unwrap_or("ui");

        self.create_order_log(
            Some(order_id),
            broker,
            symbol,
            exchange,
            side,
            quantity,
            price,
            order_type,
            product,
            status,
            message,
            Some(action),
        )
    }

    // ========== Analyzer Logs Methods (Paper Trading) ==========

    /// Create analyzer log entry
    pub fn create_analyzer_log(
        &self,
        api_type: &str,
        request_data: &str,
        response_data: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::create_log(&conn, api_type, request_data, response_data)?)
    }

    /// Get analyzer logs with pagination
    pub fn get_analyzer_logs(
        &self,
        api_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AnalyzerLog>> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::get_logs(&conn, api_type, limit, offset)?)
    }

    /// Get recent analyzer logs
    pub fn get_recent_analyzer_logs(&self, limit: i64) -> Result<Vec<AnalyzerLog>> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::get_recent_logs(&conn, limit)?)
    }

    /// Count analyzer logs
    pub fn count_analyzer_logs(&self, api_type: Option<&str>) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::count_logs(&conn, api_type)?)
    }

    /// Clear old analyzer logs
    pub fn clear_old_analyzer_logs(&self, days: i64) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::clear_old_logs(&conn, days)?)
    }

    /// Clear all analyzer logs
    pub fn clear_all_analyzer_logs(&self) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::clear_all_logs(&conn)?)
    }

    /// Get analyzer log statistics
    pub fn get_analyzer_log_stats(&self) -> Result<AnalyzerLogStats> {
        let conn = self.conn.lock();
        Ok(analyzer_logs::get_stats(&conn)?)
    }

    // ========== Latency Logs Methods (Performance Monitoring) ==========

    /// Log latency for an order/request
    #[allow(clippy::too_many_arguments)]
    pub fn log_latency(
        &self,
        order_id: &str,
        broker: &str,
        symbol: &str,
        order_type: &str,
        rtt_ms: f64,
        validation_ms: f64,
        broker_response_ms: f64,
        overhead_ms: f64,
        total_ms: f64,
        status: &str,
        error: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(latency_logs::log_latency(
            &conn, order_id, broker, symbol, order_type,
            rtt_ms, validation_ms, broker_response_ms, overhead_ms, total_ms,
            status, error,
        )?)
    }

    /// Get recent latency logs
    pub fn get_recent_latency_logs(&self, limit: i64) -> Result<Vec<LatencyLog>> {
        let conn = self.conn.lock();
        Ok(latency_logs::get_recent_logs(&conn, limit)?)
    }

    /// Get latency statistics
    pub fn get_latency_stats(&self) -> Result<LatencyStats> {
        let conn = self.conn.lock();
        Ok(latency_logs::get_stats(&conn)?)
    }

    /// Purge old non-order latency logs
    pub fn purge_old_latency_logs(&self, days: i64) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(latency_logs::purge_old_data_logs(&conn, days)?)
    }

    /// Clear all latency logs
    pub fn clear_all_latency_logs(&self) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(latency_logs::clear_all_logs(&conn)?)
    }

    // ========== Traffic Logs Methods (HTTP Monitoring) ==========

    /// Log HTTP request
    pub fn log_traffic(
        &self,
        client_ip: &str,
        method: &str,
        path: &str,
        status_code: i32,
        duration_ms: f64,
        host: Option<&str>,
        error: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(traffic_logs::log_request(&conn, client_ip, method, path, status_code, duration_ms, host, error)?)
    }

    /// Get recent traffic logs
    pub fn get_recent_traffic_logs(&self, limit: i64) -> Result<Vec<TrafficLog>> {
        let conn = self.conn.lock();
        Ok(traffic_logs::get_recent_logs(&conn, limit)?)
    }

    /// Get traffic statistics
    pub fn get_traffic_stats(&self) -> Result<TrafficStats> {
        let conn = self.conn.lock();
        Ok(traffic_logs::get_stats(&conn)?)
    }

    /// Clear old traffic logs
    pub fn clear_old_traffic_logs(&self, days: i64) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(traffic_logs::clear_old_logs(&conn, days)?)
    }

    // ========== IP Ban Methods (Security) ==========

    /// Check if IP is banned
    pub fn is_ip_banned(&self, ip_address: &str) -> Result<bool> {
        let conn = self.conn.lock();
        Ok(traffic_logs::is_ip_banned(&conn, ip_address)?)
    }

    /// Ban an IP address
    pub fn ban_ip(
        &self,
        ip_address: &str,
        reason: &str,
        duration_hours: Option<i64>,
        permanent: bool,
        created_by: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        Ok(traffic_logs::ban_ip(&conn, ip_address, reason, duration_hours, permanent, created_by)?)
    }

    /// Unban an IP address
    pub fn unban_ip(&self, ip_address: &str) -> Result<bool> {
        let conn = self.conn.lock();
        Ok(traffic_logs::unban_ip(&conn, ip_address)?)
    }

    /// Get all IP bans
    pub fn get_all_ip_bans(&self) -> Result<Vec<IPBan>> {
        let conn = self.conn.lock();
        Ok(traffic_logs::get_all_bans(&conn)?)
    }

    // ========== Error Tracking Methods (Security) ==========

    /// Track 404 error
    pub fn track_404(&self, ip_address: &str, path: &str) -> Result<()> {
        let conn = self.conn.lock();
        Ok(traffic_logs::track_404(&conn, ip_address, path)?)
    }

    /// Get suspicious IPs with high 404 counts
    pub fn get_suspicious_404_ips(&self, min_errors: i32) -> Result<Vec<(String, i32, String)>> {
        let conn = self.conn.lock();
        Ok(traffic_logs::get_suspicious_404_ips(&conn, min_errors)?)
    }

    /// Track invalid API key attempt
    pub fn track_invalid_api_key(&self, ip_address: &str, api_key_hash: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        Ok(traffic_logs::track_invalid_api_key(&conn, ip_address, api_key_hash)?)
    }

    /// Get suspicious API users
    pub fn get_suspicious_api_users(&self, min_attempts: i32) -> Result<Vec<(String, i32)>> {
        let conn = self.conn.lock();
        Ok(traffic_logs::get_suspicious_api_users(&conn, min_attempts)?)
    }

    // ========== Provider API Key Methods ==========

    /// Save an encrypted provider API key
    pub fn save_provider_key(
        &self,
        provider: &str,
        api_key: &str,
        security: &SecurityManager,
    ) -> Result<()> {
        let conn = self.conn.lock();
        provider_keys::save_provider_key(&conn, provider, api_key, security)
    }

    /// Get a decrypted provider API key
    pub fn get_provider_key(
        &self,
        provider: &str,
        security: &SecurityManager,
    ) -> Result<Option<String>> {
        let conn = self.conn.lock();
        provider_keys::get_provider_key(&conn, provider, security)
    }

    /// Delete a provider API key
    pub fn delete_provider_key(&self, provider: &str) -> Result<bool> {
        let conn = self.conn.lock();
        provider_keys::delete_provider_key(&conn, provider)
    }

    /// Get list of configured provider names
    pub fn get_configured_provider_names(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        provider_keys::get_configured_providers(&conn)
    }

    // ========== Manual Portfolio Methods ==========

    /// Add a portfolio position
    #[allow(clippy::too_many_arguments)]
    pub fn add_portfolio_position(
        &self,
        symbol: &str,
        exchange: &str,
        quantity: f64,
        avg_price: f64,
        position_type: &str,
        asset_class: Option<&str>,
        notes: Option<&str>,
    ) -> Result<crate::providers::types::PortfolioPosition> {
        let conn = self.conn.lock();
        portfolio::add_position(&conn, symbol, exchange, quantity, avg_price, position_type, asset_class, notes)
    }

    /// Update a portfolio position
    pub fn update_portfolio_position(
        &self,
        id: i64,
        quantity: Option<f64>,
        avg_price: Option<f64>,
        notes: Option<&str>,
    ) -> Result<crate::providers::types::PortfolioPosition> {
        let conn = self.conn.lock();
        portfolio::update_position(&conn, id, quantity, avg_price, notes)
    }

    /// Delete a portfolio position
    pub fn delete_portfolio_position(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        portfolio::delete_position(&conn, id)
    }

    /// Get all portfolio positions
    pub fn get_portfolio_positions(&self) -> Result<Vec<crate::providers::types::PortfolioPosition>> {
        let conn = self.conn.lock();
        portfolio::get_positions(&conn)
    }

    /// Import portfolio positions from CSV data
    pub fn import_portfolio_positions(
        &self,
        positions: &[crate::providers::types::PortfolioPosition],
    ) -> Result<usize> {
        let conn = self.conn.lock();
        portfolio::import_positions(&conn, positions)
    }

    /// Export portfolio positions
    pub fn export_portfolio_positions(&self) -> Result<Vec<crate::providers::types::PortfolioPosition>> {
        let conn = self.conn.lock();
        portfolio::export_positions(&conn)
    }

    /// Get generic mode flag
    pub fn get_generic_mode(&self) -> Result<bool> {
        let conn = self.conn.lock();
        portfolio::get_generic_mode(&conn)
    }

    /// Set generic mode flag
    pub fn set_generic_mode(&self, enabled: bool) -> Result<()> {
        let conn = self.conn.lock();
        portfolio::set_generic_mode(&conn, enabled)
    }

    // ========== Configured Brokers Methods (Keychain Optimization) ==========

    /// Mark a broker as configured (called when credentials are saved)
    pub fn mark_broker_configured(&self, broker_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO configured_brokers (broker_id) VALUES (?1)",
            [broker_id],
        )?;
        Ok(())
    }

    /// Remove broker from configured list (called when credentials are deleted)
    pub fn unmark_broker_configured(&self, broker_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM configured_brokers WHERE broker_id = ?1",
            [broker_id],
        )?;
        Ok(())
    }

    /// Get list of configured broker IDs
    pub fn get_configured_brokers(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT broker_id FROM configured_brokers")?;
        let brokers = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(brokers)
    }

    /// Check if a broker is configured
    pub fn is_broker_configured(&self, broker_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM configured_brokers WHERE broker_id = ?1)",
            [broker_id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    // ========== Broker Credentials Methods (Encrypted SQLite Storage) ==========

    /// Store broker credentials (encrypted)
    pub fn store_broker_credentials(
        &self,
        broker_id: &str,
        api_key_encrypted: &str,
        api_key_nonce: &str,
        api_secret_encrypted: Option<&str>,
        api_secret_nonce: Option<&str>,
        client_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO broker_credentials
             (broker_id, api_key_encrypted, api_key_nonce, api_secret_encrypted, api_secret_nonce, client_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            rusqlite::params![
                broker_id,
                api_key_encrypted,
                api_key_nonce,
                api_secret_encrypted,
                api_secret_nonce,
                client_id,
            ],
        )?;
        Ok(())
    }

    /// Get broker credentials (encrypted)
    pub fn get_broker_credentials(
        &self,
        broker_id: &str,
    ) -> Result<Option<(String, String, Option<String>, Option<String>, Option<String>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT api_key_encrypted, api_key_nonce, api_secret_encrypted, api_secret_nonce, client_id
             FROM broker_credentials WHERE broker_id = ?1",
        )?;

        let result = stmt.query_row([broker_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        });

        match result {
            Ok(creds) => Ok(Some(creds)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete broker credentials
    pub fn delete_broker_credentials(&self, broker_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM broker_credentials WHERE broker_id = ?1",
            [broker_id],
        )?;
        Ok(())
    }

    // ========== Research Reports Methods ==========

    pub fn save_research_report(
        &self,
        title: &str,
        summary: Option<&str>,
        tags: Option<&str>,
        messages_json: &str,
        tool_calls_json: Option<&str>,
    ) -> Result<ResearchReport> {
        let conn = self.conn.lock();
        reports::save_report(&conn, title, summary, tags, messages_json, tool_calls_json)
    }

    pub fn get_research_reports(&self) -> Result<Vec<ResearchReportSummary>> {
        let conn = self.conn.lock();
        reports::get_reports(&conn)
    }

    pub fn get_research_report(&self, id: i64) -> Result<ResearchReport> {
        let conn = self.conn.lock();
        reports::get_report(&conn, id)
    }

    pub fn delete_research_report(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        reports::delete_report(&conn, id)
    }

    pub fn update_research_report_title(&self, id: i64, title: &str) -> Result<ResearchReport> {
        let conn = self.conn.lock();
        reports::update_report_title(&conn, id, title)
    }

    pub fn add_report_note(&self, report_id: i64, content: &str) -> Result<ReportNote> {
        let conn = self.conn.lock();
        reports::add_note(&conn, report_id, content)
    }

    pub fn get_report_notes(&self, report_id: i64) -> Result<Vec<ReportNote>> {
        let conn = self.conn.lock();
        reports::get_notes(&conn, report_id)
    }

    pub fn update_report_note(&self, note_id: i64, content: &str) -> Result<ReportNote> {
        let conn = self.conn.lock();
        reports::update_note(&conn, note_id, content)
    }

    pub fn delete_report_note(&self, note_id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        reports::delete_note(&conn, note_id)
    }

    // ========== Watchlist Methods ==========

    pub fn add_watchlist_symbol(&self, symbol: &str) -> Result<WatchlistItem> {
        let conn = self.conn.lock();
        watchlist::add_symbol(&conn, symbol)
    }

    pub fn remove_watchlist_symbol(&self, symbol: &str) -> Result<bool> {
        let conn = self.conn.lock();
        watchlist::remove_symbol(&conn, symbol)
    }

    pub fn get_watchlist_symbols(&self) -> Result<Vec<WatchlistItem>> {
        let conn = self.conn.lock();
        watchlist::get_symbols(&conn)
    }

    // ========== Alert Methods ==========

    pub fn create_alert(&self, alert: &AlertConfig) -> Result<AlertConfig> {
        let conn = self.conn.lock();
        alerts::create_alert(&conn, alert)
    }

    pub fn get_alerts(&self) -> Result<Vec<AlertConfig>> {
        let conn = self.conn.lock();
        alerts::get_alerts(&conn)
    }

    pub fn get_enabled_alerts(&self) -> Result<Vec<AlertConfig>> {
        let conn = self.conn.lock();
        alerts::get_enabled_alerts(&conn)
    }

    pub fn update_alert(&self, id: i64, alert: &AlertConfig) -> Result<AlertConfig> {
        let conn = self.conn.lock();
        alerts::update_alert(&conn, id, alert)
    }

    pub fn delete_alert(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        alerts::delete_alert(&conn, id)
    }

    pub fn toggle_alert(&self, id: i64, enabled: bool) -> Result<()> {
        let conn = self.conn.lock();
        alerts::toggle_alert(&conn, id, enabled)
    }

    pub fn log_alert(
        &self,
        config_id: Option<i64>,
        alert_type: &str,
        symbol: &str,
        title: &str,
        message: &str,
        metadata_json: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        alerts::log_alert(&conn, config_id, alert_type, symbol, title, message, metadata_json)
    }

    pub fn get_alert_history(&self, limit: i64, offset: i64) -> Result<Vec<AlertHistoryEntry>> {
        let conn = self.conn.lock();
        alerts::get_alert_history(&conn, limit, offset)
    }

    pub fn acknowledge_alert(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        alerts::acknowledge_alert(&conn, id)
    }

    pub fn acknowledge_all_alerts(&self) -> Result<i64> {
        let conn = self.conn.lock();
        alerts::acknowledge_all_alerts(&conn)
    }

    pub fn get_unacknowledged_count(&self) -> Result<i64> {
        let conn = self.conn.lock();
        alerts::get_unacknowledged_count(&conn)
    }

    pub fn get_alert_settings(&self) -> Result<AlertGlobalSettings> {
        let conn = self.conn.lock();
        alerts::get_global_settings(&conn)
    }

    pub fn update_alert_settings(&self, settings: &AlertGlobalSettings) -> Result<AlertGlobalSettings> {
        let conn = self.conn.lock();
        alerts::update_global_settings(&conn, settings)
    }

    pub fn update_alert_last_triggered(&self, alert_id: i64) -> Result<()> {
        let conn = self.conn.lock();
        alerts::update_last_triggered(&conn, alert_id)
    }

    // ========== Client Management Methods ==========

    /// Add a new client
    #[allow(clippy::too_many_arguments)]
    pub fn add_client(
        &self,
        name: &str,
        email: Option<&str>,
        phone: Option<&str>,
        broker: Option<&str>,
        account_id: Option<&str>,
        account_type: Option<&str>,
        notes: Option<&str>,
    ) -> Result<crate::providers::types::Client> {
        let conn = self.conn.lock();
        clients::add_client(&conn, name, email, phone, broker, account_id, account_type, notes)
    }

    /// Get all clients
    pub fn get_clients(&self) -> Result<Vec<crate::providers::types::Client>> {
        let conn = self.conn.lock();
        clients::get_clients(&conn)
    }

    /// Get a single client by ID
    pub fn get_client_by_id(&self, id: i64) -> Result<crate::providers::types::Client> {
        let conn = self.conn.lock();
        clients::get_client_by_id(&conn, id)
    }

    /// Update a client
    #[allow(clippy::too_many_arguments)]
    pub fn update_client(
        &self,
        id: i64,
        name: Option<&str>,
        email: Option<&str>,
        phone: Option<&str>,
        broker: Option<&str>,
        account_id: Option<&str>,
        account_type: Option<&str>,
        notes: Option<&str>,
    ) -> Result<crate::providers::types::Client> {
        let conn = self.conn.lock();
        clients::update_client(&conn, id, name, email, phone, broker, account_id, account_type, notes)
    }

    /// Delete a client
    pub fn delete_client(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        clients::delete_client(&conn, id)
    }

    /// Add a trade for a client
    #[allow(clippy::too_many_arguments)]
    pub fn add_client_trade(
        &self,
        client_id: i64,
        symbol: &str,
        exchange: &str,
        trade_date: &str,
        trade_type: &str,
        quantity: f64,
        price: f64,
        fees: f64,
        order_id: Option<&str>,
        notes: Option<&str>,
        import_batch_id: Option<i64>,
    ) -> Result<crate::providers::types::ClientTrade> {
        let conn = self.conn.lock();
        clients::add_trade(
            &conn, client_id, symbol, exchange, trade_date, trade_type, quantity, price, fees,
            order_id, notes, import_batch_id,
        )
    }

    /// Get all trades for a client
    pub fn get_client_trades(&self, client_id: i64) -> Result<Vec<crate::providers::types::ClientTrade>> {
        let conn = self.conn.lock();
        clients::get_trades(&conn, client_id)
    }

    /// Delete a single trade
    pub fn delete_client_trade(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        clients::delete_trade(&conn, id)
    }

    /// Create an import batch
    pub fn add_import_batch(
        &self,
        client_id: i64,
        filename: &str,
        row_count: i64,
        account_type: Option<&str>,
    ) -> Result<crate::providers::types::ImportBatch> {
        let conn = self.conn.lock();
        clients::add_import_batch(&conn, client_id, filename, row_count, account_type)
    }

    /// Get import batches for a client
    pub fn get_import_batches(&self, client_id: i64) -> Result<Vec<crate::providers::types::ImportBatch>> {
        let conn = self.conn.lock();
        clients::get_import_batches(&conn, client_id)
    }

    /// Delete an import batch and its trades
    pub fn delete_import_batch(&self, batch_id: i64) -> Result<usize> {
        let conn = self.conn.lock();
        clients::delete_import_batch(&conn, batch_id)
    }

    /// Update account_type on an existing import batch
    pub fn update_import_batch_account_type(
        &self,
        batch_id: i64,
        account_type: Option<&str>,
    ) -> Result<crate::providers::types::ImportBatch> {
        let conn = self.conn.lock();
        clients::update_import_batch_account_type(&conn, batch_id, account_type)
    }

    /// Get computed positions for a client
    pub fn get_client_positions(&self, client_id: i64) -> Result<Vec<crate::providers::types::ClientPosition>> {
        let conn = self.conn.lock();
        clients::get_client_positions(&conn, client_id)
    }

    /// Get computed positions filtered by account_type
    pub fn get_client_positions_by_account(&self, client_id: i64, account_type: &str) -> Result<Vec<crate::providers::types::ClientPosition>> {
        let conn = self.conn.lock();
        clients::get_client_positions_by_account(&conn, client_id, account_type)
    }

    /// Get computed positions split by each account (not aggregated)
    pub fn get_client_positions_by_each_account(&self, client_id: i64) -> Result<Vec<crate::providers::types::ClientPosition>> {
        let conn = self.conn.lock();
        clients::get_client_positions_by_each_account(&conn, client_id)
    }

    /// Get distinct accounts for a client (from import batches)
    pub fn get_client_accounts(&self, client_id: i64) -> Result<Vec<clients::ClientAccount>> {
        let conn = self.conn.lock();
        clients::get_client_accounts(&conn, client_id)
    }

    /// Get trades filtered by account_type
    pub fn get_client_trades_by_account(&self, client_id: i64, account_type: &str) -> Result<Vec<crate::providers::types::ClientTrade>> {
        let conn = self.conn.lock();
        clients::get_trades_by_account(&conn, client_id, account_type)
    }

    // ========== Client Scenario Methods ==========

    /// Create a new client scenario
    pub fn create_client_scenario(
        &self,
        client_id: i64,
        name: &str,
        description: Option<&str>,
        is_baseline: bool,
    ) -> Result<crate::providers::types::ClientScenario> {
        let conn = self.conn.lock();
        client_scenarios::create_scenario(&conn, client_id, name, description, is_baseline)
    }

    /// Get all scenarios for a client
    pub fn get_client_scenarios(&self, client_id: i64) -> Result<Vec<crate::providers::types::ClientScenario>> {
        let conn = self.conn.lock();
        client_scenarios::get_scenarios(&conn, client_id)
    }

    /// Get a single scenario by ID
    pub fn get_client_scenario_by_id(&self, id: i64) -> Result<crate::providers::types::ClientScenario> {
        let conn = self.conn.lock();
        client_scenarios::get_scenario_by_id(&conn, id)
    }

    /// Update a scenario
    pub fn update_client_scenario(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<crate::providers::types::ClientScenario> {
        let conn = self.conn.lock();
        client_scenarios::update_scenario(&conn, id, name, description)
    }

    /// Delete a scenario
    pub fn delete_client_scenario(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        client_scenarios::delete_scenario(&conn, id)
    }

    /// Clone a scenario with all positions
    pub fn clone_client_scenario(
        &self,
        source_id: i64,
        client_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<crate::providers::types::ClientScenario> {
        let conn = self.conn.lock();
        client_scenarios::clone_scenario(&conn, source_id, client_id, name, description)
    }

    /// Sync baseline scenarios from trade history (one per account type)
    pub fn sync_baseline_scenarios(&self, client_id: i64) -> Result<Vec<crate::providers::types::ClientScenario>> {
        let conn = self.conn.lock();
        client_scenarios::sync_baseline_scenarios(&conn, client_id)
    }

    /// Get all positions for a scenario
    pub fn get_scenario_positions(&self, scenario_id: i64) -> Result<Vec<crate::providers::types::ScenarioPosition>> {
        let conn = self.conn.lock();
        client_scenarios::get_scenario_positions(&conn, scenario_id)
    }

    /// Add a position to a scenario
    #[allow(clippy::too_many_arguments)]
    pub fn add_scenario_position(
        &self,
        scenario_id: i64,
        symbol: &str,
        exchange: &str,
        quantity: f64,
        avg_price: f64,
        side: &str,
        notes: Option<&str>,
    ) -> Result<crate::providers::types::ScenarioPosition> {
        let conn = self.conn.lock();
        client_scenarios::add_scenario_position(&conn, scenario_id, symbol, exchange, quantity, avg_price, side, notes)
    }

    /// Update a scenario position
    pub fn update_scenario_position(
        &self,
        id: i64,
        quantity: Option<f64>,
        avg_price: Option<f64>,
        side: Option<&str>,
        notes: Option<&str>,
    ) -> Result<crate::providers::types::ScenarioPosition> {
        let conn = self.conn.lock();
        client_scenarios::update_scenario_position(&conn, id, quantity, avg_price, side, notes)
    }

    /// Delete a scenario position
    pub fn delete_scenario_position(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        client_scenarios::delete_scenario_position(&conn, id)
    }

    /// Apply a simulated trade to a scenario
    pub fn apply_scenario_trade(
        &self,
        scenario_id: i64,
        symbol: &str,
        exchange: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<crate::providers::types::ScenarioPosition> {
        let conn = self.conn.lock();
        client_scenarios::apply_trade_to_scenario(&conn, scenario_id, symbol, exchange, side, quantity, price)
    }
}
