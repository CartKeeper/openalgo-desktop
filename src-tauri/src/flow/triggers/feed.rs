//! The market feed the subscribe nodes read in the app. Upstream reads
//! its broker streaming manager here; this app has no stream Flow can
//! subscribe to, so the feed is [`crate::flow::support::quote_feed`]
//! (one-shot quotes through `QuotesService`), re-exported under upstream's
//! path so the trigger side is unchanged.

pub use crate::flow::support::quote_feed::{AppFeed, NO_BROKER_SESSION};
