//! Valuation series: the holdings' daily value from trades and daily closes.
//!
//! The date axis is the union of close dates across the held symbols inside
//! `[from, to]`. The quantity on a date is the pre-history quantity (see
//! `lots::pre_history_quantities`) plus every trade dated on or before it; a
//! close is forward-filled from the last known one. A symbol without any close
//! is skipped and named in `warnings` — nothing panics on a bad ticker.

use super::lots::{pre_history_quantities, OpeningPosition, TradeEvent, TradeSide};
use super::returns::ValuationPoint;
use crate::error::Result;
use crate::providers::fmp::FmpClient;
use crate::providers::types::DailyClose;
use async_trait::async_trait;
use chrono::NaiveDate;
use futures_util::StreamExt;
use std::collections::{BTreeMap, BTreeSet};

/// Provider requests in flight at once (closes per symbol, sector lookups).
pub const FETCH_CONCURRENCY: usize = 4;

/// A source of daily closes, so the series can be fed from FMP or a fixture.
#[async_trait]
pub trait DailyCloses: Send + Sync {
    /// Closes of `symbol` inside `[from, to]`, any order.
    async fn closes(&self, symbol: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, f64)>>;
}

/// Daily closes from FMP's end-of-day endpoint (`FmpClient::get_daily_closes`).
pub struct FmpCloses<'a> {
    pub client: &'a FmpClient,
}

#[async_trait]
impl DailyCloses for FmpCloses<'_> {
    async fn closes(&self, symbol: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, f64)>> {
        let raw = self
            .client
            .get_daily_closes(symbol, &from.format("%Y-%m-%d").to_string(), &to.format("%Y-%m-%d").to_string())
            .await?;
        Ok(parse_daily_closes(raw))
    }
}

/// `DailyClose` rows to `(date, close)`, ascending; rows with an unparseable
/// date or a non-positive close are dropped.
pub fn parse_daily_closes(raw: Vec<DailyClose>) -> Vec<(NaiveDate, f64)> {
    let mut closes: Vec<(NaiveDate, f64)> = raw
        .into_iter()
        .filter(|row| row.close > 0.0)
        .filter_map(|row| {
            NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, row.close))
        })
        .collect();
    closes.sort_by_key(|(date, _)| *date);
    closes
}

/// Mask the value of every `apikey=` parameter in `text` (provider errors quote
/// the request URL, which carries the key).
pub fn redact_api_key(text: &str) -> String {
    const NEEDLE: &str = "apikey=";
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(found) = lower[cursor..].find(NEEDLE) {
        let value_start = cursor + found + NEEDLE.len();
        out.push_str(&text[cursor..value_start]);
        out.push_str("***");
        let value_end = text[value_start..]
            .find(|c: char| matches!(c, '&' | ')' | '"' | '\'' | ']') || c.is_whitespace())
            .map(|offset| value_start + offset)
            .unwrap_or(text.len());
        cursor = value_end;
    }
    out.push_str(&text[cursor..]);
    out
}

#[derive(Clone, Debug, Default)]
pub struct SeriesResult {
    pub points: Vec<ValuationPoint>,
    pub warnings: Vec<String>,
}

/// The holdings' value on every close date in `[from, to]`, with that day's
/// net flow: `Σ buy cost (incl. fees) − Σ sell proceeds (net of fees)`.
/// A trade dated between two closes counts on the next close.
pub async fn holdings_value_series(
    trades: &[TradeEvent],
    openings: &[OpeningPosition],
    from: NaiveDate,
    to: NaiveDate,
    closes: &dyn DailyCloses,
) -> Result<SeriesResult> {
    let symbols: BTreeSet<&str> = trades
        .iter()
        .map(|trade| trade.symbol.as_str())
        .chain(openings.iter().map(|opening| opening.symbol.as_str()))
        .collect();

    // One request per symbol, at most FETCH_CONCURRENCY in flight; results keep the symbol order.
    let requests: Vec<_> = symbols.iter().map(|symbol| closes.closes(symbol, from, to)).collect();
    let fetched: Vec<Result<Vec<(NaiveDate, f64)>>> = futures_util::stream::iter(requests)
        .buffered(FETCH_CONCURRENCY)
        .collect()
        .await;

    let mut warnings = Vec::new();
    let mut history: BTreeMap<&str, Vec<(NaiveDate, f64)>> = BTreeMap::new();
    for (&symbol, result) in symbols.iter().zip(fetched) {
        match result {
            Ok(mut rows) => {
                rows.retain(|(date, close)| *date >= from && *date <= to && *close > 0.0);
                if rows.is_empty() {
                    warnings.push(format!(
                        "{symbol}: no price history (left out of the valuation and its trades out of the flows)"
                    ));
                } else {
                    rows.sort_by_key(|(date, _)| *date);
                    history.insert(symbol, rows);
                }
            }
            Err(err) => {
                // Provider error text can carry the API key: log it redacted, never show it.
                tracing::warn!(
                    "analytics: price history for {symbol} failed: {}",
                    redact_api_key(&err.to_string())
                );
                warnings.push(format!("{symbol}: price history unavailable"));
            }
        }
    }

    let axis: BTreeSet<NaiveDate> = history.values().flatten().map(|(date, _)| *date).collect();
    if axis.is_empty() {
        return Ok(SeriesResult { points: Vec::new(), warnings });
    }

    let mut sorted_trades: Vec<&TradeEvent> = trades.iter().collect();
    sorted_trades.sort_by(|a, b| (a.date, a.seq).cmp(&(b.date, b.seq)));

    // Running quantity per symbol, seeded with what predates the history.
    let pre_history = pre_history_quantities(trades, openings);
    let mut quantity: BTreeMap<&str, f64> = history
        .keys()
        .map(|symbol| (*symbol, pre_history.get(*symbol).copied().unwrap_or(0.0).max(0.0)))
        .collect();
    // Forward-fill cursor per symbol: index of the next unseen close, and the last close seen.
    let mut cursor: BTreeMap<&str, (usize, Option<f64>)> =
        history.keys().map(|symbol| (*symbol, (0, None))).collect();

    let mut next_trade = 0;
    let mut points = Vec::with_capacity(axis.len());
    for date in axis.iter() {
        let mut flow = 0.0;
        while next_trade < sorted_trades.len() && sorted_trades[next_trade].date <= *date {
            let trade = sorted_trades[next_trade];
            next_trade += 1;
            let signed = match trade.side {
                TradeSide::Buy => trade.quantity,
                TradeSide::Sell => -trade.quantity,
            };
            // A symbol without closes is outside the measured portfolio: neither
            // its shares nor the money that bought or sold them count.
            let Some(held) = quantity.get_mut(trade.symbol.as_str()) else { continue };
            *held = (*held + signed).max(0.0);
            // Trades before the window only set the quantity; they are not flows.
            if trade.date >= from {
                flow += match trade.side {
                    TradeSide::Buy => trade.quantity * trade.price + trade.fees,
                    TradeSide::Sell => -(trade.quantity * trade.price - trade.fees),
                };
            }
        }

        let mut value = 0.0;
        for (symbol, rows) in &history {
            let (next, last) = cursor.get_mut(symbol).expect("cursor per symbol");
            while *next < rows.len() && rows[*next].0 <= *date {
                *last = Some(rows[*next].1);
                *next += 1;
            }
            if let Some(close) = last {
                value += quantity[symbol] * *close;
            }
        }
        points.push(ValuationPoint { date: *date, value, flow });
    }

    Ok(SeriesResult { points, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::lots::{OpeningPosition, TradeEvent, TradeSide};
    use crate::providers::types::DailyClose;
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn opening(symbol: &str, quantity: f64) -> OpeningPosition {
        OpeningPosition { symbol: symbol.to_string(), quantity, avg_cost: 1.0 }
    }

    fn trade(symbol: &str, date: NaiveDate, seq: i64, side: TradeSide, qty: f64, price: f64, fees: f64) -> TradeEvent {
        TradeEvent { date, seq, symbol: symbol.to_string(), side, quantity: qty, price, fees }
    }

    /// Closes per symbol; a symbol that is missing has no history at all.
    struct FakeCloses(HashMap<&'static str, Vec<(NaiveDate, f64)>>);

    #[async_trait]
    impl DailyCloses for FakeCloses {
        async fn closes(&self, symbol: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, f64)>> {
            Ok(self
                .0
                .get(symbol)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|(date, _)| *date >= from && *date <= to)
                .collect())
        }
    }

    /// Every fetch fails, as a bad ticker or an API outage would.
    struct FailingCloses;

    #[async_trait]
    impl DailyCloses for FailingCloses {
        async fn closes(&self, symbol: &str, _from: NaiveDate, _to: NaiveDate) -> Result<Vec<(NaiveDate, f64)>> {
            // Shaped like a reqwest error: the URL carries the FMP key.
            Err(crate::error::AppError::Provider(format!(
                "HTTP error: error sending request for url (https://fmp/eod?symbol={symbol}&apikey=SECRET123)"
            )))
        }
    }

    fn close_enough(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn values(result: &SeriesResult) -> Vec<(NaiveDate, f64, f64)> {
        result.points.iter().map(|p| (p.date, p.value, p.flow)).collect()
    }

    #[tokio::test]
    async fn gap_days_are_forward_filled_across_symbols() {
        let closes = FakeCloses(HashMap::from([
            ("AAA", vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0), (d(2026, 1, 7), 12.0)]),
            ("BBB", vec![(d(2026, 1, 5), 100.0), (d(2026, 1, 7), 102.0)]),
        ]));
        let openings = vec![opening("AAA", 2.0), opening("BBB", 1.0)];

        let result = holdings_value_series(&[], &openings, d(2026, 1, 5), d(2026, 1, 7), &closes)
            .await
            .unwrap();

        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        let points = values(&result);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].0, d(2026, 1, 5));
        assert!(close_enough(points[0].1, 120.0), "{:?}", points);
        // BBB has no close on the 6th: its last close (100) carries forward.
        assert!(close_enough(points[1].1, 122.0), "{:?}", points);
        assert!(close_enough(points[2].1, 126.0), "{:?}", points);
        assert!(points.iter().all(|p| close_enough(p.2, 0.0)));
    }

    #[tokio::test]
    async fn a_buy_mid_window_adds_the_flow_and_the_quantity_from_that_day() {
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0), (d(2026, 1, 7), 12.0)],
        )]));
        // Holding today: 3 shares; the history explains 2 of them, so 1 predates it.
        let openings = vec![opening("AAA", 3.0)];
        let trades = vec![trade("AAA", d(2026, 1, 6), 1, TradeSide::Buy, 2.0, 11.0, 1.0)];

        let result = holdings_value_series(&trades, &openings, d(2026, 1, 5), d(2026, 1, 7), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert_eq!(points.len(), 3);
        assert!(close_enough(points[0].1, 10.0), "{:?}", points);
        assert!(close_enough(points[0].2, 0.0));
        assert!(close_enough(points[1].1, 33.0), "{:?}", points);
        assert!(close_enough(points[1].2, 23.0), "buy cost incl. fee: {:?}", points);
        assert!(close_enough(points[2].1, 36.0), "{:?}", points);
        assert!(close_enough(points[2].2, 0.0));
    }

    #[tokio::test]
    async fn sell_flows_are_negative_and_net_of_fees() {
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0)],
        )]));
        let trades = vec![
            trade("AAA", d(2026, 1, 5), 1, TradeSide::Buy, 2.0, 10.0, 0.0),
            trade("AAA", d(2026, 1, 6), 2, TradeSide::Sell, 1.0, 12.0, 1.0),
        ];

        let result = holdings_value_series(&trades, &[], d(2026, 1, 5), d(2026, 1, 6), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert!(close_enough(points[0].1, 20.0), "{:?}", points);
        assert!(close_enough(points[0].2, 20.0), "{:?}", points);
        assert!(close_enough(points[1].1, 11.0), "{:?}", points);
        assert!(close_enough(points[1].2, -11.0), "{:?}", points);
    }

    #[tokio::test]
    async fn missing_closes_are_reported() {
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0)],
        )]));
        let openings = vec![opening("AAA", 1.0), opening("ZZZ", 5.0)];

        let result = holdings_value_series(&[], &openings, d(2026, 1, 5), d(2026, 1, 6), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert_eq!(points.len(), 2);
        assert!(close_enough(points[0].1, 10.0));
        assert!(close_enough(points[1].1, 11.0));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].starts_with("ZZZ: no price history"), "{:?}", result.warnings);
    }

    #[tokio::test]
    async fn a_failed_fetch_is_a_warning_not_an_error() {
        let openings = vec![opening("BAD", 1.0)];

        let result = holdings_value_series(&[], &openings, d(2026, 1, 5), d(2026, 1, 6), &FailingCloses)
            .await
            .unwrap();

        assert!(result.points.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].starts_with("BAD:"), "{:?}", result.warnings);
        // Provider error text never reaches the user: it can carry the API key.
        assert!(!result.warnings[0].contains("SECRET123"), "{:?}", result.warnings);
        assert!(!result.warnings[0].contains("apikey"), "{:?}", result.warnings);
    }

    #[test]
    fn redact_api_key_masks_the_key_value() {
        assert_eq!(
            redact_api_key("https://x/y?symbol=VOO&apikey=SECRET123&from=a"),
            "https://x/y?symbol=VOO&apikey=***&from=a"
        );
        assert_eq!(
            redact_api_key("error sending request for url (https://x?apikey=SECRET)"),
            "error sending request for url (https://x?apikey=***)"
        );
        assert_eq!(redact_api_key("apiKey=abc apikey=def"), "apiKey=*** apikey=***");
        assert_eq!(redact_api_key("apikey=tail"), "apikey=***");
        assert_eq!(redact_api_key("nothing here"), "nothing here");
    }

    #[tokio::test]
    async fn a_trade_between_closes_lands_on_the_next_close() {
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 8), 10.0)],
        )]));
        let openings = vec![opening("AAA", 1.0)];
        let trades = vec![trade("AAA", d(2026, 1, 6), 1, TradeSide::Buy, 1.0, 10.0, 0.0)];

        let result = holdings_value_series(&trades, &openings, d(2026, 1, 5), d(2026, 1, 8), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert_eq!(points.len(), 2);
        assert!(close_enough(points[0].1, 0.0), "{:?}", points);
        assert!(close_enough(points[1].1, 10.0), "{:?}", points);
        assert!(close_enough(points[1].2, 10.0), "{:?}", points);
    }

    #[tokio::test]
    async fn trades_before_the_window_set_the_quantity_without_a_flow() {
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0)],
        )]));
        let openings = vec![opening("AAA", 2.0)];
        let trades = vec![trade("AAA", d(2026, 1, 1), 1, TradeSide::Buy, 2.0, 5.0, 0.0)];

        let result = holdings_value_series(&trades, &openings, d(2026, 1, 5), d(2026, 1, 6), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert!(close_enough(points[0].1, 20.0), "{:?}", points);
        assert!(close_enough(points[0].2, 0.0), "{:?}", points);
        assert!(close_enough(points[1].1, 22.0), "{:?}", points);
    }

    #[tokio::test]
    async fn without_a_holding_row_nothing_predates_the_trades() {
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0), (d(2026, 1, 7), 12.0)],
        )]));
        let trades = vec![trade("AAA", d(2026, 1, 6), 1, TradeSide::Buy, 1.0, 10.0, 0.0)];

        let result = holdings_value_series(&trades, &[], d(2026, 1, 5), d(2026, 1, 7), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert!(close_enough(points[0].1, 0.0), "{:?}", points);
        assert!(close_enough(points[1].1, 11.0), "{:?}", points);
        assert!(close_enough(points[1].2, 10.0), "{:?}", points);
        assert!(close_enough(points[2].1, 12.0), "{:?}", points);
    }

    #[tokio::test]
    async fn nothing_held_gives_an_empty_series() {
        let result = holdings_value_series(&[], &[], d(2026, 1, 5), d(2026, 1, 7), &FailingCloses)
            .await
            .unwrap();
        assert!(result.points.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn fmp_closes_parse_dates_and_drop_bad_rows() {
        let raw = vec![
            DailyClose { date: "2026-01-06".to_string(), close: 11.0, volume: None },
            DailyClose { date: "not a date".to_string(), close: 9.0, volume: None },
            DailyClose { date: "2026-01-05".to_string(), close: 10.0, volume: None },
            DailyClose { date: "2026-01-07".to_string(), close: 0.0, volume: None },
        ];
        let parsed = parse_daily_closes(raw);
        assert_eq!(parsed, vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0)]);
    }

    #[tokio::test]
    async fn a_trade_in_a_symbol_without_closes_adds_no_flow_and_no_value() {
        // ZZZ (a money-market sweep, a delisted name) has no closes: its buy must
        // not count as money leaving the measured portfolio.
        let closes = FakeCloses(HashMap::from([(
            "AAA",
            vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 6), 11.0)],
        )]));
        let openings = vec![opening("AAA", 1.0), opening("ZZZ", 10.0)];
        let trades = vec![trade("ZZZ", d(2026, 1, 6), 1, TradeSide::Buy, 10.0, 50.0, 0.0)];

        let result = holdings_value_series(&trades, &openings, d(2026, 1, 5), d(2026, 1, 6), &closes)
            .await
            .unwrap();

        let points = values(&result);
        assert!(close_enough(points[0].1, 10.0), "{:?}", points);
        assert!(close_enough(points[1].1, 11.0), "{:?}", points);
        assert!(close_enough(points[1].2, 0.0), "no flow for an unpriced symbol: {:?}", points);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].starts_with("ZZZ: no price history"), "{:?}", result.warnings);
    }
}
