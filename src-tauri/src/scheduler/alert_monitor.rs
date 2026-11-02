//! Background alert monitoring loop
//!
//! Periodically checks price alerts, movement alerts, and scans for rising stars.
//! Follows the same spawned-thread pattern as `AutoLogoutScheduler`.

use crate::providers::fmp::FmpClient;
use crate::providers::yahoo::YahooClient;
use crate::services::alert_service::{self, AlertConfig, TriggeredAlert};
use crate::state::AppState;
use chrono::Utc;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

/// Default check interval in seconds (2 minutes).
const DEFAULT_CHECK_INTERVAL_SECS: u64 = 120;

/// Default movement alert threshold (%).
const DEFAULT_MOVEMENT_THRESHOLD: f64 = 5.0;

/// Default rising-star scan interval in seconds (10 minutes).
const DEFAULT_RISING_STAR_INTERVAL_SECS: u64 = 600;

/// Default minimum % change for rising star candidates.
const DEFAULT_RISING_STAR_MIN_CHANGE_PCT: f64 = 8.0;

/// Maximum number of rising-star alerts emitted per scan (top movers only).
const RISING_STAR_MAX_PER_SCAN: usize = 3;

/// Per-symbol cooldown for rising-star alerts (seconds). The same symbol will
/// not re-fire a rising-star toast again until this much time has elapsed.
const RISING_STAR_PER_SYMBOL_COOLDOWN_SECS: i64 = 4 * 60 * 60; // 4 hours

/// Default cooldown for alert configs (minutes).
const DEFAULT_COOLDOWN_MINUTES: i64 = 30;

/// Event payload emitted to the frontend when an alert fires.
#[derive(Clone, Serialize)]
pub struct AlertTriggeredPayload {
    pub id: Option<i64>,
    pub alert_type: String,
    pub symbol: String,
    pub title: String,
    pub message: String,
    pub metadata_json: Option<String>,
    pub timestamp: String,
}

/// Background alert monitor.
pub struct AlertMonitor {
    app_handle: AppHandle,
}

impl AlertMonitor {
    /// Create a new alert monitor.
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Start the monitoring loop in a background thread.
    ///
    /// The loop:
    /// 1. Sleeps for the configured check interval.
    /// 2. Reads global settings to see if alerting is enabled.
    /// 3. Fetches quotes for watchlist + alert-config symbols.
    /// 4. Runs price and movement checks.
    /// 5. Periodically scans for rising stars via FMP.
    /// 6. Emits events and logs history for every triggered alert.
    pub fn start(self) {
        std::thread::spawn(move || {
            info!("Alert monitor started");

            // Create a tokio runtime for async provider calls inside this thread.
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create tokio runtime for alert monitor: {}", e);
                    return;
                }
            };

            // Mutable state kept across cycles.
            let mut previous_prices: HashMap<String, f64> = HashMap::new();
            let mut cycles_since_rising_star: u64 = 0;
            // Tracks the last unix timestamp (seconds) a rising-star alert was
            // emitted for each symbol, so the same mover does not re-fire every
            // scan. See RISING_STAR_PER_SYMBOL_COOLDOWN_SECS.
            let mut rising_star_last_emitted: HashMap<String, i64> = HashMap::new();

            loop {
                // --- Determine check interval ---
                let check_interval = DEFAULT_CHECK_INTERVAL_SECS;

                std::thread::sleep(Duration::from_secs(check_interval));

                // --- Obtain AppState ---
                let state = match self.app_handle.try_state::<AppState>() {
                    Some(s) => s,
                    None => {
                        warn!("AppState not available in alert monitor, will retry next cycle");
                        continue;
                    }
                };

                // --- Read alert configs ---
                // Since there is no dedicated DB module for alerts yet, we build
                // configs from the watchlist (movement alerts) and any future
                // alert_configs table.  For now we operate on watchlist symbols
                // with sensible defaults so the monitor is functional immediately.
                let watchlist_symbols: Vec<String> = match state.sqlite.get_watchlist_symbols() {
                    Ok(items) => items.into_iter().map(|w| w.symbol).collect(),
                    Err(e) => {
                        warn!("Failed to read watchlist: {}", e);
                        Vec::new()
                    }
                };

                if watchlist_symbols.is_empty() {
                    // Nothing to monitor — skip this cycle.
                    continue;
                }

                // Build symbol refs for the Yahoo call.
                let symbol_refs: Vec<&str> = watchlist_symbols.iter().map(|s| s.as_str()).collect();

                // --- Fetch quotes ---
                let yahoo = YahooClient::new(state.http_client.as_ref().clone());
                let quotes_result = rt.block_on(yahoo.get_quotes(&symbol_refs));

                let generic_quotes = match quotes_result {
                    Ok(q) => q,
                    Err(e) => {
                        warn!("Alert monitor: failed to fetch quotes: {}", e);
                        continue;
                    }
                };

                // Build (symbol, price) tuples.
                let quote_tuples: Vec<(String, f64)> = generic_quotes
                    .iter()
                    .map(|q| (q.symbol.clone(), q.price))
                    .collect();

                // --- Build implicit movement configs from watchlist ---
                let movement_configs: Vec<AlertConfig> = watchlist_symbols
                    .iter()
                    .map(|sym| AlertConfig {
                        id: None,
                        alert_type: "movement".to_string(),
                        symbol: sym.clone(),
                        enabled: true,
                        threshold: Some(DEFAULT_MOVEMENT_THRESHOLD),
                        cooldown_minutes: DEFAULT_COOLDOWN_MINUTES,
                        last_triggered_at: None,
                        config_json: None,
                    })
                    .collect();

                // --- Check price alerts (placeholder — no persisted configs yet) ---
                // When an alert_configs table exists, load them here and call:
                //   alert_service::check_price_alerts(&price_configs, &quote_tuples);
                let price_triggered: Vec<TriggeredAlert> = Vec::new();

                // --- Check movement alerts ---
                let movement_triggered = alert_service::check_movement_alerts(
                    &movement_configs,
                    &quote_tuples,
                    &previous_prices,
                    DEFAULT_MOVEMENT_THRESHOLD,
                );

                // Update previous prices for the next cycle.
                for (sym, price) in &quote_tuples {
                    previous_prices.insert(sym.clone(), *price);
                }

                // --- Emit triggered alerts ---
                let all_triggered: Vec<TriggeredAlert> = price_triggered
                    .into_iter()
                    .chain(movement_triggered)
                    .collect();

                for alert in &all_triggered {
                    self.emit_alert(alert);
                }

                // --- Rising star scanner (runs less frequently) ---
                cycles_since_rising_star += 1;
                let rising_star_cycles =
                    DEFAULT_RISING_STAR_INTERVAL_SECS / check_interval.max(1);

                if cycles_since_rising_star >= rising_star_cycles {
                    cycles_since_rising_star = 0;
                    self.scan_rising_stars(
                        &rt,
                        &state,
                        &watchlist_symbols,
                        &mut rising_star_last_emitted,
                    );
                }
            }
        });
    }

    // ----- private helpers -----

    /// Emit an alert event to the frontend.
    fn emit_alert(&self, alert: &TriggeredAlert) {
        let payload = AlertTriggeredPayload {
            id: alert.alert_config_id,
            alert_type: alert.alert_type.clone(),
            symbol: alert.symbol.clone(),
            title: alert.title.clone(),
            message: alert.message.clone(),
            metadata_json: alert.metadata_json.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };

        info!(
            "Alert triggered: [{}] {} — {}",
            payload.alert_type, payload.symbol, payload.title
        );

        if let Err(e) = self.app_handle.emit("alert_triggered", payload) {
            warn!("Failed to emit alert_triggered event: {}", e);
        }
    }

    /// Scan FMP gainers + most active for rising star candidates.
    fn scan_rising_stars(
        &self,
        rt: &tokio::runtime::Runtime,
        state: &AppState,
        watchlist_symbols: &[String],
        last_emitted: &mut HashMap<String, i64>,
    ) {
        // FMP key is required for this feature.
        let fmp_key = match state
            .sqlite
            .get_provider_key("fmp", &state.security)
        {
            Ok(Some(key)) => key,
            Ok(None) => {
                // No FMP key configured — silently skip.
                return;
            }
            Err(e) => {
                warn!("Alert monitor: failed to retrieve FMP key: {}", e);
                return;
            }
        };

        let fmp = FmpClient::new(state.http_client.as_ref().clone(), fmp_key);

        let watchlist_set: HashSet<&str> = watchlist_symbols.iter().map(|s| s.as_str()).collect();

        // Fetch gainers.
        let gainers = match rt.block_on(fmp.get_market_gainers()) {
            Ok(g) => g,
            Err(e) => {
                warn!("Alert monitor: FMP gainers fetch failed: {}", e);
                Vec::new()
            }
        };

        // Fetch most active.
        let most_active = match rt.block_on(fmp.get_market_most_active()) {
            Ok(a) => a,
            Err(e) => {
                warn!("Alert monitor: FMP most_active fetch failed: {}", e);
                Vec::new()
            }
        };

        let min_change = DEFAULT_RISING_STAR_MIN_CHANGE_PCT;
        let now = Utc::now().timestamp();

        // --- Build the candidate list (filtered, deduped, cooldown-respecting) ---
        let mut seen = HashSet::new();
        let mut candidates: Vec<(&crate::providers::types::MarketMover, f64)> = Vec::new();

        for mover in gainers.iter().chain(most_active.iter()) {
            if mover.symbol.is_empty() {
                continue;
            }

            // Skip symbols already on watchlist.
            if watchlist_set.contains(mover.symbol.as_str()) {
                continue;
            }

            // Skip duplicates within this scan.
            if !seen.insert(mover.symbol.clone()) {
                continue;
            }

            let change_pct = match mover.change_percent {
                Some(cp) if cp.abs() >= min_change => cp,
                _ => continue,
            };

            // Per-symbol cooldown: skip if this symbol fired recently.
            if let Some(&last) = last_emitted.get(&mover.symbol) {
                if now - last < RISING_STAR_PER_SYMBOL_COOLDOWN_SECS {
                    continue;
                }
            }

            candidates.push((mover, change_pct));
        }

        // --- Keep only the top movers by % change, capped per scan ---
        candidates.sort_by(|a, b| {
            b.1.abs()
                .partial_cmp(&a.1.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(RISING_STAR_MAX_PER_SCAN);

        // --- Emit the selected alerts and record their timestamps ---
        for (mover, change_pct) in candidates {
            let metadata = serde_json::json!({
                "price": mover.price,
                "change": mover.change,
                "change_percent": change_pct,
                "name": mover.name,
            });

            let alert = TriggeredAlert {
                alert_config_id: None,
                alert_type: "rising_star".to_string(),
                symbol: mover.symbol.clone(),
                title: format!(
                    "{} +{:.1}%",
                    mover.symbol,
                    change_pct
                ),
                message: format!(
                    "{} ({}) is up {:.2}% — not on your watchlist",
                    mover.symbol,
                    mover.name.as_deref().unwrap_or(""),
                    change_pct
                ),
                metadata_json: Some(metadata.to_string()),
            };

            self.emit_alert(&alert);
            last_emitted.insert(mover.symbol.clone(), now);
        }
    }
}
