//! Portfolio analytics service (WP2): feeds the pure `analytics` engine from
//! `client_trades`, `client_holdings`, scenarios, live prices and FMP closes.
//!
//! Lots and performance are per client; drift and the rebalance plan are per
//! scenario, measured against the client's saved targets. Current prices come
//! from `portfolio_service::enrich_positions_with_prices`; daily closes from
//! FMP's end-of-day endpoint (key from the provider settings); sectors from the
//! FMP company profile, with ETFs labelled "ETF".

use crate::analytics::allocation::{
    drift, rebalance, AllocationTarget, DriftReport, Holding, RebalancePlan, TargetScope,
};
use crate::analytics::lots::{build_lots, LotMethod, LotReport, OpeningPosition, TradeEvent, TradeSide};
use crate::analytics::returns::{annualize, time_weighted_return, xirr, CashFlow, ValuationPoint};
use crate::analytics::series::{holdings_value_series, FmpCloses};
use crate::error::{AppError, Result};
use crate::providers::fmp::FmpClient;
use crate::providers::types::{ClientHolding, ClientTrade, PortfolioPosition, ScenarioPosition};
use crate::services::portfolio_service::enrich_positions_with_prices;
use crate::state::AppState;
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use chrono_tz::{America::New_York, Tz};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

/// Performance windows, in display order.
pub const PERIOD_KEYS: [&str; 6] = ["1M", "3M", "6M", "YTD", "1Y", "SI"];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PerformancePeriod {
    pub key: &'static str,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub start_value: f64,
    pub end_value: f64,
    /// Money into securities inside the window (buys positive, sells negative).
    pub net_flows: f64,
    pub twr: Option<f64>,
    /// Only for windows of a year or more.
    pub twr_annualized: Option<f64>,
    /// XIRR of the window's flows — annualized by construction.
    pub mwr: Option<f64>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PerformanceReport {
    pub as_of: NaiveDate,
    /// e.g. `Nov 11, 2026 close · 3:45 PM ET`
    pub prices_as_of: String,
    pub periods: Vec<PerformancePeriod>,
    pub warnings: Vec<String>,
}

pub struct PortfolioAnalyticsService;

/// `client_trades` rows as engine events: only buys and sells count, `seq` is
/// the row id, symbols are upper-cased, and a row whose date does not start
/// with `YYYY-MM-DD` is skipped (and logged).
pub fn trade_events_from(trades: &[ClientTrade]) -> Vec<TradeEvent> {
    trades
        .iter()
        .filter_map(|trade| {
            let side = match trade.trade_type.trim().to_ascii_lowercase().as_str() {
                "buy" => TradeSide::Buy,
                "sell" => TradeSide::Sell,
                _ => return None,
            };
            let date_text: String = trade.trade_date.chars().take(10).collect();
            let Ok(date) = NaiveDate::parse_from_str(&date_text, "%Y-%m-%d") else {
                tracing::warn!(
                    "analytics: skipping trade {:?} with unparseable date {:?}",
                    trade.id,
                    trade.trade_date
                );
                return None;
            };
            Some(TradeEvent {
                date,
                seq: trade.id.unwrap_or(0),
                symbol: trade.symbol.trim().to_uppercase(),
                side,
                quantity: trade.quantity.abs(),
                price: trade.price,
                fees: trade.fees,
            })
        })
        .collect()
}

/// `client_holdings` rows that still hold shares, as the engine's opening positions.
pub fn openings_from(holdings: &[ClientHolding]) -> Vec<OpeningPosition> {
    holdings
        .iter()
        .filter(|holding| holding.quantity > 0.0)
        .map(|holding| OpeningPosition {
            symbol: holding.symbol.trim().to_uppercase(),
            quantity: holding.quantity,
            avg_cost: holding.avg_cost,
        })
        .collect()
}

/// Start date of each window ending at `as_of`, in display order; `None` for
/// since-inception. Month arithmetic clamps to the last day of a shorter month.
pub fn period_bounds(as_of: NaiveDate) -> Vec<(&'static str, Option<NaiveDate>)> {
    let back = |months: u32| as_of.checked_sub_months(Months::new(months)).unwrap_or(as_of);
    vec![
        ("1M", Some(back(1))),
        ("3M", Some(back(3))),
        ("6M", Some(back(6))),
        ("YTD", NaiveDate::from_ymd_opt(as_of.year(), 1, 1)),
        ("1Y", Some(back(12))),
        ("SI", None),
    ]
}

/// One row per window. Each window is the series from the last point on or
/// before its start date (so YTD starts at the last close of the prior year)
/// to the last point; when the history is shorter than the window it starts at
/// the first point and says so in `note`.
pub fn build_periods(points: &[ValuationPoint], as_of: NaiveDate) -> Vec<PerformancePeriod> {
    let mut sorted: Vec<&ValuationPoint> = points.iter().collect();
    sorted.sort_by_key(|point| point.date);

    period_bounds(as_of)
        .into_iter()
        .map(|(key, start)| {
            if sorted.len() < 2 {
                return PerformancePeriod {
                    key,
                    start: start.unwrap_or(as_of),
                    end: as_of,
                    start_value: 0.0,
                    end_value: sorted.last().map(|p| p.value).unwrap_or(0.0),
                    net_flows: 0.0,
                    twr: None,
                    twr_annualized: None,
                    mwr: None,
                    note: Some("Not enough price history".to_string()),
                };
            }
            let first = sorted[0];
            let last = sorted[sorted.len() - 1];
            let (start_index, mut note) = match start {
                None => (0, None),
                Some(start) => match sorted.iter().rposition(|point| point.date <= start) {
                    Some(index) => (index, None),
                    None => (
                        0,
                        Some(format!("History begins {}, shorter than this window", first.date)),
                    ),
                },
            };
            if start_index == sorted.len() - 1 {
                note = Some("No price history inside this window".to_string());
            }

            let slice: Vec<ValuationPoint> = sorted[start_index..].iter().map(|p| (*p).clone()).collect();
            let start_point = &slice[0];
            let days = (last.date - start_point.date).num_days();
            let net_flows: f64 = slice[1..].iter().map(|point| point.flow).sum();
            let twr = time_weighted_return(&slice);
            let twr_annualized = twr.and_then(|total| annualize(total, days));

            let mut flows = vec![CashFlow { date: start_point.date, amount: -start_point.value }];
            flows.extend(
                slice[1..]
                    .iter()
                    .filter(|point| point.flow != 0.0)
                    .map(|point| CashFlow { date: point.date, amount: -point.flow }),
            );
            flows.push(CashFlow { date: last.date, amount: last.value });
            let mwr = xirr(&flows);

            PerformancePeriod {
                key,
                start: start_point.date,
                end: last.date,
                start_value: start_point.value,
                end_value: last.value,
                net_flows,
                twr,
                twr_annualized,
                mwr,
                note,
            }
        })
        .collect()
}

/// `Nov 11, 2026 close · 3:45 PM ET` — the last close used and when it was computed.
pub fn prices_as_of_label(last_close: NaiveDate, now_et: DateTime<Tz>) -> String {
    format!(
        "{} close · {} ET",
        last_close.format("%b %-d, %Y"),
        now_et.format("%-I:%M %p")
    )
}

impl PortfolioAnalyticsService {
    fn now_et() -> DateTime<Tz> {
        Utc::now().with_timezone(&New_York)
    }

    /// The FMP client when a key is saved under the provider settings.
    fn fmp_client(state: &AppState) -> Result<Option<FmpClient>> {
        Ok(state
            .sqlite
            .get_provider_key("fmp", &state.security)?
            .filter(|key| !key.trim().is_empty())
            .map(|key| FmpClient::new((*state.http_client).clone(), key)))
    }

    /// Current prices per symbol through the portfolio service's batch quote;
    /// symbols without a quote are absent.
    async fn current_prices(state: &AppState, symbols: impl IntoIterator<Item = String>) -> HashMap<String, f64> {
        let mut positions: Vec<PortfolioPosition> = symbols
            .into_iter()
            .map(|symbol| PortfolioPosition {
                id: None,
                symbol,
                exchange: "US".to_string(),
                quantity: 1.0,
                avg_price: 0.0,
                current_price: None,
                pnl: None,
                pnl_percent: None,
                position_type: "long".to_string(),
                asset_class: None,
                notes: None,
                added_at: None,
                updated_at: None,
            })
            .collect();
        if let Err(err) = enrich_positions_with_prices(&state.http_client, &mut positions).await {
            tracing::warn!("analytics: price lookup failed: {err}");
        }
        positions
            .into_iter()
            .filter_map(|position| {
                position
                    .current_price
                    .filter(|price| *price > 0.0)
                    .map(|price| (position.symbol, price))
            })
            .collect()
    }

    /// Tax lots of a client's trade history with `method`, priced now.
    pub async fn client_lots(state: &AppState, client_id: i64, method: LotMethod) -> Result<LotReport> {
        let trades = trade_events_from(&state.sqlite.get_client_trades(client_id)?);
        let openings = openings_from(&state.sqlite.get_client_holdings(client_id)?);
        let symbols: BTreeSet<String> = trades
            .iter()
            .map(|trade| trade.symbol.clone())
            .chain(openings.iter().map(|opening| opening.symbol.clone()))
            .collect();
        let prices = Self::current_prices(state, symbols).await;
        Ok(build_lots(&trades, &openings, method, Self::now_et().date_naive(), &prices))
    }

    /// Returns over 1M, 3M, 6M, YTD, 1Y and since inception from one FMP
    /// close series (inception = the day before the first trade).
    pub async fn client_performance(state: &AppState, client_id: i64) -> Result<PerformanceReport> {
        let trades = trade_events_from(&state.sqlite.get_client_trades(client_id)?);
        let openings = openings_from(&state.sqlite.get_client_holdings(client_id)?);
        let now = Self::now_et();
        let as_of = now.date_naive();
        if trades.is_empty() && openings.is_empty() {
            return Ok(PerformanceReport {
                as_of,
                prices_as_of: prices_as_of_label(as_of, now),
                periods: Vec::new(),
                warnings: Vec::new(),
            });
        }
        let fmp = Self::fmp_client(state)?.ok_or_else(|| {
            AppError::Config(
                "FMP API key not configured. Add your Financial Modeling Prep key in Settings > Providers to compute performance."
                    .to_string(),
            )
        })?;
        let from = trades
            .iter()
            .map(|trade| trade.date)
            .min()
            .map(|first| first - Duration::days(1))
            .unwrap_or_else(|| as_of - Duration::days(365));
        let series = holdings_value_series(&trades, &openings, from, as_of, &FmpCloses { client: &fmp }).await?;
        let last_close = series.points.last().map(|point| point.date).unwrap_or(as_of);
        Ok(PerformanceReport {
            as_of,
            prices_as_of: prices_as_of_label(last_close, now),
            periods: build_periods(&series.points, as_of),
            warnings: series.warnings,
        })
    }

    /// Each target's drift for a scenario's positions and cash.
    pub async fn scenario_drift(state: &AppState, scenario_id: i64) -> Result<DriftReport> {
        let (holdings, cash, targets, notes) = Self::scenario_inputs(state, scenario_id).await?;
        let mut report = drift(&holdings, cash, &targets);
        report.warnings.extend(notes);
        Ok(report)
    }

    /// Whole-share trades that bring a scenario back to its client's targets.
    pub async fn rebalance_plan(state: &AppState, scenario_id: i64) -> Result<RebalancePlan> {
        let (holdings, cash, targets, notes) = Self::scenario_inputs(state, scenario_id).await?;
        let mut plan = rebalance(&holdings, cash, &targets);
        plan.unresolved.extend(notes);
        Ok(plan)
    }

    /// The scenario's long positions priced and sectored, its cash, the client's
    /// targets, and notes about what was left out.
    async fn scenario_inputs(
        state: &AppState,
        scenario_id: i64,
    ) -> Result<(Vec<Holding>, f64, Vec<AllocationTarget>, Vec<String>)> {
        let scenario = state.sqlite.get_client_scenario_by_id(scenario_id)?;
        let positions = state.sqlite.get_scenario_positions(scenario_id)?;
        let targets = state.sqlite.list_allocation_targets(scenario.client_id)?;

        let mut notes = Vec::new();
        let mut longs: Vec<&ScenarioPosition> = Vec::new();
        for position in &positions {
            if position.side.eq_ignore_ascii_case("short") || position.quantity <= 0.0 {
                notes.push(format!("{}: short position left out of the allocation", position.symbol));
            } else {
                longs.push(position);
            }
        }

        let symbols: BTreeSet<String> = longs.iter().map(|p| p.symbol.trim().to_uppercase()).collect();
        let prices = Self::current_prices(state, symbols.iter().cloned()).await;
        let sectors = match Self::fmp_client(state)? {
            Some(fmp) => Self::sectors(&fmp, &symbols).await,
            None => {
                if targets.iter().any(|target| target.scope == TargetScope::Sector) {
                    notes.push(
                        "FMP API key not configured: sectors are unknown, so sector targets cannot be measured."
                            .to_string(),
                    );
                }
                HashMap::new()
            }
        };

        let holdings = longs
            .iter()
            .map(|position| {
                let symbol = position.symbol.trim().to_uppercase();
                Holding {
                    exchange: position.exchange.clone(),
                    quantity: position.quantity,
                    price: prices.get(&symbol).copied(),
                    sector: sectors.get(&symbol).cloned().flatten(),
                    symbol,
                }
            })
            .collect();

        let cash = match scenario.cash {
            Some(cash) => cash,
            None => {
                notes.push("Cash is not recorded for this scenario; the plan assumes $0.".to_string());
                0.0
            }
        };
        Ok((holdings, cash, targets, notes))
    }

    /// Sector per symbol, looked up concurrently and once per call.
    async fn sectors(fmp: &FmpClient, symbols: &BTreeSet<String>) -> HashMap<String, Option<String>> {
        let lookups = symbols
            .iter()
            .map(|symbol| async move { (symbol.clone(), Self::sector_of(fmp, symbol).await) });
        futures_util::future::join_all(lookups).await.into_iter().collect()
    }

    /// The FMP profile's sector; an ETF (no sector, but listed by etf-info) is "ETF".
    async fn sector_of(fmp: &FmpClient, symbol: &str) -> Option<String> {
        match fmp.get_company_profile(symbol).await {
            Ok(Some(profile)) => {
                if let Some(sector) = profile.sector.filter(|sector| !sector.trim().is_empty()) {
                    return Some(sector);
                }
            }
            Ok(None) => {}
            Err(err) => tracing::warn!("analytics: profile lookup failed for {symbol}: {err}"),
        }
        match fmp.get_etf_info(symbol).await {
            Ok(info) if !info.is_empty() => Some("ETF".to_string()),
            Ok(_) => None,
            Err(err) => {
                tracing::warn!("analytics: etf lookup failed for {symbol}: {err}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::lots::TradeSide;
    use crate::providers::types::{ClientHolding, ClientTrade};
    use chrono::{Duration, NaiveDate, TimeZone};
    use chrono_tz::America::New_York;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn client_trade(id: i64, date: &str, trade_type: &str, qty: f64, price: f64, fees: f64) -> ClientTrade {
        ClientTrade {
            id: Some(id),
            client_id: 1,
            import_batch_id: None,
            symbol: "voo".to_string(),
            exchange: "NYSE".to_string(),
            trade_date: date.to_string(),
            trade_type: trade_type.to_string(),
            quantity: qty,
            price,
            fees,
            order_id: None,
            notes: None,
            created_at: None,
        }
    }

    fn client_holding(symbol: &str, quantity: f64, avg_cost: f64) -> ClientHolding {
        ClientHolding {
            id: Some(1),
            client_id: 1,
            symbol: symbol.to_string(),
            description: None,
            quantity,
            avg_cost,
            total_cost: quantity * avg_cost,
            realized_pnl: 0.0,
            last_activity_date: None,
            updated_at: None,
            current_price: None,
            market_value: None,
            gain_percent: None,
        }
    }

    fn near(a: Option<f64>, b: f64) -> bool {
        a.map(|x| (x - b).abs() < 1e-9).unwrap_or(false)
    }

    #[test]
    fn trade_events_keep_buys_and_sells_only() {
        let trades = vec![
            client_trade(7, "2026-01-05", "BUY", 10.0, 100.0, 1.0),
            client_trade(8, "2026-01-06", "dividend", 0.0, 12.5, 0.0),
            client_trade(9, "2026-01-07", "Sell", 5.0, 130.0, 1.0),
            client_trade(10, "2026-01-08", "interest", 0.0, 0.4, 0.0),
        ];
        let events = trade_events_from(&trades);

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            TradeEvent {
                date: d(2026, 1, 5),
                seq: 7,
                symbol: "VOO".to_string(),
                side: TradeSide::Buy,
                quantity: 10.0,
                price: 100.0,
                fees: 1.0,
            }
        );
        assert_eq!(events[1].side, TradeSide::Sell);
        assert_eq!(events[1].seq, 9);
        assert_eq!(events[1].date, d(2026, 1, 7));
    }

    #[test]
    fn trade_events_accept_timestamps_and_skip_unparseable_dates() {
        let trades = vec![
            client_trade(1, "2026-01-05 09:30:00", "buy", 1.0, 10.0, 0.0),
            client_trade(2, "01/06/2026", "buy", 1.0, 10.0, 0.0),
        ];
        let events = trade_events_from(&trades);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].date, d(2026, 1, 5));
    }

    #[test]
    fn openings_keep_positive_holdings_with_upper_case_symbols() {
        let holdings = vec![client_holding("voo", 5.0, 600.0), client_holding("SOLD", 0.0, 10.0)];
        let openings = openings_from(&holdings);
        assert_eq!(
            openings,
            vec![OpeningPosition { symbol: "VOO".to_string(), quantity: 5.0, avg_cost: 600.0 }]
        );
    }

    #[test]
    fn period_bounds_give_the_start_of_each_window() {
        let bounds: std::collections::HashMap<&str, Option<NaiveDate>> =
            period_bounds(d(2026, 11, 12)).into_iter().collect();
        assert_eq!(bounds["1M"], Some(d(2026, 10, 12)));
        assert_eq!(bounds["3M"], Some(d(2026, 8, 12)));
        assert_eq!(bounds["6M"], Some(d(2026, 5, 12)));
        assert_eq!(bounds["YTD"], Some(d(2026, 1, 1)));
        assert_eq!(bounds["1Y"], Some(d(2025, 11, 12)));
        assert_eq!(bounds["SI"], None);
        assert_eq!(
            period_bounds(d(2026, 11, 12)).iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            vec!["1M", "3M", "6M", "YTD", "1Y", "SI"]
        );
        // Month arithmetic clamps to the last day of a shorter month.
        let march: std::collections::HashMap<&str, Option<NaiveDate>> =
            period_bounds(d(2026, 3, 31)).into_iter().collect();
        assert_eq!(march["1M"], Some(d(2026, 2, 28)));
    }

    /// Daily points 2025-10-01 … 2026-11-12: flat at 1,000; a 100 buy on
    /// 2026-06-01 (value 1,100, flow 100); +10% on the last day (1,210).
    fn series() -> Vec<ValuationPoint> {
        let first = d(2025, 10, 1);
        let last = d(2026, 11, 12);
        let buy_day = d(2026, 6, 1);
        let mut points = Vec::new();
        let mut date = first;
        while date <= last {
            let (value, flow) = if date < buy_day {
                (1000.0, 0.0)
            } else if date == buy_day {
                (1100.0, 100.0)
            } else if date < last {
                (1100.0, 0.0)
            } else {
                (1210.0, 0.0)
            };
            points.push(ValuationPoint { date, value, flow });
            date += Duration::days(1);
        }
        points
    }

    #[test]
    fn build_periods_slices_the_series_per_window() {
        let periods = build_periods(&series(), d(2026, 11, 12));
        let by_key: std::collections::HashMap<&str, &PerformancePeriod> =
            periods.iter().map(|p| (p.key, p)).collect();
        assert_eq!(periods.iter().map(|p| p.key).collect::<Vec<_>>(), vec!["1M", "3M", "6M", "YTD", "1Y", "SI"]);

        let one_month = by_key["1M"];
        assert_eq!(one_month.start, d(2026, 10, 12));
        assert_eq!(one_month.end, d(2026, 11, 12));
        assert!((one_month.start_value - 1100.0).abs() < 1e-9);
        assert!((one_month.end_value - 1210.0).abs() < 1e-9);
        assert!(near(one_month.twr, 0.10), "{:?}", one_month.twr);
        assert_eq!(one_month.twr_annualized, None);
        assert!((one_month.net_flows).abs() < 1e-9);
        assert_eq!(one_month.note, None);

        let three_months = by_key["3M"];
        assert!(three_months.net_flows.abs() < 1e-9);

        let six_months = by_key["6M"];
        assert_eq!(six_months.start, d(2026, 5, 12));
        assert!((six_months.net_flows - 100.0).abs() < 1e-9);
        assert!(near(six_months.twr, 0.10), "{:?}", six_months.twr);

        let ytd = by_key["YTD"];
        assert_eq!(ytd.start, d(2026, 1, 1));
        assert!(near(ytd.twr, 0.10));

        let one_year = by_key["1Y"];
        assert_eq!(one_year.start, d(2025, 11, 12));
        assert!(near(one_year.twr, 0.10));
        assert!(near(one_year.twr_annualized, 0.10), "{:?}", one_year.twr_annualized);

        let since_inception = by_key["SI"];
        assert_eq!(since_inception.start, d(2025, 10, 1));
        assert!((since_inception.start_value - 1000.0).abs() < 1e-9);
        assert!((since_inception.net_flows - 100.0).abs() < 1e-9);
        assert!(near(since_inception.twr, 0.10));
        let annualized = since_inception.twr_annualized.expect("407 days annualizes");
        assert!((annualized - (1.1f64.powf(365.0 / 407.0) - 1.0)).abs() < 1e-9);
        let mwr = since_inception.mwr.expect("a money-weighted rate");
        assert!(mwr > 0.0 && mwr < 0.2, "mwr {mwr}");
    }

    #[test]
    fn build_periods_notes_when_the_history_is_shorter_than_the_window() {
        let short: Vec<ValuationPoint> = series().into_iter().filter(|p| p.date >= d(2026, 6, 1)).collect();
        let periods = build_periods(&short, d(2026, 11, 12));
        let by_key: std::collections::HashMap<&str, &PerformancePeriod> =
            periods.iter().map(|p| (p.key, p)).collect();

        let one_year = by_key["1Y"];
        assert_eq!(one_year.start, d(2026, 6, 1));
        assert!(one_year.note.as_deref().unwrap_or("").contains("2026-06-01"), "{:?}", one_year.note);
        assert!(by_key["YTD"].note.is_some());
        assert_eq!(by_key["1M"].note, None);
        assert_eq!(by_key["SI"].note, None);
    }

    #[test]
    fn build_periods_without_enough_points_explains_itself() {
        let one = vec![ValuationPoint { date: d(2026, 11, 12), value: 1000.0, flow: 0.0 }];
        for points in [Vec::new(), one] {
            let periods = build_periods(&points, d(2026, 11, 12));
            assert_eq!(periods.len(), 6);
            assert!(periods.iter().all(|p| p.twr.is_none() && p.mwr.is_none()));
            assert!(periods.iter().all(|p| p.note.as_deref() == Some("Not enough price history")));
        }
    }

    #[test]
    fn prices_as_of_label_is_eastern_12_hour() {
        let now = New_York.with_ymd_and_hms(2026, 11, 12, 15, 45, 0).unwrap();
        assert_eq!(prices_as_of_label(d(2026, 11, 11), now), "Nov 11, 2026 close · 3:45 PM ET");
        let morning = New_York.with_ymd_and_hms(2026, 11, 12, 9, 5, 0).unwrap();
        assert_eq!(prices_as_of_label(d(2026, 11, 12), morning), "Nov 12, 2026 close · 9:05 AM ET");
    }
}
