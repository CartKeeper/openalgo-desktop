//! The trading session a moment belongs to (upstream
//! `strategy::session::session_day`): a day that starts at the configured
//! boundary in market time, so 01:00 on Tuesday belongs to Monday's session
//! when the boundary is 03:00.

use crate::flow::contract::time::market_tz;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

/// The market-time session day a moment belongs to.
pub fn session_day(moment: DateTime<Utc>, hour: u32, minute: u32) -> NaiveDate {
    let local = moment.with_timezone(&market_tz());
    let reset = NaiveTime::from_hms_opt(hour.min(23), minute.min(59), 0).unwrap_or_default();
    if local.time() < reset {
        local.date_naive().pred_opt().unwrap_or(local.date_naive())
    } else {
        local.date_naive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_turns_at_the_boundary_in_market_time() {
        // 2026-10-27 06:30Z is 02:30 in New York (EDT): before a 03:00
        // boundary, so still the 26th's session.
        let early: DateTime<Utc> = "2026-10-27T06:30:00Z".parse().unwrap();
        assert_eq!(session_day(early, 3, 0), NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        // 07:30Z is 03:30 in New York: the 27th.
        let later: DateTime<Utc> = "2026-10-27T07:30:00Z".parse().unwrap();
        assert_eq!(session_day(later, 3, 0), NaiveDate::from_ymd_opt(2026, 10, 27).unwrap());
    }
}
