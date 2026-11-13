//! WP8 Phase D, Task D3: a workflow exported from OpenAlgo web and this
//! app's own US workflow, through import, export and a run.
//!
//! - `web-8.6-nifty-custom-iron-fly.json` (upstream's e2e fixture, carried
//!   to `tests/fixtures/flow/`): an Indian options workflow. The app takes
//!   it (the validator finds nothing to refuse) and says its schedule is
//!   read as Eastern time; a run with a US broker logged in stops at the
//!   options node with the sentence naming the Indian broker it needs.
//! - `us-schedule-quote-order.json` (this app's): Schedule, Get Quote AAPL,
//!   Price Condition, Place Order 1 share MARKET CNC on `US`, Telegram
//!   Alert. It imports, exports byte for byte (keys sorted), and runs in
//!   analyzer mode: the quote is scripted (no broker in tests), the order
//!   goes through the app's own `/api/v1/placeorder` to the sandbox, and the
//!   alert through the app's Telegram send to a fake Bot API.

use super::*;
use crate::flow::contract::client::app::AppFlowClient;
use crate::flow::contract::client::{ApiCall, FlowClient};
use crate::flow::contract::node::{NodeCtx, NodeCtxParts};
use crate::flow::executor::walk::Walk;
use crate::flow::service::transfer;
use crate::messaging::telegram::{self, fake::FakeHttp};
use crate::state::testing::TestCtx;
use std::collections::BTreeMap;

const TOKEN: &str = "123456789:AAFlowTestTokenNotARealOneWXYZ";

fn fixture(name: &str) -> Value {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/flow")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} could not be read: {}", path.display(), e));
    serde_json::from_str(&text).expect("the fixture is JSON")
}

/// The app at 10:00 AM ET on Thursday 2026-11-12, signed in, Eastern time.
fn eastern() -> TestCtx {
    crate::flow::contract::time::set_market_tz_for_tests(chrono_tz::America::New_York);
    let t = crate::state::testing::build(
        crate::brokers::BrokerRegistry::new(),
        "2026-11-12T15:00:00Z".parse().unwrap(),
    );
    crate::flow::support::testkit::sign_in(&t.ctx);
    t
}

/// `v` written with every object's keys in order, so two values compare
/// as bytes whatever order their keys were written in.
fn sorted(v: &Value) -> String {
    fn order(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let o: BTreeMap<&String, Value> = m.iter().map(|(k, x)| (k, order(x))).collect();
                serde_json::to_value(o).unwrap_or(Value::Null)
            }
            Value::Array(a) => Value::Array(a.iter().map(order).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string(&order(v)).unwrap_or_default()
}

/// The web's iron fly imports as a new inactive workflow, with the note
/// that its schedule is read as Eastern time; the validator refuses
/// nothing in it.
#[tokio::test]
async fn the_webs_iron_fly_imports_with_the_eastern_time_note() {
    let t = eastern();
    let fly = fixture("web-8.6-nifty-custom-iron-fly.json");
    assert_eq!(
        crate::flow::validator::validate_workflow(&fly, ValidateOptions::COMPLETE),
        vec![],
        "the validator's answer for the web's iron fly"
    );
    let r = transfer::import(&t.ctx, Body::Json(fly)).await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert_eq!(
        r.body["notes"],
        json!(["Schedule and time-window times are read as Eastern time in this app"])
    );
    let id = r.body["workflow_id"].as_i64().expect("an id");
    let w = load(&t.ctx, id).await.unwrap().expect("stored");
    assert_eq!(w.name, "NIFTY Custom Iron Fly (imported)");
    assert!(!w.is_active);
}

/// What the app refuses: run with a US broker logged in, the iron fly
/// stops at its options node. The node first asks for NIFTY's expiries on
/// NFO, which the app answers with the sentence naming the Indian broker
/// they need (no call reaches a broker); the run's log carries it, then the
/// node's own (the web's) failure, that the expiry could not be resolved.
#[tokio::test]
async fn the_iron_fly_run_with_a_us_broker_names_the_indian_broker_it_needs() {
    let t = eastern();
    t.ctx.set_broker_session(Some(crate::state::BrokerSession {
        broker_id: "alpaca".into(),
        auth_token: "key:secret".into(),
        feed_token: None,
        user_id: "ACCOUNT1".into(),
        user_name: None,
        authenticated_at: chrono::Utc::now(),
    }));
    let r = transfer::import(
        &t.ctx,
        Body::Json(fixture("web-8.6-nifty-custom-iron-fly.json")),
    )
    .await;
    let id = r.body["workflow_id"].as_i64().expect("an id");
    let run = run_now(&t.ctx, id).await;
    assert_eq!(run.body["status"], "error", "{}", run.body);
    assert_eq!(
        run.body["message"],
        "1 node(s) failed: optionsMultiOrder: Could not resolve expiry for current_week"
    );
    let lines: Vec<String> = run.body["logs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|l| l["message"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        lines.iter().any(|l| l.starts_with("Failed to fetch expiry: ")
            && l.contains("'NIFTY on NFO needs an Indian broker: log in to Zerodha, Angel One or Fyers. The broker logged in now (Alpaca Markets) trades US markets in this app.'")),
        "{:?}",
        lines
    );
}

/// This app's US workflow imports, and exports with the same graph and
/// description, byte for byte once keys are sorted.
#[tokio::test]
async fn the_us_workflow_imports_and_exports_byte_for_byte() {
    let t = eastern();
    let us = fixture("us-schedule-quote-order.json");
    let r = transfer::import(&t.ctx, Body::Json(us.clone())).await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert_eq!(
        r.body["notes"],
        json!(["Schedule and time-window times are read as Eastern time in this app"])
    );
    let id = r.body["workflow_id"].as_i64().expect("an id");
    let out = transfer::export(&t.ctx, id).await;
    assert_eq!(out.status, 200, "{}", out.body);
    for key in ["nodes", "edges", "description"] {
        assert_eq!(sorted(&out.body[key]), sorted(&us[key]), "{key}");
    }
    assert_eq!(out.body["name"], "AAPL morning buy (imported)");
    assert_eq!(out.body["version"], "1.0");
}

/// The run's client: AAPL's quote scripted (no broker in tests), every
/// other call and the alert through the app's own client.
struct QuotedApp {
    quote: Value,
    app: AppFlowClient,
}

#[async_trait]
impl FlowClient for QuotedApp {
    async fn api(&self, call: ApiCall) -> Value {
        if call.endpoint == "quotes" {
            return self.quote.clone();
        }
        self.app.api(call).await
    }

    async fn telegram(&self, message: &str) -> Value {
        self.app.telegram(message).await
    }

    async fn whatsapp(&self, message: &str, to: Option<&str>) -> Value {
        self.app.whatsapp(message, to).await
    }
}

/// The imported US workflow runs end to end in analyzer mode: the quote
/// passes the condition, one AAPL share goes to the sandbox at market as
/// CNC on `US`, and the Telegram alert reaches the saved chat with the
/// quote and the order id filled in.
#[tokio::test]
async fn the_us_workflow_runs_end_to_end_in_analyzer_mode() {
    let t = eastern();
    t.ctx.sqlite.set_analyze_mode(true).unwrap();
    telegram::save(&t.ctx.sqlite, &t.ctx.security, Some(TOKEN), "987654321").unwrap();
    let http = FakeHttp::delivering();
    telegram::send_through_for_tests(http.clone());
    let r = transfer::import(&t.ctx, Body::Json(fixture("us-schedule-quote-order.json"))).await;
    let id = r.body["workflow_id"].as_i64().expect("an id");
    let w = load(&t.ctx, id).await.unwrap().expect("stored");
    let client = QuotedApp {
        quote: json!({"status": "success", "data": {"ltp": 271.5, "open": 270.1, "high": 272.0,
                      "low": 269.8, "prev_close": 269.4, "volume": 1200000, "bid": 271.45, "ask": 271.55}}),
        app: AppFlowClient::new(
            Arc::downgrade(&t.ctx),
            crate::security::Secret::new(t.ctx.internal_token.clone()),
        ),
    };
    let ctx = NodeCtx::new(NodeCtxParts {
        client: Arc::new(client),
        clock: t.ctx.clock.clone(),
        cancel: tokio_util::sync::CancellationToken::new(),
        workflow_id: Some(id),
        default_strategy: w.name.clone(),
        subscriptions: t.ctx.flow.subscriptions.clone(),
        session_boundary: t.ctx.session_boundary(),
        env: t.ctx.flow.env(),
        app: Arc::downgrade(&t.ctx),
    });
    let graph = w.graph().expect("a graph");
    let trigger = graph.trigger().expect("a trigger");
    let mut walk = Walk::new(&graph, &t.ctx.flow.nodes, &ctx);
    walk.visit(&trigger.id, 0).await.expect("the walk ends");
    assert!(walk.errors.is_empty(), "{:?} {:?}", walk.errors, ctx.logs());
    let orders = t.ctx.sqlite.get_sandbox_orders().unwrap();
    assert_eq!(orders.len(), 1, "{:?}", orders);
    let o = &orders[0];
    assert_eq!(
        (
            o.symbol.as_str(),
            o.exchange.as_str(),
            o.side.as_str(),
            o.quantity,
            o.order_type.as_str(),
            o.product.as_str()
        ),
        ("AAPL", "US", "BUY", 1, "MARKET", "CNC")
    );
    let sent = http.calls();
    assert_eq!(sent.len(), 1, "{:?}", sent);
    assert_eq!(
        sent[0].0,
        format!("https://api.telegram.org/bot{}/sendMessage", TOKEN)
    );
    assert_eq!(sent[0].1["chat_id"], "987654321");
    let text = sent[0].1["text"].as_str().unwrap_or_default();
    assert!(
        text.contains(&format!(
            "Bought 1 AAPL share at about 271.5 (order {})",
            o.order_id
        )),
        "{}",
        text
    );
}
