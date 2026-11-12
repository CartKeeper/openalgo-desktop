//! Returns: time-weighted (TWR), annualization, money-weighted (XIRR) and Modified Dietz.
//!
//! Conventions (spec thresholds table): TWR with end-of-day flows,
//! `r_t = (V_t − F_t) / V_{t−1} − 1`; XIRR and annualization on actual/365.

use chrono::NaiveDate;
use serde::Serialize;

/// Day count for annualization and XIRR (actual/365).
pub const DAYS_PER_YEAR: f64 = 365.0;

const EPS: f64 = 1e-9;

/// The portfolio's value at the end of `date` and the money that moved into it
/// that day: `flow > 0` is money into securities (buys incl. fees), `flow < 0`
/// is money out (sell proceeds net of fees).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ValuationPoint {
    pub date: NaiveDate,
    pub value: f64,
    pub flow: f64,
}

/// A dated amount in the investor's view: buys negative, sells and dividends
/// positive, the terminal value positive.
#[derive(Clone, Debug, PartialEq)]
pub struct CashFlow {
    pub date: NaiveDate,
    pub amount: f64,
}

/// Time-weighted return over `points`: `Π(1 + r_t) − 1` with
/// `r_t = (V_t − F_t) / V_{t−1} − 1`. When yesterday's value was zero the day's
/// flow is the base (`r_t = V_t / F_t − 1`); with neither, the day is skipped.
/// `None` when no day could be computed.
pub fn time_weighted_return(points: &[ValuationPoint]) -> Option<f64> {
    let mut sorted: Vec<&ValuationPoint> = points.iter().collect();
    sorted.sort_by_key(|point| point.date);

    let mut growth = 1.0;
    let mut computed = false;
    for pair in sorted.windows(2) {
        let (previous, current) = (pair[0], pair[1]);
        let daily = if previous.value > EPS {
            (current.value - current.flow) / previous.value - 1.0
        } else if current.flow > EPS {
            current.value / current.flow - 1.0
        } else {
            continue;
        };
        growth *= 1.0 + daily;
        computed = true;
    }
    computed.then_some(growth - 1.0)
}

/// `(1 + R)^(365 / days) − 1`; `None` for periods shorter than a year, where an
/// annualized figure would overstate.
pub fn annualize(total_return: f64, days: i64) -> Option<f64> {
    if days < DAYS_PER_YEAR as i64 {
        return None;
    }
    let base = 1.0 + total_return;
    if base <= 0.0 {
        // Everything was lost: −100% a year is as low as it goes.
        return Some(-1.0);
    }
    Some(base.powf(DAYS_PER_YEAR / days as f64) - 1.0)
}

/// Internal rate of return of dated flows (actual/365). Newton from 10%
/// (≤ 100 iterations, tolerance 1e-7), then bisection on [−0.99, 10] when
/// Newton leaves the domain or fails to settle. `None` without both an inflow
/// and an outflow, or when no root lies in the bracket.
pub fn xirr(flows: &[CashFlow]) -> Option<f64> {
    let mut sorted: Vec<&CashFlow> = flows.iter().filter(|flow| flow.amount != 0.0).collect();
    sorted.sort_by_key(|flow| flow.date);
    let has_inflow = sorted.iter().any(|flow| flow.amount > 0.0);
    let has_outflow = sorted.iter().any(|flow| flow.amount < 0.0);
    if !(has_inflow && has_outflow) {
        return None;
    }

    let origin = sorted[0].date;
    let terms: Vec<(f64, f64)> = sorted
        .iter()
        .map(|flow| ((flow.date - origin).num_days() as f64 / DAYS_PER_YEAR, flow.amount))
        .collect();
    let npv = |rate: f64| -> f64 {
        terms.iter().map(|(t, amount)| amount / (1.0 + rate).powf(*t)).sum()
    };
    let slope = |rate: f64| -> f64 {
        terms
            .iter()
            .map(|(t, amount)| -t * amount / (1.0 + rate).powf(t + 1.0))
            .sum()
    };

    let mut rate = 0.1;
    for _ in 0..100 {
        let value = npv(rate);
        let derivative = slope(rate);
        if !value.is_finite() || !derivative.is_finite() || derivative.abs() < 1e-12 {
            break;
        }
        let next = rate - value / derivative;
        if !next.is_finite() || next <= -1.0 {
            break;
        }
        if (next - rate).abs() < 1e-7 {
            return Some(next);
        }
        rate = next;
    }

    let (mut low, mut high) = (-0.99_f64, 10.0_f64);
    let mut at_low = npv(low);
    let at_high = npv(high);
    if !at_low.is_finite() || !at_high.is_finite() || at_low.signum() == at_high.signum() {
        return None;
    }
    for _ in 0..200 {
        let mid = 0.5 * (low + high);
        let at_mid = npv(mid);
        if at_mid.abs() < 1e-10 || high - low < 1e-9 {
            return Some(mid);
        }
        if at_mid.signum() == at_low.signum() {
            low = mid;
            at_low = at_mid;
        } else {
            high = mid;
        }
    }
    Some(0.5 * (low + high))
}

/// Modified Dietz return: `(end − start − ΣF) / (start + Σ w_i·F_i)` with each
/// contribution weighted by the share of the period still ahead of it.
/// `flows` are in the investor's view (a buy is negative), so a contribution
/// into the portfolio is `−amount`. `None` when the period has no length or the
/// weighted capital is zero.
pub fn modified_dietz(
    start_value: f64,
    end_value: f64,
    flows: &[CashFlow],
    start: NaiveDate,
    end: NaiveDate,
) -> Option<f64> {
    let total_days = (end - start).num_days();
    if total_days <= 0 {
        return None;
    }
    let mut net_contribution = 0.0;
    let mut weighted_contribution = 0.0;
    for flow in flows {
        let contribution = -flow.amount;
        let days_ahead = (end - flow.date).num_days().clamp(0, total_days);
        let weight = days_ahead as f64 / total_days as f64;
        net_contribution += contribution;
        weighted_contribution += weight * contribution;
    }
    let capital = start_value + weighted_contribution;
    if capital.abs() < EPS {
        return None;
    }
    Some((end_value - start_value - net_contribution) / capital)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn point(date: NaiveDate, value: f64, flow: f64) -> ValuationPoint {
        ValuationPoint { date, value, flow }
    }

    fn near(a: Option<f64>, b: f64, tol: f64) -> bool {
        a.map(|x| (x - b).abs() < tol).unwrap_or(false)
    }

    #[test]
    fn twr_chains_daily_returns_net_of_flows() {
        let points = vec![
            point(d(2026, 1, 1), 1000.0, 0.0),
            point(d(2026, 1, 2), 1100.0, 0.0),
            point(d(2026, 1, 3), 1650.0, 500.0),
        ];
        let twr = time_weighted_return(&points);
        assert!(near(twr, 0.15, 1e-9), "twr {:?}", twr);
    }

    #[test]
    fn twr_starting_from_zero_uses_the_first_flow_as_the_base() {
        let points = vec![
            point(d(2026, 1, 1), 0.0, 0.0),
            point(d(2026, 1, 2), 500.0, 500.0),
            point(d(2026, 1, 3), 550.0, 0.0),
        ];
        let twr = time_weighted_return(&points);
        assert!(near(twr, 0.10, 1e-9), "twr {:?}", twr);
    }

    #[test]
    fn twr_skips_days_with_no_base_and_no_flow() {
        let points = vec![
            point(d(2026, 1, 1), 0.0, 0.0),
            point(d(2026, 1, 2), 0.0, 0.0),
            point(d(2026, 1, 3), 200.0, 200.0),
            point(d(2026, 1, 4), 220.0, 0.0),
        ];
        let twr = time_weighted_return(&points);
        assert!(near(twr, 0.10, 1e-9), "twr {:?}", twr);
    }

    #[test]
    fn twr_is_none_without_a_computable_day() {
        assert_eq!(time_weighted_return(&[]), None);
        assert_eq!(time_weighted_return(&[point(d(2026, 1, 1), 1000.0, 0.0)]), None);
        let zeros = vec![point(d(2026, 1, 1), 0.0, 0.0), point(d(2026, 1, 2), 0.0, 0.0)];
        assert_eq!(time_weighted_return(&zeros), None);
    }

    #[test]
    fn twr_sorts_points_by_date() {
        let points = vec![
            point(d(2026, 1, 3), 1650.0, 500.0),
            point(d(2026, 1, 1), 1000.0, 0.0),
            point(d(2026, 1, 2), 1100.0, 0.0),
        ];
        assert!(near(time_weighted_return(&points), 0.15, 1e-9));
    }

    #[test]
    fn annualize_compounds_over_365_day_years() {
        assert!(near(annualize(0.21, 730), 0.10, 1e-6));
        assert!(near(annualize(0.10, 365), 0.10, 1e-9));
    }

    #[test]
    fn annualize_is_none_under_a_year() {
        assert_eq!(annualize(0.1, 100), None);
        assert_eq!(annualize(0.1, 364), None);
    }

    #[test]
    fn xirr_solves_a_one_year_round_trip() {
        let flows = vec![
            CashFlow { date: d(2025, 1, 1), amount: -1000.0 },
            CashFlow { date: d(2026, 1, 1), amount: 1100.0 },
        ];
        assert!(near(xirr(&flows), 0.10, 1e-6), "xirr {:?}", xirr(&flows));
    }

    #[test]
    fn xirr_handles_a_mid_period_contribution() {
        // 1000 in at t=0, 500 in at t=0.5y, 1650 out at t=1y: solves
        // 1000(1+r) + 500(1+r)^0.5 = 1650.
        let flows = vec![
            CashFlow { date: d(2025, 1, 1), amount: -1000.0 },
            CashFlow { date: d(2025, 7, 2), amount: -500.0 },
            CashFlow { date: d(2026, 1, 1), amount: 1650.0 },
        ];
        let r = xirr(&flows).expect("a rate");
        let t_mid = (d(2025, 7, 2) - d(2025, 1, 1)).num_days() as f64 / 365.0;
        let npv = -1000.0 - 500.0 / (1.0 + r).powf(t_mid) + 1650.0 / (1.0 + r);
        assert!(npv.abs() < 1e-6, "npv {} at r {}", npv, r);
        assert!(r > 0.11 && r < 0.13, "r {}", r);
    }

    #[test]
    fn xirr_is_none_without_a_sign_change() {
        let negative = vec![
            CashFlow { date: d(2025, 1, 1), amount: -1000.0 },
            CashFlow { date: d(2026, 1, 1), amount: -100.0 },
        ];
        assert_eq!(xirr(&negative), None);
        assert_eq!(xirr(&[]), None);
        assert_eq!(xirr(&[CashFlow { date: d(2025, 1, 1), amount: 10.0 }]), None);
    }

    #[test]
    fn xirr_finds_a_loss() {
        let flows = vec![
            CashFlow { date: d(2025, 1, 1), amount: -1000.0 },
            CashFlow { date: d(2026, 1, 1), amount: 500.0 },
        ];
        assert!(near(xirr(&flows), -0.5, 1e-6), "xirr {:?}", xirr(&flows));
    }

    #[test]
    fn modified_dietz_with_no_flows_is_the_simple_return() {
        let r = modified_dietz(1000.0, 1150.0, &[], d(2026, 1, 1), d(2026, 1, 31));
        assert!(near(r, 0.15, 1e-9), "dietz {:?}", r);
    }

    #[test]
    fn modified_dietz_weights_a_mid_period_flow_by_time_remaining() {
        // Investor view: a buy of 500 half way through the period is amount −500,
        // i.e. a +500 contribution weighted 0.5.
        let flows = vec![CashFlow { date: d(2026, 1, 16), amount: -500.0 }];
        let r = modified_dietz(1000.0, 1650.0, &flows, d(2026, 1, 1), d(2026, 1, 31));
        assert!(near(r, 0.12, 1e-9), "dietz {:?}", r);
    }

    #[test]
    fn modified_dietz_is_none_without_capital_or_time() {
        assert_eq!(modified_dietz(0.0, 100.0, &[], d(2026, 1, 1), d(2026, 1, 31)), None);
        assert_eq!(modified_dietz(1000.0, 1100.0, &[], d(2026, 1, 1), d(2026, 1, 1)), None);
    }
}
