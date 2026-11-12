//! Injected clock. Anything that depends on wall time reads it from here
//! so tests are deterministic (carried from upstream OpenAlgo Desktop; its
//! IST constants and `ist_slots_between`, which nothing here calls, were
//! not carried: market time is `flow::contract::time::market_tz`).

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::sync::Arc;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Settable clock for tests.
#[derive(Debug)]
pub struct ManualClock {
    now: RwLock<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(now: DateTime<Utc>) -> Arc<Self> {
        Arc::new(Self {
            now: RwLock::new(now),
        })
    }

    pub fn set(&self, now: DateTime<Utc>) {
        *self.now.write() = now;
    }

    pub fn advance(&self, by: chrono::Duration) {
        let mut n = self.now.write();
        *n += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manual_clock_is_set_and_advanced() {
        let clock = ManualClock::new("2026-10-30T13:30:00Z".parse().unwrap());
        clock.advance(chrono::Duration::minutes(5));
        assert_eq!(clock.now(), "2026-10-30T13:35:00Z".parse::<DateTime<Utc>>().unwrap());
        clock.set("2026-11-02T14:35:00Z".parse().unwrap());
        assert_eq!(clock.now().to_rfc3339(), "2026-11-02T14:35:00+00:00");
    }
}
