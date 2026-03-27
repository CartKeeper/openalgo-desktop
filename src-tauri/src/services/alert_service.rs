//! Alert checking logic
//!
//! Stateless functions that take alert configurations and market data,
//! returning any alerts that have triggered. No internal state is held.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a single alert rule.
/// This mirrors what would be stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub id: Option<i64>,
    pub alert_type: String,
    pub symbol: String,
    pub enabled: bool,
    /// Threshold value (e.g., target price, % change threshold)
    pub threshold: Option<f64>,
    /// Cooldown in minutes before the same alert can fire again
    pub cooldown_minutes: i64,
    /// ISO 8601 timestamp of the last time this alert fired
    pub last_triggered_at: Option<String>,
    /// Optional JSON blob for extra config (e.g., time window for movement alerts)
    pub config_json: Option<String>,
}

/// A triggered alert ready to be emitted / logged.
#[derive(Debug, Clone, Serialize)]
pub struct TriggeredAlert {
    pub alert_config_id: Option<i64>,
    pub alert_type: String,
    pub symbol: String,
    pub title: String,
    pub message: String,
    pub metadata_json: Option<String>,
}

/// Check price-based alerts (price_above / price_below).
///
/// `configs` — enabled alert configs with `alert_type` of `"price_above"` or `"price_below"`.
/// `quotes` — slice of `(symbol, current_price)` tuples.
///
/// Returns a vec of triggered alerts for configs whose conditions are met and
/// that are not in cooldown.
pub fn check_price_alerts(
    configs: &[AlertConfig],
    quotes: &[(String, f64)],
) -> Vec<TriggeredAlert> {
    let price_map: HashMap<&str, f64> = quotes
        .iter()
        .map(|(sym, price)| (sym.as_str(), *price))
        .collect();

    let mut triggered = Vec::new();

    for config in configs {
        if !config.enabled {
            continue;
        }

        let threshold = match config.threshold {
            Some(t) => t,
            None => continue,
        };

        let current_price = match price_map.get(config.symbol.as_str()) {
            Some(&p) => p,
            None => continue,
        };

        if is_in_cooldown(config.last_triggered_at.as_deref(), config.cooldown_minutes) {
            continue;
        }

        let fired = match config.alert_type.as_str() {
            "price_above" => current_price >= threshold,
            "price_below" => current_price <= threshold,
            _ => continue,
        };

        if fired {
            let direction = if config.alert_type == "price_above" {
                "above"
            } else {
                "below"
            };

            let metadata = serde_json::json!({
                "current_price": current_price,
                "threshold": threshold,
            });

            triggered.push(TriggeredAlert {
                alert_config_id: config.id,
                alert_type: config.alert_type.clone(),
                symbol: config.symbol.clone(),
                title: format!("{} {} {:.2}", config.symbol, direction, threshold),
                message: format!(
                    "{} is now {:.2}, which is {} your threshold of {:.2}",
                    config.symbol, current_price, direction, threshold
                ),
                metadata_json: Some(metadata.to_string()),
            });
        }
    }

    triggered
}

/// Check movement-based alerts (large % change within a time window).
///
/// For each symbol present in both `current_prices` and `previous_prices`,
/// compute the absolute percentage change. If it exceeds the per-config
/// threshold (falling back to `default_threshold`), fire an alert.
///
/// `configs` — enabled alert configs with `alert_type` of `"movement"`.
/// `current_prices` — slice of `(symbol, current_price)` tuples.
/// `previous_prices` — map of symbol -> price from the previous check cycle.
/// `default_threshold` — fallback % change threshold when config has none.
pub fn check_movement_alerts(
    configs: &[AlertConfig],
    current_prices: &[(String, f64)],
    previous_prices: &HashMap<String, f64>,
    default_threshold: f64,
) -> Vec<TriggeredAlert> {
    let current_map: HashMap<&str, f64> = current_prices
        .iter()
        .map(|(sym, price)| (sym.as_str(), *price))
        .collect();

    let mut triggered = Vec::new();

    for config in configs {
        if !config.enabled {
            continue;
        }

        if config.alert_type != "movement" {
            continue;
        }

        let current = match current_map.get(config.symbol.as_str()) {
            Some(&p) => p,
            None => continue,
        };

        let previous = match previous_prices.get(&config.symbol) {
            Some(&p) if p > 0.0 => p,
            _ => continue,
        };

        if is_in_cooldown(config.last_triggered_at.as_deref(), config.cooldown_minutes) {
            continue;
        }

        let pct_change = ((current - previous) / previous) * 100.0;
        let threshold = config.threshold.unwrap_or(default_threshold);

        if pct_change.abs() >= threshold {
            let direction = if pct_change > 0.0 { "up" } else { "down" };

            let metadata = serde_json::json!({
                "current_price": current,
                "previous_price": previous,
                "change_percent": pct_change,
                "threshold": threshold,
            });

            triggered.push(TriggeredAlert {
                alert_config_id: config.id,
                alert_type: "movement".to_string(),
                symbol: config.symbol.clone(),
                title: format!(
                    "{} moved {:.1}% {}",
                    config.symbol,
                    pct_change.abs(),
                    direction
                ),
                message: format!(
                    "{} moved {:.2}% {} (from {:.2} to {:.2}), exceeding {:.1}% threshold",
                    config.symbol,
                    pct_change.abs(),
                    direction,
                    previous,
                    current,
                    threshold
                ),
                metadata_json: Some(metadata.to_string()),
            });
        }
    }

    triggered
}

/// Check whether an alert is still in its cooldown period.
///
/// `last_triggered_at` — ISO 8601 timestamp of the last trigger, or `None` if never fired.
/// `cooldown_minutes` — minimum minutes between firings.
///
/// Returns `true` if the alert should NOT fire yet (still cooling down).
pub fn is_in_cooldown(last_triggered_at: Option<&str>, cooldown_minutes: i64) -> bool {
    let ts = match last_triggered_at {
        Some(s) => s,
        None => return false,
    };

    match chrono::DateTime::parse_from_rfc3339(ts) {
        Ok(last) => {
            let elapsed = Utc::now().signed_duration_since(last.with_timezone(&Utc));
            elapsed.num_minutes() < cooldown_minutes
        }
        Err(_) => {
            // If the timestamp is unparseable, assume no cooldown so the alert can fire.
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(alert_type: &str, symbol: &str, threshold: f64) -> AlertConfig {
        AlertConfig {
            id: Some(1),
            alert_type: alert_type.to_string(),
            symbol: symbol.to_string(),
            enabled: true,
            threshold: Some(threshold),
            cooldown_minutes: 5,
            last_triggered_at: None,
            config_json: None,
        }
    }

    #[test]
    fn test_price_above_triggers() {
        let configs = vec![make_config("price_above", "AAPL", 150.0)];
        let quotes = vec![("AAPL".to_string(), 155.0)];
        let result = check_price_alerts(&configs, &quotes);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "AAPL");
    }

    #[test]
    fn test_price_above_does_not_trigger_below_threshold() {
        let configs = vec![make_config("price_above", "AAPL", 150.0)];
        let quotes = vec![("AAPL".to_string(), 140.0)];
        let result = check_price_alerts(&configs, &quotes);
        assert!(result.is_empty());
    }

    #[test]
    fn test_price_below_triggers() {
        let configs = vec![make_config("price_below", "TSLA", 200.0)];
        let quotes = vec![("TSLA".to_string(), 190.0)];
        let result = check_price_alerts(&configs, &quotes);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_movement_triggers() {
        let configs = vec![make_config("movement", "AAPL", 5.0)];
        let current = vec![("AAPL".to_string(), 110.0)];
        let mut previous = HashMap::new();
        previous.insert("AAPL".to_string(), 100.0);
        let result = check_movement_alerts(&configs, &current, &previous, 3.0);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_movement_below_threshold_does_not_trigger() {
        let configs = vec![make_config("movement", "AAPL", 5.0)];
        let current = vec![("AAPL".to_string(), 101.0)];
        let mut previous = HashMap::new();
        previous.insert("AAPL".to_string(), 100.0);
        let result = check_movement_alerts(&configs, &current, &previous, 3.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_cooldown_active() {
        let now = Utc::now().to_rfc3339();
        assert!(is_in_cooldown(Some(&now), 5));
    }

    #[test]
    fn test_cooldown_expired() {
        let old = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        assert!(!is_in_cooldown(Some(&old), 5));
    }

    #[test]
    fn test_cooldown_none() {
        assert!(!is_in_cooldown(None, 5));
    }

    #[test]
    fn test_disabled_config_skipped() {
        let mut config = make_config("price_above", "AAPL", 150.0);
        config.enabled = false;
        let quotes = vec![("AAPL".to_string(), 200.0)];
        let result = check_price_alerts(&[config], &quotes);
        assert!(result.is_empty());
    }
}
