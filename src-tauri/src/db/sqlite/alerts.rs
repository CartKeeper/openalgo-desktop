use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub id: Option<i64>,
    pub alert_type: String,
    pub symbol: String,
    pub enabled: bool,
    pub threshold_value: Option<f64>,
    pub threshold_period_minutes: Option<i64>,
    pub cooldown_minutes: i64,
    pub last_triggered_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertHistoryEntry {
    pub id: i64,
    pub alert_config_id: Option<i64>,
    pub alert_type: String,
    pub symbol: String,
    pub title: String,
    pub message: String,
    pub metadata_json: Option<String>,
    pub acknowledged: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertGlobalSettings {
    pub alerts_enabled: bool,
    pub sound_enabled: bool,
    pub native_notifications_enabled: bool,
    pub movement_default_threshold: f64,
    pub movement_check_interval_seconds: i64,
    pub news_check_interval_seconds: i64,
    pub rising_star_enabled: bool,
    pub rising_star_interval_seconds: i64,
    pub rising_star_min_change_pct: f64,
    pub rising_star_min_volume_ratio: f64,
}

/// Create a new alert configuration
pub fn create_alert(conn: &Connection, alert: &AlertConfig) -> Result<AlertConfig> {
    conn.execute(
        "INSERT INTO alert_configs (alert_type, symbol, enabled, threshold_value, threshold_period_minutes, cooldown_minutes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            alert.alert_type,
            alert.symbol,
            alert.enabled as i32,
            alert.threshold_value,
            alert.threshold_period_minutes,
            alert.cooldown_minutes,
        ],
    )?;

    let id = conn.last_insert_rowid();

    let created = conn.query_row(
        "SELECT id, alert_type, symbol, enabled, threshold_value, threshold_period_minutes,
                cooldown_minutes, last_triggered_at, created_at, updated_at
         FROM alert_configs WHERE id = ?1",
        [id],
        |row| {
            Ok(AlertConfig {
                id: Some(row.get(0)?),
                alert_type: row.get(1)?,
                symbol: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                threshold_value: row.get(4)?,
                threshold_period_minutes: row.get(5)?,
                cooldown_minutes: row.get(6)?,
                last_triggered_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )?;

    Ok(created)
}

/// Get all alert configurations
pub fn get_alerts(conn: &Connection) -> Result<Vec<AlertConfig>> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, symbol, enabled, threshold_value, threshold_period_minutes,
                cooldown_minutes, last_triggered_at, created_at, updated_at
         FROM alert_configs ORDER BY created_at DESC",
    )?;

    let items = stmt
        .query_map([], |row| {
            Ok(AlertConfig {
                id: Some(row.get(0)?),
                alert_type: row.get(1)?,
                symbol: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                threshold_value: row.get(4)?,
                threshold_period_minutes: row.get(5)?,
                cooldown_minutes: row.get(6)?,
                last_triggered_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Get only enabled alert configurations
pub fn get_enabled_alerts(conn: &Connection) -> Result<Vec<AlertConfig>> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, symbol, enabled, threshold_value, threshold_period_minutes,
                cooldown_minutes, last_triggered_at, created_at, updated_at
         FROM alert_configs WHERE enabled = 1 ORDER BY created_at DESC",
    )?;

    let items = stmt
        .query_map([], |row| {
            Ok(AlertConfig {
                id: Some(row.get(0)?),
                alert_type: row.get(1)?,
                symbol: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                threshold_value: row.get(4)?,
                threshold_period_minutes: row.get(5)?,
                cooldown_minutes: row.get(6)?,
                last_triggered_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Update an existing alert configuration
pub fn update_alert(conn: &Connection, id: i64, alert: &AlertConfig) -> Result<AlertConfig> {
    conn.execute(
        "UPDATE alert_configs
         SET alert_type = ?1, symbol = ?2, threshold_value = ?3,
             threshold_period_minutes = ?4, cooldown_minutes = ?5,
             updated_at = datetime('now')
         WHERE id = ?6",
        rusqlite::params![
            alert.alert_type,
            alert.symbol,
            alert.threshold_value,
            alert.threshold_period_minutes,
            alert.cooldown_minutes,
            id,
        ],
    )?;

    let updated = conn.query_row(
        "SELECT id, alert_type, symbol, enabled, threshold_value, threshold_period_minutes,
                cooldown_minutes, last_triggered_at, created_at, updated_at
         FROM alert_configs WHERE id = ?1",
        [id],
        |row| {
            Ok(AlertConfig {
                id: Some(row.get(0)?),
                alert_type: row.get(1)?,
                symbol: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                threshold_value: row.get(4)?,
                threshold_period_minutes: row.get(5)?,
                cooldown_minutes: row.get(6)?,
                last_triggered_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )?;

    Ok(updated)
}

/// Delete an alert configuration
pub fn delete_alert(conn: &Connection, id: i64) -> Result<bool> {
    let affected = conn.execute(
        "DELETE FROM alert_configs WHERE id = ?1",
        [id],
    )?;
    Ok(affected > 0)
}

/// Toggle alert enabled/disabled
pub fn toggle_alert(conn: &Connection, id: i64, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE alert_configs SET enabled = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![enabled as i32, id],
    )?;
    Ok(())
}

/// Log an alert trigger to history
pub fn log_alert(
    conn: &Connection,
    config_id: Option<i64>,
    alert_type: &str,
    symbol: &str,
    title: &str,
    message: &str,
    metadata_json: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO alert_history (alert_config_id, alert_type, symbol, title, message, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![config_id, alert_type, symbol, title, message, metadata_json],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Get alert history with pagination
pub fn get_alert_history(conn: &Connection, limit: i64, offset: i64) -> Result<Vec<AlertHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_config_id, alert_type, symbol, title, message,
                metadata_json, acknowledged, created_at
         FROM alert_history ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
    )?;

    let items = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
            Ok(AlertHistoryEntry {
                id: row.get(0)?,
                alert_config_id: row.get(1)?,
                alert_type: row.get(2)?,
                symbol: row.get(3)?,
                title: row.get(4)?,
                message: row.get(5)?,
                metadata_json: row.get(6)?,
                acknowledged: row.get::<_, i32>(7)? == 1,
                created_at: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Acknowledge a single alert
pub fn acknowledge_alert(conn: &Connection, id: i64) -> Result<bool> {
    let affected = conn.execute(
        "UPDATE alert_history SET acknowledged = 1 WHERE id = ?1",
        [id],
    )?;
    Ok(affected > 0)
}

/// Acknowledge all unacknowledged alerts
pub fn acknowledge_all_alerts(conn: &Connection) -> Result<i64> {
    let affected = conn.execute(
        "UPDATE alert_history SET acknowledged = 1 WHERE acknowledged = 0",
        [],
    )?;
    Ok(affected as i64)
}

/// Get count of unacknowledged alerts
pub fn get_unacknowledged_count(conn: &Connection) -> Result<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM alert_history WHERE acknowledged = 0",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Get global alert settings
pub fn get_global_settings(conn: &Connection) -> Result<AlertGlobalSettings> {
    let settings = conn.query_row(
        "SELECT alerts_enabled, sound_enabled, native_notifications_enabled,
                movement_default_threshold, movement_check_interval_seconds,
                news_check_interval_seconds, rising_star_enabled,
                rising_star_interval_seconds, rising_star_min_change_pct,
                rising_star_min_volume_ratio
         FROM alert_global_settings WHERE id = 1",
        [],
        |row| {
            Ok(AlertGlobalSettings {
                alerts_enabled: row.get::<_, i32>(0)? == 1,
                sound_enabled: row.get::<_, i32>(1)? == 1,
                native_notifications_enabled: row.get::<_, i32>(2)? == 1,
                movement_default_threshold: row.get(3)?,
                movement_check_interval_seconds: row.get(4)?,
                news_check_interval_seconds: row.get(5)?,
                rising_star_enabled: row.get::<_, i32>(6)? == 1,
                rising_star_interval_seconds: row.get(7)?,
                rising_star_min_change_pct: row.get(8)?,
                rising_star_min_volume_ratio: row.get(9)?,
            })
        },
    )?;

    Ok(settings)
}

/// Update global alert settings
pub fn update_global_settings(conn: &Connection, settings: &AlertGlobalSettings) -> Result<AlertGlobalSettings> {
    conn.execute(
        "UPDATE alert_global_settings
         SET alerts_enabled = ?1, sound_enabled = ?2, native_notifications_enabled = ?3,
             movement_default_threshold = ?4, movement_check_interval_seconds = ?5,
             news_check_interval_seconds = ?6, rising_star_enabled = ?7,
             rising_star_interval_seconds = ?8, rising_star_min_change_pct = ?9,
             rising_star_min_volume_ratio = ?10
         WHERE id = 1",
        rusqlite::params![
            settings.alerts_enabled as i32,
            settings.sound_enabled as i32,
            settings.native_notifications_enabled as i32,
            settings.movement_default_threshold,
            settings.movement_check_interval_seconds,
            settings.news_check_interval_seconds,
            settings.rising_star_enabled as i32,
            settings.rising_star_interval_seconds,
            settings.rising_star_min_change_pct,
            settings.rising_star_min_volume_ratio,
        ],
    )?;

    get_global_settings(conn)
}

/// Update the last_triggered_at timestamp for an alert
pub fn update_last_triggered(conn: &Connection, alert_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE alert_configs SET last_triggered_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
        [alert_id],
    )?;
    Ok(())
}
