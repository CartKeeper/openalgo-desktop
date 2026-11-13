//! WP8 Phase C, Task C2: price alerts on this app's quote poll.
//!
//! The price monitor is upstream's, carried in Phase A: every 5 s
//! (`price_monitor::POLL_INTERVAL`) each armed alert asks for a quote
//! through the app's own `/api/v1/quotes` (`QuotesService::get_quote`, in
//! process) and applies its condition to the price. Here the quotes come
//! from a scripted client, one answer per poll, so a test walks the price
//! (189, then 191, then 192) without a broker; each poll's queued run is
//! awaited before the next (`settle`).

use super::support::*;
use super::*;
use crate::flow::contract::client::fake::FakeFlowClient;
use crate::flow::store::executions;

/// The quotes the polls get, in order (the last one repeats).
fn quotes(prices: &[f64]) -> FakeFlowClient {
    let client = FakeFlowClient::new();
    for p in prices {
        client.reply("quotes", json!({"status": "success", "data": {"ltp": p}}));
    }
    client
}

/// One poll of every armed alert, then the run it queued.
async fn poll(s: &Arc<TriggerSide>, client: &FakeFlowClient) {
    s.check_alerts(client).await;
    settle(s).await;
}

fn run_count(app: &Arc<AppState>, id: i64) -> usize {
    executions::list(&app.sqlite.conn().unwrap(), id, 20).unwrap().len()
}

/// An active Price Alert workflow, registered as activation registers it.
async fn alert(t: &crate::state::testing::TestCtx, s: &Arc<TriggerSide>, data_: Value) -> i64 {
    let id = save(&t.ctx, node("priceAlert", data_.clone()), true);
    s.register(id, "priceAlert", &data(data_)).await.unwrap();
    id
}

/// "Crosses above 190" on AAPL on the US exchange (a new node's default):
/// the poll asks for AAPL on US; 189 is below, 189 -> 191 crosses and runs
/// the workflow once, and the one-time alert then stands its workflow down,
/// so 192 asks for nothing.
#[tokio::test]
async fn a_crosses_above_190_alert_fires_once_when_the_quote_goes_189_to_191() {
    // Tuesday 2026-11-03, 10:00 AM EST.
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let id = alert(
        &t,
        &s,
        json!({"symbol": "AAPL", "exchange": "US", "condition": "crosses_above", "price": 190}),
    )
    .await;
    let client = quotes(&[189.0, 191.0, 192.0]);
    poll(&s, &client).await;
    assert_eq!(run_count(&t.ctx, id), 0, "189 is below 190");
    poll(&s, &client).await;
    assert_eq!(run_count(&t.ctx, id), 1, "189 -> 191 crossed 190");
    poll(&s, &client).await;
    assert_eq!(run_count(&t.ctx, id), 1, "latched: not again at 192");
    assert!(!active(&t.ctx, id), "a one-time alert stands down after its run");
    let asked: Vec<Value> = client
        .calls_to("quotes")
        .into_iter()
        .map(|c| Value::Object(c.payload))
        .collect();
    assert_eq!(
        asked,
        vec![json!({"symbol": "AAPL", "exchange": "US"}); 2],
        "AAPL on US, and no quote once it fired"
    );
}

/// An every-time alert fires on each new cross and never while the price
/// stays above: 189, 191 (runs), 192, 189, 191 (runs again).
#[tokio::test]
async fn an_every_time_alert_fires_on_each_new_cross_and_not_while_above() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let id = alert(
        &t,
        &s,
        json!({"symbol": "AAPL", "exchange": "NASDAQ", "condition": "crosses_above", "price": 190, "trigger": "every_time"}),
    )
    .await;
    let client = quotes(&[189.0, 191.0, 192.0, 189.0, 191.0]);
    let mut counts = Vec::new();
    for _ in 0..5 {
        poll(&s, &client).await;
        counts.push(run_count(&t.ctx, id));
    }
    assert_eq!(counts, vec![0, 1, 1, 1, 2]);
    assert!(active(&t.ctx, id), "an every-time alert keeps watching");
    assert_eq!(
        client.calls_to("quotes")[0].payload["exchange"],
        json!("NASDAQ")
    );
}

/// One run at a time per workflow: a second cross while the first cross's
/// run is still queued is dropped, not queued behind it.
#[tokio::test]
async fn an_alert_has_one_run_at_a_time() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let id = alert(
        &t,
        &s,
        json!({"symbol": "AAPL", "exchange": "US", "condition": "crosses_above", "price": 190, "trigger": "every_time"}),
    )
    .await;
    let client = quotes(&[191.0, 189.0, 191.0]);
    // Three polls before the first run has started: it fires at 191, is
    // quiet at 189, and fires again at 191 while its run is queued.
    for _ in 0..3 {
        s.check_alerts(&client).await;
    }
    settle(&s).await;
    assert_eq!(run_count(&t.ctx, id), 1);
    assert_eq!(client.calls_to("quotes").len(), 3);
}

/// An alert past its expiration window (one hour here) stops watching and
/// stands its workflow down, without asking for a quote.
#[tokio::test]
async fn an_alert_past_its_window_stands_down_without_a_quote() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let id = alert(
        &t,
        &s,
        json!({"symbol": "AAPL", "exchange": "US", "condition": "crosses_above", "price": 190, "expiration": "1h"}),
    )
    .await;
    t.clock.advance(chrono::Duration::minutes(60));
    let client = quotes(&[191.0]);
    poll(&s, &client).await;
    assert!(s.alerts.get(id).is_none());
    assert!(!active(&t.ctx, id));
    assert!(client.calls_to("quotes").is_empty());
    assert_eq!(run_count(&t.ctx, id), 0);
}

/// WP8 Phase D (decision 6d): an every-time alert with a level condition
/// fires when its condition becomes true, and again only after a poll where
/// it was false: "greater than 100" over 101, 102, 103 runs once, not every
/// 5 seconds while the price stays above; 99 then 101 runs it a second time.
#[tokio::test]
async fn an_every_time_greater_than_alert_runs_when_it_becomes_true_not_while_it_stays_true() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let id = alert(
        &t,
        &s,
        json!({"symbol": "AAPL", "exchange": "US", "condition": "greater_than", "price": 100, "trigger": "every_time"}),
    )
    .await;
    let client = quotes(&[101.0, 102.0, 103.0, 99.0, 101.0]);
    let mut counts = Vec::new();
    for _ in 0..5 {
        poll(&s, &client).await;
        counts.push(run_count(&t.ctx, id));
    }
    assert_eq!(counts, vec![1, 1, 1, 1, 2]);
    assert!(active(&t.ctx, id), "an every-time alert keeps watching");
}

/// Any condition that can stay true across polls is a level: an every-time
/// "inside the channel" 95 to 105 runs when the price comes inside, not
/// again while it stays inside, and again after it was outside.
#[tokio::test]
async fn an_every_time_channel_alert_runs_when_the_price_comes_inside() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let id = alert(
        &t,
        &s,
        json!({"symbol": "AAPL", "exchange": "US", "condition": "inside_channel",
               "priceLower": 95, "priceUpper": 105, "trigger": "every_time"}),
    )
    .await;
    let client = quotes(&[100.0, 101.0, 110.0, 100.0]);
    let mut counts = Vec::new();
    for _ in 0..4 {
        poll(&s, &client).await;
        counts.push(run_count(&t.ctx, id));
    }
    assert_eq!(counts, vec![1, 1, 1, 2]);
}
