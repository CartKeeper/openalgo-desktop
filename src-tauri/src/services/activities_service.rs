//! Account activities service
//!
//! Fetches trade fills and non-trade events (cash deposits/withdrawals, ACH
//! transfers, dividends, fees) from the connected broker. In analyze (sandbox)
//! mode there is no real broker activity, so an empty list is returned.

use crate::brokers::types::AccountActivity;
use crate::error::{AppError, Result};
use crate::state::AppState;
use tracing::info;

pub struct ActivitiesService;

impl ActivitiesService {
    /// Get recent account activity from the connected broker.
    pub async fn get_activities(
        state: &AppState,
        api_key: Option<&str>,
        page_size: u32,
    ) -> Result<Vec<AccountActivity>> {
        info!("ActivitiesService::get_activities");

        // Analyze (sandbox) mode has no broker-side activity.
        if state.sqlite.get_analyze_mode().unwrap_or(false) {
            return Ok(Vec::new());
        }

        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;

        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;

        broker.get_activities(&auth_token, page_size).await
    }

    fn get_auth(state: &AppState, api_key: Option<&str>) -> Result<(String, String)> {
        if let Some(key) = api_key {
            let _ = state.sqlite.validate_api_key(key, &state.security)?;
        }
        let session = state
            .get_broker_session()
            .ok_or_else(|| AppError::Auth("Broker not connected".to_string()))?;
        Ok((session.auth_token, session.broker_id))
    }
}
