//! Order updates for Flow's order-update triggers.
//!
//! Upstream OpenAlgo Desktop has an event bus every broker feed publishes
//! to; this app has no bus and no order stream. Flow's order-update
//! monitor subscribes to a broadcast channel instead
//! ([`OrderUpdates::subscribe`]), and an order-book poller (WP8 Phase C,
//! `flow::triggers::order_poll`) sends one [`OrderUpdate`] per order whose
//! status changed between two polls. The struct keeps upstream's fields so
//! the carried monitor matches watches exactly as upstream does.

use tokio::sync::broadcast;

/// One order's state change, as upstream's `events::OrderUpdate`.
#[derive(Debug, Clone, Default)]
pub struct OrderUpdate {
    pub mode: String,
    pub broker: String,
    pub orderid: String,
    pub symbol: String,
    pub exchange: String,
    pub action: String,
    pub quantity: i64,
    pub price: f64,
    pub trigger_price: f64,
    pub pricetype: String,
    pub product: String,
    pub order_status: String,
    pub filled_quantity: i64,
    pub pending_quantity: i64,
    pub average_price: f64,
    pub rejection_reason: String,
    /// The broker session this update came from; 0 for updates not tied to
    /// a session (the sandbox).
    pub session_generation: u64,
}

/// The channel order updates travel on. Subscribers that fall behind miss
/// the oldest updates (a lagged receiver), never block the sender.
pub type OrderUpdates = broadcast::Sender<OrderUpdate>;

/// Updates kept for a slow subscriber before the oldest is dropped.
pub const CHANNEL_CAPACITY: usize = 256;

/// A fresh channel (the receiver side is made by `subscribe()` on demand).
pub fn channel() -> OrderUpdates {
    broadcast::channel(CHANNEL_CAPACITY).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_subscriber_receives_every_update_sent_after_it_subscribed() {
        let tx = channel();
        let mut rx = tx.subscribe();
        tx.send(OrderUpdate {
            orderid: "1".into(),
            order_status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
        let got = rx.recv().await.unwrap();
        assert_eq!(got.orderid, "1");
        assert_eq!(got.order_status, "complete");
    }

    #[test]
    fn sending_with_no_subscriber_is_not_an_error_the_sender_must_handle() {
        let tx = channel();
        // `broadcast::Sender::send` errors when nobody listens; the poller
        // ignores that, as nothing was lost that anyone wanted.
        assert!(tx.send(OrderUpdate::default()).is_err());
    }
}
