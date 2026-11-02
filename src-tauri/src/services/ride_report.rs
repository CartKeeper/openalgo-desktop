//! Ride Report: rules, builder and prompt renderer
//!
//! Deterministic, I/O-free rules for the "hitchhiker" briefing strategy:
//! 30-day net move, daily swing, analyst upside, ride status, trailing-stop
//! percent, entry date from fill history, and trading-day counting.

use crate::brokers::types::{AccountActivity, Order};
use crate::providers::fmp::FmpClient;
use crate::providers::types::PriceTarget;
use crate::providers::yahoo::YahooClient;
use crate::services::activities_service::ActivitiesService;
use crate::services::copilot_service::CopilotService;
use crate::services::funds_service::FundsService;
use crate::services::market_status_service::MarketStatusService;
use crate::services::orderbook_service::OrderbookService;
use crate::services::position_service::PositionService;
use crate::state::AppState;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc, Weekday};
use chrono_tz::America::New_York;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RideStatus {
    TooNew,
    Falling,
    Dormant,
    FlatWithUpside,
    PullingBack,
    Climbing,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TrailReason {
    TwoTimesSwing,
    ThreeTimesSwingHighUpside,
    UnknownSwingFloor,
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// 30-trading-day net move in percent. Needs at least 31 closes.
pub fn net_move_pct(closes: &[f64]) -> Option<f64> {
    if closes.len() < 31 {
        return None;
    }
    let last = closes[closes.len() - 1];
    let base = closes[closes.len() - 31];
    if base == 0.0 {
        return None;
    }
    Some(round1((last - base) / base * 100.0))
}

/// Mean absolute day-over-day percent change over the last 30 steps. Needs at least 31 closes.
pub fn daily_swing_pct(closes: &[f64]) -> Option<f64> {
    if closes.len() < 31 {
        return None;
    }
    let window = &closes[closes.len() - 31..];
    let mut sum = 0.0;
    for pair in window.windows(2) {
        if pair[0] == 0.0 {
            return None;
        }
        sum += ((pair[1] - pair[0]) / pair[0] * 100.0).abs();
    }
    Some(round1(sum / 30.0))
}

/// Analyst consensus upside versus the current price, in percent.
pub fn upside_pct(consensus: f64, price: f64) -> f64 {
    round1((consensus - price) / price * 100.0)
}

/// Ride status, evaluated in table order. Unknown net move counts as too new to judge.
pub fn classify(
    age_trading_days: Option<u32>,
    price: f64,
    avg_cost: f64,
    net_move: Option<f64>,
    upside: Option<f64>,
) -> RideStatus {
    let net = match (age_trading_days, net_move) {
        (Some(age), Some(net)) if age >= 30 => net,
        _ => return RideStatus::TooNew,
    };
    if price < avg_cost {
        return RideStatus::Falling;
    }
    let in_band = net.abs() <= 3.0;
    if in_band && upside.map_or(true, |u| u <= 5.0) {
        return RideStatus::Dormant;
    }
    if in_band {
        return RideStatus::FlatWithUpside;
    }
    if net < -3.0 {
        return RideStatus::PullingBack;
    }
    RideStatus::Climbing
}

/// Trailing-stop percent for a proposed stop, clamped to [3.0, 15.0].
pub fn trail_percent(daily_swing: Option<f64>, upside: Option<f64>) -> (f64, TrailReason) {
    let swing = match daily_swing {
        Some(s) => s,
        None => return (3.0, TrailReason::UnknownSwingFloor),
    };
    let (multiplier, reason) = if upside.map_or(false, |u| u >= 15.0) {
        (3.0, TrailReason::ThreeTimesSwingHighUpside)
    } else {
        (2.0, TrailReason::TwoTimesSwing)
    };
    (round1(swing * multiplier).clamp(3.0, 15.0), reason)
}

/// Date of the fill that opened the current holding run. `fills` are newest-first.
pub fn entry_date_from_fills(
    fills: &[AccountActivity],
    symbol: &str,
    held_qty: f64,
) -> Option<String> {
    let mut running = 0.0;
    for f in fills {
        let same_symbol = f
            .symbol
            .as_deref()
            .map_or(false, |s| s.eq_ignore_ascii_case(symbol));
        if !same_symbol {
            continue;
        }
        let qty = f.qty.unwrap_or(0.0);
        let is_buy = f
            .side
            .as_deref()
            .map_or(false, |s| s.to_ascii_lowercase().starts_with("buy"));
        running += if is_buy { qty } else { -qty };
        if running >= held_qty - 1e-9 {
            return f.date.clone();
        }
    }
    None
}

/// A UTC instant from an ISO timestamp or a zone-less SQLite value (`YYYY-MM-DD HH:MM:SS`, UTC).
fn parse_utc_instant(ts: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
        return Some(dt.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").ok().map(|naive| naive.and_utc())
}

/// A cached entry date is stale when the symbol has a fill newer than the cache row
/// (`computed_at`, SQLite UTC): the holding run may have changed since. No fill: fresh.
/// Either side unparsable: stale, so the date is recomputed from fills.
pub fn ride_entry_is_stale(computed_at: &str, newest_fill_at: Option<&str>) -> bool {
    let Some(fill_at) = newest_fill_at else {
        return false;
    };
    match (parse_utc_instant(computed_at), parse_utc_instant(fill_at)) {
        (Some(computed), Some(fill)) => fill > computed,
        _ => true,
    }
}

/// A dismissal (SQLite UTC) applies only to the ride it was made on: when the ride's
/// entry date (New York) is known, the dismissal must fall on or after it. A re-bought
/// ride is a new ride.
pub fn dismissal_applies(dismissed_at: &str, entered_at: Option<&str>) -> bool {
    let Some(entered) = entered_at else {
        return true;
    };
    let entered_day = entered.get(..10).unwrap_or(entered);
    match ny_date_of_timestamp(dismissed_at) {
        Some(dismissed_day) => dismissed_day.as_str() >= entered_day,
        None => false,
    }
}

/// Sessions strictly after `from_date` and up to and including `to_date`.
pub fn count_trading_days(sessions: &[String], from_date: &str, to_date: &str) -> u32 {
    let day = |s: &str| s.get(..10).unwrap_or(s).to_string();
    let from = day(from_date);
    let to = day(to_date);
    sessions
        .iter()
        .filter(|s| {
            let d = day(s);
            d > from && d <= to
        })
        .count() as u32
}

// ============================================================================
// Report types
// ============================================================================

#[derive(Serialize, Clone)]
pub struct ExistingStop {
    pub order_id: String,
    pub order_type: String,
    pub quantity: f64,
    pub validity: String,
    pub trail_percent: Option<f64>,
    pub trigger_price: f64,
}

#[derive(Serialize, Clone)]
pub struct RidePosition {
    pub symbol: String,
    pub exchange: String,
    pub quantity: f64,
    pub avg_cost: f64,
    pub price: f64,
    pub pnl_pct: f64,
    pub entered_at: Option<String>,
    pub age_trading_days: Option<u32>,
    pub net_move_30d_pct: Option<f64>,
    pub daily_swing_pct: Option<f64>,
    pub consensus_target: Option<f64>,
    pub upside_pct: Option<f64>,
    pub targets_last_90d: Option<i32>,
    pub target_source: Option<String>,
    pub status: RideStatus,
    pub protected: bool,
    /// False when the open-orders read failed: `protected`/`working_sells` are then not evidence of anything.
    pub protection_known: bool,
    pub working_sells: Vec<ExistingStop>,
    pub proposed_trail_pct: f64,
    pub trail_reason: TrailReason,
    pub protection_dismissed_at: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Scoreboard {
    pub equity_now: f64,
    pub equity_1w: Option<f64>,
    pub equity_1m: Option<f64>,
    pub as_of: String,
}

#[derive(Serialize, Clone)]
pub struct RideReport {
    pub positions: Vec<RidePosition>,
    pub scoreboard: Option<Scoreboard>,
    pub paper_mode: bool,
    pub generated_at: String,
}

// ============================================================================
// Report builder (best effort per field; a failed lookup yields None, never an Err)
// ============================================================================

const NY_STAMP: &str = "%Y-%m-%d %-I:%M %p ET";
const FILL_PAGES: u32 = 5;
const CLOSES_LOOKBACK_DAYS: i64 = 75;
const TARGET_WINDOW_DAYS: i64 = 90;

/// A held position from the broker (or manual tracking), before any enrichment.
struct HeldPosition {
    symbol: String,
    exchange: String,
    quantity: f64,
    avg_cost: f64,
    ltp: f64,
    /// Manual positions carry their own entry date.
    added_at: Option<String>,
}

/// Where the analyst target came from and how many targets back it.
#[derive(Debug, PartialEq)]
pub(crate) struct TargetPick {
    pub(crate) consensus: f64,
    pub(crate) count: Option<i32>,
    pub(crate) source: &'static str,
}

/// One daily bar from whichever source answered.
pub(crate) struct CloseBar {
    pub(crate) date: String,
    pub(crate) close: f64,
    pub(crate) volume: Option<i64>,
}

fn ok_or_warn<T, E: std::fmt::Display>(what: &str, symbol: &str, r: std::result::Result<T, E>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("ride report: {} failed for {}: {}", what, symbol, e);
            None
        }
    }
}

/// New York calendar date of an ISO timestamp; a bare date passes through.
fn ny_date_of_timestamp(ts: &str) -> Option<String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
        return Some(dt.with_timezone(&New_York).format("%Y-%m-%d").to_string());
    }
    // Zone-less SQLite values (`YYYY-MM-DD HH:MM:SS`) are UTC.
    if let Ok(naive) = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
        return Some(naive.and_utc().with_timezone(&New_York).format("%Y-%m-%d").to_string());
    }
    let day = ts.get(..10)?;
    NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
    Some(day.to_string())
}

/// New York calendar date of a SQLite `datetime('now')` value (UTC, `YYYY-MM-DD HH:MM:SS`).
fn ny_date_of_sqlite_utc(ts: &str) -> String {
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
        .map(|naive| naive.and_utc().with_timezone(&New_York).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| ts.to_string())
}

/// Mon-Fri days strictly after `entered_at` through `today` (used when the calendar is unavailable).
fn weekdays_after(entered_at: &str, today: &str) -> Option<u32> {
    let from = NaiveDate::parse_from_str(entered_at.get(..10)?, "%Y-%m-%d").ok()?;
    let to = NaiveDate::parse_from_str(today.get(..10)?, "%Y-%m-%d").ok()?;
    let mut count = 0;
    let mut day = from.succ_opt()?;
    while day <= to {
        if !matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            count += 1;
        }
        day = day.succ_opt()?;
    }
    Some(count)
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 { (values[mid - 1] + values[mid]) / 2.0 } else { values[mid] })
}

/// Positive price targets published within the window ending `today`.
pub(crate) fn recent_target_prices(targets: &[PriceTarget], today: NaiveDate) -> Vec<f64> {
    let cutoff = (today - Duration::days(TARGET_WINDOW_DAYS)).format("%Y-%m-%d").to_string();
    targets
        .iter()
        .filter(|t| t.published_date.as_deref().and_then(|d| d.get(..10)).map_or(false, |d| d >= cutoff.as_str()))
        .filter_map(|t| t.price_target)
        .filter(|p| *p > 0.0)
        .collect()
}

/// Source order: FMP consensus, then median of recent FMP targets (needs at least 2), then Yahoo mean.
fn pick_target(
    consensus: Option<f64>,
    summary_count: Option<i32>,
    recent_prices: &[f64],
    yahoo: Option<(f64, i32)>,
) -> Option<TargetPick> {
    let recent_count = if recent_prices.is_empty() { None } else { Some(recent_prices.len() as i32) };
    let count = |yahoo_count: Option<i32>| summary_count.or(recent_count).or(yahoo_count);
    if let Some(c) = consensus.filter(|c| *c > 0.0) {
        return Some(TargetPick { consensus: c, count: count(None), source: "fmp_consensus" });
    }
    if recent_prices.len() >= 2 {
        if let Some(m) = median(recent_prices.to_vec()) {
            return Some(TargetPick { consensus: m, count: count(None), source: "fmp_median" });
        }
    }
    yahoo
        .filter(|(mean, _)| *mean > 0.0)
        .map(|(mean, n)| TargetPick { consensus: mean, count: count(Some(n)), source: "yahoo" })
}

/// Equity scoreboard: last point now, the point 5 sessions back for one week, 21 back for one month.
fn scoreboard_from(timestamps: &[i64], equity: &[f64]) -> Option<Scoreboard> {
    let now = *equity.last()?;
    let len = equity.len();
    let as_of = timestamps
        .last()
        .and_then(|t| DateTime::from_timestamp(*t, 0))
        .map(|dt| dt.with_timezone(&New_York).format(NY_STAMP).to_string())
        .unwrap_or_else(|| "unknown".to_string());
    Some(Scoreboard {
        equity_now: now,
        equity_1w: if len >= 6 { Some(equity[len - 6]) } else { None },
        equity_1m: if len >= 22 { Some(equity[len - 22]) } else { None },
        as_of,
    })
}

fn existing_stop(o: &Order) -> ExistingStop {
    ExistingStop {
        order_id: o.order_id.clone(),
        order_type: o.order_type.clone(),
        quantity: o.quantity,
        validity: o.validity.clone(),
        trail_percent: o.trail_percent,
        trigger_price: o.trigger_price,
    }
}

fn is_full_size_trailing_stop(s: &ExistingStop, position_qty: f64) -> bool {
    s.order_type == "TRAILING_STOP" && (s.quantity - position_qty).abs() < 1e-9
}

/// Positions exactly as `generate_briefing` reads them: broker first, manual portfolio as fallback.
async fn load_held_positions(state: &AppState) -> Vec<HeldPosition> {
    let live = match PositionService::get_positions(state, None).await {
        Ok(result) if !result.positions.is_empty() => Some(result.positions),
        Ok(_) => None,
        Err(e) => {
            warn!("ride report: live positions unavailable: {}", e);
            None
        }
    };
    let held: Vec<HeldPosition> = match live {
        Some(positions) => positions
            .into_iter()
            .map(|p| HeldPosition {
                symbol: p.symbol,
                exchange: p.exchange,
                quantity: p.quantity,
                avg_cost: p.average_price,
                ltp: p.ltp,
                added_at: None,
            })
            .collect(),
        None => state
            .sqlite
            .get_portfolio_positions()
            .unwrap_or_default()
            .into_iter()
            .map(|p| HeldPosition {
                symbol: p.symbol,
                exchange: p.exchange,
                quantity: p.quantity,
                avg_cost: p.avg_price,
                ltp: p.current_price.unwrap_or(0.0),
                added_at: p.added_at,
            })
            .collect(),
    };
    held.into_iter().filter(|p| p.quantity > 0.0).collect()
}

/// Daily bars, ascending, with the source that answered. FMP first over `from`..`to`;
/// Yahoo (`yahoo_range`, e.g. "3mo") when FMP is absent, fails or returns nothing.
pub(crate) async fn load_daily_bars(
    fmp: Option<&FmpClient>,
    yahoo: &YahooClient,
    symbol: &str,
    from: &str,
    to: &str,
    yahoo_range: &str,
) -> (&'static str, Vec<CloseBar>) {
    if let Some(fmp) = fmp {
        let fetched = fmp.get_daily_closes(symbol, from, to).await;
        if let Some(bars) = ok_or_warn("FMP daily closes", symbol, fetched).filter(|b| !b.is_empty()) {
            info!("ride report: {} closes from FMP ({} bars)", symbol, bars.len());
            let bars = bars.into_iter().map(|b| CloseBar { date: b.date, close: b.close, volume: b.volume }).collect();
            return ("fmp", bars);
        }
    }
    let fetched = yahoo.get_historical(symbol, "1d", yahoo_range).await;
    let bars = ok_or_warn("Yahoo daily closes", symbol, fetched).unwrap_or_default();
    info!("ride report: {} closes from Yahoo ({} bars)", symbol, bars.len());
    let bars = bars
        .into_iter()
        .map(|b| CloseBar { date: b.0.chars().take(10).collect(), close: b.4, volume: Some(b.5) })
        .collect();
    ("yahoo", bars)
}

/// Analyst target for a symbol: FMP consensus, FMP 90-day median, then Yahoo mean, with the count chain.
pub(crate) async fn load_target(
    fmp: Option<&FmpClient>,
    yahoo: &YahooClient,
    symbol: &str,
    today: NaiveDate,
) -> Option<TargetPick> {
    let (mut consensus, mut summary_count, mut recent) = (None, None, Vec::new());
    if let Some(fmp) = fmp {
        let fetched = fmp.get_price_target_consensus(symbol).await;
        consensus = ok_or_warn("FMP target consensus", symbol, fetched).flatten().and_then(|c| c.target_consensus);
        let fetched = fmp.get_price_target_summary(symbol).await;
        summary_count = ok_or_warn("FMP target summary", symbol, fetched).flatten().and_then(|s| s.last_quarter_count);
        if consensus.is_none() || summary_count.is_none() {
            let fetched = fmp.get_price_targets(symbol).await;
            let individual = ok_or_warn("FMP price targets", symbol, fetched).unwrap_or_default();
            recent = recent_target_prices(&individual, today);
        }
    }
    if let Some(mut pick) = pick_target(consensus, summary_count, &recent, None) {
        if pick.count.is_none() {
            // Both FMP counts are missing: Yahoo is consulted for the count only.
            pick.count = yahoo.get_target_mean(symbol).await.map(|(_, n)| n);
        }
        return Some(pick);
    }
    let yahoo_target = yahoo.get_target_mean(symbol).await;
    pick_target(consensus, summary_count, &recent, yahoo_target)
}

/// Per-report lookups shared by every position.
struct ReportContext<'a> {
    state: &'a AppState,
    paper_mode: bool,
    today: NaiveDate,
    open_orders: Vec<Order>,
    /// False when the open-orders read failed, so an empty list proves nothing.
    open_orders_known: bool,
    fmp: Option<FmpClient>,
    yahoo: YahooClient,
    /// Fill history, fetched at most once and only if some position needs it.
    fills: Option<Vec<AccountActivity>>,
    /// Newest page of fills, fetched at most once, to check cached entry dates.
    recent_fills: Option<Vec<AccountActivity>>,
}

impl ReportContext<'_> {
    async fn fills(&mut self) -> &[AccountActivity] {
        if self.fills.is_none() {
            let fetched = ActivitiesService::get_fills(self.state, None, FILL_PAGES).await;
            let all = ok_or_warn("fill history", "account", fetched).unwrap_or_default();
            self.fills = Some(all.into_iter().filter(|a| a.activity_type == "FILL").collect());
        }
        self.fills.as_deref().unwrap_or(&[])
    }

    /// Timestamp of the symbol's newest fill on the first page of fill history.
    async fn newest_fill_at(&mut self, symbol: &str) -> Option<String> {
        if self.recent_fills.is_none() {
            let fetched = ActivitiesService::get_fills(self.state, None, 1).await;
            let page = ok_or_warn("recent fills", "account", fetched).unwrap_or_default();
            self.recent_fills = Some(page.into_iter().filter(|a| a.activity_type == "FILL").collect());
        }
        self.recent_fills
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .find(|f| f.symbol.as_deref().map_or(false, |s| s.eq_ignore_ascii_case(symbol)))
            .and_then(|f| f.date.clone())
    }

    /// Daily closes, ascending. FMP first, Yahoo when FMP fails or returns nothing.
    async fn closes(&self, symbol: &str) -> Vec<f64> {
        let from = (self.today - Duration::days(CLOSES_LOOKBACK_DAYS)).format("%Y-%m-%d").to_string();
        let to = self.today.format("%Y-%m-%d").to_string();
        let (_, bars) = load_daily_bars(self.fmp.as_ref(), &self.yahoo, symbol, &from, &to, "3mo").await;
        bars.into_iter().map(|b| b.close).collect()
    }

    async fn target(&self, symbol: &str) -> Option<TargetPick> {
        load_target(self.fmp.as_ref(), &self.yahoo, symbol, self.today).await
    }

    /// Date the current holding run began: cache (unless a newer fill makes it stale), then
    /// fill history. Manual positions use `added_at`.
    async fn entry_date(&mut self, held: &HeldPosition) -> Option<String> {
        if let Some(added) = &held.added_at {
            return ny_date_of_timestamp(added);
        }
        let cached = self.state.sqlite.get_ride_entry(&held.symbol, held.quantity);
        if let Some((date, computed_at)) = ok_or_warn("entry-date cache read", &held.symbol, cached).flatten() {
            let newest_fill_at = self.newest_fill_at(&held.symbol).await;
            if !ride_entry_is_stale(&computed_at, newest_fill_at.as_deref()) {
                return Some(date);
            }
            info!("ride report: cached entry date for {} predates its newest fill; recomputing", held.symbol);
        }
        let from_fills = entry_date_from_fills(self.fills().await, &held.symbol, held.quantity);
        let date = ny_date_of_timestamp(&from_fills?)?;
        let stored = self.state.sqlite.upsert_ride_entry(&held.symbol, held.quantity, &date);
        ok_or_warn("entry-date cache write", &held.symbol, stored);
        Some(date)
    }

    /// Trading sessions after the entry date through today; weekdays when the calendar is unavailable.
    async fn ride_age(&self, symbol: &str, entered_at: &str) -> Option<u32> {
        let today = self.today.format("%Y-%m-%d").to_string();
        let fetched = MarketStatusService::get_calendar(self.state, None, Some(entered_at), Some(&today)).await;
        match ok_or_warn("market calendar", symbol, fetched) {
            Some(days) => {
                let sessions: Vec<String> = days.into_iter().map(|d| d.date).collect();
                Some(count_trading_days(&sessions, entered_at, &today))
            }
            None => weekdays_after(entered_at, &today),
        }
    }

    async fn build_position(&mut self, held: HeldPosition) -> RidePosition {
        let symbol = held.symbol.clone();
        let closes = self.closes(&symbol).await;
        let price = if held.ltp > 0.0 { held.ltp } else { closes.last().copied().unwrap_or(0.0) };
        let net_move = net_move_pct(&closes);
        let swing = daily_swing_pct(&closes);

        let target = self.target(&symbol).await;
        let upside = match &target {
            Some(t) if price > 0.0 => Some(upside_pct(t.consensus, price)),
            _ => None,
        };
        let (proposed_trail_pct, trail_reason) = trail_percent(swing, upside);

        let (entered_at, age_trading_days) = if self.paper_mode {
            (None, None)
        } else {
            let entered = self.entry_date(&held).await;
            let age = match &entered {
                Some(d) => self.ride_age(&symbol, d).await,
                None => None,
            };
            (entered, age)
        };
        let status = if self.paper_mode {
            RideStatus::TooNew
        } else {
            classify(age_trading_days, price, held.avg_cost, net_move, upside)
        };

        let working_sells: Vec<ExistingStop> = self
            .open_orders
            .iter()
            .filter(|o| o.side.eq_ignore_ascii_case("SELL") && o.symbol.eq_ignore_ascii_case(&symbol))
            .map(existing_stop)
            .collect();
        let protected = working_sells.iter().any(|s| is_full_size_trailing_stop(s, held.quantity));
        let dismissed = self.state.sqlite.is_protection_dismissed(&symbol, held.quantity);
        let protection_dismissed_at = ok_or_warn("dismissal lookup", &symbol, dismissed)
            .flatten()
            .filter(|ts| dismissal_applies(ts, entered_at.as_deref()))
            .map(|ts| ny_date_of_sqlite_utc(&ts));

        let pnl_pct = if price > 0.0 && held.avg_cost > 0.0 {
            round1((price - held.avg_cost) / held.avg_cost * 100.0)
        } else {
            0.0
        };
        RidePosition {
            symbol,
            exchange: held.exchange,
            quantity: held.quantity,
            avg_cost: held.avg_cost,
            price,
            pnl_pct,
            entered_at,
            age_trading_days,
            net_move_30d_pct: net_move,
            daily_swing_pct: swing,
            consensus_target: target.as_ref().map(|t| t.consensus),
            upside_pct: upside,
            targets_last_90d: target.as_ref().and_then(|t| t.count),
            target_source: target.as_ref().map(|t| t.source.to_string()),
            status,
            protected,
            protection_known: self.open_orders_known,
            working_sells,
            proposed_trail_pct,
            trail_reason,
            protection_dismissed_at,
        }
    }
}

async fn load_scoreboard(state: &AppState) -> Option<Scoreboard> {
    let fetched = FundsService::get_portfolio_history(state, None, "1M", "1D").await;
    let history = ok_or_warn("equity history", "account", fetched)?;
    scoreboard_from(&history.timestamp, &history.equity)
}

/// Build the Ride Report for every held position. Never fails; missing data becomes `None` per field.
pub async fn build_ride_report(state: &AppState) -> RideReport {
    let paper_mode = state.sqlite.get_analyze_mode().unwrap_or(false);
    let now = Utc::now().with_timezone(&New_York);

    let open_orders_fetched = OrderbookService::get_open_orders(state, None).await;
    let open_orders_result = ok_or_warn("open orders", "account", open_orders_fetched);
    let open_orders_known = open_orders_result.is_some();
    let open_orders = open_orders_result.map(|r| r.orders).unwrap_or_default();
    let fmp = ok_or_warn("FMP client", "account", CopilotService::get_fmp_client(state));

    let mut ctx = ReportContext {
        state,
        paper_mode,
        today: now.date_naive(),
        open_orders,
        open_orders_known,
        fmp,
        yahoo: YahooClient::new((*state.http_client).clone()),
        fills: None,
        recent_fills: None,
    };

    let mut positions = Vec::new();
    for held in load_held_positions(state).await {
        positions.push(ctx.build_position(held).await);
    }
    let scoreboard = if paper_mode { None } else { load_scoreboard(state).await };

    RideReport { positions, scoreboard, paper_mode, generated_at: now.format(NY_STAMP).to_string() }
}

// ============================================================================
// Prompt renderer
// ============================================================================

fn status_label(status: RideStatus) -> &'static str {
    match status {
        RideStatus::TooNew => "Too new to judge",
        RideStatus::Falling => "Falling",
        RideStatus::Dormant => "Dormant",
        RideStatus::FlatWithUpside => "Flat, with upside",
        RideStatus::PullingBack => "Pulling back",
        RideStatus::Climbing => "Climbing",
    }
}

fn fmt_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        format!("{}", q)
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("{:.1}", x))
}

fn fmt_price(p: f64) -> String {
    if p > 0.0 { format!("${:.2}", p) } else { "n/a".to_string() }
}

fn got_on_cell(p: &RidePosition, paper_mode: bool) -> String {
    if paper_mode {
        return "paper mode: no fill history".to_string();
    }
    match (&p.entered_at, p.age_trading_days) {
        (Some(d), Some(age)) => format!("{} @ ${:.2} ({} sessions)", d, p.avg_cost, age),
        (Some(d), None) => format!("{} @ ${:.2}", d, p.avg_cost),
        (None, _) => format!("unknown @ ${:.2}", p.avg_cost),
    }
}

fn target_cell(p: &RidePosition) -> String {
    match p.consensus_target {
        None => "no analyst target available".to_string(),
        Some(t) => format!(
            "${:.2} ({}, {} targets/90d)",
            t,
            p.target_source.as_deref().unwrap_or("unknown source"),
            p.targets_last_90d.map_or_else(|| "?".to_string(), |n| n.to_string()),
        ),
    }
}

fn working_sell_text(s: &ExistingStop) -> String {
    format!("{} ({} x{})", s.order_id, s.order_type, fmt_qty(s.quantity))
}

fn protection_cell(p: &RidePosition) -> String {
    let mut cell = if !p.protection_known {
        "unknown (open orders could not be read; do not propose or cancel stops)".to_string()
    } else if p.protected {
        let stop = p.working_sells.iter().find(|s| {
            s.order_type == "TRAILING_STOP" && (s.quantity - p.quantity).abs() < 1e-9
        });
        match stop {
            Some(s) => {
                let mut text = format!(
                    "protected (trailing {}% {}, id {})",
                    s.trail_percent.map_or_else(|| "?".to_string(), |t| format!("{:.1}", t)),
                    s.validity,
                    s.order_id
                );
                let others: Vec<String> = p
                    .working_sells
                    .iter()
                    .filter(|o| o.order_id != s.order_id)
                    .map(working_sell_text)
                    .collect();
                if !others.is_empty() {
                    text.push_str(&format!("; also working: {}", others.join(", ")));
                }
                text
            }
            None => "protected".to_string(),
        }
    } else if p.working_sells.is_empty() {
        "none".to_string()
    } else {
        let list: Vec<String> = p.working_sells.iter().map(working_sell_text).collect();
        format!("not protected — working sells to cancel first: {}", list.join(", "))
    };
    if let Some(d) = &p.protection_dismissed_at {
        cell.push_str(&format!("; protection dismissed by you on {}", d));
    }
    cell
}

fn trail_cell(p: &RidePosition) -> String {
    let reason = match p.trail_reason {
        TrailReason::TwoTimesSwing => "2× daily swing".to_string(),
        TrailReason::ThreeTimesSwingHighUpside => format!(
            "widened because analysts see {}% upside (3× swing)",
            fmt_pct(p.upside_pct)
        ),
        TrailReason::UnknownSwingFloor => "swing unknown, floor".to_string(),
    };
    format!("{:.1}% ({})", p.proposed_trail_pct, reason)
}

fn scoreboard_block(report: &RideReport) -> String {
    let sb = match (&report.scoreboard, report.paper_mode) {
        (Some(sb), _) => sb,
        (None, true) => return "Scoreboard: not available in paper mode".to_string(),
        (None, false) => return "Scoreboard: not available".to_string(),
    };
    let change = |then: Option<f64>| match then {
        Some(t) if t > 0.0 => format!(
            "${:.2} ({:+.2}, {:+.1}%)",
            t,
            sb.equity_now - t,
            (sb.equity_now - t) / t * 100.0
        ),
        _ => "not available".to_string(),
    };
    format!(
        "Scoreboard (account equity, as of {}):\n- now: ${:.2}\n- 1 week ago: {}\n- 1 month ago: {}",
        sb.as_of,
        sb.equity_now,
        change(sb.equity_1w),
        change(sb.equity_1m)
    )
}

const COLUMN_LEGEND: [&str; 12] = [
    "Symbol: ticker held",
    "Qty: shares held",
    "Got on: date the current holding run began (New York date), average cost, and trading sessions held",
    "Price: latest price",
    "P&L %: gain or loss versus average cost",
    "30d net %: close-to-close move over the last 30 trading sessions",
    "Daily swing %: mean absolute day-over-day move over the last 30 sessions",
    "Analyst target: consensus price target, its source and how many targets in the last 90 days",
    "Upside %: analyst target versus current price",
    "Status: where the position sits in the ride rules (see statuses below)",
    "Protection: whether a trailing stop for the full quantity is already working, and any other working sells",
    "Proposed trail: trailing-stop percent computed from the daily swing, with the reason",
];

const STATUS_LEGEND: [&str; 6] = [
    "Too new to judge: held fewer than 30 trading sessions, or not enough price history",
    "Falling: price is below average cost",
    "Dormant: 30d net move within ±3% and upside 5% or less (or unknown)",
    "Flat, with upside: 30d net move within ±3% but analysts see more than 5% upside",
    "Pulling back: 30d net move below -3%, price at or above cost",
    "Climbing: 30d net move above +3%, price at or above cost",
];

/// Markdown block the briefing prompt embeds: optional scoreboard, one table row per position, legend.
pub fn render_for_prompt(report: &RideReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Ride Report generated {}\n\n", report.generated_at));
    out.push_str(&scoreboard_block(report));
    out.push_str("\n\n");
    out.push_str("| Symbol | Qty | Got on | Price | P&L % | 30d net % | Daily swing % | Analyst target | Upside % | Status | Protection | Proposed trail |\n");
    out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for p in &report.positions {
        let status = if report.paper_mode { RideStatus::TooNew } else { p.status };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            p.symbol,
            fmt_qty(p.quantity),
            got_on_cell(p, report.paper_mode),
            fmt_price(p.price),
            if p.price > 0.0 { fmt_pct(Some(p.pnl_pct)) } else { "n/a".to_string() },
            fmt_pct(p.net_move_30d_pct),
            fmt_pct(p.daily_swing_pct),
            target_cell(p),
            fmt_pct(p.upside_pct),
            status_label(status),
            protection_cell(p),
            trail_cell(p),
        ));
    }
    out.push_str("\nColumns:\n");
    for line in COLUMN_LEGEND {
        out.push_str(&format!("- {}\n", line));
    }
    out.push_str("\nStatuses:\n");
    for line in STATUS_LEGEND {
        out.push_str(&format!("- {}\n", line));
    }
    out
}

#[cfg(test)]
mod rules_tests {
    use super::*;

    fn fill(symbol: &str, side: &str, qty: f64, date: &str) -> AccountActivity {
        AccountActivity {
            id: String::new(),
            activity_type: "FILL".to_string(),
            date: Some(date.to_string()),
            symbol: Some(symbol.to_string()),
            side: Some(side.to_string()),
            qty: Some(qty),
            price: Some(1.0),
            net_amount: None,
            per_share_amount: None,
            order_id: None,
            status: None,
        }
    }

    #[test]
    fn classify_table() {
        assert!(matches!(classify(None, 30.0, 25.0, Some(0.0), Some(0.0)), RideStatus::TooNew));
        assert!(matches!(classify(Some(10), 30.0, 25.0, Some(0.0), Some(0.0)), RideStatus::TooNew));
        assert!(matches!(classify(Some(40), 24.0, 25.0, Some(5.0), Some(50.0)), RideStatus::Falling));
        assert!(matches!(classify(Some(40), 30.0, 25.0, Some(1.2), Some(2.0)), RideStatus::Dormant));
        assert!(matches!(classify(Some(40), 30.0, 25.0, Some(-3.0), None), RideStatus::Dormant)); // unknown upside counts as <= 5
        assert!(matches!(classify(Some(40), 30.0, 25.0, Some(2.9), Some(5.1)), RideStatus::FlatWithUpside));
        assert!(matches!(classify(Some(40), 204.64, 199.91, Some(-7.9), Some(65.7)), RideStatus::PullingBack));
        assert!(matches!(classify(Some(40), 30.52, 25.71, Some(10.3), Some(-4.4)), RideStatus::Climbing));
        assert!(matches!(classify(Some(40), 106.20, 90.60, Some(3.5), Some(14.3)), RideStatus::Climbing));
        // Not enough price history to judge
        assert!(matches!(classify(Some(40), 30.0, 25.0, None, Some(10.0)), RideStatus::TooNew));
    }

    #[test]
    fn trail_rule() {
        assert_eq!(trail_percent(Some(1.0), Some(-4.4)).0, 3.0); // 2x1.0 floored to 3.0
        assert_eq!(trail_percent(Some(1.5), Some(65.7)).0, 4.5); // 3x swing because upside >= 15.0
        assert_eq!(trail_percent(Some(2.1), Some(10.0)).0, 4.2);
        assert_eq!(trail_percent(Some(9.0), Some(50.0)).0, 15.0); // capped
        assert!(matches!(trail_percent(None, Some(50.0)), (3.0, TrailReason::UnknownSwingFloor)));
        assert!(matches!(trail_percent(Some(1.5), Some(65.7)).1, TrailReason::ThreeTimesSwingHighUpside));
        assert!(matches!(trail_percent(Some(2.1), Some(10.0)).1, TrailReason::TwoTimesSwing));
        // Upside unknown: never widened, 2x swing.
        assert!(matches!(trail_percent(Some(2.1), None), (t, TrailReason::TwoTimesSwing) if t == 4.2));
    }

    #[test]
    fn entry_date_picks_current_holding_run() {
        // Fills newest first. Held today: 4. The current run is the two buys after the Oct 1 sell,
        // so the entry is the Oct 15 buy, not the Sep 1 buy that started the first run.
        let fills = vec![
            fill("PFE", "buy", 2.0, "2026-10-20"),
            fill("PFE", "buy", 2.0, "2026-10-15"),
            fill("PFE", "sell", 5.0, "2026-10-01"),
            fill("PFE", "buy", 5.0, "2026-09-01"),
        ];
        assert_eq!(entry_date_from_fills(&fills, "PFE", 4.0).as_deref(), Some("2026-10-15"));
    }

    #[test]
    fn entry_date_none_when_fills_do_not_cover_held_qty() {
        let fills = vec![fill("PFE", "buy", 1.0, "2026-10-20")];
        assert_eq!(entry_date_from_fills(&fills, "PFE", 4.0), None);
    }

    #[test]
    fn entry_date_matches_symbol_and_side_case_insensitively() {
        let fills = vec![
            fill("AAPL", "buy", 9.0, "2026-10-25"),
            fill("pfe", "BUY", 4.0, "2026-10-20T14:30:00.123Z"),
        ];
        assert_eq!(
            entry_date_from_fills(&fills, "PFE", 4.0).as_deref(),
            Some("2026-10-20T14:30:00.123Z")
        );
    }

    #[test]
    fn ride_entry_is_stale_when_a_fill_is_newer_than_the_cache_row() {
        // computed_at is SQLite UTC; fills are ISO timestamps.
        assert!(ride_entry_is_stale("2026-10-20 14:00:00", Some("2026-10-27T15:31:02.123Z")));
        assert!(ride_entry_is_stale("2026-10-20 14:00:00", Some("2026-10-20T10:30:00-04:00"))); // 14:30 UTC
        assert!(!ride_entry_is_stale("2026-10-20 14:00:00", Some("2026-10-20T09:30:00-04:00"))); // 13:30 UTC
        assert!(!ride_entry_is_stale("2026-10-20 14:00:00", Some("2026-01-05T15:00:00Z")));
        assert!(!ride_entry_is_stale("2026-10-20 14:00:00", None));
        // Unparsable on either side: recompute.
        assert!(ride_entry_is_stale("garbage", Some("2026-01-05T15:00:00Z")));
        assert!(ride_entry_is_stale("2026-10-20 14:00:00", Some("not a time")));
    }

    #[test]
    fn dismissal_applies_only_to_the_current_ride() {
        // Dismissed (SQLite UTC) on or after the ride's entry date (New York): applies.
        assert!(dismissal_applies("2026-10-28 15:00:00", Some("2026-10-27")));
        assert!(dismissal_applies("2026-10-27 15:00:00", Some("2026-10-27")));
        // Dismissed before a re-bought ride began: a new ride, the dismissal is void.
        assert!(!dismissal_applies("2026-01-10 15:00:00", Some("2026-10-27")));
        // 1 AM UTC on Oct 27 is still Oct 26 in New York.
        assert!(!dismissal_applies("2026-10-27 01:00:00", Some("2026-10-27")));
        // Entry date unknown: the dismissal stands.
        assert!(dismissal_applies("2026-01-10 15:00:00", None));
        // Unreadable dismissal time with a known entry: cannot show it is for this ride.
        assert!(!dismissal_applies("garbage", Some("2026-10-27")));
    }

    #[test]
    fn net_move_and_swing_need_31_closes() {
        assert!(net_move_pct(&[1.0; 30]).is_none());
        let mut c: Vec<f64> = (0..31).map(|i| 100.0 + i as f64).collect();
        assert_eq!(net_move_pct(&c), Some(30.0));
        c.reverse();
        assert_eq!(daily_swing_pct(&c).map(|x| x > 0.0), Some(true));
    }

    #[test]
    fn upside_rounds_to_tenth() {
        assert_eq!(upside_pct(29.17, 30.52), -4.4);
        assert_eq!(upside_pct(121.36, 106.20), 14.3);
    }

    #[test]
    fn trading_days_counts_sessions_after_entry() {
        let s = ["2026-10-28", "2026-10-29", "2026-10-30", "2026-11-02"].map(String::from);
        assert_eq!(count_trading_days(&s, "2026-10-28", "2026-11-02"), 3);
        // ISO timestamps compare by date; unsorted input is fine
        let u = ["2026-11-02", "2026-10-28", "2026-10-30"].map(String::from);
        assert_eq!(count_trading_days(&u, "2026-10-28T14:30:00.123Z", "2026-11-02"), 2);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    fn position() -> RidePosition {
        RidePosition {
            symbol: "PFE".to_string(),
            exchange: "NYSE".to_string(),
            quantity: 4.0,
            avg_cost: 25.71,
            price: 30.52,
            pnl_pct: 18.7,
            entered_at: Some("2026-09-01".to_string()),
            age_trading_days: Some(41),
            net_move_30d_pct: Some(10.3),
            daily_swing_pct: Some(1.5),
            consensus_target: Some(37.0),
            upside_pct: Some(21.2),
            targets_last_90d: Some(12),
            target_source: Some("fmp_consensus".to_string()),
            status: RideStatus::Climbing,
            protected: false,
            protection_known: true,
            working_sells: vec![],
            proposed_trail_pct: 4.5,
            trail_reason: TrailReason::TwoTimesSwing,
            protection_dismissed_at: None,
        }
    }

    fn report(positions: Vec<RidePosition>, paper_mode: bool) -> RideReport {
        RideReport {
            positions,
            scoreboard: None,
            paper_mode,
            generated_at: "2026-11-02 9:30 AM ET".to_string(),
        }
    }

    #[test]
    fn render_marks_unknown_consensus() {
        let mut p = position();
        p.consensus_target = None;
        p.upside_pct = None;
        p.targets_last_90d = None;
        p.target_source = None;
        let out = render_for_prompt(&report(vec![p], false));
        assert!(out.contains("no analyst target available"), "{out}");
    }

    #[test]
    fn render_shows_dismissal_and_paper_mode() {
        let mut p = position();
        p.protection_dismissed_at = Some("2026-11-01 4:05 PM ET".to_string());
        let out = render_for_prompt(&report(vec![p], false));
        assert!(out.contains("protection dismissed by you on"), "{out}");

        let mut q = position();
        q.entered_at = None;
        q.age_trading_days = None;
        let out = render_for_prompt(&report(vec![q], true));
        assert!(out.contains("paper mode: no fill history"), "{out}");
    }

    #[test]
    fn render_marks_protection_unknown() {
        let mut p = position();
        p.protection_known = false;
        let out = render_for_prompt(&report(vec![p], false));
        assert!(
            out.contains("unknown (open orders could not be read; do not propose or cancel stops)"),
            "{out}"
        );
        assert!(!out.contains("not protected"), "{out}");
    }

    #[test]
    fn render_protected_shows_validity_and_other_working_sells() {
        let stop = |id: &str, order_type: &str, validity: &str| ExistingStop {
            order_id: id.to_string(),
            order_type: order_type.to_string(),
            quantity: 4.0,
            validity: validity.to_string(),
            trail_percent: Some(4.5),
            trigger_price: 0.0,
        };
        let mut p = position();
        p.protected = true;
        p.working_sells = vec![stop("T1", "TRAILING_STOP", "DAY"), stop("S2", "SL-M", "DAY")];
        let out = render_for_prompt(&report(vec![p.clone()], false));
        assert!(out.contains("protected (trailing 4.5% DAY, id T1); also working: S2 (SL-M x4)"), "{out}");
        assert!(!out.contains("GTC"), "{out}");

        p.working_sells = vec![stop("T1", "TRAILING_STOP", "GTC")];
        let out = render_for_prompt(&report(vec![p], false));
        assert!(out.contains("protected (trailing 4.5% GTC, id T1)"), "{out}");
        assert!(!out.contains("also working"), "{out}");
    }

    #[test]
    fn render_trail_reason_copy() {
        let mut p = position();
        p.trail_reason = TrailReason::ThreeTimesSwingHighUpside;
        p.proposed_trail_pct = 4.5;
        let out = render_for_prompt(&report(vec![p], false));
        assert!(out.contains("widened because analysts see"), "{out}");
    }
}


#[cfg(test)]
mod builder_tests {
    use super::*;

    #[test]
    fn target_source_order() {
        let fmp = pick_target(Some(37.0), Some(12), &[30.0, 40.0], Some((50.0, 9))).unwrap();
        assert_eq!((fmp.source, fmp.consensus, fmp.count), ("fmp_consensus", 37.0, Some(12)));

        let median = pick_target(None, None, &[30.0, 40.0, 38.0], Some((50.0, 9))).unwrap();
        assert_eq!((median.source, median.consensus, median.count), ("fmp_median", 38.0, Some(3)));

        // One recent record is not enough for a median; Yahoo takes over.
        let yahoo = pick_target(None, None, &[30.0], Some((50.0, 9))).unwrap();
        assert_eq!((yahoo.source, yahoo.consensus), ("yahoo", 50.0));

        // No FMP count of either kind: count stays None so the caller asks Yahoo for it.
        assert_eq!(pick_target(Some(37.0), None, &[], None).unwrap().count, None);

        assert_eq!(pick_target(None, None, &[], None), None);
    }

    #[test]
    fn fill_timestamps_convert_to_new_york_dates() {
        // 8 PM ET on Oct 20 is already Oct 21 in UTC.
        assert_eq!(ny_date_of_timestamp("2026-10-21T00:00:00Z").as_deref(), Some("2026-10-20"));
        assert_eq!(ny_date_of_timestamp("2026-10-20").as_deref(), Some("2026-10-20"));
        assert_eq!(ny_date_of_timestamp("garbage"), None);
        // Manual `added_at` is a zone-less UTC SQLite value.
        assert_eq!(ny_date_of_timestamp("2026-11-02 23:30:00").as_deref(), Some("2026-11-02"));
        assert_eq!(ny_date_of_timestamp("2026-11-03 02:30:00").as_deref(), Some("2026-11-02"));
        assert_eq!(ny_date_of_sqlite_utc("2026-11-02 02:30:00"), "2026-11-01");
    }

    #[test]
    fn weekday_fallback_counts_mon_to_fri_after_entry() {
        // Fri Oct 30 -> Mon Nov 2: only Mon counts.
        assert_eq!(weekdays_after("2026-10-30", "2026-11-02"), Some(1));
        assert_eq!(weekdays_after("2026-10-26", "2026-11-02"), Some(5));
    }

    #[test]
    fn scoreboard_points() {
        // 23 daily points: one month ago is 21 sessions back (index 1), one week 5 back.
        let ts: Vec<i64> = (0..23).map(|i| 1_790_000_000 + i * 86_400).collect();
        let eq: Vec<f64> = (0..23).map(|i| 1000.0 + i as f64).collect();
        let sb = scoreboard_from(&ts, &eq).unwrap();
        assert_eq!((sb.equity_now, sb.equity_1w, sb.equity_1m), (1022.0, Some(1017.0), Some(1001.0)));
        assert!(sb.as_of.ends_with(" ET"));
        // Exactly 22 points: one month ago is the first point.
        let month = scoreboard_from(&ts[..22], &eq[..22]).unwrap();
        assert_eq!(month.equity_1m, Some(1000.0));
        // Too little history for a month (or a week): None, never a shorter look-back.
        let short = scoreboard_from(&ts[..3], &eq[..3]).unwrap();
        assert_eq!((short.equity_1w, short.equity_1m), (None, None));
        let under_a_month = scoreboard_from(&ts[..21], &eq[..21]).unwrap();
        assert_eq!((under_a_month.equity_1w, under_a_month.equity_1m), (Some(1015.0), None));
        assert!(scoreboard_from(&[], &[]).is_none());
    }
}
