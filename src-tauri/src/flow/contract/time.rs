//! Flow's clock: market time (Asia/Kolkata) from the app's injected clock,
//! the web's `parse_time_string`, and the bounds on waiting nodes. Every
//! Flow time is IST on any machine (lead decision 1; the web reads the
//! host's local clock).

use crate::clock::Clock;
use chrono::NaiveDateTime;
use chrono_tz::Asia::Kolkata;
use chrono_tz::Tz;

/// The market's time zone in this app: America/New_York (CLAUDE.md), the
/// zone every Flow wall-clock time is read and written in. Upstream fixed
/// Asia/Kolkata; the carried goldens and the tests ported from the web
/// were recorded in IST and set it for themselves
/// ([`set_market_tz_for_tests`]).
pub const DEFAULT_MARKET_TZ: Tz = chrono_tz::America::New_York;

#[cfg(test)]
thread_local! {
    static MARKET_TZ_OVERRIDE: std::cell::Cell<Option<Tz>> = const { std::cell::Cell::new(None) };
}

/// The market's time zone.
pub fn market_tz() -> Tz {
    #[cfg(test)]
    if let Some(tz) = MARKET_TZ_OVERRIDE.with(|c| c.get()) {
        return tz;
    }
    DEFAULT_MARKET_TZ
}

/// Make `tz` the market zone on this thread for the rest of the test (a
/// test runtime keeps its tasks on the thread; a thread-local rather than a
/// process-wide cell, so IST goldens and Eastern tests share one test
/// process).
#[cfg(test)]
pub fn set_market_tz_for_tests(tz: Tz) {
    MARKET_TZ_OVERRIDE.with(|c| c.set(Some(tz)));
}

/// The longest a Delay node waits; longer belongs in a schedule
/// (`DELAY_MAX_SECONDS`).
pub const DELAY_MAX_SECONDS: i64 = 300;

/// The longest a Wait Until node waits (`WAIT_UNTIL_MAX_SECONDS`).
pub const WAIT_UNTIL_MAX_SECONDS: i64 = 1800;

/// Now, in IST, as a naive date-time (the web's `datetime.now()` on an
/// IST host).
pub fn ist_now(clock: &dyn Clock) -> NaiveDateTime {
    clock.now().with_timezone(&Kolkata).naive_local()
}

/// `HH:MM` or `HH:MM:SS` as (hour, minute, second), never failing: a part
/// that is not all digits takes its default (seconds 0), each clamped to
/// its range, and anything unreadable is the defaults
/// (`parse_time_string`).
pub fn parse_time_string(
    text: Option<&str>,
    default_hour: u32,
    default_minute: u32,
) -> (u32, u32, u32) {
    let Some(t) = text.filter(|t| !t.is_empty()) else {
        return (default_hour, default_minute, 0);
    };
    let parts: Vec<&str> = t.trim().split(':').collect();
    let part = |i: usize, default: u32| -> u32 {
        parts
            .get(i)
            .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
            .and_then(|p| p.parse::<u64>().ok())
            .map(|n| n.min(u32::MAX as u64) as u32)
            .unwrap_or(default)
    };
    (
        part(0, default_hour).min(23),
        part(1, default_minute).min(59),
        part(2, 0).min(59),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse_as_the_web_parses_them() {
        assert_eq!(parse_time_string(Some("09:20"), 9, 15), (9, 20, 0));
        assert_eq!(parse_time_string(Some("15:29:30"), 9, 15), (15, 29, 30));
        assert_eq!(parse_time_string(Some("25:75:99"), 9, 15), (23, 59, 59));
        assert_eq!(parse_time_string(Some("x:30"), 9, 15), (9, 30, 0));
        assert_eq!(parse_time_string(Some(" 10 :05"), 9, 15), (9, 5, 0));
        assert_eq!(parse_time_string(None, 9, 30), (9, 30, 0));
        assert_eq!(parse_time_string(Some(""), 9, 30), (9, 30, 0));
    }

    #[test]
    fn now_is_market_time() {
        let clock = crate::clock::ManualClock::new("2026-10-30T03:45:00Z".parse().unwrap());
        assert_eq!(ist_now(clock.as_ref()).format("%H:%M").to_string(), "09:15");
    }
}
