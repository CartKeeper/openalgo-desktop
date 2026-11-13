//! The orders each workflow's own runs placed (WP8 Phase D, coordinator
//! decision 6d): an order-update watch never fires on an order its own
//! workflow placed, so an "every time" watch on AAPL whose run buys AAPL
//! does not re-run itself on each of its own fills. The app's client
//! records the order ids from the replies to a run's order calls
//! (`contract::client::app`); the order-update dispatch skips them
//! (`drive::on_order_update`). In memory and bounded: the newest
//! [`PER_WORKFLOW`] ids of each workflow.

use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

/// Order ids kept per workflow, newest first out last.
pub const PER_WORKFLOW: usize = 200;

/// The calls that place orders: their replies carry the new orders' ids.
pub const PLACING: &[&str] = &[
    "placeorder",
    "placesmartorder",
    "basketorder",
    "splitorder",
    "optionsorder",
    "optionsmultiorder",
];

#[derive(Default)]
pub struct OwnOrders {
    placed: Mutex<HashMap<i64, VecDeque<String>>>,
}

impl OwnOrders {
    /// Remember that `workflow_id`'s run placed these orders.
    pub fn record(&self, workflow_id: i64, ids: impl IntoIterator<Item = String>) {
        let mut placed = self.placed.lock();
        let kept = placed.entry(workflow_id).or_default();
        for id in ids {
            if kept.contains(&id) {
                continue;
            }
            kept.push_back(id);
            if kept.len() > PER_WORKFLOW {
                kept.pop_front();
            }
        }
    }

    /// Whether one of `workflow_id`'s runs placed `order_id`.
    pub fn placed(&self, workflow_id: i64, order_id: &str) -> bool {
        self.placed
            .lock()
            .get(&workflow_id)
            .is_some_and(|ids| ids.iter().any(|id| id == order_id))
    }
}

/// The order ids in an order call's reply: every non-empty `orderid` at
/// any depth (one order's, or a basket's, split's or multi-leg's
/// `results`).
pub fn order_ids(reply: &Value) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    match (k.as_str(), x) {
                        ("orderid", Value::String(id)) if !id.trim().is_empty() => {
                            out.push(id.trim().to_string())
                        }
                        ("orderid", Value::Number(n)) => out.push(n.to_string()),
                        _ => walk(x, out),
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(reply, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_order_reply_gives_its_order_ids() {
        assert_eq!(
            order_ids(&json!({"status": "success", "orderid": "SB-1"})),
            vec!["SB-1"]
        );
        assert_eq!(
            order_ids(&json!({"status": "success", "results": [
                {"symbol": "AAPL", "orderid": "B-1", "status": "success"},
                {"symbol": "MSFT", "orderid": "B-2", "status": "success"},
                {"symbol": "TSLA", "status": "error", "message": "rejected"}
            ]})),
            vec!["B-1", "B-2"]
        );
        assert!(order_ids(&json!({"status": "error", "error": "Broker not connected"})).is_empty());
        assert!(order_ids(&json!({"orderid": ""})).is_empty());
    }

    #[test]
    fn each_workflow_keeps_its_own_newest_orders() {
        let own = OwnOrders::default();
        own.record(1, (0..PER_WORKFLOW + 5).map(|i| format!("O-{i}")));
        own.record(2, ["O-0".to_string()]);
        assert!(!own.placed(1, "O-4"), "the oldest went past the bound");
        assert!(own.placed(1, "O-5"));
        assert!(own.placed(1, &format!("O-{}", PER_WORKFLOW + 4)));
        assert!(own.placed(2, "O-0"));
        assert!(!own.placed(3, "O-5"), "another workflow placed nothing");
    }
}
