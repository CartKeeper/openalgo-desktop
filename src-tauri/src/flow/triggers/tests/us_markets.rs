//! WP8 Phase D (coordinator decision 6f): the US labels are one market when
//! a watch or an alert is matched. This app's Alpaca adapter puts `US` in
//! every order and quote it reads (`brokers/alpaca`: `map_alpaca_orders`,
//! `get_quote`), the sandbox keeps the exchange an order was placed on (`US`
//! for a new node, or the venue the trader picked), and a trader may pick
//! NASDAQ for a watch: a US ticker is unique across US venues, so NYSE,
//! NASDAQ, AMEX, ARCA, BATS, OTC and US all match each other. Indian and
//! crypto labels keep matching exactly.

use super::support::*;
use super::*;
use crate::brokers::types::Order;
use crate::flow::store::executions;
use crate::flow::triggers::order_monitor::Watch;
use crate::flow::triggers::order_poll::Book;

/// An order as the app's order book reads it, on `exchange`.
fn order(id: &str, exchange: &str, status: &str) -> Order {
    serde_json::from_value(json!({
        "order_id": id, "exchange_order_id": null, "symbol": "AAPL", "exchange": exchange,
        "side": "BUY", "quantity": 1.0, "filled_quantity": 1.0, "pending_quantity": 0.0,
        "price": 0.0, "trigger_price": 0.0, "average_price": 190.25, "order_type": "MARKET",
        "product": "CNC", "status": status, "validity": "DAY",
        "order_timestamp": "2026-11-03T15:00:00Z", "exchange_timestamp": null,
        "rejection_reason": null
    }))
    .unwrap()
}

/// The update the order-book poller sends for an order that filled on
/// `exchange` (the poller's own diff, `Book::changes`).
fn filled_on(exchange: &str) -> crate::events::OrderUpdate {
    let mut book = Book::default();
    book.changes(
        "live",
        "alpaca",
        "ACCOUNT1",
        &[order("A-1", exchange, "OPEN")],
    );
    let mut updates = book.changes(
        "live",
        "alpaca",
        "ACCOUNT1",
        &[order("A-1", exchange, "COMPLETE")],
    );
    assert_eq!(updates.len(), 1);
    updates.remove(0)
}

fn watch_on(exchange: &str) -> Watch {
    Watch::from_node(
        1,
        &data(json!({"symbol": "AAPL", "exchange": exchange, "status": "complete"})),
    )
    .unwrap()
}

#[test]
fn a_nasdaq_watch_matches_alpacas_us_order() {
    assert!(watch_on("NASDAQ").matches(&filled_on("US")));
}

#[test]
fn a_us_watch_matches_an_order_on_any_us_venue() {
    for venue in ["NYSE", "NASDAQ", "AMEX", "ARCA", "BATS", "OTC", "US"] {
        assert!(watch_on("US").matches(&filled_on(venue)), "{venue}");
        assert!(watch_on(venue).matches(&filled_on("US")), "{venue}");
    }
    assert!(watch_on("NYSE").matches(&filled_on("ARCA")));
}

#[test]
fn indian_and_crypto_labels_match_only_themselves() {
    assert!(watch_on("NSE").matches(&filled_on("NSE")));
    assert!(!watch_on("NSE").matches(&filled_on("BSE")));
    assert!(!watch_on("NSE").matches(&filled_on("US")));
    assert!(watch_on("CRYPTO").matches(&filled_on("CRYPTO")));
    assert!(!watch_on("CRYPTO").matches(&filled_on("US")));
    assert!(!watch_on("US").matches(&filled_on("CRYPTO")));
    assert!(!watch_on("US").matches(&filled_on("NSE")));
}

/// The sandbox's own label: an order placed on NASDAQ (a trader's pick)
/// fills between two reads of the book, and a watch on `US` (a new node's
/// default) runs its workflow.
#[tokio::test]
async fn a_us_watch_runs_on_a_sandbox_order_placed_on_nasdaq() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    t.ctx.sqlite.set_analyze_mode(true).unwrap();
    let s = side(&t);
    s.watches.attach();
    let watched = json!({"symbol": "AAPL", "exchange": "US", "status": "complete"});
    let id = save(&t.ctx, node("orderUpdateTrigger", watched.clone()), true);
    s.register(id, "orderUpdateTrigger", &data(watched))
        .await
        .unwrap();
    let placed = t
        .ctx
        .sqlite
        .place_sandbox_order("AAPL", "NASDAQ", "BUY", 1, 190.0, "LIMIT", "CNC")
        .unwrap();
    let mut updates = t.ctx.order_updates.subscribe();
    let mut book = Book::default();
    s.poll_order_book(&mut book).await;
    t.ctx
        .sqlite
        .conn()
        .unwrap()
        .execute(
            "UPDATE sandbox_orders SET status = 'complete', filled_quantity = quantity,
                    average_price = price WHERE order_id = ?1",
            rusqlite::params![placed.order_id],
        )
        .unwrap();
    assert_eq!(s.poll_order_book(&mut book).await, 1);
    let u = updates.try_recv().unwrap();
    assert_eq!(u.exchange, "NASDAQ", "the sandbox keeps the venue");
    s.on_order_update(&u);
    settle(&s).await;
    assert_eq!(
        executions::list(&t.ctx.sqlite.conn().unwrap(), id, 5)
            .unwrap()
            .len(),
        1
    );
}

/// An alert asks for its quote on its own label and reads the price, never
/// the quote's label: an alert on NASDAQ fires on the quote this app's
/// Alpaca adapter answers with `US` (already so; pinned here).
#[tokio::test]
async fn a_nasdaq_alert_fires_on_alpacas_us_quote() {
    let t = eastern("2026-11-03T15:00:00Z").await;
    let s = side(&t);
    let alert =
        json!({"symbol": "AAPL", "exchange": "NASDAQ", "condition": "crosses_above", "price": 190});
    let id = save(&t.ctx, node("priceAlert", alert.clone()), true);
    s.register(id, "priceAlert", &data(alert)).await.unwrap();
    let client = crate::flow::contract::client::fake::FakeFlowClient::new();
    for ltp in [189.0, 191.0] {
        client.reply(
            "quotes",
            json!({"status": "success", "data": {"symbol": "AAPL", "exchange": "US", "ltp": ltp}}),
        );
    }
    for _ in 0..2 {
        s.check_alerts(&client).await;
        settle(&s).await;
    }
    assert_eq!(
        executions::list(&t.ctx.sqlite.conn().unwrap(), id, 5)
            .unwrap()
            .len(),
        1
    );
}
