//! WP8 Phase D, Task D2: the leg builder's strike lookup for a US
//! underlying. AAPL's options are looked up where AAPL trades (its US
//! venue), never on NFO, from this app's chain service (here a seeded fake
//! chain); this app's symbol cache lists no US option expiries, so an
//! empty expiry list is not a refusal for a US underlying.

use crate::flow::lookups::{option_strikes_from, StrikeQuery, StrikeSource};
use crate::flow::support::Reply;
use async_trait::async_trait;
use chrono::NaiveDate;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;

/// AAPL at 270.40: five strikes around 270 with their contracts, no listed
/// expiry (the symbol cache of a US broker holds equities only), and the
/// venue the cache lists AAPL on.
#[derive(Default)]
struct UsChain {
    asked: Mutex<Vec<(String, String, String, i64)>>,
    expiries_asked: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl StrikeSource for UsChain {
    async fn expiries(&self, symbol: &str, exchange: &str) -> Reply {
        self.expiries_asked
            .lock()
            .push((symbol.into(), exchange.into()));
        Reply::ok(json!({"status": "success", "data": []}))
    }

    async fn chain(
        &self,
        symbol: &str,
        exchange: &str,
        expiry: &str,
        count: i64,
    ) -> Result<Value, Reply> {
        self.asked
            .lock()
            .push((symbol.into(), exchange.into(), expiry.into(), count));
        let leg = |strike: i64, side: &str, label: &str| json!({"symbol": format!("AAPL261120{}{:08}", side, strike * 1000), "label": label});
        Ok(json!({
            "expiry_date": "",
            "atm_strike": 270.0,
            "underlying_ltp": 270.4,
            "underlying_symbol": "AAPL",
            "chain": [
                {"strike": 260.0, "ce": leg(260, "C", "ITM2"), "pe": leg(260, "P", "OTM2")},
                {"strike": 265.0, "ce": leg(265, "C", "ITM1"), "pe": leg(265, "P", "OTM1")},
                {"strike": 270.0, "ce": leg(270, "C", "ATM"), "pe": leg(270, "P", "ATM")},
                {"strike": 275.0, "ce": leg(275, "C", "OTM1"), "pe": leg(275, "P", "ITM1")},
                {"strike": 280.0, "ce": leg(280, "C", "OTM2"), "pe": leg(280, "P", "ITM2")},
            ],
        }))
    }

    fn today(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 12).unwrap_or_default()
    }

    /// A US broker's cache: AAPL on NASDAQ, and Barrick's GOLD on NYSE.
    fn us_venue(&self, underlying: &str) -> Option<String> {
        match underlying {
            "AAPL" => Some("NASDAQ".into()),
            "GOLD" => Some("NYSE".into()),
            _ => None,
        }
    }
}

fn query(pairs: &[(&str, &str)]) -> StrikeQuery {
    let q: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    StrikeQuery::from_params(&q)
}

/// AAPL on NASDAQ: the expiries and the chain are asked on NASDAQ, for
/// AAPL itself, and the answer lists the chain's strikes with their
/// contracts and the call side's labels.
#[tokio::test]
async fn aapl_strikes_come_from_the_chain_on_its_us_venue() {
    let src = UsChain::default();
    let r = option_strikes_from(
        &src,
        true,
        &query(&[
            ("underlying", "aapl"),
            ("exchange", "NASDAQ"),
            ("optionType", "CE"),
        ]),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        *src.expiries_asked.lock(),
        vec![("AAPL".to_string(), "NASDAQ".to_string())]
    );
    assert_eq!(
        *src.asked.lock(),
        vec![("AAPL".to_string(), "NASDAQ".to_string(), String::new(), 25)]
    );
    let d = &r.body["data"];
    assert_eq!(d["underlying"], "AAPL");
    assert_eq!(d["exchange"], "NASDAQ");
    assert_eq!(d["expiries"], json!([]));
    assert_eq!(d["atm"], 270.0);
    assert_eq!(d["underlyingLtp"], 270.4);
    let strikes: Vec<(f64, String, String)> = d["strikes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            (
                s["strike"].as_f64().unwrap_or_default(),
                s["symbol"].as_str().unwrap_or_default().to_string(),
                s["label"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        strikes,
        vec![
            (260.0, "AAPL261120C00260000".into(), "ITM2".into()),
            (265.0, "AAPL261120C00265000".into(), "ITM1".into()),
            (270.0, "AAPL261120C00270000".into(), "ATM".into()),
            (275.0, "AAPL261120C00275000".into(), "OTM1".into()),
            (280.0, "AAPL261120C00280000".into(), "OTM2".into()),
        ]
    );
}

/// Any US label reads the same: `US` (the app's default exchange) asks on
/// `US`, and the put side carries the put contracts.
#[tokio::test]
async fn the_us_label_asks_on_us_and_the_put_side_reads_the_puts() {
    let src = UsChain::default();
    let r = option_strikes_from(
        &src,
        true,
        &query(&[
            ("underlying", "AAPL"),
            ("exchange", "US"),
            ("optionType", "PE"),
        ]),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(src.asked.lock()[0].1, "US");
    assert_eq!(
        r.body["data"]["strikes"][0]["symbol"],
        "AAPL261120P00260000"
    );
    assert_eq!(r.body["data"]["strikes"][0]["label"], "OTM2");
}

/// The leg builder sends no exchange: a US underlying is asked on the venue
/// the symbol cache lists it on.
#[tokio::test]
async fn a_query_without_an_exchange_is_asked_where_the_cache_lists_it() {
    let src = UsChain::default();
    let r = option_strikes_from(&src, true, &query(&[("underlying", "AAPL")])).await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(src.asked.lock()[0].1, "NASDAQ");
    assert_eq!(r.body["data"]["exchange"], "NASDAQ");
}

/// GOLD is the editor's MCX commodity (the options node lists it), so it
/// keeps the web's resolution even where a US cache lists Barrick's GOLD.
#[tokio::test]
async fn gold_stays_the_editors_mcx_commodity() {
    let src = UsChain::default();
    let r = option_strikes_from(&src, true, &query(&[("underlying", "GOLD")])).await;
    assert_eq!(
        *src.expiries_asked.lock(),
        vec![("GOLD".to_string(), "MCX".to_string())]
    );
    assert_eq!(r.status, 404, "{}", r.body);
    assert_eq!(
        r.body["message"],
        "No option expiries found for GOLD on MCX"
    );
}
