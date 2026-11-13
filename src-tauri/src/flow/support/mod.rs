//! What the carried Flow module needs from the app that upstream OpenAlgo
//! Desktop took from modules this app does not have (its Agent, its
//! `services::core`, its broker symbol store, its tunnel, its feed).
//!
//! Each file is either a copy of the upstream helper (`values`, `numbers`,
//! `order_vocab`, `underlying`, `mapping`, `session`: pure functions the
//! nodes and the validator were written against, kept byte-for-byte where
//! the goldens depend on them) or this app's implementation of an upstream
//! service seam (`symbols`, `reply`, `dispatch`, `options`, `calendar`,
//! `market_hours`, `quote_feed`, `public_url`, `db`). `docs/flow/PORT.md`
//! lists every seam.

pub mod calendar;
pub mod db;
pub mod dispatch;
pub mod mapping;
pub mod market_hours;
pub mod numbers;
pub mod options;
pub mod order_vocab;
pub mod public_url;
pub mod quote_feed;
pub mod reply;
pub mod session;
pub mod symbols;
pub mod underlying;
pub mod values;

#[cfg(test)]
pub mod testkit;

pub use public_url::public_base_url;
pub use reply::Reply;

/// Every exchange a Flow node may name: upstream's list (OpenAlgo web's
/// `VALID_EXCHANGES`, in its order, so a workflow exported from the web
/// imports unchanged) followed by this app's US venues.
pub const FLOW_EXCHANGES: &[&str] = &[
    "NSE",
    "NFO",
    "CDS",
    "BSE",
    "BFO",
    "BCD",
    "MCX",
    "NCDEX",
    "NCO",
    "NSE_INDEX",
    "BSE_INDEX",
    "MCX_INDEX",
    "GLOBAL_INDEX",
    "CRYPTO",
    "NYSE",
    "NASDAQ",
    "AMEX",
    "ARCA",
    "BATS",
    "OTC",
    "US",
];

/// This app's US venues (the tail of [`FLOW_EXCHANGES`]): cash segments
/// whose default product is `CNC`.
pub const US_EXCHANGES: &[&str] = &["NYSE", "NASDAQ", "AMEX", "ARCA", "BATS", "OTC", "US"];

/// Whether `label` (any case) is one of this app's US venues.
pub fn is_us_venue(label: &str) -> bool {
    US_EXCHANGES
        .iter()
        .any(|v| v.eq_ignore_ascii_case(label.trim()))
}

/// Whether two exchange labels are one market when a watch or an alert is
/// matched (WP8 Phase D, decision 6f): the US labels are one market, since
/// a US ticker is the same on every US venue (this app's Alpaca adapter
/// writes `US` on every order and quote, the sandbox keeps the venue an
/// order was placed on); every other label, Indian or crypto, matches only
/// itself, as on the web.
pub fn same_market(a: &str, b: &str) -> bool {
    a == b || (is_us_venue(a) && is_us_venue(b))
}

/// What a broker-backed call answers while no broker is logged in
/// (upstream's `strategy::dispatch::NO_BROKER_SESSION`).
pub const NO_BROKER_SESSION: &str =
    "Broker session is not available or has expired. Log in to your broker to restore it.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exchange_list_is_upstreams_followed_by_the_us_venues() {
        assert_eq!(&FLOW_EXCHANGES[..14], &["NSE", "NFO", "CDS", "BSE", "BFO", "BCD", "MCX", "NCDEX", "NCO", "NSE_INDEX", "BSE_INDEX", "MCX_INDEX", "GLOBAL_INDEX", "CRYPTO"]);
        assert_eq!(&FLOW_EXCHANGES[14..], US_EXCHANGES);
        assert!(!FLOW_EXCHANGES.contains(&"XYZ"));
    }
}
