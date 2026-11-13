//! WP8 Phase C, Task C1: the scheduler on this app, in Eastern time.
//!
//! The scheduler, its driver and the start-up restore are upstream's,
//! carried in Phase A and installed at app start (`flow::start`). These
//! tests are this app's evidence for the plan's C1 list, on the headless
//! harness with a manual clock and the market zone this app runs in
//! (`America/New_York`), where the carried tests replay the web in IST.
//! Each fire is stepped as the driver steps it (`Scheduler::tick`, then
//! `run_scheduled`), so nothing here waits on wall time; run times are
//! read back as stored (UTC).

use super::*;
use crate::flow::store::executions;

/// The harness in Eastern time, signed in, with the app's own trigger side
/// stopped: its start-up restore runs on a task of its own and would
/// register the workflows saved below a second time.
async fn eastern(now: &str) -> crate::state::testing::TestCtx {
    crate::flow::contract::time::set_market_tz_for_tests(chrono_tz::America::New_York);
    let t = crate::state::testing::build(
        crate::brokers::BrokerRegistry::new(),
        now.parse().unwrap(),
    );
    t.ctx.flow.triggers().stop().await;
    crate::flow::support::testkit::sign_in(&t.ctx);
    t
}

fn utc(s: &str) -> chrono::DateTime<chrono::Utc> {
    s.parse().unwrap()
}

/// The workflow's runs, oldest first: (status, started_at as stored).
fn runs(app: &Arc<AppState>, id: i64) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = executions::list(&app.sqlite.conn().unwrap(), id, 20)
        .unwrap()
        .into_iter()
        .map(|r| (r.status, r.started_at.unwrap_or_default()))
        .collect();
    v.reverse();
    v
}

fn next_run(app: &Arc<AppState>, id: i64) -> Option<String> {
    store::next_run(&app.sqlite.conn().unwrap(), id).unwrap()
}

/// Fire what is due at `at`, as the driver does: tick, then each due run.
async fn step(t: &crate::state::testing::TestCtx, s: &Arc<TriggerSide>, at: &str) -> usize {
    t.clock.set(utc(at));
    let due = s.scheduler.tick(t.ctx.now());
    let fired = due.len();
    for d in due {
        s.run_scheduled(d).await;
    }
    fired
}

/// An active workflow whose Start node is `start`, registered as
/// activation registers it.
async fn scheduled(t: &crate::state::testing::TestCtx, s: &Arc<TriggerSide>, start: Value) -> i64 {
    let id = save(&t.ctx, node("start", start.clone()), true);
    s.register(id, "start", &data(start)).await.unwrap();
    id
}

/// "Every 1 minute", activated at 10:00:30 AM ET, runs at 10:01:02 and
/// 10:02:02 AM ET (upstream's alignment: the minute boundary plus 2 s) and
/// not in between: one run each minute, its time stored in UTC.
#[tokio::test]
async fn an_every_minute_schedule_runs_once_each_minute_on_the_eastern_clock() {
    // Tuesday 2026-11-03, 10:00:30 AM EST.
    let t = eastern("2026-11-03T15:00:30Z").await;
    let s = side(&t);
    let id = scheduled(
        &t,
        &s,
        json!({"scheduleType": "interval", "intervalValue": 1, "intervalUnit": "minutes", "marketHoursOnly": false}),
    )
    .await;
    assert_eq!(next_run(&t.ctx, id).as_deref(), Some("2026-11-03T15:01:02"));
    assert_eq!(step(&t, &s, "2026-11-03T15:01:01Z").await, 0, "not before 10:01:02");
    assert_eq!(step(&t, &s, "2026-11-03T15:01:02Z").await, 1);
    assert_eq!(step(&t, &s, "2026-11-03T15:01:40Z").await, 0, "once a minute");
    assert_eq!(step(&t, &s, "2026-11-03T15:02:02Z").await, 1);
    assert_eq!(
        runs(&t.ctx, id),
        vec![
            ("completed".to_string(), "2026-11-03T15:01:02".to_string()),
            ("completed".to_string(), "2026-11-03T15:02:02".to_string()),
        ]
    );
}

/// The driver wakes a few milliseconds after a fire's instant and, while
/// nothing is due, ticks again at most every 50 ms. A tick just after the
/// fire must move the job a whole interval on: an every-minute schedule
/// fires once at 10:03:02 AM ET, never again inside that second (a short
/// run would otherwise run again, and an order node send again).
#[tokio::test]
async fn a_fire_moves_an_interval_job_a_whole_interval_on() {
    // Tuesday 2026-11-03, 10:02:30 AM EST.
    let t = eastern("2026-11-03T15:02:30Z").await;
    let s = side(&t);
    let id = scheduled(
        &t,
        &s,
        json!({"scheduleType": "interval", "intervalValue": 1, "intervalUnit": "minutes", "marketHoursOnly": false}),
    )
    .await;
    t.clock.set(utc("2026-11-03T15:03:02.004Z"));
    let due = s.scheduler.tick(t.ctx.now());
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].next, Some(utc("2026-11-03T15:04:02Z")));
    for d in due {
        s.run_scheduled(d).await;
    }
    for at in ["2026-11-03T15:03:02.054Z", "2026-11-03T15:03:02.104Z", "2026-11-03T15:03:02.954Z"] {
        assert_eq!(step(&t, &s, at).await, 0, "fired again at {}", at);
    }
    assert_eq!(runs(&t.ctx, id).len(), 1);
}

/// A daily 9:35 AM ET schedule fires once a day at 9:35 AM ET on both
/// sides of the end of daylight saving (Sunday 2026-11-01): 13:35Z while
/// the zone is EDT, 14:35Z once it is EST (Review Focus 2).
#[tokio::test]
async fn a_daily_0935_schedule_fires_once_a_day_across_the_daylight_saving_change() {
    // Friday 2026-10-30, 9:00 AM EDT.
    let t = eastern("2026-10-30T13:00:00Z").await;
    let s = side(&t);
    let id = scheduled(
        &t,
        &s,
        json!({"scheduleType": "daily", "time": "09:35", "marketHoursOnly": false}),
    )
    .await;
    assert_eq!(next_run(&t.ctx, id).as_deref(), Some("2026-10-30T13:35:00"));
    assert_eq!(step(&t, &s, "2026-10-30T13:35:00Z").await, 1);
    assert_eq!(step(&t, &s, "2026-10-30T13:36:00Z").await, 0, "once a day");
    assert_eq!(step(&t, &s, "2026-10-30T23:59:00Z").await, 0, "once a day");
    // Saturday 9:35 AM EDT fires; the next is Sunday 9:35 AM EST.
    t.clock.set(utc("2026-10-31T13:35:00Z"));
    let due = s.scheduler.tick(t.ctx.now());
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].next, Some(utc("2026-11-01T14:35:00Z")));
    s.run_scheduled(due[0].clone()).await;
    assert_eq!(step(&t, &s, "2026-11-01T13:35:00Z").await, 0, "9:35 AM EDT is gone");
    assert_eq!(step(&t, &s, "2026-11-01T14:35:00Z").await, 1);
    let started: Vec<String> = runs(&t.ctx, id).into_iter().map(|(_, at)| at).collect();
    assert_eq!(
        started,
        vec![
            "2026-10-30T13:35:00".to_string(),
            "2026-10-31T13:35:00".to_string(),
            "2026-11-01T14:35:00".to_string(),
        ]
    );
}

/// A one-time schedule for 10:05 AM ET fires once at 10:05 AM ET, then
/// stands its workflow down so a restart can never run it again.
#[tokio::test]
async fn a_one_time_schedule_fires_at_its_eastern_time_and_deactivates() {
    // Tuesday 2026-11-03, 10:00:30 AM EST.
    let t = eastern("2026-11-03T15:00:30Z").await;
    let s = side(&t);
    let id = scheduled(
        &t,
        &s,
        json!({"scheduleType": "once", "executeAt": "2026-11-03", "time": "10:05", "marketHoursOnly": false}),
    )
    .await;
    assert_eq!(next_run(&t.ctx, id).as_deref(), Some("2026-11-03T15:05:00"));
    assert_eq!(step(&t, &s, "2026-11-03T15:04:59Z").await, 0);
    assert_eq!(step(&t, &s, "2026-11-03T15:05:00Z").await, 1);
    assert_eq!(
        runs(&t.ctx, id),
        vec![("completed".to_string(), "2026-11-03T15:05:00".to_string())]
    );
    assert!(!active(&t.ctx, id), "the one-time schedule stood its workflow down");
    assert!(!s.scheduler.has(id));
    assert_eq!(step(&t, &s, "2026-11-04T15:05:00Z").await, 0);
}

/// While OpenAlgo is signed out a scheduled fire does not run: the skip is
/// one run in the history with the reason, the workflow stays active, and
/// the next fire after sign-in runs.
#[tokio::test]
async fn a_fire_while_signed_out_is_recorded_with_the_reason_and_the_next_runs() {
    // Tuesday 2026-11-03, 10:00:30 AM EST.
    let t = eastern("2026-11-03T15:00:30Z").await;
    let s = side(&t);
    let id = scheduled(
        &t,
        &s,
        json!({"scheduleType": "interval", "intervalValue": 1, "intervalUnit": "minutes", "marketHoursOnly": false}),
    )
    .await;
    crate::flow::support::testkit::sign_out(&t.ctx).await;
    assert_eq!(step(&t, &s, "2026-11-03T15:01:02Z").await, 1);
    let skipped = executions::list(&t.ctx.sqlite.conn().unwrap(), id, 5).unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].status, "failed");
    assert_eq!(
        skipped[0].error.as_deref(),
        Some("Skipped: OpenAlgo was signed out when this schedule fired, so the workflow did not run. Its triggers run again after you sign in.")
    );
    assert!(active(&t.ctx, id), "a skip never deactivates");
    crate::flow::support::testkit::sign_in(&t.ctx);
    assert_eq!(step(&t, &s, "2026-11-03T15:02:02Z").await, 1);
    assert_eq!(
        runs(&t.ctx, id),
        vec![
            ("failed".to_string(), "2026-11-03T15:01:02".to_string()),
            ("completed".to_string(), "2026-11-03T15:02:02".to_string()),
        ]
    );
}

/// Quitting and reopening the app keeps an active schedule: the reopened
/// app's own start-up restore registers it again and records its next fire
/// at 9:35 AM ET (`AppState::open` -> `flow::start` -> the restore).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reopening_the_app_registers_an_active_schedule_again() {
    crate::flow::contract::time::set_market_tz_for_tests(chrono_tz::America::New_York);
    let dir = tempfile::tempdir().unwrap();
    // Tuesday 2026-11-03, 9:00 AM EST.
    let clock = crate::clock::ManualClock::new(utc("2026-11-03T14:00:00Z"));
    let open = || {
        AppState::open(
            dir.path(),
            crate::state::OpenOptions {
                app_handle: None,
                clock: clock.clone(),
                brokers: Arc::new(crate::brokers::BrokerRegistry::new()),
            },
        )
        .unwrap()
    };
    let first = open();
    first.flow.triggers().stop().await;
    let id = save(
        &first,
        node(
            "start",
            json!({"scheduleType": "daily", "time": "09:35", "marketHoursOnly": false}),
        ),
        true,
    );
    first.shutdown().await;
    drop(first);
    let again = open();
    let mut recorded = None;
    for _ in 0..12_000 {
        recorded = next_run(&again, id);
        if recorded.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(recorded.as_deref(), Some("2026-11-03T14:35:00"));
    assert!(active(&again, id));
    again.shutdown().await;
}
