//! Review Focus 5 (WP8 plan): the broker is gone mid-run (here: no broker
//! logged in, analyzer mode off). A broker-backed node fails with the
//! handler's cause as the no-broker-session sentence, the run ends
//! `failed` and is recorded, and the workflow stays active and free for
//! its next run. (A data node such as `getQuote` stores the client's error
//! dict and goes on, as the web's does — the data goldens pin that.)

use super::*;
use crate::brokers::BrokerRegistry;
use crate::flow::store::workflows::{self, NewWorkflow};
use crate::flow::store::executions;
use crate::flow::support::NO_BROKER_SESSION;
use crate::security::Secret;

#[tokio::test]
async fn a_broker_backed_node_without_a_broker_fails_the_run_and_keeps_the_workflow() {
    let t = crate::state::testing::build(
        BrokerRegistry::new(),
        "2026-10-30T14:00:00Z".parse().unwrap(),
    );
    crate::flow::support::testkit::sign_in(&t.ctx);
    assert!(
        !t.ctx.sqlite.get_analyze_mode().unwrap_or(false),
        "analyzer mode off: the live broker path"
    );
    let nodes = json!([
        node("s", "start", json!({})),
        node(
            "o",
            "placeOrder",
            json!({"symbol": "AAPL", "exchange": "NASDAQ", "action": "BUY", "quantity": 1,
                   "product": "CNC", "priceType": "MARKET"})
        ),
    ]);
    let edges = json!([edge("s", "o", None)]);
    let id = {
        let conn = t.ctx.sqlite.conn().unwrap();
        let w = workflows::create(
            &conn,
            &t.ctx.security,
            &NewWorkflow {
                name: "Buy one",
                description: None,
                nodes: &nodes,
                edges: &edges,
            },
            t.ctx.now(),
        )
        .unwrap();
        workflows::set_active(&conn, w.id, true, t.ctx.now()).unwrap();
        w.id
    };
    let req = RunRequest {
        api_key: Some(Secret::new(t.ctx.internal_token.clone())),
        ..Default::default()
    };
    let out = execute(&t.ctx, id, req).await;
    assert_eq!(out["status"], "error", "{}", out);
    assert_eq!(out["errors"][0]["type"], "placeOrder", "{}", out);
    assert!(
        out["errors"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains(NO_BROKER_SESSION),
        "{}",
        out
    );
    let runs = executions::list(&t.ctx.sqlite.conn().unwrap(), id, 5).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
    assert!(
        runs[0].error.as_deref().unwrap_or_default().contains(NO_BROKER_SESSION),
        "{:?}",
        runs[0].error
    );
    let stored = workflows::get(&t.ctx.sqlite.conn().unwrap(), &t.ctx.security, id)
        .unwrap()
        .unwrap();
    assert!(stored.is_active, "a failed run never stands the workflow down");
    assert!(t.ctx.flow.try_claim(id).is_some(), "free for the next run");
}
