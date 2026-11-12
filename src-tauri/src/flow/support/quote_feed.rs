//! The market feed the subscribe and price nodes read in this app
//! (upstream `flow::triggers::feed::AppFeed` over its broker streaming
//! manager). This app has no stream Flow can subscribe to, so a frame is a
//! one-shot quote through `QuotesService::get_quote` (modes `LTP` and
//! `Quote`) or `get_market_depth` plus the quote's price (mode `Depth`),
//! shaped as the web's websocket frame the nodes read (`{symbol, exchange,
//! mode, data}`, `nodes/streaming.rs`). The node's own timeout bounds the
//! call; a frame `accept` refuses is none in time, so the node's REST
//! fallback runs; `unsubscribe` has nothing to give back.

use crate::brokers::types::{DepthLevel, MarketDepth, Quote};
use crate::flow::contract::env::{FeedError, MarketFeed};
use crate::services::QuotesService;
use crate::state::AppState;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// What the feed answers while no broker is logged in.
pub const NO_BROKER_SESSION: &str = "No broker is logged in, so there is no live market data feed.";

/// What the feed answers once the app is closing.
pub const CLOSING: &str = "OpenAlgo is shutting down.";

/// The app's feed for Flow.
pub struct AppFeed {
    app: Weak<AppState>,
}

impl AppFeed {
    pub fn new(app: Weak<AppState>) -> Self {
        Self { app }
    }

    /// The app, with a broker logged in.
    fn live(&self) -> Result<Arc<AppState>, FeedError> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| FeedError::Connect(CLOSING.into()))?;
        if app.get_broker_session().is_none() {
            return Err(FeedError::Connect(NO_BROKER_SESSION.into()));
        }
        Ok(app)
    }
}

/// The frame's numeric mode (the web's websocket modes).
fn mode_number(mode: &str) -> u8 {
    match mode {
        "LTP" => 1,
        "Depth" => 3,
        _ => 2,
    }
}

/// A quote as the frame the subscribe nodes read: mode 1 carries the
/// price, mode 2 the day's values (`prev_close` is the quote's close).
pub fn quote_frame(symbol: &str, exchange: &str, mode: &str, q: &Quote) -> Value {
    let data = match mode {
        "LTP" => json!({"ltp": q.ltp}),
        _ => json!({
            "ltp": q.ltp, "open": q.open, "high": q.high, "low": q.low,
            "prev_close": q.close, "volume": q.volume,
        }),
    };
    json!({"symbol": symbol, "exchange": exchange, "mode": mode_number(mode), "data": data})
}

/// A book as the depth frame the subscribe nodes read (mode 3): the price,
/// each side as `{price, quantity}`, and the two totals.
pub fn depth_frame(symbol: &str, exchange: &str, ltp: f64, d: &MarketDepth) -> Value {
    let side = |levels: &[DepthLevel]| -> Vec<Value> {
        levels
            .iter()
            .map(|l| json!({"price": l.price, "quantity": l.quantity}))
            .collect()
    };
    let total = |levels: &[DepthLevel]| -> i64 { levels.iter().map(|l| i64::from(l.quantity)).sum() };
    json!({
        "symbol": symbol,
        "exchange": exchange,
        "mode": 3,
        "data": {
            "ltp": ltp,
            "totalbuyqty": total(&d.bids),
            "totalsellqty": total(&d.asks),
            "bids": side(&d.bids),
            "asks": side(&d.asks),
        },
    })
}

#[async_trait]
impl MarketFeed for AppFeed {
    async fn first_frame(
        &self,
        symbol: &str,
        exchange: &str,
        mode: &str,
        timeout: Duration,
        accept: &(dyn for<'a> Fn(&'a Value) -> bool + Send + Sync),
    ) -> Result<Option<Value>, FeedError> {
        let app = self.live()?;
        let frame = tokio::time::timeout(timeout, async {
            if mode == "Depth" {
                let depth = QuotesService::get_market_depth(&app, exchange, symbol, None)
                    .await
                    .map_err(|e| FeedError::Failed(e.to_string()))?;
                // The book's price: the quote's, 0 when it cannot be read
                // (upstream's rule for a book before any price).
                let ltp = QuotesService::get_quote(&app, exchange, symbol, None)
                    .await
                    .map(|q| q.ltp)
                    .unwrap_or(0.0);
                Ok(depth_frame(symbol, exchange, ltp, &depth.depth))
            } else {
                let q = QuotesService::get_quote(&app, exchange, symbol, None)
                    .await
                    .map_err(|e| FeedError::Failed(e.to_string()))?;
                Ok(quote_frame(symbol, exchange, mode, &q))
            }
        })
        .await;
        match frame {
            Ok(Ok(frame)) => Ok(accept(&frame).then_some(frame)),
            Ok(Err(e)) => Err(e),
            // No frame in time: the node's REST fallback runs.
            Err(_) => Ok(None),
        }
    }

    async fn session(&self) -> Result<(), FeedError> {
        self.live().map(|_| ())
    }

    async fn unsubscribe(&self, _symbol: &str, _exchange: &str, _mode: &str) -> Result<(), FeedError> {
        // A one-shot frame holds no reference to give back.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::BrokerRegistry;

    fn quote() -> Quote {
        Quote {
            symbol: "AAPL".into(),
            exchange: "NASDAQ".into(),
            ltp: 190.25,
            open: 189.0,
            high: 191.5,
            low: 188.75,
            close: 188.0,
            volume: 1_250_000,
            bid: 190.2,
            ask: 190.3,
            bid_qty: 100,
            ask_qty: 200,
            oi: 0,
            change: 2.25,
            change_percent: 1.2,
            timestamp: "2026-10-30T13:30:00Z".into(),
        }
    }

    /// The frame the subscribe nodes read (`nodes/streaming.rs`, the web's
    /// websocket frame): the symbol, the exchange, the numeric mode and the
    /// values under `data`.
    #[test]
    fn a_quote_is_the_frame_the_subscribe_nodes_read() {
        let q = quote();
        assert_eq!(
            quote_frame("AAPL", "NASDAQ", "LTP", &q),
            json!({"symbol": "AAPL", "exchange": "NASDAQ", "mode": 1, "data": {"ltp": 190.25}})
        );
        let full = quote_frame("AAPL", "NASDAQ", "Quote", &q);
        assert_eq!(full["mode"], 2);
        assert_eq!(
            full["data"],
            json!({"ltp": 190.25, "open": 189.0, "high": 191.5, "low": 188.75, "prev_close": 188.0, "volume": 1_250_000})
        );
    }

    #[test]
    fn a_book_is_the_depth_frame_the_subscribe_nodes_read() {
        let depth = MarketDepth {
            symbol: "AAPL".into(),
            exchange: "NASDAQ".into(),
            bids: vec![DepthLevel { price: 190.2, quantity: 100, orders: 3 }],
            asks: vec![
                DepthLevel { price: 190.3, quantity: 200, orders: 5 },
                DepthLevel { price: 190.35, quantity: 50, orders: 1 },
            ],
        };
        let frame = depth_frame("AAPL", "NASDAQ", 190.25, &depth);
        assert_eq!(frame["mode"], 3);
        assert_eq!(frame["data"]["ltp"], 190.25);
        assert_eq!(frame["data"]["bids"], json!([{"price": 190.2, "quantity": 100}]));
        assert_eq!(frame["data"]["asks"][1]["quantity"], 50);
        assert_eq!(frame["data"]["totalbuyqty"], 100);
        assert_eq!(frame["data"]["totalsellqty"], 250);
    }

    /// Without a broker logged in the feed says so at once, on every call,
    /// and has nothing to give back; once the app is gone it says that.
    #[tokio::test]
    async fn without_a_broker_the_feed_says_so() {
        let t = crate::state::testing::build(BrokerRegistry::new(), chrono::Utc::now());
        let feed = AppFeed::new(Arc::downgrade(&t.ctx));
        let frame = feed
            .first_frame("AAPL", "NASDAQ", "LTP", Duration::from_secs(1), &|_| true)
            .await;
        assert_eq!(frame, Err(FeedError::Connect(NO_BROKER_SESSION.into())));
        assert_eq!(
            feed.session().await,
            Err(FeedError::Connect(NO_BROKER_SESSION.into()))
        );
        assert_eq!(feed.unsubscribe("AAPL", "NASDAQ", "LTP").await, Ok(()));
        drop(t);
        assert_eq!(feed.session().await, Err(FeedError::Connect(CLOSING.into())));
    }
}
