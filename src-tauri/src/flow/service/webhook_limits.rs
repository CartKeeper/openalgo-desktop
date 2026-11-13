//! The public webhook's request limits (WP8 Phase D, coordinator decision
//! 6e), carried from upstream OpenAlgo Desktop: the rows `FlowWebhook` and
//! `WebhookFail` of `server/ratelimit.rs`, and `admit` and `rate_limited`
//! of `server/routes/flow/triggers.rs`.
//!
//! - 100 requests a minute per caller address, before anything is looked
//!   up or read, except this computer's own address: every sender behind a
//!   local tunnel arrives from 127.0.0.1, and upstream exempted its tunnel's
//!   traffic;
//! - 100 a minute per token, once the token names a workflow (an invented
//!   token never gets a window of its own: it is a failed authentication);
//! - 10 failed authentications a minute per caller: an unknown token or a
//!   wrong secret. Once a caller has spent them, its unknown tokens and
//!   wrong (or missing) secrets are answered 429 at once, a wrong secret
//!   still counted by the lockout (`webhook_guard`), while an alert with a
//!   workflow's right secret goes on: a stranger's misses never shut out a
//!   sender that knows the secret (every sender behind a local tunnel shares
//!   one address). Upstream refused only unknown tokens once the budget was
//!   spent; a wrong secret here is refused too, and the lockout still
//!   counts it.
//!
//! Over a limit the caller gets upstream's JSON 429 with `Retry-After`. The
//! windows are in memory and move (a hit older than a minute no longer
//! counts); they are trimmed on every check and swept when the table
//! grows, and a full table counts new callers in one shared window (it
//! fails closed). A token's window is keyed by a keyed hash, never the
//! token, and nothing here logs a token. The routes stay outside the app's
//! `/api/v1` limiter (`webhook::server`).

use super::webhooks::{self, SecretGiven, WebhookCall};
use crate::state::AppState;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasher, RandomState};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Requests a minute per caller address, and per token (upstream's
/// `WEBHOOK_RATE_LIMIT`).
pub const REQUESTS_PER_MINUTE: usize = 100;
/// Failed authentications a minute per caller (upstream's `WebhookFail`).
pub const FAILURES_PER_MINUTE: usize = 10;
/// The moving window.
pub const WINDOW: Duration = Duration::from_secs(60);
/// The `Retry-After` an over-limit caller is told, in seconds.
pub const RETRY_AFTER_SECS: u64 = 60;
/// Windows kept before the table is swept.
pub const MAX_WINDOWS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Caller(IpAddr),
    Token(u64),
    Failures(IpAddr),
    /// Where new keys of one kind count while the table is full of live
    /// windows (0 callers, 1 tokens, 2 failures).
    Overflow(u8),
}

impl Key {
    fn overflow(self) -> Key {
        match self {
            Key::Caller(_) => Key::Overflow(0),
            Key::Token(_) => Key::Overflow(1),
            Key::Failures(_) | Key::Overflow(_) => Key::Overflow(2),
        }
    }
}

/// Upstream's 429 body (`rate_limited`).
pub fn rate_limited() -> Value {
    json!({
        "status": "error",
        "message": "Rate limit exceeded. Please slow down your requests.",
        "limit": format!("{} per 1 minute", REQUESTS_PER_MINUTE),
        "retry_after": RETRY_AFTER_SECS,
    })
}

/// The webhook's windows, one table for the app (`FlowModule`).
pub struct WebhookLimits {
    windows: Mutex<HashMap<Key, VecDeque<Instant>>>,
    hasher: RandomState,
}

impl Default for WebhookLimits {
    fn default() -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            hasher: RandomState::new(),
        }
    }
}

/// Whether `ip` is this computer's own (loopback, or loopback mapped into
/// IPv6).
fn this_computer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_loopback(),
        IpAddr::V6(v) => v.is_loopback() || v.to_ipv4_mapped().is_some_and(|m| m.is_loopback()),
    }
}

/// Drop the hits older than the window.
fn trim(q: &mut VecDeque<Instant>, now: Instant) {
    while q
        .front()
        .is_some_and(|t| now.saturating_duration_since(*t) >= WINDOW)
    {
        q.pop_front();
    }
}

impl WebhookLimits {
    /// The key a window lives under: its own, or its kind's overflow while
    /// the table is full of live windows.
    fn slot(map: &mut HashMap<Key, VecDeque<Instant>>, key: Key, now: Instant) -> Key {
        if map.contains_key(&key) || map.len() < MAX_WINDOWS {
            return key;
        }
        map.retain(|_, q| {
            trim(q, now);
            !q.is_empty()
        });
        if map.len() < MAX_WINDOWS {
            key
        } else {
            key.overflow()
        }
    }

    /// Count a hit unless `key`'s window is full; `false` when full (the
    /// hit is not counted, so a caller that backs off recovers).
    fn hit(&self, key: Key, limit: usize, now: Instant) -> bool {
        let mut map = self.windows.lock();
        let key = Self::slot(&mut map, key, now);
        let q = map.entry(key).or_default();
        trim(q, now);
        if q.len() >= limit {
            return false;
        }
        q.push_back(now);
        true
    }

    /// Whether `key`'s window is full, counting nothing.
    fn full(&self, key: Key, limit: usize, now: Instant) -> bool {
        let mut map = self.windows.lock();
        let key = if map.contains_key(&key) || map.len() < MAX_WINDOWS {
            key
        } else {
            key.overflow()
        };
        map.get_mut(&key).is_some_and(|q| {
            trim(q, now);
            q.len() >= limit
        })
    }

    fn token_key(&self, token: &str) -> Key {
        Key::Token(self.hasher.hash_one(token))
    }

    /// One request from `ip`; `false` when it is over its 100 a minute.
    /// This computer's own address has no per-address window: every sender
    /// behind a local tunnel (ngrok forwarding to 127.0.0.1) arrives from
    /// it, so one window there would let a stranger's flood hold every real
    /// alert (upstream exempted its built-in tunnel's traffic; this app
    /// cannot tell a tunnel's from a local program's). Each webhook's own
    /// window and the failure budget still apply.
    pub fn admit_caller(&self, ip: IpAddr, now: Instant) -> bool {
        if this_computer(ip) {
            return true;
        }
        self.hit(Key::Caller(ip), REQUESTS_PER_MINUTE, now)
    }

    /// One request on `token`; `false` when it is over its 100 a minute.
    pub fn admit_token(&self, token: &str, now: Instant) -> bool {
        self.hit(self.token_key(token), REQUESTS_PER_MINUTE, now)
    }

    /// Whether `ip` has spent its 10 failed authentications this minute.
    pub fn failures_spent(&self, ip: IpAddr, now: Instant) -> bool {
        self.full(Key::Failures(ip), FAILURES_PER_MINUTE, now)
    }

    /// One failed authentication from `ip`.
    pub fn count_failure(&self, ip: IpAddr, now: Instant) {
        let _ = self.hit(Key::Failures(ip), FAILURES_PER_MINUTE, now);
    }

    /// Windows held (bounded by [`MAX_WINDOWS`] and three overflows).
    pub fn len(&self) -> usize {
        self.windows.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Whether a call may go on to the webhook, after the caller's own window
/// (checked before the body is read, [`WebhookLimits::admit_caller`]):
/// `false` is upstream's 429. An unknown token is a failed authentication;
/// a known token counts against its own window; once the caller has spent
/// its failures, only a call with the right secret goes on (a wrong one is
/// still counted by the lockout).
pub async fn admit(app: &Arc<AppState>, ip: IpAddr, call: &WebhookCall) -> bool {
    let limits = &app.flow.webhook_limits;
    let now = Instant::now();
    if !webhooks::token_known(app, &call.token).await {
        if limits.failures_spent(ip, now) {
            return false;
        }
        limits.count_failure(ip, now);
        return true;
    }
    if !limits.admit_token(&call.token, now) {
        return false;
    }
    if !limits.failures_spent(ip, now) {
        return true;
    }
    match webhooks::secret_given(app, call).await {
        Some((_, SecretGiven::Right)) => true,
        Some((id, SecretGiven::Wrong)) => {
            app.flow.webhook_guard.record_failure(id);
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([203, 0, 113, last])
    }

    #[test]
    fn a_caller_has_100_requests_a_minute_and_the_window_moves() {
        let l = WebhookLimits::default();
        let t0 = Instant::now();
        for n in 0..100 {
            assert!(
                l.admit_caller(ip(1), t0 + Duration::from_millis(n)),
                "request {n}"
            );
        }
        assert!(
            !l.admit_caller(ip(1), t0 + Duration::from_secs(30)),
            "the 101st"
        );
        assert!(
            l.admit_caller(ip(2), t0 + Duration::from_secs(30)),
            "another caller"
        );
        // A minute after the first hits they no longer count.
        assert!(l.admit_caller(ip(1), t0 + Duration::from_secs(60)));
    }

    #[test]
    fn a_token_has_100_requests_a_minute_whoever_sends_them() {
        let l = WebhookLimits::default();
        let t0 = Instant::now();
        for _ in 0..100 {
            assert!(l.admit_token("tok-a", t0));
        }
        assert!(!l.admit_token("tok-a", t0));
        assert!(l.admit_token("tok-b", t0), "another token");
    }

    #[test]
    fn ten_failures_a_minute_spend_a_callers_budget() {
        let l = WebhookLimits::default();
        let t0 = Instant::now();
        for _ in 0..9 {
            l.count_failure(ip(1), t0);
        }
        assert!(!l.failures_spent(ip(1), t0));
        l.count_failure(ip(1), t0);
        assert!(l.failures_spent(ip(1), t0));
        assert!(!l.failures_spent(ip(2), t0));
        assert!(!l.failures_spent(ip(1), t0 + WINDOW));
    }

    #[test]
    fn a_token_window_is_kept_under_a_hash_never_the_token() {
        let l = WebhookLimits::default();
        l.admit_token("the-secret-token", Instant::now());
        let held = format!("{:?}", l.windows.lock().keys().collect::<Vec<_>>());
        assert!(!held.contains("the-secret-token"), "{held}");
    }

    #[test]
    fn a_full_table_counts_new_callers_together_and_drops_spent_windows() {
        let l = WebhookLimits::default();
        let t0 = Instant::now();
        for n in 0..MAX_WINDOWS as u32 {
            assert!(l.admit_caller(IpAddr::from(n.to_be_bytes()), t0));
        }
        assert_eq!(l.len(), MAX_WINDOWS);
        // Every window is live: a new caller is counted in the shared one.
        assert!(l.admit_caller(ip(250), t0));
        assert_eq!(l.len(), MAX_WINDOWS + 1);
        // A minute on, the old windows are swept to make room.
        assert!(l.admit_caller(ip(251), t0 + WINDOW));
        assert!(l.len() <= 3, "{}", l.len());
        assert!(!l.is_empty());
    }

    #[test]
    fn the_429_is_upstreams() {
        assert_eq!(
            rate_limited(),
            json!({"status": "error", "message": "Rate limit exceeded. Please slow down your requests.",
                   "limit": "100 per 1 minute", "retry_after": 60})
        );
    }
}
