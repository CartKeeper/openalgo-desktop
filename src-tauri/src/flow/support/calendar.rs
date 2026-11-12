//! The trading calendar the Calendar and Holidays nodes read (upstream
//! `services::market_calendar_service::holidays_api`, the web's
//! `POST /api/v1/market/holidays`), over this app's own holiday table
//! (`market_holidays`: a date, a description and the exchanges closed).
//!
//! This app's table has no special sessions, so `open_exchanges` is always
//! empty and every entry is a `trading` holiday.

use crate::error::Result;
use crate::state::AppState;
use serde_json::{json, Value};

/// One holiday as the API lists it.
fn holiday_api_json(h: &crate::db::sqlite::MarketHoliday) -> Value {
    json!({
        "date": h.date,
        "description": h.description,
        "holiday_type": "trading",
        "closed_exchanges": h.exchanges,
        "open_exchanges": [],
    })
}

/// `POST /api/v1/market/holidays` body for `year` (this year in market
/// time when none is given).
pub fn holidays_api(ctx: &AppState, year: Option<i32>) -> Result<Value> {
    use chrono::Datelike;
    let year = year.unwrap_or_else(|| {
        ctx.now()
            .with_timezone(&crate::flow::contract::time::market_tz())
            .year()
    });
    let list = ctx.sqlite.get_market_holidays_by_year(year)?;
    Ok(json!({
        "status": "success",
        "year": year,
        "timezone": crate::flow::contract::time::market_tz().name(),
        "data": list.iter().map(holiday_api_json).collect::<Vec<_>>(),
    }))
}
