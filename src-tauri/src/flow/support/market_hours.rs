//! The day's trading window the schedule trigger's market-hours gate reads
//! (upstream `db::sqlite::market_calendar::{SessionWindow,
//! effective_session_window}` over its holiday and timing tables).
//!
//! Task A1 carries the type and the regular window only; Task A3 reads the
//! live window from `MarketStatusService::get_clock` with this as the
//! logged fallback.

use chrono::{Datelike, NaiveDate, NaiveTime, TimeZone, Weekday};

/// One exchange's trading window on a date, epoch ms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionWindow {
    pub start_ms: i64,
    pub end_ms: i64,
    /// A special session or a holiday's own window, not the usual hours.
    pub is_special: bool,
}

/// The regular US session: 09:30 to 16:00 in market time.
pub const REGULAR_OPEN: (u32, u32) = (9, 30);
pub const REGULAR_CLOSE: (u32, u32) = (16, 0);

/// The regular window on `date` in market time; `None` on a weekend.
pub fn regular_window(date: NaiveDate) -> Option<SessionWindow> {
    if matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
        return None;
    }
    let tz = crate::flow::contract::time::market_tz();
    let at = |(h, m): (u32, u32)| {
        tz.from_local_datetime(&date.and_time(NaiveTime::from_hms_opt(h, m, 0)?))
            .single()
            .map(|t| t.timestamp_millis())
    };
    Some(SessionWindow {
        start_ms: at(REGULAR_OPEN)?,
        end_ms: at(REGULAR_CLOSE)?,
        is_special: false,
    })
}

/// The window the gate reads when it has nothing better: the regular
/// session (upstream read its calendar tables here).
pub fn effective_session_window(
    _conn: &rusqlite::Connection,
    date: NaiveDate,
    exchange: &str,
) -> Option<SessionWindow> {
    if exchange.is_empty() {
        return None;
    }
    regular_window(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regular_window_is_0930_to_1600_market_time_on_weekdays() {
        // Friday 2026-10-30 (EDT): 13:30Z to 20:00Z.
        let w = regular_window(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()).unwrap();
        assert_eq!(w.start_ms, 1_793_367_000_000);
        assert_eq!(w.end_ms, 1_793_390_400_000);
        assert!(!w.is_special);
        assert!(regular_window(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()).is_none());
    }
}
