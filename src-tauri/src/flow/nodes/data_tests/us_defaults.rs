//! A node with no `exchange` reads this app's `US` (WP8 Phase B follow-up),
//! the default the editor gives a new node; the goldens and the tests
//! ported from the web, replayed in IST, keep the web's `NSE`.

use crate::clock::ManualClock;
use crate::flow::contract::client::fake::FakeFlowClient;
use crate::flow::contract::env::NodeEnv;
use crate::flow::contract::node::{NodeCtx, NodeCtxParts};
use crate::flow::contract::subscriptions::Subscriptions;
use crate::flow::contract::time::{set_market_tz_for_tests, DEFAULT_MARKET_TZ};
use crate::flow::nodes::{data, NodeRegistry};
use serde_json::json;
use std::sync::{Arc, Weak};
use tokio_util::sync::CancellationToken;

/// A context on Thursday 29 October 2026, 10:15 ET.
fn eastern(client: Arc<FakeFlowClient>) -> NodeCtx {
    set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    NodeCtx::new(NodeCtxParts {
        client,
        clock: ManualClock::new("2026-10-29T14:15:00Z".parse().unwrap()),
        cancel: CancellationToken::new(),
        workflow_id: None,
        default_strategy: "US Flow".into(),
        subscriptions: Arc::new(Subscriptions::default()),
        session_boundary: (3, 0),
        env: NodeEnv::default(),
        app: Weak::new(),
    })
}

#[tokio::test]
async fn a_quote_with_no_exchange_asks_for_the_us_venue() {
    let client = Arc::new(FakeFlowClient::new());
    client.reply("quotes", json!({"status": "success", "data": {"ltp": 190.0}}));
    let ctx = eastern(client.clone());
    let registry = NodeRegistry::new(&[data::FAMILY]);
    let node = registry.get("getQuote").expect("the Get Quote node");
    node.run(&ctx, json!({"symbol": "AAPL"}).as_object().unwrap())
        .await;
    let calls = client.calls_to("quotes");
    assert_eq!(calls.len(), 1, "{:?}", client.calls());
    assert_eq!(calls[0].payload["exchange"], "US", "{:?}", calls[0].payload);
}
