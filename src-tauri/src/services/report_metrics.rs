//! Trailing-window performance metrics for the comparison report: the
//! figures a client sees in the Metrics movement (CAGR, volatility, Sharpe,
//! Sortino, drawdown, Calmar, best and worst month, VaR), the monthly-returns
//! grid, the drawdown episodes and a seeded bootstrap Monte Carlo.
//!
//! Pure functions over a daily value series (dates ascending, one value per
//! date). The math that `quant_service` already has is reused; nothing here
//! does I/O.

use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use futures_util::{stream, StreamExt};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::providers::fmp::FmpClient;
use crate::providers::types::DailyClose;
use crate::services::quant_service::{
    annualized_return, annualized_volatility, calmar_ratio, compute_returns, conditional_var,
    max_drawdown, sharpe_ratio, sortino_ratio, value_at_risk,
};

/// Fewest daily values a window needs before any metric is reported; below
/// it every row is `None` and the Monte Carlo is skipped.
pub const MIN_DAYS: usize = 30;

/// How a metric's value is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricFormat {
    /// A fraction printed as a percentage: 0.123 -> "12.3%".
    Pct,
    /// A dimensionless ratio printed to two decimals: 1.234 -> "1.23".
    Ratio,
}

/// One line of the metrics table. `value` is `None` when the metric cannot be
/// computed (short history, or a ratio whose denominator is zero).
#[derive(Debug, Clone, PartialEq)]
pub struct MetricRow {
    pub label: String,
    pub value: Option<f64>,
    pub format: MetricFormat,
}

/// A log return as the simple return a reader expects: ln(1.01) -> 0.01.
fn simple(log_return: f64) -> f64 {
    log_return.exp() - 1.0
}

/// The ten metric rows for a daily value series, in table order. Ratios are
/// computed with a 0% risk-free rate. All values are `None` below `MIN_DAYS`;
/// Sharpe is `None` without volatility, Sortino without a down day and Calmar
/// without a drawdown, since each is undefined rather than zero then.
pub fn metric_rows(dates: &[NaiveDate], values: &[f64]) -> Vec<MetricRow> {
    let enough = values.len() >= MIN_DAYS && dates.len() == values.len();
    let returns = if enough { compute_returns(values) } else { Vec::new() };
    let some = |v: f64| if enough { Some(v) } else { None };

    let (mdd, _, _) = max_drawdown(values);
    let vol = annualized_volatility(&returns);
    let months: Vec<f64> = if enough {
        monthly_returns(dates, values).iter().flat_map(|r| r.months.iter().flatten().copied()).collect()
    } else {
        Vec::new()
    };
    let best = months.iter().copied().reduce(f64::max);
    let worst = months.iter().copied().reduce(f64::min);

    let row = |label: &str, value: Option<f64>, format: MetricFormat| MetricRow {
        label: label.to_string(),
        value,
        format,
    };
    vec![
        row("CAGR", some(annualized_return(&returns)), MetricFormat::Pct),
        row("Volatility (annualized)", some(vol), MetricFormat::Pct),
        row("Sharpe ratio (rf 0%)", some(sharpe_ratio(&returns, 0.0)).filter(|_| vol > 0.0), MetricFormat::Ratio),
        row(
            "Sortino ratio (rf 0%)",
            some(sortino_ratio(&returns, 0.0)).filter(|_| returns.iter().any(|r| *r < 0.0)),
            MetricFormat::Ratio,
        ),
        row("Max drawdown", some(if mdd > 0.0 { -mdd } else { 0.0 }), MetricFormat::Pct),
        row("Calmar ratio", some(calmar_ratio(&returns, values)).filter(|_| mdd > 0.0), MetricFormat::Ratio),
        row("Best month", best, MetricFormat::Pct),
        row("Worst month", worst, MetricFormat::Pct),
        row("VaR 95% (daily)", some(simple(-value_at_risk(&returns, 0.95))), MetricFormat::Pct),
        row("CVaR 95% (daily)", some(simple(-conditional_var(&returns, 0.95))), MetricFormat::Pct),
    ]
}

/// One calendar year of the monthly-returns grid: `months[0]` is January.
/// A month is `None` when the window holds no return inside it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonthlyRow {
    pub year: i32,
    pub months: [Option<f64>; 12],
    /// The year's months present, compounded.
    pub ytd: Option<f64>,
}

/// Compound a set of period returns: [0.10, 0.10] -> 0.21.
fn compound<'a>(returns: impl Iterator<Item = &'a f64>) -> Option<f64> {
    let mut any = false;
    let growth = returns.fold(1.0, |acc, r| {
        any = true;
        acc * (1.0 + r)
    });
    if any { Some(growth - 1.0) } else { None }
}

/// Each calendar month's return, compounded from the daily values inside it
/// (the month's last value over the previous month's last value). The first
/// month starts from the window's first value, so it covers the days present.
pub fn monthly_returns(dates: &[NaiveDate], values: &[f64]) -> Vec<MonthlyRow> {
    let n = dates.len().min(values.len());
    let mut rows: Vec<MonthlyRow> = Vec::new();
    let mut i = 0;
    while i < n {
        let (year, month) = (dates[i].year(), dates[i].month0() as usize);
        let mut j = i;
        while j + 1 < n && dates[j + 1].year() == year && dates[j + 1].month0() as usize == month {
            j += 1;
        }
        let base = if i == 0 { values[0] } else { values[i - 1] };
        let ret = if i == 0 && j == 0 {
            None
        } else if base > 0.0 {
            Some(values[j] / base - 1.0)
        } else {
            None
        };
        let row = match rows.iter_mut().find(|r| r.year == year) {
            Some(r) => r,
            None => {
                rows.push(MonthlyRow { year, months: [None; 12], ytd: None });
                rows.last_mut().expect("just pushed")
            }
        };
        row.months[month] = ret;
        i = j + 1;
    }
    for row in &mut rows {
        row.ytd = compound(row.months.iter().flatten());
    }
    rows
}

/// A peak-to-recovery drawdown episode. `end` is `None` while the series has
/// not climbed back to the peak; `days` then counts to the window's last date.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DrawdownPeriod {
    pub start: NaiveDate,
    pub trough: NaiveDate,
    pub end: Option<NaiveDate>,
    /// Trough over peak minus one: -0.20 for a 20% fall.
    pub depth_pct: f64,
    pub days: i64,
}

/// The `top` deepest drawdown episodes, deepest first.
pub fn drawdown_periods(dates: &[NaiveDate], values: &[f64], top: usize) -> Vec<DrawdownPeriod> {
    let n = dates.len().min(values.len());
    let mut out = Vec::new();
    if n == 0 {
        return out;
    }
    let period = |peak_i: usize, trough_i: usize, end_i: Option<usize>| DrawdownPeriod {
        start: dates[peak_i],
        trough: dates[trough_i],
        end: end_i.map(|e| dates[e]),
        depth_pct: values[trough_i] / values[peak_i] - 1.0,
        days: (end_i.map_or(dates[n - 1], |e| dates[e]) - dates[peak_i]).num_days(),
    };
    let mut peak_i = 0;
    // An open episode: (peak index, trough index).
    let mut open: Option<(usize, usize)> = None;
    for i in 1..n {
        let v = values[i];
        match open {
            None if v >= values[peak_i] => peak_i = i,
            None => open = Some((peak_i, i)),
            Some((p, t)) if v >= values[p] => {
                out.push(period(p, t, Some(i)));
                open = None;
                peak_i = i;
            }
            Some((p, t)) if v < values[t] => open = Some((p, i)),
            Some(_) => {}
        }
    }
    if let Some((p, t)) = open {
        out.push(period(p, t, None));
    }
    out.sort_by(|a, b| a.depth_pct.total_cmp(&b.depth_pct));
    out.truncate(top);
    out
}

/// Terminal-return percentiles and odds from bootstrapped paths.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonteCarlo {
    pub p5: f64,
    pub p50: f64,
    pub p95: f64,
    /// Share of paths whose drawdown reached the bust threshold.
    pub bust_probability: f64,
    /// Share of paths that ended at or above the goal.
    pub goal_probability: f64,
}

/// Linear interpolation between the closest ranks of an ascending slice.
fn percentile(sorted: &[f64], q: f64) -> f64 {
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// `sims` paths of `horizon` days, each day a draw with replacement from
/// `daily_returns` (daily log returns, as `compute_returns` gives them). The
/// same `seed` reproduces the same paths, so a re-run prints the same odds.
/// `bust` is a drawdown fraction (-0.20) and `goal` a terminal return (0.10).
pub fn monte_carlo(
    daily_returns: &[f64],
    sims: usize,
    horizon: usize,
    seed: u64,
    bust: f64,
    goal: f64,
) -> Option<MonteCarlo> {
    if daily_returns.len() < MIN_DAYS || sims == 0 || horizon == 0 {
        return None;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut terminals = Vec::with_capacity(sims);
    let (mut busts, mut goals) = (0usize, 0usize);
    for _ in 0..sims {
        let (mut value, mut peak, mut worst) = (1.0_f64, 1.0_f64, 0.0_f64);
        for _ in 0..horizon {
            value *= daily_returns[rng.gen_range(0..daily_returns.len())].exp();
            peak = peak.max(value);
            worst = worst.min(value / peak - 1.0);
        }
        let terminal = value - 1.0;
        busts += usize::from(worst <= bust);
        goals += usize::from(terminal >= goal);
        terminals.push(terminal);
    }
    terminals.sort_by(f64::total_cmp);
    Some(MonteCarlo {
        p5: percentile(&terminals, 0.05),
        p50: percentile(&terminals, 0.50),
        p95: percentile(&terminals, 0.95),
        bust_probability: busts as f64 / sims as f64,
        goal_probability: goals as f64 / sims as f64,
    })
}

// ---------------------------------------------------------------------------
// The hypothetical series: today's positions held through the window
// ---------------------------------------------------------------------------

/// A portfolio's daily values over the window, built from its positions and
/// each symbol's closes.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesBuild {
    pub dates: Vec<NaiveDate>,
    pub values: Vec<f64>,
    /// Positions with no closes at all, left out of `values` (once each, in
    /// position order), so the report can name them.
    pub skipped: Vec<String>,
}

/// Value the positions day by day at their closes. The date axis is the union
/// of the symbols' dates; a symbol without a close on a day keeps its last one
/// (forward fill). The axis opens on the first day every priced symbol has a
/// close, since a holding entering later would read as a jump in value.
pub fn hypothetical_series(
    positions: &[(String, f64)],
    closes: &HashMap<String, Vec<(NaiveDate, f64)>>,
) -> SeriesBuild {
    let mut skipped: Vec<String> = Vec::new();
    let mut held: Vec<(f64, Vec<(NaiveDate, f64)>)> = Vec::new();
    for (symbol, qty) in positions {
        if *qty == 0.0 {
            continue;
        }
        match closes.get(symbol).filter(|c| !c.is_empty()) {
            Some(c) => {
                let mut c = c.clone();
                c.sort_by_key(|(d, _)| *d);
                held.push((*qty, c));
            }
            None if skipped.contains(symbol) => {}
            None => skipped.push(symbol.clone()),
        }
    }
    let Some(start) = held.iter().map(|(_, c)| c[0].0).max() else {
        return SeriesBuild { dates: Vec::new(), values: Vec::new(), skipped };
    };
    let dates: Vec<NaiveDate> = held
        .iter()
        .flat_map(|(_, c)| c.iter().map(|(d, _)| *d))
        .filter(|d| *d >= start)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut values = vec![0.0; dates.len()];
    for (qty, c) in &held {
        let (mut i, mut last) = (0, c[0].1);
        for (k, date) in dates.iter().enumerate() {
            while i < c.len() && c[i].0 <= *date {
                last = c[i].1;
                i += 1;
            }
            values[k] += qty * last;
        }
    }
    SeriesBuild { dates, values, skipped }
}

/// FMP's daily rows as dated closes, ascending. A row with an unreadable date
/// or a non-positive close is dropped.
pub fn closes_from_daily(rows: Vec<DailyClose>) -> Vec<(NaiveDate, f64)> {
    let mut out: Vec<(NaiveDate, f64)> = rows
        .into_iter()
        .filter(|r| r.close > 0.0)
        .filter_map(|r| NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").ok().map(|d| (d, r.close)))
        .collect();
    out.sort_by_key(|(d, _)| *d);
    out
}

/// Symbols fetched at once; the FMP client paces the per-minute budget itself.
const FETCH_CONCURRENCY: usize = 8;

/// Daily closes for every symbol from `from` to `to` inclusive. A symbol whose
/// fetch fails or returns nothing is simply absent from the map; when every
/// symbol fails, the provider itself is unavailable and the result is `Err`.
pub async fn fetch_closes(
    fmp: &FmpClient,
    symbols: &[String],
    from: NaiveDate,
    to: NaiveDate,
) -> Result<HashMap<String, Vec<(NaiveDate, f64)>>> {
    let (from, to) = (from.format("%Y-%m-%d").to_string(), to.format("%Y-%m-%d").to_string());
    let results: Vec<_> = stream::iter(symbols.iter().cloned())
        .map(|symbol| {
            let (from, to) = (from.clone(), to.clone());
            async move {
                let fetched = fmp.get_daily_closes(&symbol, &from, &to).await;
                (symbol, fetched)
            }
        })
        .buffer_unordered(FETCH_CONCURRENCY)
        .collect()
        .await;
    let mut out = HashMap::new();
    let (mut failures, mut last_error) = (0usize, String::new());
    for (symbol, fetched) in results {
        match fetched {
            Ok(rows) => {
                let points = closes_from_daily(rows);
                if !points.is_empty() {
                    out.insert(symbol, points);
                }
            }
            Err(e) => {
                tracing::warn!("comparison report: daily closes for {} failed: {}", symbol, e);
                failures += 1;
                last_error = e.to_string();
            }
        }
    }
    if !symbols.is_empty() && failures == symbols.len() {
        return Err(AppError::Provider(format!("Price history could not be fetched: {last_error}")));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn day(n: usize) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 2).unwrap() + chrono::Duration::days(n as i64)
    }

    fn days(n: usize) -> Vec<NaiveDate> {
        (0..n).map(day).collect()
    }

    /// Rises every day by about 1% (the gains vary, so the series has volatility).
    fn rising(n: usize) -> Vec<f64> {
        let mut v = vec![100.0];
        for i in 1..n {
            let step = if i % 2 == 0 { 0.012 } else { 0.008 };
            v.push(v[i - 1] * (1.0 + step));
        }
        v
    }

    fn get(rows: &[MetricRow], label: &str) -> Option<f64> {
        rows.iter().find(|r| r.label == label).unwrap_or_else(|| panic!("no row {label}")).value
    }

    #[test]
    fn a_rising_series_has_positive_cagr_no_drawdown_and_positive_sharpe() {
        let values = rising(60);
        let rows = metric_rows(&days(60), &values);
        assert!(get(&rows, "CAGR").unwrap() > 0.0);
        assert_eq!(get(&rows, "Max drawdown"), Some(0.0));
        assert!(get(&rows, "Sharpe ratio (rf 0%)").unwrap() > 0.0);
        assert_eq!(rows.len(), 10);
    }

    #[test]
    fn short_series_yields_n_a() {
        let values = rising(29);
        let rows = metric_rows(&days(29), &values);
        assert_eq!(rows.len(), 10);
        assert!(rows.iter().all(|r| r.value.is_none()), "{rows:?}");
        let returns = compute_returns(&values);
        assert!(monte_carlo(&returns, 1000, 252, 20261112, -0.20, 0.10).is_none());
        assert!(monte_carlo(&[0.001; 29], 1000, 252, 20261112, -0.20, 0.10).is_none());
    }

    #[test]
    fn monthly_returns_compound_within_the_calendar_month() {
        // Jan 31 closes at 100; February climbs to 110 by the 28th.
        let mut dates = vec![NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()];
        let mut values = vec![100.0];
        for d in 1..=28 {
            dates.push(NaiveDate::from_ymd_opt(2026, 2, d).unwrap());
            values.push(100.0 + 10.0 * d as f64 / 28.0);
        }
        let rows = monthly_returns(&dates, &values);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].year, 2026);
        assert!(rows[0].months[0].is_none(), "January has no return inside the window");
        assert!((rows[0].months[1].unwrap() - 0.10).abs() < 1e-9);
        assert!((rows[0].ytd.unwrap() - 0.10).abs() < 1e-9);
        assert!(rows[0].months[2..].iter().all(Option::is_none));
    }

    #[test]
    fn drawdown_periods_run_from_peak_to_recovery() {
        let values = [100.0, 90.0, 80.0, 100.0, 95.0];
        let periods = drawdown_periods(&days(5), &values, 5);
        assert_eq!(periods.len(), 2);
        let first = &periods[0];
        assert!((first.depth_pct - (-0.20)).abs() < 1e-12);
        assert_eq!(first.start, day(0));
        assert_eq!(first.trough, day(2));
        assert_eq!(first.end, Some(day(3)));
        assert_eq!(first.days, 3);
        let second = &periods[1];
        assert!((second.depth_pct - (-0.05)).abs() < 1e-12);
        assert_eq!(second.end, None);
        assert_eq!(second.days, 1);
    }

    #[test]
    fn drawdown_periods_keep_only_the_deepest() {
        let values = [100.0, 99.0, 100.0, 50.0, 100.0, 90.0, 100.0];
        let periods = drawdown_periods(&days(7), &values, 2);
        let depths: Vec<f64> = periods.iter().map(|p| p.depth_pct).collect();
        assert_eq!(depths.len(), 2);
        assert!((depths[0] - (-0.5)).abs() < 1e-12 && (depths[1] - (-0.1)).abs() < 1e-12, "{depths:?}");
    }

    #[test]
    fn monte_carlo_is_deterministic_and_bounded() {
        let returns: Vec<f64> = (0..60).map(|i| if i % 3 == 0 { -0.012 } else { 0.009 }).collect();
        let a = monte_carlo(&returns, 1000, 252, 20261112, -0.20, 0.10).unwrap();
        let b = monte_carlo(&returns, 1000, 252, 20261112, -0.20, 0.10).unwrap();
        assert_eq!(a, b);
        for p in [a.bust_probability, a.goal_probability] {
            assert!((0.0..=1.0).contains(&p), "{p}");
        }
        assert!(a.p5 <= a.p50 && a.p50 <= a.p95, "{a:?}");
        let other = monte_carlo(&returns, 1000, 252, 7, -0.20, 0.10).unwrap();
        assert_ne!(a, other, "a different seed draws different paths");
    }

    #[test]
    fn all_positive_returns_never_bust() {
        let returns = [0.005; 40];
        let mc = monte_carlo(&returns, 500, 252, 20261112, -0.20, 0.10).unwrap();
        assert_eq!(mc.bust_probability, 0.0);
        assert_eq!(mc.goal_probability, 1.0);
    }

    fn closes(rows: &[(&str, &[(usize, f64)])]) -> HashMap<String, Vec<(NaiveDate, f64)>> {
        rows.iter()
            .map(|(symbol, points)| (symbol.to_string(), points.iter().map(|(d, c)| (day(*d), *c)).collect()))
            .collect()
    }

    #[test]
    fn hypothetical_series_forward_fills_a_missing_day() {
        // BBB has no close on day 1 (a holiday on its exchange); its day-0 close carries over.
        let positions = [("AAA".to_string(), 1.0), ("BBB".to_string(), 2.0)];
        let closes = closes(&[("AAA", &[(0, 10.0), (1, 11.0), (2, 12.0)]), ("BBB", &[(0, 20.0), (2, 22.0)])]);
        let built = hypothetical_series(&positions, &closes);
        assert_eq!(built.dates, days(3));
        assert_eq!(built.values, vec![50.0, 51.0, 56.0]);
        assert!(built.skipped.is_empty());
    }

    #[test]
    fn unpriced_symbol_is_noted() {
        let positions = [("AAA".to_string(), 1.0), ("ZZZ".to_string(), 3.0)];
        let closes = closes(&[("AAA", &[(0, 10.0), (1, 11.0)])]);
        let built = hypothetical_series(&positions, &closes);
        assert_eq!(built.skipped, vec!["ZZZ".to_string()]);
        assert_eq!(built.values, vec![10.0, 11.0]);
    }

    #[test]
    fn the_series_starts_when_every_priced_holding_has_a_close() {
        // BBB only started trading on day 2: measuring from day 0 would show a
        // jump when it appears, so the window starts on day 2.
        let positions = [("AAA".to_string(), 1.0), ("BBB".to_string(), 1.0)];
        let closes = closes(&[("AAA", &[(0, 10.0), (1, 11.0), (2, 12.0), (3, 13.0)]), ("BBB", &[(2, 5.0), (3, 6.0)])]);
        let built = hypothetical_series(&positions, &closes);
        assert_eq!(built.dates, vec![day(2), day(3)]);
        assert_eq!(built.values, vec![17.0, 19.0]);
    }

    #[test]
    fn daily_closes_parse_dates_and_drop_bad_rows() {
        let rows = vec![
            DailyClose { date: "2026-03-03".to_string(), close: 11.0, volume: None },
            DailyClose { date: "2026-03-02".to_string(), close: 10.0, volume: Some(1) },
            DailyClose { date: "not a date".to_string(), close: 12.0, volume: None },
            DailyClose { date: "2026-03-04".to_string(), close: 0.0, volume: None },
        ];
        assert_eq!(closes_from_daily(rows), vec![(day(0), 10.0), (day(1), 11.0)]);
    }
}
