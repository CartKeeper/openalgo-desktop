//! Tax lots: match sells against purchase lots and classify the holding term.
//!
//! Pure functions over fixtures — no I/O. The service layer turns `client_trades`
//! into [`TradeEvent`]s and `client_holdings` into [`OpeningPosition`]s.
//!
//! An `OpeningPosition` is the holding as it stands today (quantity and average
//! cost). Trade history often covers only a window (Schwab exports), so the
//! shares that predate the history are inferred as `holding − net trades` and
//! carried as one estimated lot at the holding's average cost.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// A holding is long-term when it was held strictly more than this many days.
pub const LONG_TERM_DAYS: i64 = 365;

const EPS: f64 = 1e-9;

/// How sells are matched to purchase lots.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LotMethod {
    Fifo,
    Lifo,
    Hifo,
    Average,
}

impl LotMethod {
    /// `"fifo" | "lifo" | "hifo" | "average"`, case-insensitive, surrounding space ignored.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fifo" => Some(Self::Fifo),
            "lifo" => Some(Self::Lifo),
            "hifo" => Some(Self::Hifo),
            "average" => Some(Self::Average),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Term {
    Short,
    Long,
}

impl Term {
    /// Long when held strictly more than [`LONG_TERM_DAYS`].
    pub fn from_holding_days(days: i64) -> Self {
        if days > LONG_TERM_DAYS {
            Self::Long
        } else {
            Self::Short
        }
    }
}

/// One buy or sell. `seq` breaks ties within a day (the trade row id).
#[derive(Clone, Debug, PartialEq)]
pub struct TradeEvent {
    pub date: NaiveDate,
    pub seq: i64,
    pub symbol: String,
    pub side: TradeSide,
    pub quantity: f64,
    pub price: f64,
    pub fees: f64,
}

/// The holding as it stands today: `quantity` shares at `avg_cost` per share.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningPosition {
    pub symbol: String,
    pub quantity: f64,
    pub avg_cost: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OpenLot {
    pub symbol: String,
    pub opened: NaiveDate,
    pub quantity: f64,
    pub cost_per_share: f64,
    pub cost_basis: f64,
    /// True when the lot predates the trade history and was priced at the
    /// holding's average cost.
    pub estimated: bool,
    pub market_value: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub term: Term,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Disposal {
    pub symbol: String,
    pub opened: NaiveDate,
    pub closed: NaiveDate,
    pub quantity: f64,
    pub proceeds: f64,
    pub cost_basis: f64,
    pub realized_pnl: f64,
    pub holding_days: i64,
    pub term: Term,
    pub estimated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LotTotals {
    pub cost_basis: f64,
    pub market_value: f64,
    pub unrealized_short: f64,
    pub unrealized_long: f64,
    pub realized_short_ytd: f64,
    pub realized_long_ytd: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LotReport {
    pub method: LotMethod,
    pub as_of: NaiveDate,
    pub open_lots: Vec<OpenLot>,
    pub disposals: Vec<Disposal>,
    pub totals: LotTotals,
    pub warnings: Vec<String>,
}

/// A purchase lot while sells are being matched against it.
#[derive(Clone, Debug)]
struct Lot {
    opened: NaiveDate,
    quantity: f64,
    cost_per_share: f64,
    estimated: bool,
}

/// Shares that predate the trade history, per symbol: `holding − (Σ buys − Σ sells)`.
///
/// Only symbols with a holding row appear; without one nothing is assumed to
/// predate the history. A negative value means the history buys more than the
/// holding shows (inconsistent data) — callers decide how to flag it.
pub fn pre_history_quantities(
    trades: &[TradeEvent],
    openings: &[OpeningPosition],
) -> HashMap<String, f64> {
    let mut net: HashMap<&str, f64> = HashMap::new();
    for trade in trades {
        let signed = match trade.side {
            TradeSide::Buy => trade.quantity,
            TradeSide::Sell => -trade.quantity,
        };
        *net.entry(trade.symbol.as_str()).or_insert(0.0) += signed;
    }
    openings
        .iter()
        .map(|opening| {
            let traded = net.get(opening.symbol.as_str()).copied().unwrap_or(0.0);
            (opening.symbol.clone(), opening.quantity - traded)
        })
        .collect()
}

/// Match every sell to purchase lots with `method` and report what is still open.
///
/// Trades are replayed in `(date, seq)` order. Buy fees add to cost; sell fees
/// reduce proceeds. A sell with nothing left to match is dropped and reported in
/// `warnings`. `prices` are current prices per symbol; a symbol without one keeps
/// `market_value`/`unrealized_pnl` empty and is named in `warnings`.
pub fn build_lots(
    trades: &[TradeEvent],
    openings: &[OpeningPosition],
    method: LotMethod,
    as_of: NaiveDate,
    prices: &HashMap<String, f64>,
) -> LotReport {
    let mut sorted: Vec<&TradeEvent> = trades.iter().collect();
    sorted.sort_by(|a, b| (a.date, a.seq).cmp(&(b.date, b.seq)));

    let mut by_symbol: BTreeMap<&str, Vec<&TradeEvent>> = BTreeMap::new();
    for trade in &sorted {
        by_symbol.entry(trade.symbol.as_str()).or_default().push(trade);
    }
    for opening in openings {
        by_symbol.entry(opening.symbol.as_str()).or_default();
    }

    let pre_history = pre_history_quantities(trades, openings);
    let mut open_lots = Vec::new();
    let mut disposals = Vec::new();
    let mut warnings = Vec::new();

    for (symbol, events) in by_symbol {
        let mut queue: VecDeque<Lot> = VecDeque::new();

        if let Some(opening) = openings.iter().find(|o| o.symbol == symbol) {
            let pre_qty = pre_history.get(symbol).copied().unwrap_or(0.0);
            if pre_qty > EPS {
                let opened = events
                    .first()
                    .map(|first| first.date - Duration::days(1))
                    .unwrap_or(as_of);
                queue.push_back(Lot {
                    opened,
                    quantity: pre_qty,
                    cost_per_share: opening.avg_cost,
                    estimated: true,
                });
            } else if pre_qty < -EPS {
                warnings.push(format!(
                    "{symbol}: the trade history buys {} more shares than the current holding of {}; lots follow the trades",
                    fmt_qty(-pre_qty),
                    fmt_qty(opening.quantity)
                ));
            }
        }

        let mut unmatched = 0.0;
        for event in events {
            if event.quantity <= EPS {
                warnings.push(format!(
                    "{symbol}: ignored a trade on {} with quantity {}",
                    event.date,
                    fmt_qty(event.quantity)
                ));
                continue;
            }
            match event.side {
                TradeSide::Buy => {
                    let cost_per_share = (event.quantity * event.price + event.fees) / event.quantity;
                    queue.push_back(Lot {
                        opened: event.date,
                        quantity: event.quantity,
                        cost_per_share,
                        estimated: false,
                    });
                    if method == LotMethod::Average {
                        pool_cost(&mut queue);
                    }
                }
                TradeSide::Sell => {
                    let fee_per_share = event.fees / event.quantity;
                    let mut remaining = event.quantity;
                    while remaining > EPS {
                        let Some(index) = pick_lot(method, &queue) else { break };
                        let lot = &mut queue[index];
                        let take = remaining.min(lot.quantity);
                        let proceeds = take * (event.price - fee_per_share);
                        let cost_basis = take * lot.cost_per_share;
                        let holding_days = (event.date - lot.opened).num_days();
                        disposals.push(Disposal {
                            symbol: symbol.to_string(),
                            opened: lot.opened,
                            closed: event.date,
                            quantity: take,
                            proceeds,
                            cost_basis,
                            realized_pnl: proceeds - cost_basis,
                            holding_days,
                            term: Term::from_holding_days(holding_days),
                            estimated: lot.estimated,
                        });
                        lot.quantity -= take;
                        remaining -= take;
                        if lot.quantity <= EPS {
                            queue.remove(index);
                        }
                    }
                    if remaining > EPS {
                        unmatched += remaining;
                    }
                }
            }
        }
        if unmatched > EPS {
            warnings.push(format!(
                "{symbol}: trades sell {} more shares than the history explains; those sells were ignored",
                fmt_qty(unmatched)
            ));
        }

        let price = prices.get(symbol).copied().filter(|p| *p > 0.0);
        if !queue.is_empty() && price.is_none() {
            warnings.push(format!("{symbol}: no current price; market value not shown"));
        }
        let lots: Vec<Lot> = if method == LotMethod::Average {
            pooled(queue)
        } else {
            queue.into_iter().collect()
        };
        for lot in lots {
            let holding_days = (as_of - lot.opened).num_days();
            let cost_basis = lot.quantity * lot.cost_per_share;
            let market_value = price.map(|p| p * lot.quantity);
            open_lots.push(OpenLot {
                symbol: symbol.to_string(),
                opened: lot.opened,
                quantity: lot.quantity,
                cost_per_share: lot.cost_per_share,
                cost_basis,
                estimated: lot.estimated,
                market_value,
                unrealized_pnl: market_value.map(|value| value - cost_basis),
                term: Term::from_holding_days(holding_days),
            });
        }
    }

    disposals.sort_by(|a, b| (a.closed, a.symbol.as_str()).cmp(&(b.closed, b.symbol.as_str())));

    let mut totals = LotTotals::default();
    for lot in &open_lots {
        totals.cost_basis += lot.cost_basis;
        totals.market_value += lot.market_value.unwrap_or(0.0);
        match (lot.term, lot.unrealized_pnl) {
            (Term::Short, Some(pnl)) => totals.unrealized_short += pnl,
            (Term::Long, Some(pnl)) => totals.unrealized_long += pnl,
            (_, None) => {}
        }
    }
    for disposal in disposals.iter().filter(|d| d.closed.year() == as_of.year()) {
        match disposal.term {
            Term::Short => totals.realized_short_ytd += disposal.realized_pnl,
            Term::Long => totals.realized_long_ytd += disposal.realized_pnl,
        }
    }

    LotReport {
        method,
        as_of,
        open_lots,
        disposals,
        totals,
        warnings,
    }
}

/// Which lot the next sold share comes from, or `None` when nothing is open.
fn pick_lot(method: LotMethod, queue: &VecDeque<Lot>) -> Option<usize> {
    if queue.is_empty() {
        return None;
    }
    match method {
        // Average cost keeps the FIFO date order for the holding period.
        LotMethod::Fifo | LotMethod::Average => Some(0),
        LotMethod::Lifo => Some(queue.len() - 1),
        LotMethod::Hifo => {
            let mut best = 0;
            for (index, lot) in queue.iter().enumerate() {
                if lot.cost_per_share > queue[best].cost_per_share + EPS {
                    best = index;
                }
            }
            Some(best)
        }
    }
}

/// Average cost: after a buy every open lot carries the pooled cost per share.
fn pool_cost(queue: &mut VecDeque<Lot>) {
    let quantity: f64 = queue.iter().map(|lot| lot.quantity).sum();
    if quantity <= EPS {
        return;
    }
    let cost: f64 = queue.iter().map(|lot| lot.quantity * lot.cost_per_share).sum();
    let average = cost / quantity;
    for lot in queue.iter_mut() {
        lot.cost_per_share = average;
    }
}

/// Average cost reports one lot per symbol, dated at its earliest open purchase.
fn pooled(queue: VecDeque<Lot>) -> Vec<Lot> {
    let mut lots = queue.into_iter();
    let Some(first) = lots.next() else { return Vec::new() };
    let merged = lots.fold(first, |mut acc, lot| {
        let quantity = acc.quantity + lot.quantity;
        if quantity > EPS {
            acc.cost_per_share =
                (acc.quantity * acc.cost_per_share + lot.quantity * lot.cost_per_share) / quantity;
        }
        acc.quantity = quantity;
        acc.opened = acc.opened.min(lot.opened);
        acc.estimated = acc.estimated || lot.estimated;
        acc
    });
    vec![merged]
}

/// Share counts in messages: whole numbers plainly, fractions to four places.
fn fmt_qty(quantity: f64) -> String {
    if (quantity - quantity.round()).abs() < EPS {
        format!("{:.0}", quantity.round())
    } else {
        format!("{quantity:.4}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn trade(date: NaiveDate, seq: i64, side: TradeSide, qty: f64, price: f64, fees: f64) -> TradeEvent {
        TradeEvent {
            date,
            seq,
            symbol: "XYZ".to_string(),
            side,
            quantity: qty,
            price,
            fees,
        }
    }

    /// XYZ: buy 10 @ 100 (fee 1) on 2026-01-05; buy 10 @ 120 (fee 1) on 2026-02-01;
    /// sell 5 @ 130 (fee 1) on 2026-03-01.
    fn fixture() -> Vec<TradeEvent> {
        vec![
            trade(d(2026, 1, 5), 1, TradeSide::Buy, 10.0, 100.0, 1.0),
            trade(d(2026, 2, 1), 2, TradeSide::Buy, 10.0, 120.0, 1.0),
            trade(d(2026, 3, 1), 3, TradeSide::Sell, 5.0, 130.0, 1.0),
        ]
    }

    fn as_of() -> NaiveDate {
        d(2026, 11, 12)
    }

    fn prices() -> HashMap<String, f64> {
        HashMap::from([("XYZ".to_string(), 125.0)])
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn fifo_matches_the_oldest_lot() {
        let report = build_lots(&fixture(), &[], LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.method, LotMethod::Fifo);
        assert_eq!(report.as_of, as_of());
        assert!(report.warnings.is_empty(), "warnings: {:?}", report.warnings);

        assert_eq!(report.disposals.len(), 1);
        let disposal = &report.disposals[0];
        assert!(close(disposal.proceeds, 649.0), "proceeds {}", disposal.proceeds);
        assert!(close(disposal.cost_basis, 500.5), "cost {}", disposal.cost_basis);
        assert!(close(disposal.realized_pnl, 148.5), "pnl {}", disposal.realized_pnl);
        assert_eq!(disposal.holding_days, 55);
        assert_eq!(disposal.term, Term::Short);
        assert_eq!(disposal.opened, d(2026, 1, 5));
        assert_eq!(disposal.closed, d(2026, 3, 1));
        assert!(!disposal.estimated);

        assert_eq!(report.open_lots.len(), 2);
        assert!(close(report.open_lots[0].quantity, 5.0));
        assert!(close(report.open_lots[0].cost_per_share, 100.1));
        assert!(close(report.open_lots[1].quantity, 10.0));
        assert!(close(report.open_lots[1].cost_per_share, 120.1));
        assert!(close(report.open_lots[0].market_value.unwrap(), 625.0));
        assert!(close(report.open_lots[0].unrealized_pnl.unwrap(), 124.5));

        assert!(close(report.totals.cost_basis, 1701.5), "cost {}", report.totals.cost_basis);
        assert!(close(report.totals.market_value, 1875.0));
        assert!(close(report.totals.unrealized_short, 173.5));
        assert!(close(report.totals.unrealized_long, 0.0));
        assert!(close(report.totals.realized_short_ytd, 148.5));
        assert!(close(report.totals.realized_long_ytd, 0.0));
    }

    #[test]
    fn lifo_matches_the_newest_lot() {
        let report = build_lots(&fixture(), &[], LotMethod::Lifo, as_of(), &prices());

        assert_eq!(report.disposals.len(), 1);
        let disposal = &report.disposals[0];
        assert!(close(disposal.cost_basis, 600.5), "cost {}", disposal.cost_basis);
        assert!(close(disposal.realized_pnl, 48.5), "pnl {}", disposal.realized_pnl);
        assert_eq!(disposal.holding_days, 28);
        assert_eq!(report.open_lots.len(), 2);
        assert!(close(report.open_lots[0].quantity, 10.0));
        assert!(close(report.open_lots[0].cost_per_share, 100.1));
        assert!(close(report.open_lots[1].quantity, 5.0));
        assert!(close(report.open_lots[1].cost_per_share, 120.1));
    }

    #[test]
    fn hifo_matches_the_highest_cost_lot() {
        let report = build_lots(&fixture(), &[], LotMethod::Hifo, as_of(), &prices());

        assert_eq!(report.disposals.len(), 1);
        let disposal = &report.disposals[0];
        assert!(close(disposal.cost_basis, 600.5), "cost {}", disposal.cost_basis);
        assert!(close(disposal.realized_pnl, 48.5), "pnl {}", disposal.realized_pnl);
        assert_eq!(disposal.holding_days, 28);
    }

    #[test]
    fn average_pools_cost_per_symbol() {
        let report = build_lots(&fixture(), &[], LotMethod::Average, as_of(), &prices());

        assert_eq!(report.disposals.len(), 1);
        let disposal = &report.disposals[0];
        assert!(close(disposal.cost_basis, 550.5), "cost {}", disposal.cost_basis);
        assert!(close(disposal.realized_pnl, 98.5), "pnl {}", disposal.realized_pnl);

        assert_eq!(report.open_lots.len(), 1, "one pooled lot: {:?}", report.open_lots);
        assert!(close(report.open_lots[0].quantity, 15.0));
        assert!(close(report.open_lots[0].cost_per_share, 110.1));
        assert!(close(report.open_lots[0].cost_basis, 1651.5));
        assert!(close(report.totals.cost_basis, 1651.5));
    }

    #[test]
    fn opening_lot_is_synthesized() {
        // The holding says 20 shares @ 90 average, the history nets only +15:
        // five shares predate the history and get an estimated lot.
        let openings = vec![OpeningPosition {
            symbol: "XYZ".to_string(),
            quantity: 20.0,
            avg_cost: 90.0,
        }];
        let report = build_lots(&fixture(), &openings, LotMethod::Lifo, as_of(), &prices());

        assert!(report.warnings.is_empty(), "warnings: {:?}", report.warnings);
        assert_eq!(report.open_lots.len(), 3, "lots: {:?}", report.open_lots);
        let estimated = &report.open_lots[0];
        assert!(estimated.estimated);
        assert!(close(estimated.quantity, 5.0));
        assert!(close(estimated.cost_per_share, 90.0));
        assert!(close(estimated.cost_basis, 450.0));
        assert_eq!(estimated.opened, d(2026, 1, 4));
        assert!(!report.open_lots[1].estimated);
        assert!(!report.open_lots[2].estimated);
    }

    #[test]
    fn fifo_disposal_from_an_estimated_lot_is_flagged() {
        let openings = vec![OpeningPosition {
            symbol: "XYZ".to_string(),
            quantity: 20.0,
            avg_cost: 90.0,
        }];
        let report = build_lots(&fixture(), &openings, LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.disposals.len(), 1);
        let disposal = &report.disposals[0];
        assert!(disposal.estimated);
        assert!(close(disposal.cost_basis, 450.0));
        assert!(close(disposal.realized_pnl, 199.0));
        assert_eq!(disposal.opened, d(2026, 1, 4));
        assert_eq!(report.open_lots.len(), 2);
        assert!(report.open_lots.iter().all(|lot| !lot.estimated));
    }

    #[test]
    fn opening_lot_with_no_trades_is_dated_as_of() {
        let openings = vec![OpeningPosition {
            symbol: "XYZ".to_string(),
            quantity: 3.0,
            avg_cost: 50.0,
        }];
        let report = build_lots(&[], &openings, LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.open_lots.len(), 1);
        assert!(report.open_lots[0].estimated);
        assert_eq!(report.open_lots[0].opened, as_of());
        assert!(close(report.open_lots[0].quantity, 3.0));
        assert_eq!(report.open_lots[0].term, Term::Short);
    }

    #[test]
    fn oversell_is_warned_and_skipped() {
        let trades = vec![trade(d(2026, 3, 1), 1, TradeSide::Sell, 5.0, 130.0, 1.0)];
        let report = build_lots(&trades, &[], LotMethod::Fifo, as_of(), &prices());

        assert!(report.disposals.is_empty());
        assert!(report.open_lots.is_empty());
        assert_eq!(report.warnings.len(), 1, "warnings: {:?}", report.warnings);
        assert!(report.warnings[0].contains("XYZ"));
        assert!(close(report.totals.cost_basis, 0.0));
        assert!(close(report.totals.realized_short_ytd, 0.0));
    }

    #[test]
    fn partial_oversell_keeps_the_explained_part() {
        // Only 5 shares are explained; the other 5 of the sell are dropped.
        let trades = vec![
            trade(d(2026, 1, 5), 1, TradeSide::Buy, 5.0, 100.0, 0.0),
            trade(d(2026, 3, 1), 2, TradeSide::Sell, 10.0, 130.0, 0.0),
        ];
        let report = build_lots(&trades, &[], LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.disposals.len(), 1);
        assert!(close(report.disposals[0].quantity, 5.0));
        assert!(close(report.disposals[0].proceeds, 650.0));
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("XYZ"));
        assert!(report.warnings[0].contains('5'));
    }

    #[test]
    fn holding_smaller_than_net_trades_is_warned() {
        let openings = vec![OpeningPosition {
            symbol: "XYZ".to_string(),
            quantity: 5.0,
            avg_cost: 90.0,
        }];
        let trades = vec![trade(d(2026, 1, 5), 1, TradeSide::Buy, 10.0, 100.0, 0.0)];
        let report = build_lots(&trades, &openings, LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.open_lots.len(), 1);
        assert!(close(report.open_lots[0].quantity, 10.0));
        assert!(!report.open_lots[0].estimated);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("XYZ"));
    }

    #[test]
    fn same_day_buy_then_sell_matches_the_buy() {
        let day = d(2026, 5, 4);
        // Given out of order on purpose: (date, seq) ordering must restore buy-then-sell.
        let trades = vec![
            trade(day, 2, TradeSide::Sell, 5.0, 12.0, 0.0),
            trade(day, 1, TradeSide::Buy, 5.0, 10.0, 0.0),
        ];
        let report = build_lots(&trades, &[], LotMethod::Fifo, as_of(), &prices());

        assert!(report.warnings.is_empty(), "warnings: {:?}", report.warnings);
        assert_eq!(report.disposals.len(), 1);
        assert!(close(report.disposals[0].realized_pnl, 10.0));
        assert_eq!(report.disposals[0].holding_days, 0);
        assert!(report.open_lots.is_empty());
    }

    #[test]
    fn term_at_366_days_is_long() {
        let long = vec![trade(d(2025, 11, 11), 1, TradeSide::Buy, 1.0, 10.0, 0.0)];
        let short = vec![trade(d(2025, 11, 12), 1, TradeSide::Buy, 1.0, 10.0, 0.0)];

        let long_report = build_lots(&long, &[], LotMethod::Fifo, as_of(), &prices());
        let short_report = build_lots(&short, &[], LotMethod::Fifo, as_of(), &prices());

        assert_eq!(long_report.open_lots[0].term, Term::Long);
        assert_eq!(short_report.open_lots[0].term, Term::Short);
        // Unrealized P/L is split by term: 125 − 10 = 115 on one share.
        assert!(close(long_report.totals.unrealized_long, 115.0));
        assert!(close(long_report.totals.unrealized_short, 0.0));
        assert!(close(short_report.totals.unrealized_short, 115.0));
    }

    #[test]
    fn disposal_term_follows_holding_days() {
        let trades = vec![
            trade(d(2025, 1, 1), 1, TradeSide::Buy, 2.0, 10.0, 0.0),
            // 366 days later: long-term.
            trade(d(2026, 1, 2), 2, TradeSide::Sell, 1.0, 20.0, 0.0),
            // 365 days later: still short-term.
            trade(d(2026, 1, 1), 3, TradeSide::Sell, 1.0, 20.0, 0.0),
        ];
        let report = build_lots(&trades, &[], LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.disposals.len(), 2);
        let by_close: HashMap<NaiveDate, Term> =
            report.disposals.iter().map(|x| (x.closed, x.term)).collect();
        assert_eq!(by_close[&d(2026, 1, 2)], Term::Long);
        assert_eq!(by_close[&d(2026, 1, 1)], Term::Short);
        assert!(close(report.totals.realized_long_ytd, 10.0));
        assert!(close(report.totals.realized_short_ytd, 10.0));
    }

    #[test]
    fn realized_ytd_counts_only_the_as_of_year() {
        let trades = vec![
            trade(d(2024, 1, 1), 1, TradeSide::Buy, 2.0, 10.0, 0.0),
            trade(d(2025, 6, 1), 2, TradeSide::Sell, 1.0, 20.0, 0.0),
            trade(d(2026, 6, 1), 3, TradeSide::Sell, 1.0, 30.0, 0.0),
        ];
        let report = build_lots(&trades, &[], LotMethod::Fifo, as_of(), &prices());

        assert_eq!(report.disposals.len(), 2);
        assert!(close(report.totals.realized_long_ytd, 20.0));
        assert!(close(report.totals.realized_short_ytd, 0.0));
    }

    #[test]
    fn missing_price_leaves_market_value_unknown_and_warns() {
        let report = build_lots(&fixture(), &[], LotMethod::Fifo, as_of(), &HashMap::new());

        assert!(report.open_lots.iter().all(|lot| lot.market_value.is_none()));
        assert!(close(report.totals.market_value, 0.0));
        assert!(close(report.totals.unrealized_short, 0.0));
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("XYZ"));
        assert!(report.warnings[0].contains("price"));
    }

    #[test]
    fn lot_method_parses_case_insensitively() {
        assert_eq!(LotMethod::parse("FIFO"), Some(LotMethod::Fifo));
        assert_eq!(LotMethod::parse("lifo"), Some(LotMethod::Lifo));
        assert_eq!(LotMethod::parse(" Hifo "), Some(LotMethod::Hifo));
        assert_eq!(LotMethod::parse("Average"), Some(LotMethod::Average));
        assert_eq!(LotMethod::parse("specific"), None);
    }

    #[test]
    fn pre_history_quantities_are_the_holding_minus_the_net_trades() {
        let openings = vec![
            OpeningPosition { symbol: "XYZ".to_string(), quantity: 20.0, avg_cost: 90.0 },
            OpeningPosition { symbol: "ABC".to_string(), quantity: 4.0, avg_cost: 10.0 },
        ];
        let pre = pre_history_quantities(&fixture(), &openings);

        assert!(close(pre["XYZ"], 5.0));
        assert!(close(pre["ABC"], 4.0));
        // No holding row: nothing predates the history.
        assert!(pre_history_quantities(&fixture(), &[]).is_empty());
    }
}
