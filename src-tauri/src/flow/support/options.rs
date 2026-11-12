//! The options helpers Flow's option nodes and editor lookups import from
//! upstream's `services::options_service` and `services::symbol_service`:
//! the exchange pairing of an underlying (pure, carried as is), today's
//! date in market time, and the two editor lookups (expiry list, option
//! chain structure) that WP8 Phase D answers from this app's option chain
//! service.

use super::Reply;
use crate::state::AppState;
use chrono::NaiveDate;
use serde_json::Value;

const NSE_INDICES: &[&str] = &[
    "NIFTY",
    "BANKNIFTY",
    "FINNIFTY",
    "MIDCPNIFTY",
    "NIFTYNXT50",
    "INDIAVIX",
];
const BSE_INDICES: &[&str] = &["SENSEX", "BANKEX", "SENSEX50"];

/// Exchanges with no spot instrument (web `NO_SPOT_EXCHANGES`): an option
/// on one is priced against the near-month future, and the future, the
/// option and the quote all live on the exchange itself.
pub const NO_SPOT: &[&str] = &["MCX", "CDS", "BCD", "NCDEX", "NCO"];

/// Today's date in market time.
pub fn today_market(ctx: &AppState) -> NaiveDate {
    ctx.now()
        .with_timezone(&crate::flow::contract::time::market_tz())
        .date_naive()
}

/// Where an underlying is quoted.
pub fn quote_exchange(base: &str, exchange: &str) -> String {
    let ex = exchange.to_ascii_uppercase();
    if ex == "NFO" || ex == "BFO" {
        if NSE_INDICES.contains(&base) {
            return "NSE_INDEX".into();
        }
        if BSE_INDICES.contains(&base) {
            return "BSE_INDEX".into();
        }
        return if ex == "NFO" {
            "NSE".into()
        } else {
            "BSE".into()
        };
    }
    ex
}

/// Where an underlying's options trade.
pub fn option_exchange(quote_exchange: &str) -> String {
    match quote_exchange.to_ascii_uppercase().as_str() {
        "NSE" | "NSE_INDEX" => "NFO".into(),
        "BSE" | "BSE_INDEX" => "BFO".into(),
        "MCX" => "MCX".into(),
        "CDS" => "CDS".into(),
        "NCO" => "NCO".into(),
        "BCD" => "BCD".into(),
        "NCDEX" => "NCDEX".into(),
        "CRYPTO" => "CRYPTO".into(),
        _ => "NFO".into(),
    }
}

/// What the editor lookups answer until WP8 Phase D wires them to this
/// app's option chain service.
pub const LOOKUPS_NOT_AVAILABLE: &str =
    "Option expiry and strike lookups are not available yet in this app";

/// `/api/v1/expiry` for the editor's leg builder (web `expiry`).
pub fn expiry(_ctx: &AppState, _symbol: &str, _exchange: &str, _instrumenttype: &str) -> Reply {
    Reply::error(501, LOOKUPS_NOT_AVAILABLE)
}

/// The option chain's structure (strikes either side of ATM, one
/// underlying quote, no per-strike quotes), as the leg builder asks for it.
pub async fn option_chain_structure(
    _ctx: &AppState,
    _underlying: &str,
    _exchange: &str,
    _expiry_date: &str,
    _strike_count: Option<i64>,
) -> Result<Value, Reply> {
    Err(Reply::error(501, LOOKUPS_NOT_AVAILABLE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_underlying_is_quoted_where_the_web_quotes_it() {
        assert_eq!(quote_exchange("NIFTY", "NFO"), "NSE_INDEX");
        assert_eq!(quote_exchange("SENSEX", "BFO"), "BSE_INDEX");
        assert_eq!(quote_exchange("RELIANCE", "NFO"), "NSE");
        assert_eq!(quote_exchange("RELIANCE", "BFO"), "BSE");
        assert_eq!(quote_exchange("CRUDEOIL", "MCX"), "MCX");
        assert_eq!(option_exchange("NSE_INDEX"), "NFO");
        assert_eq!(option_exchange("BSE"), "BFO");
        assert_eq!(option_exchange("junk"), "NFO");
    }
}
