//! Market status service
//!
//! Real-time market clock and trading calendar from the connected broker.
//! Additive to the app's local holiday/timings system — it does not replace it.

use crate::brokers::types::{MarketCalendarDay, MarketClock};
use crate::error::{AppError, Result};
use crate::state::AppState;
use tracing::info;

pub struct MarketStatusService;

impl MarketStatusService {
    /// Real-time market open/closed status and next session boundaries.
    pub async fn get_clock(state: &AppState, api_key: Option<&str>) -> Result<MarketClock> {
        info!("MarketStatusService::get_clock");
        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;
        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;
        broker.get_market_clock(&auth_token).await
    }

    /// Trading calendar between optional start/end dates ("YYYY-MM-DD").
    pub async fn get_calendar(
        state: &AppState,
        api_key: Option<&str>,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<Vec<MarketCalendarDay>> {
        info!("MarketStatusService::get_calendar");
        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;
        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;
        broker.get_market_calendar(&auth_token, start, end).await
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
