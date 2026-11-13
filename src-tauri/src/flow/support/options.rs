//! The options helpers Flow's option nodes and editor lookups import from
//! upstream's `services::options_service` and `services::symbol_service`:
//! the exchange pairing of an underlying (pure, carried as is), today's
//! date in market time, and the two editor lookups (expiry list, option
//! chain structure), answered since WP8 Phase D (Task D2) from this app's
//! own services: `SymbolService::get_expiry_dates` (the symbol cache) and
//! `OptionsService::get_option_chain`.

use super::Reply;
use crate::services::options_service::{OptionChainEntry, OptionChainResult};
use crate::services::{OptionsService, SymbolService};
use crate::state::AppState;
use chrono::NaiveDate;
use serde_json::{json, Value};

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

/// `/api/v1/expiry` for the editor's leg builder (web `expiry`): this
/// app's expiry service over the symbol cache, in the route's shape
/// (`{status, data: [dates]}`).
pub fn expiry(ctx: &AppState, symbol: &str, exchange: &str, instrumenttype: &str) -> Reply {
    match SymbolService::get_expiry_dates(ctx, symbol, exchange, instrumenttype) {
        Ok(found) => Reply::ok(json!({"status": "success", "data": found.expiry_dates})),
        Err(e) => Reply::error(500, format!("The expiries could not be read: {}", e)),
    }
}

/// The option chain's structure (strikes either side of ATM, one
/// underlying quote, no per-strike quotes), as the leg builder asks for it:
/// this app's `OptionsService::get_option_chain` ([`chain_structure`]). An
/// empty `expiry_date` asks without one (a US underlying: the symbol cache
/// lists no US option expiries).
pub async fn option_chain_structure(
    ctx: &AppState,
    underlying: &str,
    exchange: &str,
    expiry_date: &str,
    strike_count: Option<i64>,
) -> Result<Value, Reply> {
    let expiry = Some(expiry_date).filter(|e| !e.is_empty());
    match OptionsService::get_option_chain(ctx, underlying, exchange, expiry, None).await {
        Ok(chain) => listed_structure(&chain, strike_count),
        // The chain starts with the underlying's quote, which needs a broker.
        Err(_) if ctx.get_broker_session().is_none() => {
            Err(Reply::error(503, super::NO_BROKER_SESSION))
        }
        Err(e) => Err(Reply::error(
            502,
            format!("The option chain could not be read: {}", e),
        )),
    }
}

/// The leg builder's view of this app's chain (upstream's chain structure:
/// `chain: [{strike, ce: {symbol, label} | null, pe: ...}]`, `atm_strike`,
/// `underlying_ltp`, `underlying_symbol`, `expiry_date`): the positive
/// strikes in order, `strike_count` either side of the strike nearest the
/// chain's ATM (all of them without a count), each side labelled from that
/// strike (ATM, ITMn, OTMn). A side carries its contract only when the
/// chain names one for that strike alone: this app's chain answers its
/// first cached contract for every strike when it cannot tell them apart,
/// and a contract named for several strikes is no strike's contract.
pub fn chain_structure(chain: &OptionChainResult, strike_count: Option<i64>) -> Value {
    let mut rows: Vec<&OptionChainEntry> = chain
        .strikes
        .iter()
        .filter(|e| e.strike.is_finite() && e.strike > 0.0)
        .collect();
    rows.sort_by(|a, b| a.strike.total_cmp(&b.strike));
    rows.dedup_by(|a, b| a.strike == b.strike);
    let named = |symbol: &str| {
        chain
            .strikes
            .iter()
            .filter(|e| {
                e.call_symbol.as_deref() == Some(symbol) || e.put_symbol.as_deref() == Some(symbol)
            })
            .count()
    };
    let leg = |symbol: &Option<String>, label: String| -> Value {
        match symbol.as_deref() {
            Some(s) if !s.is_empty() && named(s) == 1 => json!({"symbol": s, "label": label}),
            _ => Value::Null,
        }
    };
    let atm = rows
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.strike - chain.atm_strike)
                .abs()
                .total_cmp(&(b.strike - chain.atm_strike).abs())
        })
        .map(|(i, _)| i);
    let (start, end) = match (atm, strike_count) {
        (Some(atm), Some(n)) if n > 0 => {
            let n = n as usize;
            (atm.saturating_sub(n), (atm + n + 1).min(rows.len()))
        }
        _ => (0, rows.len()),
    };
    let window: Vec<Value> = rows[start..end]
        .iter()
        .enumerate()
        .map(|(offset, e)| {
            let (ce, pe) = side_labels(start + offset, atm.unwrap_or(0));
            json!({"strike": e.strike, "ce": leg(&e.call_symbol, ce), "pe": leg(&e.put_symbol, pe)})
        })
        .collect();
    json!({
        "status": "success",
        "underlying": chain.underlying,
        "underlying_symbol": chain.underlying,
        "underlying_ltp": chain.underlying_ltp,
        "expiry_date": chain.expiry,
        "atm_strike": atm.map(|i| json!(rows[i].strike)).unwrap_or(Value::Null),
        "chain": window,
    })
}

/// The leg builder's structure of `chain`, or a refusal when none of its
/// strikes has a listed contract: this app's chain service lists a ladder
/// of strikes for an underlying it holds no contracts for (every US
/// underlying today: a US broker's symbol list has no option contracts),
/// and a strike no contract has is no strike to choose.
pub fn listed_structure(
    chain: &OptionChainResult,
    strike_count: Option<i64>,
) -> Result<Value, Reply> {
    let structure = chain_structure(chain, strike_count);
    let listed = structure["chain"].as_array().is_some_and(|rows| {
        rows.iter()
            .any(|r| !r["ce"].is_null() || !r["pe"].is_null())
    });
    if !listed {
        return Err(Reply::error(
            404,
            format!(
                "No listed option contracts for {} in this app's symbol list, so there are no strikes to choose from. A typed strike still works.",
                chain.underlying
            ),
        ));
    }
    Ok(structure)
}

/// The call's and the put's label of the strike at `index` when ATM is at
/// `atm` (a lower strike is in the money for a call, out for a put).
fn side_labels(index: usize, atm: usize) -> (String, String) {
    match index.cmp(&atm) {
        std::cmp::Ordering::Equal => ("ATM".into(), "ATM".into()),
        std::cmp::Ordering::Less => (format!("ITM{}", atm - index), format!("OTM{}", atm - index)),
        std::cmp::Ordering::Greater => {
            (format!("OTM{}", index - atm), format!("ITM{}", index - atm))
        }
    }
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

    /// One strike of the app's chain, with the call's and the put's contract.
    fn entry(strike: f64, call: Option<&str>, put: Option<&str>) -> OptionChainEntry {
        OptionChainEntry {
            strike,
            call_symbol: call.map(String::from),
            call_token: None,
            call_ltp: None,
            call_oi: None,
            call_volume: None,
            call_iv: None,
            put_symbol: put.map(String::from),
            put_token: None,
            put_ltp: None,
            put_oi: None,
            put_volume: None,
            put_iv: None,
        }
    }

    fn chain(
        underlying: &str,
        ltp: f64,
        atm: f64,
        strikes: Vec<OptionChainEntry>,
    ) -> OptionChainResult {
        OptionChainResult {
            success: true,
            underlying: underlying.into(),
            underlying_ltp: ltp,
            expiry: String::new(),
            atm_strike: atm,
            strikes,
        }
    }

    /// Task D2: a seeded AAPL chain (seven strikes, 255 to 285, ATM 270) as
    /// the leg builder reads it: two strikes either side of ATM, each side
    /// with its contract and its label.
    #[test]
    fn the_apps_chain_is_the_leg_builders_structure() {
        let strikes = (0..7)
            .map(|i| {
                let k = 255.0 + 5.0 * i as f64;
                let c = format!("AAPL261120C{:08}", (k * 1000.0) as i64);
                let p = format!("AAPL261120P{:08}", (k * 1000.0) as i64);
                entry(k, Some(&c), Some(&p))
            })
            .collect();
        let out = chain_structure(&chain("AAPL", 270.4, 270.0, strikes), Some(2));
        assert_eq!(out["atm_strike"], 270.0);
        assert_eq!(out["underlying_ltp"], 270.4);
        assert_eq!(out["underlying_symbol"], "AAPL");
        let rows = out["chain"].as_array().cloned().unwrap_or_default();
        assert_eq!(
            rows.iter()
                .map(|r| r["strike"].as_f64().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![260.0, 265.0, 270.0, 275.0, 280.0]
        );
        assert_eq!(
            rows[0]["ce"],
            json!({"symbol": "AAPL261120C00260000", "label": "ITM2"})
        );
        assert_eq!(
            rows[0]["pe"],
            json!({"symbol": "AAPL261120P00260000", "label": "OTM2"})
        );
        assert_eq!(rows[2]["ce"]["label"], "ATM");
        assert_eq!(rows[4]["ce"]["label"], "OTM2");
        assert_eq!(rows[4]["pe"]["label"], "ITM2");
    }

    /// What this app's chain answers for a $9 stock: its $50 ladder centred
    /// on a rounded ATM of 0 (strikes down to -500) and its first cached
    /// contract named for every strike. Only the positive strikes are
    /// offered, ATM is the one nearest 0, and no strike claims the shared
    /// contract.
    #[test]
    fn a_ladder_below_zero_and_one_contract_for_every_strike_are_left_out() {
        let strikes = (-10..=10)
            .map(|i| entry(50.0 * i as f64, Some("XYZ-FIRST-CE"), None))
            .collect();
        let out = chain_structure(&chain("XYZ", 9.0, 0.0, strikes), Some(1));
        let rows = out["chain"].as_array().cloned().unwrap_or_default();
        assert_eq!(
            rows.iter()
                .map(|r| r["strike"].as_f64().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![50.0, 100.0]
        );
        assert_eq!(out["atm_strike"], 50.0);
        assert!(
            rows.iter().all(|r| r["ce"].is_null() && r["pe"].is_null()),
            "{out}"
        );
    }

    /// The review's finding: this app's chain lists strikes for an
    /// underlying it has no contracts for (its 50-point ladder). With no
    /// listed contract on any strike, the lookup refuses with a sentence
    /// rather than offer strikes no contract has; with contracts, it lists.
    #[test]
    fn a_chain_without_one_listed_contract_is_refused() {
        let ladder = (-10..=10)
            .map(|i| entry(250.0 + 50.0 * i as f64, None, None))
            .collect();
        let refused = listed_structure(&chain("AAPL", 270.4, 250.0, ladder), Some(25)).unwrap_err();
        assert_eq!(refused.status, 404);
        assert_eq!(
            refused.body["message"],
            "No listed option contracts for AAPL in this app's symbol list, so there are no strikes to choose from. A typed strike still works."
        );
        let listed = vec![
            entry(265.0, Some("AAPL261120C00265000"), None),
            entry(270.0, Some("AAPL261120C00270000"), None),
        ];
        let ok = listed_structure(&chain("AAPL", 270.4, 270.0, listed), Some(25)).unwrap();
        assert_eq!(ok["chain"].as_array().map(Vec::len), Some(2));
    }

    /// No strike above zero: an empty chain, no ATM.
    #[test]
    fn a_chain_with_no_positive_strike_is_empty() {
        let out = chain_structure(
            &chain("XYZ", 0.0, 0.0, vec![entry(0.0, None, None)]),
            Some(2),
        );
        assert_eq!(out["chain"], json!([]));
        assert!(out["atm_strike"].is_null());
    }
}
