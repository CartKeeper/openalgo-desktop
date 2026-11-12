//! The market-hours gate's edges (the audit's full sweep, decision 35):
//! the first start node, a failed read at fire time, minute precision, a
//! reversed window, a failed calendar read and an exchange's default hours.

use super::*;
use crate::brokers::BrokerRegistry;
use crate::flow::store::workflows::{self, NewWorkflow};

/// N-marketHours-02: the settings come from the FIRST start node.
#[test]
fn the_first_start_node_is_read() {
    let off = json!({"type": "start", "data": {"marketHoursOnly": false}});
    let on =
        json!({"type": "start", "data": {"marketHoursOnly": true, "marketHoursExchange": "MCX"}});
    assert!(!market_hours_config(&json!([off.clone(), on.clone()])).enabled);
    let first_on = market_hours_config(&json!([{"type": "log"}, on, off]));
    assert!(first_on.enabled);
    assert_eq!(first_on.exchange, Some(json!("MCX")));
}

/// A Saturday noon in IST (06:30 UTC), far from 03:00 IST.
const SATURDAY_NOON: &str = "2026-08-08T06:30:00Z";

fn scheduled_workflow(t: &crate::state::testing::TestCtx) -> i64 {
    let nodes = json!([{"id": "t", "type": "start", "position": {"x": 0, "y": 0},
        "data": {"scheduleType": "daily", "time": "09:15", "marketHoursOnly": false}}]);
    let conn = t.ctx.sqlite.conn().unwrap();
    let new = NewWorkflow {
        name: "Gate",
        description: None,
        nodes: &nodes,
        edges: &json!([]),
    };
    let w = workflows::create(&conn, &t.ctx.security, &new, t.ctx.now()).unwrap();
    workflows::set_active(&conn, w.id, true, t.ctx.now()).unwrap();
    w.id
}

/// What one scheduled fire logged.
async fn fire_logs(side: &crate::flow::triggers::TriggerSide, id: i64, stored: bool) -> String {
    let captured = crate::flow::support::testkit::capture_logs();
    side.scheduled_run(id, stored).await;
    captured.text()
}

/// N-marketHours-02: when the workflow cannot be read at fire time, the
/// gate keeps the schedule's stored flag: stored on, a Saturday fire is
/// skipped as outside market hours; stored off, it goes on to run.
#[tokio::test]
async fn a_failed_read_keeps_the_stored_flag() {
    let t = crate::state::testing::build(BrokerRegistry::new(), SATURDAY_NOON.parse().unwrap());
    t.ctx.flow.triggers().stop().await;
    crate::flow::support::testkit::sign_in(&t.ctx);
    let id = scheduled_workflow(&t);
    let side = crate::flow::triggers::TriggerSide::new(
        std::sync::Arc::downgrade(&t.ctx),
        t.ctx.clock.clone(),
    );
    t.ctx
        .sqlite
        .conn()
        .unwrap()
        .execute_batch("ALTER TABLE flow_workflows RENAME TO flow_workflows_gone")
        .unwrap();
    let unread = format!("Could not read scheduling config for workflow {};", id);
    let gated = format!("Skipping scheduled workflow {}: outside market hours", id);
    let ran = format!("Scheduled execution result for workflow {}", id);
    let on = fire_logs(&side, id, true).await;
    assert!(on.contains(&unread) && on.contains(&gated), "{}", on);
    assert!(!on.contains(&ran), "{}", on);
    let off = fire_logs(&side, id, false).await;
    assert!(off.contains(&unread) && off.contains(&ran), "{}", off);
    assert!(!off.contains(&gated), "{}", off);
}

/// N-marketHours-03: the comparison is by minute, so 15:30:59 is inside
/// a 15:30 close; a window whose start is after its end is never open.
#[test]
fn minutes_decide_and_a_reversed_window_is_never_open() {
    assert!(within_window(nse(), ist("2026-08-04T15:30:59"), None, None));
    assert!(!within_window(
        nse(),
        ist("2026-08-04T15:31:00"),
        None,
        None
    ));
    assert!(within_window(nse(), ist("2026-08-04T09:15:00"), None, None));
    let (late, early) = (json!("14:00"), json!("10:00"));
    for hhmm in ["09:00", "12:00", "15:00"] {
        assert!(
            !within_window(nse(), now(hhmm, 4), Some(&late), Some(&early)),
            "{}",
            hhmm
        );
    }
    for hhmm in ["08:00", "12:00", "16:00"] {
        assert!(
            !inside(session("15:00", "09:00"), hhmm, None, None),
            "{}",
            hhmm
        );
    }
}

