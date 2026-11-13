//! WP8 Phase D (decision 6a): deleting runs from a workflow's history,
//! from the run-history screen. Only that workflow's rows go, and a run
//! still going is never deleted (its end would have nowhere to be written).

use super::super::testing::db;
use super::super::workflows::{create as create_workflow, NewWorkflow};
use super::*;
use serde_json::json;

fn now() -> DateTime<Utc> {
    "2026-11-12T15:00:00Z".parse().unwrap()
}

fn workflow(conn: &Connection, name: &str) -> i64 {
    let (_, sec) = db();
    create_workflow(
        conn,
        &sec,
        &NewWorkflow {
            name,
            description: None,
            nodes: &json!([]),
            edges: &json!([]),
        },
        now(),
    )
    .unwrap()
    .id
}

/// A finished run of `w`.
fn finished(conn: &Connection, w: i64) -> i64 {
    let id = create(conn, w, now()).unwrap();
    finish(conn, id, RunStatus::Completed, None, &[], now()).unwrap();
    id
}

fn ids(conn: &Connection, w: i64) -> Vec<i64> {
    let mut v: Vec<i64> = list(conn, w, 200).unwrap().iter().map(|e| e.id).collect();
    v.sort_unstable();
    v
}

#[test]
fn the_chosen_runs_of_that_workflow_are_deleted_and_no_other() {
    let (conn, _) = db();
    let (a, b) = (workflow(&conn, "A"), workflow(&conn, "B"));
    let a1 = finished(&conn, a);
    let a2 = finished(&conn, a);
    let a3 = finished(&conn, a);
    let b1 = finished(&conn, b);
    // b1 is another workflow's run: named here, it is left alone.
    assert_eq!(
        delete(&conn, a, &[a1, a3, b1]).unwrap(),
        Deleted::Removed(2)
    );
    assert_eq!(ids(&conn, a), vec![a2]);
    assert_eq!(ids(&conn, b), vec![b1]);
}

#[test]
fn a_run_still_going_refuses_the_whole_delete() {
    let (conn, _) = db();
    let a = workflow(&conn, "A");
    let done = finished(&conn, a);
    let going = create(&conn, a, now()).unwrap();
    assert_eq!(
        delete(&conn, a, &[done, going]).unwrap(),
        Deleted::StillRunning(vec![going])
    );
    assert_eq!(ids(&conn, a), vec![done, going], "nothing was deleted");
}

#[test]
fn nothing_named_deletes_nothing() {
    let (conn, _) = db();
    let a = workflow(&conn, "A");
    let done = finished(&conn, a);
    assert_eq!(delete(&conn, a, &[]).unwrap(), Deleted::Removed(0));
    assert_eq!(ids(&conn, a), vec![done]);
}
