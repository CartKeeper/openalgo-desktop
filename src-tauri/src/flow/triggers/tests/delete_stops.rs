//! WP8 Phase D (coordinator decision 6b): deleting a workflow from the
//! Flow page stands it down first, so no schedule, price alert, order
//! watch or webhook outlives it, even when the row itself cannot be
//! deleted.

use super::support::*;
use super::*;
use crate::flow::service::webhooks::{self, WebhookCall};

/// The app with `s` as its trigger side, so the service's delete acts on
/// the registrations the test can see.
async fn installed(now: &str) -> (crate::state::testing::TestCtx, Arc<TriggerSide>) {
    let t = eastern(now).await;
    let s = side(&t);
    s.watches.attach();
    t.ctx.flow.set_triggers(s.clone());
    (t, s)
}

/// An active workflow with `kind`'s trigger registered, as activation
/// leaves it.
async fn active_with(
    t: &crate::state::testing::TestCtx,
    s: &TriggerSide,
    kind: &str,
    d: Value,
) -> i64 {
    let id = save(&t.ctx, node(kind, d.clone()), true);
    s.register(id, kind, &data(d)).await.unwrap();
    id
}

#[tokio::test]
async fn a_deleted_workflow_leaves_no_schedule_alert_watch_or_webhook() {
    let (t, s) = installed("2026-11-12T15:00:00Z").await;
    let scheduled = active_with(
        &t,
        &s,
        "start",
        json!({"scheduleType": "interval", "intervalValue": 1, "intervalUnit": "minutes"}),
    )
    .await;
    let alerted = active_with(
        &t,
        &s,
        "priceAlert",
        json!({"symbol": "AAPL", "exchange": "US", "condition": "greater_than", "price": 100}),
    )
    .await;
    let watched = active_with(
        &t,
        &s,
        "orderUpdateTrigger",
        json!({"symbol": "AAPL", "status": "complete"}),
    )
    .await;
    assert!(s.scheduler.has(scheduled));
    assert!(s.alerts.get(alerted).is_some());
    assert!(s.watches.get(watched).is_some());
    // The scheduled one also takes alerts on its webhook.
    assert_eq!(webhooks::enable(&t.ctx, scheduled).await.status, 200);
    let token = crate::flow::service::load(&t.ctx, scheduled)
        .await
        .unwrap()
        .and_then(|w| w.webhook_token)
        .expect("a token");

    for id in [scheduled, alerted, watched] {
        let r = crate::flow::service::delete(&t.ctx, id).await;
        assert_eq!(r.status, 200, "{}", r.body);
    }
    assert!(
        !s.scheduler.has(scheduled),
        "the schedule outlived its workflow"
    );
    assert!(
        s.alerts.get(alerted).is_none(),
        "the price alert outlived its workflow"
    );
    assert!(
        s.watches.get(watched).is_none(),
        "the order watch outlived its workflow"
    );
    let call = webhooks::trigger(
        &t.ctx,
        WebhookCall {
            token,
            payload: data(json!({"secret": "anything"})),
            url_secret: None,
        },
    )
    .await;
    assert_eq!(call.reply.status, 404, "{}", call.reply.body);
    assert_eq!(call.reply.body["error"], "Invalid webhook token");
}

/// The row cannot be deleted (the store refuses): the workflow stays, but
/// stood down, inactive with nothing registered, so what the page shows
/// is what runs, and a restart restores nothing for it.
#[tokio::test]
async fn a_delete_the_store_refuses_still_leaves_the_workflow_stood_down() {
    let (t, s) = installed("2026-11-12T15:00:00Z").await;
    let id = active_with(
        &t,
        &s,
        "priceAlert",
        json!({"symbol": "AAPL", "exchange": "US", "condition": "greater_than", "price": 100}),
    )
    .await;
    t.ctx
        .sqlite
        .conn()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER kept BEFORE DELETE ON flow_workflows
             BEGIN SELECT RAISE(ABORT, 'kept'); END;",
        )
        .unwrap();
    let r = crate::flow::service::delete(&t.ctx, id).await;
    assert_eq!(r.status, 500, "{}", r.body);
    assert!(s.alerts.get(id).is_none(), "nothing stays registered");
    assert!(!active(&t.ctx, id), "the workflow reads inactive");
}
