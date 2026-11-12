//! Importing a workflow written on OpenAlgo web: its wall-clock times are
//! read as Eastern market time here (the web read IST), and the import's
//! reply says so.

use super::*;
use crate::flow::contract::time::{set_market_tz_for_tests, DEFAULT_MARKET_TZ};
use crate::flow::triggers::scheduler::Schedule;
use chrono::{DateTime, Utc};

fn start(data: Value) -> Value {
    json!({"id": "s", "type": "start", "position": {"x": 0, "y": 0}, "data": data})
}

fn log_node() -> Value {
    json!({"id": "l", "type": "log", "position": {"x": 200, "y": 0}, "data": {"message": "hello"}})
}

fn edge() -> Value {
    json!([{"id": "e1", "source": "s", "target": "l"}])
}

/// An app with Flow's tables (the migration itself is wired in Task A6).
fn migrated() -> crate::state::testing::TestCtx {
    let t = app();
    set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    crate::flow::store::migrate(&t.ctx.sqlite.conn().unwrap()).unwrap();
    t
}

#[tokio::test]
async fn an_imported_daily_0915_is_0915_eastern_and_the_reply_carries_the_note() {
    let t = migrated();
    let workflow = json!({
        "name": "Opening bell",
        "nodes": [start(json!({"scheduleType": "daily", "time": "09:15"})), log_node()],
        "edges": edge(),
    });
    let r = transfer::import(&t.ctx, Body::Json(workflow)).await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert_eq!(r.body["notes"], json!([transfer::EASTERN_TIME_NOTE]));
    let id = r.body["workflow_id"].as_i64().unwrap();
    let stored = load(&t.ctx, id).await.unwrap().unwrap();
    let data = stored.nodes[0]["data"].as_object().unwrap().clone();
    // Friday 2026-10-30, EDT: 09:15 Eastern is 13:15Z.
    let now: DateTime<Utc> = "2026-10-30T12:00:00Z".parse().unwrap();
    let schedule = Schedule::from_node(&data, now).unwrap().unwrap();
    assert_eq!(
        schedule.next_at_or_after(now),
        Some("2026-10-30T13:15:00Z".parse().unwrap())
    );
}

#[tokio::test]
async fn a_workflow_without_wall_clock_times_gets_no_note() {
    let t = migrated();
    let workflow = json!({
        "name": "Manual",
        "nodes": [start(json!({"scheduleType": "manual"})), log_node()],
        "edges": edge(),
    });
    let r = transfer::import(&t.ctx, Body::Json(workflow)).await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert!(r.body.get("notes").is_none(), "{}", r.body);
}
