//! Order updates from the order book (WP8 Phase C).
//!
//! Upstream's order-update monitor hears every order change on its event
//! bus, which each broker's order stream feeds. This app has no order
//! stream, so while any order-update watch is active the trigger side reads
//! the order book every [`POLL_INTERVAL`] through the app's own
//! `OrderbookService` (the sandbox's book in analyzer mode, the broker's
//! otherwise) and sends one [`OrderUpdate`] for each order that is new to
//! the book or whose status changed since the last read, on the app's order
//! updates (`crate::events`), which the watches read as upstream's bus.
//!
//! The first read of a book is its baseline and reports nothing, so turning
//! a watch on never fires for orders already done; a new source (analyzer
//! mode switched, another broker, or another account on the same broker:
//! Alpaca's paper keys, then its live keys) is a new baseline, so another
//! account's history never runs a workflow. A read made while the broker
//! session changed is dropped. With no watch the poller reads nothing. A change is seen at most [`POLL_INTERVAL`] late,
//! and only the status a read finds (a partial fill shows as `open` until
//! the order completes).

use super::TriggerSide;
use crate::brokers::types::Order;
use crate::events::OrderUpdate;
use crate::services::orderbook_service::OrderbookService;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// How often the book is read while an order-update watch is active.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// An order's status in the watches' words (upstream's: `open`, `trigger
/// pending`, `complete`, `cancelled`, `rejected`), from this app's: the
/// brokers' mapped statuses (`COMPLETE`, `PARTIALLY_FILLED`, `PENDING`, ...)
/// and the sandbox's (`complete`, `pending`, `cancelled`). Anything else is
/// passed on as given, lower-cased, so a watch on `any` still hears it.
pub fn watch_status(status: &str) -> String {
    let given = status.trim().to_lowercase().replace('_', " ");
    match given.as_str() {
        "complete" | "filled" | "done for day" => "complete".to_string(),
        "open" | "partially filled" | "pending" | "new" | "accepted" | "pending new"
        | "working" | "submitted" | "presubmitted" | "pending cancel" | "pending replace" => {
            "open".to_string()
        }
        "trigger pending" => "trigger pending".to_string(),
        "cancelled" | "canceled" | "expired" | "replaced" => "cancelled".to_string(),
        "rejected" => "rejected".to_string(),
        _ => given,
    }
}

/// What the last read of the book showed: whose book it was (the mode, the
/// broker and the account) and each order's status in the watches' words.
#[derive(Default)]
pub struct Book {
    source: Option<(String, String, String)>,
    statuses: HashMap<String, String>,
}

impl Book {
    /// The updates since the last read: one for each order new to the book
    /// or whose status changed. The first read of a source is its baseline
    /// and reports nothing. Orders gone from the book are forgotten.
    pub fn changes(
        &mut self,
        mode: &str,
        broker: &str,
        account: &str,
        orders: &[Order],
    ) -> Vec<OrderUpdate> {
        let source = (mode.to_string(), broker.to_string(), account.to_string());
        let baseline = self.source.as_ref() != Some(&source);
        let mut statuses = HashMap::with_capacity(orders.len());
        let mut updates = Vec::new();
        for o in orders {
            let status = watch_status(&o.status);
            if !baseline && self.statuses.get(&o.order_id) != Some(&status) {
                updates.push(update(mode, broker, o, &status));
            }
            statuses.insert(o.order_id.clone(), status);
        }
        self.source = Some(source);
        self.statuses = statuses;
        updates
    }
}

/// The update for one order, in upstream's `OrderUpdate` fields.
fn update(mode: &str, broker: &str, o: &Order, status: &str) -> OrderUpdate {
    OrderUpdate {
        mode: mode.to_string(),
        broker: broker.to_string(),
        orderid: o.order_id.clone(),
        symbol: o.symbol.clone(),
        exchange: o.exchange.clone(),
        action: o.side.clone(),
        quantity: o.quantity as i64,
        price: o.price,
        trigger_price: o.trigger_price,
        pricetype: o.order_type.clone(),
        product: o.product.clone(),
        order_status: status.to_string(),
        filled_quantity: o.filled_quantity as i64,
        pending_quantity: o.pending_quantity as i64,
        average_price: o.average_price,
        rejection_reason: o.rejection_reason.clone().unwrap_or_default(),
        session_generation: 0,
    }
}

impl TriggerSide {
    /// The poller: while any order-update watch is active, the order book
    /// every [`POLL_INTERVAL`] (the first read at once); idle, reading
    /// nothing, while there is none. Woken when a watch is added; ends at
    /// app exit.
    pub async fn poll_orders(self: Arc<Self>) {
        let mut book = Book::default();
        while self.app().is_some() {
            if self.watches.is_empty() {
                // Watching again later starts from a fresh baseline.
                book = Book::default();
                tokio::select! {
                    _ = self.wake_orders.notified() => continue,
                    _ = self.stop.cancelled() => break,
                }
            }
            self.poll_order_book(&mut book).await;
            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = self.stop.cancelled() => break,
            }
        }
    }

    /// One read of the book: each change since the last read sent to the
    /// app's order updates; how many. A failed read (no broker logged in)
    /// keeps the last book, so a change made meanwhile is still reported.
    /// The book is labelled with the broker session it was read under (the
    /// broker and its account, the session's user id); a read made while
    /// the session changed is dropped, never labelled as either account's.
    pub(crate) async fn poll_order_book(&self, book: &mut Book) -> usize {
        let Some(app) = self.app() else {
            return 0;
        };
        let session = |app: &crate::state::AppState| {
            app.get_broker_session()
                .map(|s| (s.broker_id, s.user_id))
        };
        let before = session(&app);
        let read = OrderbookService::get_orderbook(&app, None).await;
        if session(&app) != before {
            tracing::debug!(
                "Flow dropped an order-book read made while the broker session changed"
            );
            return 0;
        }
        match read {
            Ok(read) => {
                let (broker, account) = if read.mode == "analyze" {
                    ("sandbox".to_string(), String::new())
                } else {
                    before.unwrap_or_default()
                };
                let updates = book.changes(&read.mode, &broker, &account, &read.orders);
                let sent = updates.len();
                for u in updates {
                    // No subscriber (none since a stop) is nothing lost.
                    let _ = app.order_updates.send(u);
                }
                sent
            }
            Err(e) => {
                tracing::debug!(
                    "Flow could not read the order book for its order-update triggers: {}",
                    e
                );
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn order(id: &str, status: &str) -> Order {
        serde_json::from_value(json!({
            "order_id": id, "exchange_order_id": null, "symbol": "AAPL", "exchange": "NASDAQ",
            "side": "BUY", "quantity": 10.0, "filled_quantity": 4.0, "pending_quantity": 6.0,
            "price": 190.5, "trigger_price": 0.0, "average_price": 190.25, "order_type": "LIMIT",
            "product": "CNC", "status": status, "validity": "DAY",
            "order_timestamp": "2026-11-03T15:00:00Z", "exchange_timestamp": null,
            "rejection_reason": null
        }))
        .unwrap()
    }

    /// This app's statuses (the brokers' mapped words and the sandbox's)
    /// read as the watches' vocabulary.
    #[test]
    fn the_apps_order_statuses_read_as_the_watches_words() {
        for (given, read) in [
            ("COMPLETE", "complete"),
            ("complete", "complete"),
            ("FILLED", "complete"),
            ("OPEN", "open"),
            ("PARTIALLY_FILLED", "open"),
            ("PENDING", "open"),
            ("pending", "open"),
            ("TRIGGER PENDING", "trigger pending"),
            ("TRIGGER_PENDING", "trigger pending"),
            ("CANCELLED", "cancelled"),
            ("canceled", "cancelled"),
            ("EXPIRED", "cancelled"),
            ("REJECTED", "rejected"),
            ("UNKNOWN", "unknown"),
        ] {
            assert_eq!(watch_status(given), read, "{}", given);
        }
    }

    /// The first read of a book is the baseline and reports nothing; after
    /// it an order whose status changed, or one new to the book, is one
    /// update each; an unchanged order is none.
    #[test]
    fn a_book_reports_each_new_order_and_each_status_change_once() {
        let mut book = Book::default();
        assert!(book
            .changes("analyze", "sandbox", "", &[order("1", "pending"), order("2", "complete")])
            .is_empty());
        let updates = book.changes(
            "analyze",
            "sandbox",
            "",
            &[order("1", "complete"), order("2", "complete"), order("3", "pending")],
        );
        let seen: Vec<(String, String)> = updates
            .iter()
            .map(|u| (u.orderid.clone(), u.order_status.clone()))
            .collect();
        assert_eq!(
            seen,
            vec![("1".into(), "complete".into()), ("3".into(), "open".into())]
        );
        let u = &updates[0];
        assert_eq!(
            (u.mode.as_str(), u.broker.as_str(), u.symbol.as_str(), u.exchange.as_str()),
            ("analyze", "sandbox", "AAPL", "NASDAQ")
        );
        assert_eq!((u.action.as_str(), u.pricetype.as_str(), u.product.as_str()), ("BUY", "LIMIT", "CNC"));
        assert_eq!((u.quantity, u.filled_quantity, u.pending_quantity), (10, 4, 6));
        assert_eq!((u.price, u.average_price), (190.5, 190.25));
        assert!(book
            .changes("analyze", "sandbox", "", &[order("1", "complete"), order("3", "pending")])
            .is_empty());
    }

    /// Another account on the same broker (Alpaca paper keys, then live
    /// keys) is a new source: the new account's history (its last orders,
    /// filled and cancelled) is its baseline, never updates that would run
    /// a watching workflow; a same-account sign-in again keeps the book.
    #[test]
    fn another_account_on_the_same_broker_is_a_new_baseline() {
        let mut book = Book::default();
        assert!(book
            .changes("live", "alpaca", "PA-PAPER", &[order("p1", "OPEN")])
            .is_empty());
        let history = [order("x1", "COMPLETE"), order("x2", "CANCELLED"), order("x3", "FILLED")];
        assert!(
            book.changes("live", "alpaca", "LIVE-ACCT", &history).is_empty(),
            "the live account's history is its baseline"
        );
        let filled = [order("x1", "COMPLETE"), order("x2", "CANCELLED"), order("x3", "FILLED"), order("x4", "COMPLETE")];
        let seen: Vec<String> = book
            .changes("live", "alpaca", "LIVE-ACCT", &filled)
            .into_iter()
            .map(|u| u.orderid)
            .collect();
        assert_eq!(seen, vec!["x4".to_string()], "the same account goes on from its book");
    }

    /// Another source (analyzer mode switched, another broker) is a new
    /// baseline: its history is not reported as updates.
    #[test]
    fn a_new_source_is_a_new_baseline() {
        let mut book = Book::default();
        book.changes("analyze", "sandbox", "", &[order("1", "pending")]);
        assert!(book
            .changes("live", "alpaca", "PA1", &[order("a", "COMPLETE"), order("b", "OPEN")])
            .is_empty());
        assert_eq!(
            book.changes("live", "alpaca", "PA1", &[order("a", "COMPLETE"), order("b", "FILLED")])
                .len(),
            1
        );
    }
}
