//! Ride Report rules
//!
//! Deterministic, I/O-free rules for the "hitchhiker" briefing strategy:
//! 30-day net move, daily swing, analyst upside, ride status, trailing-stop
//! percent, entry date from fill history, and trading-day counting.

use crate::brokers::types::AccountActivity;
use serde::{Deserialize, Serialize};

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
