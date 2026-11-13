//! WP8 Phase C, Task C4: order-update triggers from the order book.
//!
//! This app has no order stream. While an order-update watch is active the
//! trigger side reads the order book every 5 s (`order_poll`), through the
//! app's own `OrderbookService` (the sandbox's book in analyzer mode), and
//! sends each order that is new or whose status changed to the app's order
//! updates, which the watches read as upstream's event bus.

use super::support::*;
use super::*;
use crate::flow::store::executions;
use crate::flow::triggers::order_poll::Book;

fn run_count(app: &Arc<AppState>, id: i64) -> usize {
    executions::list(&app.sqlite.conn().unwrap(), id, 20).unwrap().len()
}

/// The sandbox fills a resting order: what a fill between two reads looks
/// like (the sandbox has no fill engine of its own).
fn fill(app: &Arc<AppState>, order_id: &str) {
    app.sqlite
        .conn()
        .unwrap()
        .execute(
            "UPDATE sandbox_orders SET status = 'complete', filled_quantity = quantity,
                    average_price = price WHERE order_id = ?1",
            rusqlite::params![order_id],
        )
        .unwrap();
}

/// One active order-update trigger watching AAPL fills. A sandbox limit
/// order rests (open) at the first read, the baseline; it fills before the
/// second, which sends exactly one update, `complete`, with the order's
/// fields; the watch runs its workflow for it, once, and the one-time
/// watch then stands the workflow down.
#[tokio::test]
async fn a_sandbox_order_filling_between_two_reads_runs_the_watching_workflow() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    t.ctx.sqlite.set_analyze_mode(true).unwrap();
    let s = side(&t);
    s.watches.attach();
    let watched = json!({"symbol": "AAPL", "status": "complete"});
    let id = save(&t.ctx, node("orderUpdateTrigger", watched.clone()), true);
    s.register(id, "orderUpdateTrigger", &data(watched))
        .await
        .unwrap();
    let order = t
        .ctx
        .sqlite
        .place_sandbox_order("AAPL", "NASDAQ", "BUY", 2, 190.0, "LIMIT", "CNC")
        .unwrap();
    let mut updates = t.ctx.order_updates.subscribe();
    let mut book = Book::default();
    assert_eq!(s.poll_order_book(&mut book).await, 0, "the first read is the baseline");
    fill(&t.ctx, &order.order_id);
    assert_eq!(s.poll_order_book(&mut book).await, 1);
    let u = updates.try_recv().unwrap();
    assert!(updates.try_recv().is_err(), "exactly one update");
    assert_eq!(
        (u.orderid.as_str(), u.order_status.as_str(), u.symbol.as_str()),
        (order.order_id.as_str(), "complete", "AAPL")
    );
    assert_eq!((u.mode.as_str(), u.broker.as_str()), ("analyze", "sandbox"));
    assert_eq!((u.filled_quantity, u.average_price), (2, 190.0));
    assert_eq!(s.poll_order_book(&mut book).await, 0, "nothing changed since");
    // What the side's own subscriber does with it (`drive::subscribe_order_updates`).
    s.on_order_update(&u);
    settle(&s).await;
    assert_eq!(run_count(&t.ctx, id), 1);
    assert_eq!(
        executions::list(&t.ctx.sqlite.conn().unwrap(), id, 1).unwrap()[0].status,
        "completed"
    );
    assert!(!active(&t.ctx, id), "the one-time watch stood its workflow down");
}

/// The poller reads the book only while a watch is active: none for half a
/// minute with no watch, a read every 5 s once one is registered, none
/// again after it is removed; no run starts.
#[tokio::test(start_paused = true)]
async fn the_order_book_is_read_only_while_a_watch_is_active() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    t.ctx.sqlite.set_analyze_mode(true).unwrap();
    let s = side(&t);
    let (_guard, logs) = logged();
    let reads = || logs.starting_with("OrderbookService::get_orderbook").len();
    s.start_drivers();
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    assert_eq!(reads(), 0, "no watch, no read");
    s.register(
        1,
        "orderUpdateTrigger",
        &data(json!({"symbol": "AAPL", "status": "complete"})),
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(12_500)).await;
    assert_eq!(reads(), 3, "at once, then every 5 s");
    s.unregister(1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let after = reads();
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    assert_eq!(reads(), after, "no watch, no read");
    assert_eq!(t.ctx.flow.running(), 0, "no run started");
    s.stop().await;
}
