//! Broker-hosted (cloud) watchlist service
//!
//! Exposes the connected broker's watchlist CRUD. This is separate from the
//! app's local watchlist (db/sqlite/watchlist.rs) — it does not auto-sync;
//! the frontend drives push/pull explicitly.

use crate::brokers::types::BrokerWatchlist;
use crate::error::{AppError, Result};
use crate::state::AppState;
use tracing::info;

pub struct BrokerWatchlistService;

impl BrokerWatchlistService {
    pub async fn list(state: &AppState, api_key: Option<&str>) -> Result<Vec<BrokerWatchlist>> {
        info!("BrokerWatchlistService::list");
        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;
        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;
        broker.get_watchlists(&auth_token).await
    }

    pub async fn get(state: &AppState, api_key: Option<&str>, id: &str) -> Result<BrokerWatchlist> {
        info!("BrokerWatchlistService::get {}", id);
        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;
        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;
        broker.get_watchlist(&auth_token, id).await
    }

    pub async fn create(
        state: &AppState,
        api_key: Option<&str>,
        name: &str,
        symbols: Vec<String>,
    ) -> Result<BrokerWatchlist> {
        info!("BrokerWatchlistService::create {}", name);
        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;
        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;
        broker.create_watchlist(&auth_token, name, symbols).await
    }

    pub async fn delete(state: &AppState, api_key: Option<&str>, id: &str) -> Result<()> {
        info!("BrokerWatchlistService::delete {}", id);
        let (auth_token, broker_id) = Self::get_auth(state, api_key)?;
        let broker = state
            .brokers
            .get(&broker_id)
            .ok_or_else(|| AppError::Broker(format!("Broker '{}' not found", broker_id)))?;
        broker.delete_watchlist(&auth_token, id).await
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
