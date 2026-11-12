//! The time nodes and the market-hours gate in Eastern market time
//! (CLAUDE.md): a time window over the spring-forward night sees each
//! wall-clock minute once, and the schedule gate falls back to the regular
//! 09:30–16:00 session, logged, when the broker's market clock cannot be
//! read.

use super::*;
use crate::brokers::BrokerRegistry;
use crate::clock::ManualClock;
use crate::flow::contract::client::fake::FakeFlowClient;
use crate::flow::contract::node::NodeCtxParts;
use crate::flow::contract::subscriptions::Subscriptions;
use crate::flow::contract::time::{market_now, set_market_tz_for_tests, DEFAULT_MARKET_TZ};
use crate::flow::support::testkit::capture_logs;
use chrono::{DateTime, Duration};
use std::sync::Weak;
use tokio_util::sync::CancellationToken;

/// A context at the instant `now`, in Eastern market time.
fn at_instant(now: DateTime<Utc>) -> NodeCtx {
    set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    NodeCtx::new(NodeCtxParts {
        client: Arc::new(FakeFlowClient::new()),
        clock: ManualClock::new(now),
        cancel: CancellationToken::new(),
        workflow_id: Some(1),
        default_strategy: "Eastern".into(),
        subscriptions: Arc::new(Subscriptions::default()),
        session_boundary: (3, 0),
        env: NodeEnv::default(),
        app: Weak::new(),
    })
}

/// 2026-03-08 05:00Z is 00:00 EST; at 07:00Z the clock jumps from 01:59
/// EST to 03:00 EDT. Over three hours of UTC minutes, no wall-clock minute
/// is seen twice and a 01:30–02:30 window is open for the thirty minutes
/// that exist.
#[tokio::test(start_paused = true)]
async fn a_time_window_over_the_spring_forward_sees_each_wall_clock_minute_once() {
    let start: DateTime<Utc> = "2026-03-08T05:00:00Z".parse().unwrap();
    let mut seen: Vec<String> = Vec::new();
    let mut open = 0;
    for minute in 0..=180 {
        let ctx = at_instant(start + Duration::minutes(minute));
        let wall = market_now(ctx.clock.as_ref()).format("%H:%M").to_string();
        assert!(!seen.contains(&wall), "{} seen twice", wall);
        seen.push(wall);
        let out = run(
            &TimeWindow,
            &ctx,
            json!({"startTime": "01:30", "endTime": "02:30"}),
        )
        .await;
        if out.condition_met() == Some(true) {
            open += 1;
        }
    }
    assert_eq!(open, 30, "01:30 to 01:59 exist, 02:00 to 02:30 do not");
    assert!(!seen.iter().any(|w| w.starts_with("02:")));
    assert!(seen.contains(&"01:59".to_string()) && seen.contains(&"03:00".to_string()));
}

/// With no broker logged in the market clock cannot be read: the gate
/// falls back to the regular session, says so in the log, and the node's
/// own start and end still replace a side.
#[tokio::test]
async fn the_gate_falls_back_to_the_regular_session_when_the_market_clock_cannot_be_read() {
    set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    // Tuesday 2026-10-27, 12:00 EDT.
    let t = crate::state::testing::build(
        BrokerRegistry::new(),
        "2026-10-27T16:00:00Z".parse().unwrap(),
    );
    let hours = MarketHours {
        enabled: true,
        ..Default::default()
    };
    let logs = capture_logs();
    assert!(is_within_market_hours(&t.ctx, t.ctx.now(), &hours).await);
    assert!(
        logs.text().contains("09:30") && logs.text().contains("16:00"),
        "the fallback is logged: {}",
        logs.text()
    );
    let saturday: DateTime<Utc> = "2026-10-31T16:00:00Z".parse().unwrap();
    assert!(!is_within_market_hours(&t.ctx, saturday, &hours).await);
    let after_close: DateTime<Utc> = "2026-10-27T20:30:00Z".parse().unwrap();
    assert!(!is_within_market_hours(&t.ctx, after_close, &hours).await);
    let late = MarketHours {
        end: Some(json!("16:45")),
        ..hours.clone()
    };
    assert!(is_within_market_hours(&t.ctx, after_close, &late).await);
    let early = MarketHours {
        start: Some(json!("08:00")),
        ..hours
    };
    let before_open: DateTime<Utc> = "2026-10-27T12:30:00Z".parse().unwrap();
    assert!(is_within_market_hours(&t.ctx, before_open, &early).await);
}
