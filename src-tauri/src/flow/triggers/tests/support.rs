//! Shared by the trigger tests: every message the code under test logs,
//! and an app with an account and its API key.

use super::*;

/// Every message logged on this thread while the guard lives (a
/// current-thread test runtime keeps its tasks on it).
#[derive(Clone, Default)]
pub(super) struct Logged(pub(super) Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Logged {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct Message(String);
        impl tracing::field::Visit for Message {
            fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                if f.name() == "message" {
                    self.0 = format!("{:?}", v);
                }
            }
        }
        let mut m = Message(String::new());
        event.record(&mut m);
        self.0.lock().push(m.0);
    }
}

pub(super) fn logged() -> (tracing::subscriber::DefaultGuard, Logged) {
    use tracing_subscriber::layer::SubscriberExt;
    every_line_stays_on();
    let l = Logged::default();
    let guard = tracing::subscriber::set_default(tracing_subscriber::registry().with(l.clone()));
    (guard, l)
}

/// tracing-core caches, for the whole process, whether any subscriber wants
/// a log line (per call site) and the highest level any subscriber wants;
/// while at most one subscriber is registered it asks only the current
/// thread's. A line first logged on a thread with none (another test's
/// task) was cached as off for every thread until the next registration,
/// so a capture here never saw it: why
/// `lifecycle::a_monitor_run_logs_its_result` failed under full-suite load
/// with the run done and its two drive.rs lines missing (WP8 Phase C). A
/// process-wide subscriber that wants every line and prints none keeps
/// every call site on, whatever the other threads hold.
fn every_line_stays_on() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Another global (none in this crate's tests) keeps every line on too.
        let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry());
    });
}

/// Wait until every task `s` spawned has finished: the runs a test queued
/// on a side without drivers (a driver never finishes; use it only on a
/// side from `side()`). Waits on the tasks themselves, not on wall time.
pub(super) async fn settle(s: &TriggerSide) {
    loop {
        let mut tasks = std::mem::take(&mut *s.tasks.lock());
        if tasks.is_empty() {
            return;
        }
        while tasks.join_next().await.is_some() {}
    }
}

impl Logged {
    pub(super) fn has(&self, line: &str) -> bool {
        self.0.lock().iter().any(|m| m == line)
    }
}

impl Logged {
    pub(super) fn starting_with(&self, prefix: &str) -> Vec<String> {
        self.0
            .lock()
            .iter()
            .filter(|m| m.starts_with(prefix))
            .cloned()
            .collect()
    }
}

/// Wait (bounded) until `line` was logged.
pub(super) async fn wait_for(logs: &Logged, line: &str) {
    for _ in 0..500 {
        if logs.has(line) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("never logged {:?}: {:?}", line, logs.0.lock());
}

/// An app with an account and its API key (made before any workflow).
pub(super) fn keyed() -> crate::state::testing::TestCtx {
    let t = app();
    crate::flow::support::testkit::sign_in(&t.ctx);
    t
}
