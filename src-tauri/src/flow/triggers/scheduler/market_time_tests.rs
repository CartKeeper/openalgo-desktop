//! The schedules in Eastern market time (CLAUDE.md): a daily time is the
//! same wall-clock time on both sides of the daylight-saving change, a
//! time that repeats on the fall-back night fires once, a time the
//! spring-forward night skips fires when the clock next shows it, and a
//! refusal names the zone.

use super::*;
use crate::flow::contract::time::{set_market_tz_for_tests, DEFAULT_MARKET_TZ};
use serde_json::json;

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn daily(time: &str, now: DateTime<Utc>) -> Schedule {
    set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    let data = json!({"scheduleType": "daily", "time": time});
    Schedule::from_node(data.as_object().unwrap(), now)
        .unwrap()
        .unwrap()
}

/// Friday 2026-10-30 is EDT (UTC-4), Monday 2026-11-02 is EST (UTC-5):
/// 09:35 Eastern is 13:35Z, then 14:35Z.
#[test]
fn a_daily_time_is_eastern_on_both_sides_of_the_daylight_saving_change() {
    let s = daily("09:35", utc("2026-10-30T12:00:00Z"));
    assert_eq!(
        s.next_at_or_after(utc("2026-10-30T12:00:00Z")),
        Some(utc("2026-10-30T13:35:00Z"))
    );
    assert_eq!(
        s.next_at_or_after(utc("2026-11-02T12:00:00Z")),
        Some(utc("2026-11-02T14:35:00Z"))
    );
}

/// 2026-11-01: 01:30 happens twice (EDT at 05:30Z, EST at 06:30Z). The
/// schedule fires once, on the later one, and next on the next day.
#[test]
fn a_time_that_repeats_on_the_fall_back_night_fires_once() {
    let s = daily("01:30", utc("2026-11-01T04:00:00Z"));
    let first = s.next_at_or_after(utc("2026-11-01T04:00:00Z")).unwrap();
    assert_eq!(first, utc("2026-11-01T06:30:00Z"));
    let after = s.next_at_or_after(first + Duration::seconds(1)).unwrap();
    assert_eq!(after, utc("2026-11-02T06:30:00Z"), "once that night");
}

/// 2026-03-08: the clock goes from 01:59 EST to 03:00 EDT (07:00Z), so
/// 02:30 never shows; the fire is the same minute past the hour once the
/// clock has jumped, 03:30 EDT.
#[test]
fn a_time_the_spring_forward_night_skips_fires_when_the_clock_reaches_it() {
    let s = daily("02:30", utc("2026-03-08T05:00:00Z"));
    assert_eq!(
        s.next_at_or_after(utc("2026-03-08T05:00:00Z")),
        Some(utc("2026-03-08T07:30:00Z"))
    );
}

/// The refusal of a past one-time schedule names the market zone.
#[test]
fn a_past_one_time_schedule_is_refused_naming_the_market_zone() {
    set_market_tz_for_tests(DEFAULT_MARKET_TZ);
    let now = utc("2026-10-30T14:00:00Z");
    let data = json!({"scheduleType": "once", "executeAt": "2026-10-30", "time": "09:15"});
    let err = Schedule::from_node(data.as_object().unwrap(), now).unwrap_err();
    assert_eq!(
        err,
        "The one-time schedule is set for 2026-10-30 09:15 ET, which has already passed. Set a later date and time, then activate it again."
    );
}
