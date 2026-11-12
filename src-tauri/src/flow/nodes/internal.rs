//! The two order calls Flow makes that the public `/api/v1` schemas do not
//! carry (upstream sends them to its services through an internal entry):
//!
//! - a multi-leg order with a leg that names an absolute `strike` (the leg
//!   schema requires `offset`), parity N-optionsMultiOrder-12;
//! - a Close Positions node naming one position (`/api/v1/closeposition`
//!   takes only a strategy and closes everything).
//!
//! In this app a scoped close is served through the same `/api/v1`
//! handlers every other call takes (`openposition`, then the opposite
//! `placeorder`), so the rate limiter, analyzer routing and Gate B apply;
//! absolute-strike legs have no service here and are refused with one
//! sentence. Task A4 implements [`run`]; until then it answers
//! [`Transport::NotSent`].

use crate::flow::contract::client::ApiCall;
use crate::flow::support::dispatch::{Raw, Transport};
use crate::state::AppState;
use serde_json::{Map, Value};
use std::sync::Arc;

/// Which internal entry a call takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Internal {
    /// `optionsmultiorder` with at least one leg naming a `strike`.
    StrikeLegs,
    /// `closeposition` naming a `symbol`.
    ScopedClose,
}

/// What a strike leg answers in this app.
pub const STRIKE_LEGS_NOT_AVAILABLE: &str = "Legs with an absolute strike are not available in this app: use an offset (ATM, ITM1, OTM1, ...) instead.";

/// The internal entry `call` needs, if any.
pub fn route(call: &ApiCall) -> Option<Internal> {
    match call.endpoint {
        "optionsmultiorder" => call
            .payload
            .get("legs")
            .and_then(Value::as_array)
            .is_some_and(|legs| legs.iter().any(has_strike))
            .then_some(Internal::StrikeLegs),
        "closeposition" => call
            .payload
            .get("symbol")
            .is_some_and(|s| !s.is_null())
            .then_some(Internal::ScopedClose),
        _ => None,
    }
}

fn has_strike(leg: &Value) -> bool {
    leg.get("strike").is_some_and(|s| !s.is_null())
}

/// Send `payload` through `which`.
pub async fn run(
    _app: &Arc<AppState>,
    _api_key: &str,
    _which: Internal,
    _payload: Map<String, Value>,
) -> Result<Raw, Transport> {
    Err(Transport::NotSent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_strike_legs_and_a_scoped_close_take_the_internal_entry() {
        let strike = ApiCall::new(
            "place_options_multi_order",
            "optionsmultiorder",
            json!({"legs": [{"offset": "ATM"}, {"strike": 24000}]}),
        );
        assert_eq!(route(&strike), Some(Internal::StrikeLegs));
        let offsets = ApiCall::new(
            "place_options_multi_order",
            "optionsmultiorder",
            json!({"legs": [{"offset": "ATM"}, {"strike": null}]}),
        );
        assert_eq!(route(&offsets), None);
        let scoped = ApiCall::new("close_position", "closeposition", json!({"symbol": "AAPL"}));
        assert_eq!(route(&scoped), Some(Internal::ScopedClose));
        let all = ApiCall::new("close_position", "closeposition", json!({"strategy": "x"}));
        assert_eq!(route(&all), None);
        let quote = ApiCall::new("quotes", "quotes", json!({"symbol": "AAPL"}));
        assert_eq!(route(&quote), None);
    }
}
