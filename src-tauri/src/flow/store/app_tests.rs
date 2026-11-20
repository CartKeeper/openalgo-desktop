//! The store in the app (WP8 Task A6): the app's own migration runner
//! creates Flow's tables under the reserved name `100_flow_core`, and
//! opening the app closes the runs the last session left running.

use super::{executions, workflows};
use crate::brokers::BrokerRegistry;
use crate::state::{AppState, OpenOptions};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn the_apps_migration_runner_creates_the_flow_tables() {
    let t = crate::state::testing::build(BrokerRegistry::new(), chrono::Utc::now());
    let conn = t.ctx.sqlite.conn().unwrap();
    let tables: Vec<String> = {
        let mut st = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table'
                   AND name IN ('flow_workflows', 'flow_workflow_executions') ORDER BY name",
            )
            .unwrap();
        st.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(tables, ["flow_workflow_executions", "flow_workflows"]);
    let recorded: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM migrations WHERE name = '100_flow_core')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(recorded, "the migration is recorded under its reserved name");
}

/// Review Focus 3: the app exits while a run is between two order legs; at
/// the next start the run is recorded as interrupted, never silently
/// resumed, and its workflow is free.
#[tokio::test]
async fn opening_the_app_closes_a_run_the_last_session_left_running() {
    let dir = tempfile::tempdir().unwrap();
    let open = || {
        AppState::open(
            dir.path(),
            OpenOptions {
                clock: Arc::new(crate::clock::SystemClock),
                brokers: Arc::new(BrokerRegistry::new()),
                app_handle: None,
            },
        )
        .unwrap()
    };
    let first = open();
    first.flow.triggers().stop().await;
    let id = {
        let conn = first.sqlite.conn().unwrap();
        let w = workflows::create(
            &conn,
            &first.security,
            &workflows::NewWorkflow {
                name: "Two legs",
                description: None,
                nodes: &json!([]),
                edges: &json!([]),
            },
            first.now(),
        )
        .unwrap();
        executions::create(&conn, w.id, first.now()).unwrap();
        w.id
    };
    // The app quits between the legs: nothing finishes the run.
    first.shutdown().await;
    for _ in 0..1000 {
        if Arc::strong_count(&first) == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    drop(first);

    let again = open();
    let runs = executions::list(&again.sqlite.conn().unwrap(), id, 10).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
    assert_eq!(runs[0].error.as_deref(), Some(executions::INTERRUPTED));
    assert!(runs[0].completed_at.is_some());
    assert!(again.flow.try_claim(id).is_some(), "the workflow is free");
    again.shutdown().await;
}
