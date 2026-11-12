//! Flow's clock: market time from the app's injected clock, the web's
//! `parse_time_string`, and the bounds on waiting nodes. Every Flow time is
//! market time on any machine (the web reads the host's local clock);
//! here the market is America/New_York (CLAUDE.md), where upstream fixed
//! Asia/Kolkata.

use crate::clock::Clock;
use chrono::NaiveDateTime;
use chrono_tz::Tz;

/// The longest a Delay node waits; longer belongs in a schedule
/// (`DELAY_MAX_SECONDS`).
pub const DELAY_MAX_SECONDS: i64 = 300;

/// The longest a Wait Until node waits (`WAIT_UNTIL_MAX_SECONDS`).
pub const WAIT_UNTIL_MAX_SECONDS: i64 = 1800;

/// The market's time zone in this app: America/New_York, the zone every
/// Flow wall-clock time is read and written in. The carried goldens and
/// the tests ported from the web were recorded in IST and set it for
/// themselves ([`set_market_tz_for_tests`]).
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

/// The market zone's short name for a sentence a trader reads (`ET`,
/// `IST`; any other zone by its IANA name).
pub fn market_label() -> &'static str {
    match market_tz() {
        Tz::America__New_York => "ET",
        Tz::Asia__Kolkata => "IST",
        other => other.name(),
    }
}

/// Make `tz` the market zone on this thread for the rest of the test (a
/// test runtime keeps its tasks on the thread, and `AppState::blocking`
/// carries it to the blocking pool); a thread-local rather than a
/// process-wide cell, so IST goldens and Eastern tests share one test
/// process.
#[cfg(test)]
pub fn set_market_tz_for_tests(tz: Tz) {
    MARKET_TZ_OVERRIDE.with(|c| c.set(Some(tz)));
}

/// Now, in market time, as a naive date-time (the web's `datetime.now()`
/// on a host in the market's zone).
pub fn market_now(clock: &dyn Clock) -> NaiveDateTime {
    clock.now().with_timezone(&market_tz()).naive_local()
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

    /// 13:45Z on 2026-10-30 is 09:45 in New York (EDT) and 19:15 in
    /// Kolkata; the tests ported from the web put themselves under IST.
    #[test]
    fn now_is_market_time_and_tests_can_choose_the_zone() {
        set_market_tz_for_tests(DEFAULT_MARKET_TZ);
        let clock = crate::clock::ManualClock::new("2026-10-30T13:45:00Z".parse().unwrap());
        assert_eq!(market_now(clock.as_ref()).format("%H:%M").to_string(), "09:45");
        assert_eq!(market_label(), "ET");
        set_market_tz_for_tests(chrono_tz::Asia::Kolkata);
        assert_eq!(market_now(clock.as_ref()).format("%H:%M").to_string(), "19:15");
        assert_eq!(market_label(), "IST");
        set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    }
}
