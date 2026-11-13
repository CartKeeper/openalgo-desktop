//! WP8 Phase D (coordinator decision 6d): an order-update watch never fires
//! on an order its own workflow placed. Without this an every-time watch on
//! AAPL whose run buys AAPL re-runs itself on each of its own fills.

use super::support::*;
use super::*;
use crate::events::OrderUpdate;
use crate::flow::store::executions;

fn run_count(app: &Arc<AppState>, id: i64) -> usize {
    executions::list(&app.sqlite.conn().unwrap(), id, 50)
        .unwrap()
        .len()
}

/// An active workflow with these nodes, its trigger first, wired in order.
fn wired(app: &Arc<AppState>, name: &str, nodes: Value) -> i64 {
    let ids: Vec<String> = nodes
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    let edges: Vec<Value> = ids
        .windows(2)
        .map(|w| json!({"id": format!("{}-{}", w[0], w[1]), "source": w[0], "target": w[1]}))
        .collect();
    let conn = app.sqlite.conn().unwrap();
    let w = workflows::create(
        &conn,
        &app.security,
        &NewWorkflow {
            name,
            description: None,
            nodes: &nodes,
            edges: &Value::Array(edges),
        },
        app.now(),
    )
    .unwrap();
    workflows::set_active(&conn, w.id, true, app.now()).unwrap();
    w.id
}

fn buy_aapl() -> Value {
    json!({"id": "buy", "type": "placeOrder", "position": {"x": 0, "y": 160},
           "data": {"symbol": "AAPL", "exchange": "US", "action": "BUY", "quantity": 1,
                    "priceType": "MARKET", "product": "CNC"}})
}

/// The sandbox's newest order id that is not one of `known`.
fn new_order(app: &Arc<AppState>, known: &[String]) -> String {
    app.sqlite
        .get_sandbox_orders()
        .unwrap()
        .into_iter()
        .map(|o| o.order_id)
        .find(|id| !known.contains(id))
        .expect("a new sandbox order")
}

/// A sandbox AAPL order filled, as the order-book poller reports it.
fn filled(order_id: &str) -> OrderUpdate {
    OrderUpdate {
        mode: "analyze".into(),
        broker: "sandbox".into(),
        orderid: order_id.into(),
        symbol: "AAPL".into(),
        exchange: "US".into(),
        action: "BUY".into(),
        quantity: 1,
        order_status: "complete".into(),
        filled_quantity: 1,
        ..Default::default()
    }
}

#[tokio::test]
async fn a_watch_is_not_run_again_by_its_own_order_but_is_by_another_workflows() {
    let (_guard, logs) = logged();
    let t = eastern("2026-11-03T15:00:00Z").await;
    t.ctx.sqlite.set_analyze_mode(true).unwrap();
    let s = side(&t);
    s.watches.attach();
    let watch = json!({"symbol": "AAPL", "status": "complete", "trigger": "every_time"});
    let watcher = wired(
        &t.ctx,
        "Buy AAPL on every AAPL fill",
        json!([
            {"id": "t", "type": "orderUpdateTrigger", "position": {"x": 0, "y": 0}, "data": watch.clone()},
            buy_aapl()
        ]),
    );
    s.register(watcher, "orderUpdateTrigger", &data(watch))
        .await
        .unwrap();
    let other = wired(
        &t.ctx,
        "Buy AAPL now",
        json!([
            {"id": "s", "type": "start", "position": {"x": 0, "y": 0}, "data": {"scheduleType": "manual"}},
            buy_aapl()
        ]),
    );

    // Another workflow's AAPL order fills: the watch runs, and its run
    // buys AAPL itself.
    let ran = crate::flow::service::run_now(&t.ctx, other).await;
    assert_eq!(ran.body["status"], "success", "{}", ran.body);
    let theirs = new_order(&t.ctx, &[]);
    s.on_order_update(&filled(&theirs));
    settle(&s).await;
    assert_eq!(run_count(&t.ctx, watcher), 1);
    let ours = new_order(&t.ctx, std::slice::from_ref(&theirs));

    // Its own order fills: the watch does not run again, and says why.
    s.on_order_update(&filled(&ours));
    settle(&s).await;
    assert_eq!(run_count(&t.ctx, watcher), 1, "its own fill ran it again");
    assert!(
        logs.has(&format!(
            "Order-update watch of workflow {} skipped order {} (complete): its own run placed it",
            watcher, ours
        )),
        "{:?}",
        logs.0.lock()
    );

    // Another of the other workflow's orders still runs it.
    crate::flow::service::run_now(&t.ctx, other).await;
    let again = new_order(&t.ctx, &[theirs, ours]);
    s.on_order_update(&filled(&again));
    settle(&s).await;
    assert_eq!(run_count(&t.ctx, watcher), 2);
}

/// The review's race: the watcher's own order shows in the book while its
/// run is still going and before the client has recorded its id, so the
/// update is kept as the every-time watch's follow-up. By the time the
/// follow-up would run the id is recorded, and it does not run.
#[tokio::test]
async fn a_follow_up_for_the_watchers_own_order_does_not_run() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    t.ctx.sqlite.set_analyze_mode(true).unwrap();
    let s = side(&t);
    s.watches.attach();
    let watch = json!({"symbol": "AAPL", "status": "complete", "trigger": "every_time"});
    let watcher = wired(
        &t.ctx,
        "Buy AAPL on every AAPL fill",
        json!([
            {"id": "t", "type": "orderUpdateTrigger", "position": {"x": 0, "y": 0}, "data": watch.clone()},
            buy_aapl()
        ]),
    );
    s.register(watcher, "orderUpdateTrigger", &data(watch))
        .await
        .unwrap();
    // Another workflow's fill claims the watch and queues its run; the
    // watcher's own order fills before that run's reply is recorded.
    s.on_order_update(&filled("THEIRS-1"));
    s.on_order_update(&filled("OWN-1"));
    t.ctx
        .flow
        .own_orders
        .record(watcher, ["OWN-1".to_string()]);
    settle(&s).await;
    assert_eq!(run_count(&t.ctx, watcher), 1, "its own fill ran it again");
}
