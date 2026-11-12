//! The market feed the subscribe and price nodes read in this app
//! (upstream `flow::triggers::feed::AppFeed` over its broker streaming
//! manager). This app streams nothing Flow can subscribe to, so the feed
//! is a one-shot quote per frame: Task A7 implements
//! [`MarketFeed::first_frame`] over `QuotesService::{get_quote,
//! get_market_depth}`; until then every call answers that no feed is
//! available, never a guess.

use crate::flow::contract::env::{FeedError, MarketFeed};
use crate::state::AppState;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Weak;
use std::time::Duration;

/// What the feed answers while no broker is logged in.
pub const NO_BROKER_SESSION: &str = "No broker is logged in, so there is no live market data feed.";

/// The app's feed for Flow.
pub struct AppFeed {
    #[allow(dead_code)]
    app: Weak<AppState>,
}

impl AppFeed {
    pub fn new(app: Weak<AppState>) -> Self {
        Self { app }
    }
}

#[async_trait]
impl MarketFeed for AppFeed {
    async fn first_frame(
        &self,
        _symbol: &str,
        _exchange: &str,
        _mode: &str,
        _timeout: Duration,
        _accept: &(dyn for<'a> Fn(&'a Value) -> bool + Send + Sync),
    ) -> Result<Option<Value>, FeedError> {
        Err(FeedError::Connect(NO_BROKER_SESSION.into()))
    }

    async fn session(&self) -> Result<(), FeedError> {
        Err(FeedError::Connect(NO_BROKER_SESSION.into()))
    }

    async fn unsubscribe(&self, _symbol: &str, _exchange: &str, _mode: &str) -> Result<(), FeedError> {
        Ok(())
    }
}
