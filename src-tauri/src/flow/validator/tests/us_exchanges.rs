//! This app's US exchanges in the validator (WP8 Task A5): an order on a
//! US venue validates like one on the web's exchanges, and an unknown
//! exchange is refused with the whole accepted list in the error.

use super::*;

const US: [&str; 7] = ["NYSE", "NASDAQ", "AMEX", "ARCA", "BATS", "OTC", "US"];

fn place_order_on(exchange: &str) -> Value {
    single_action(
        "placeOrder",
        json!({"symbol": "AAPL", "exchange": exchange, "action": "BUY", "quantity": 1}),
    )
}

#[test]
fn a_place_order_on_a_us_exchange_validates() {
    for exchange in US {
        let errors = strict(&place_order_on(exchange));
        assert!(errors.is_empty(), "{exchange}: {:?}", codes(&errors));
    }
    // The web's own exchanges still validate.
    assert!(strict(&place_order_on("NSE")).is_empty());
}

#[test]
fn an_unknown_exchange_is_refused_with_the_accepted_list() {
    let errors = strict(&place_order_on("XYZ"));
    assert_eq!(codes(&errors), vec!["invalid_constant"]);
    assert!(errors[0].path.ends_with("/data/exchange"), "{}", errors[0].path);
    let listed = serde_json::to_value(&errors[0]).unwrap()["expected"].clone();
    let listed: Vec<String> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let mut accepted: Vec<String> = crate::flow::support::FLOW_EXCHANGES
        .iter()
        .map(|e| e.to_string())
        .collect();
    accepted.sort();
    assert_eq!(listed, accepted);
    for exchange in US {
        assert!(listed.iter().any(|e| e == exchange), "{exchange} missing");
    }
    assert!(listed.iter().any(|e| e == "NSE"));
    assert!(!listed.iter().any(|e| e == "XYZ"));
}
