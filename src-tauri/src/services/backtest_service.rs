//! Backtesting Service
//!
//! Deterministic single-symbol backtester. Pure simulation core (no I/O in the
//! loop); candles are loaded once up front. Strategies implement `SignalGenerator`
//! so a future rule builder can plug into the same engine, costs, taxes, metrics.

use serde::{Deserialize, Serialize};

/// One OHLC bar the engine works on (volume as f64 for indicator reuse).
#[derive(Debug, Clone)]
pub struct Bar {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Per-bar trading signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Buy,
    Sell,
    Hold,
}

/// Which built-in strategy + its parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum StrategySpec {
    SmaCrossover { fast: usize, slow: usize },
    EmaCrossover { fast: usize, slow: usize },
    RsiThreshold { period: usize, oversold: f64, overbought: f64 },
    MacdCross { fast: usize, slow: usize, signal: usize },
    BollingerReversion { period: usize, std_dev: f64 },
}

/// Position sizing per entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Sizing {
    AllIn,
    FixedFraction(f64), // 0.0..=1.0 of current equity
    FixedShares(f64),
}

/// US/Alpaca trading costs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Costs {
    pub commission_per_trade: f64, // default 0.0
    pub slippage_bps: f64,         // default 5.0
    pub reg_fees_enabled: bool,    // SEC + FINRA TAF on sells
}

/// Capital-gains tax rates (fractions, e.g. 0.35).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TaxConfig {
    pub st_rate: f64, // short-term (< 365 days held)
    pub lt_rate: f64, // long-term  (>= 365 days held)
}

/// Full backtest configuration (echoed back in the result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
    pub from_date: String,
    pub to_date: String,
    pub starting_capital: f64,
    pub sizing: Sizing,
    pub costs: Costs,
    pub tax: TaxConfig,
    pub fractional: bool,
    pub risk_free_rate: f64, // annualized, e.g. 0.05
    pub strategy: StrategySpec,
}

/// A completed round-trip trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub entry_time: String,
    pub exit_time: String,
    pub entry_price: f64, // fill price incl. slippage
    pub exit_price: f64,  // fill price incl. slippage
    pub shares: f64,
    pub pnl_after_fees: f64, // exit_value - entry_cost (both incl. their fees); pre-tax
    pub fees: f64,           // commissions + reg fees across both legs
    pub holding_days: i64,
    pub long_term: bool,     // holding_days >= 365
}

/// One point on the (gross, mark-to-market) equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: String,
    pub equity: f64,
    pub drawdown: f64, // fraction below running peak, <= 0
}

/// Buy & hold benchmark on the same symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub total_return: f64,
    pub cagr: f64,
    pub max_drawdown: f64,
}

/// All computed metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_return: f64,
    pub cagr: f64,
    pub volatility: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub max_drawdown: f64,
    pub max_dd_peak: String,
    pub max_dd_trough: String,
    pub num_trades: usize,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub max_consecutive_losses: usize,
    pub avg_holding_days: f64,
    pub time_in_market: f64,
    // costs / tax
    pub total_fees: f64,
    pub st_tax: f64,
    pub lt_tax: f64,
    pub total_tax: f64,
    pub net_total_return: f64, // after total_tax
    pub net_cagr: f64,
}

/// Complete backtest result returned to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub config: BacktestConfig,
    pub equity_curve: Vec<EquityPoint>,
    pub trades: Vec<Trade>,
    pub metrics: BacktestMetrics,
    pub benchmark: BenchmarkResult,
    pub warnings: Vec<String>,
}

pub struct BacktestService;

use crate::services::indicators_service::{compute_sma, OhlcvData};

/// Convert engine bars to indicator input.
fn to_ohlcv(bars: &[Bar]) -> Vec<OhlcvData> {
    bars.iter()
        .map(|b| OhlcvData {
            timestamp: b.timestamp.clone(),
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        })
        .collect()
}

/// `a` crossed from <= `b` to > `b` between i-1 and i.
fn crossed_above(a: &[f64], b: &[f64], i: usize) -> bool {
    i > 0 && a[i - 1] <= b[i - 1] && a[i] > b[i]
}
/// `a` crossed from >= `b` to < `b` between i-1 and i.
fn crossed_below(a: &[f64], b: &[f64], i: usize) -> bool {
    i > 0 && a[i - 1] >= b[i - 1] && a[i] < b[i]
}
/// scalar-threshold variants
fn crossed_above_scalar(a: &[f64], t: f64, i: usize) -> bool {
    i > 0 && a[i - 1] <= t && a[i] > t
}
fn crossed_below_scalar(a: &[f64], t: f64, i: usize) -> bool {
    i > 0 && a[i - 1] >= t && a[i] < t
}

/// A strategy that produces per-bar signals from precomputed indicator series.
pub trait SignalGenerator {
    /// Precompute indicator series over the full bar set (called once).
    fn prepare(&mut self, bars: &[Bar]);
    /// Bars before this index cannot produce a valid signal.
    fn warmup(&self) -> usize;
    /// Signal at bar `i` using only data at or before `i` (no look-ahead).
    fn signal(&self, i: usize) -> Signal;
}

pub struct SmaCrossover {
    fast: usize,
    slow: usize,
    fast_v: Vec<f64>,
    slow_v: Vec<f64>,
}
impl SmaCrossover {
    pub fn new(fast: usize, slow: usize) -> Self {
        Self { fast, slow, fast_v: Vec::new(), slow_v: Vec::new() }
    }
}
impl SignalGenerator for SmaCrossover {
    fn prepare(&mut self, bars: &[Bar]) {
        let o = to_ohlcv(bars);
        self.fast_v = compute_sma(&o, self.fast).values.iter().map(|p| p.value).collect();
        self.slow_v = compute_sma(&o, self.slow).values.iter().map(|p| p.value).collect();
    }
    fn warmup(&self) -> usize {
        self.slow.max(self.fast)
    }
    fn signal(&self, i: usize) -> Signal {
        if crossed_above(&self.fast_v, &self.slow_v, i) {
            Signal::Buy
        } else if crossed_below(&self.fast_v, &self.slow_v, i) {
            Signal::Sell
        } else {
            Signal::Hold
        }
    }
}

use crate::services::indicators_service::{
    compute_bollinger, compute_ema, compute_macd, compute_rsi,
};

pub struct EmaCrossover { fast: usize, slow: usize, fast_v: Vec<f64>, slow_v: Vec<f64> }
impl EmaCrossover { pub fn new(fast: usize, slow: usize) -> Self { Self { fast, slow, fast_v: vec![], slow_v: vec![] } } }
impl SignalGenerator for EmaCrossover {
    fn prepare(&mut self, bars: &[Bar]) {
        let o = to_ohlcv(bars);
        self.fast_v = compute_ema(&o, self.fast).values.iter().map(|p| p.value).collect();
        self.slow_v = compute_ema(&o, self.slow).values.iter().map(|p| p.value).collect();
    }
    fn warmup(&self) -> usize { self.slow.max(self.fast) }
    fn signal(&self, i: usize) -> Signal {
        if crossed_above(&self.fast_v, &self.slow_v, i) { Signal::Buy }
        else if crossed_below(&self.fast_v, &self.slow_v, i) { Signal::Sell }
        else { Signal::Hold }
    }
}

pub struct RsiThreshold { period: usize, oversold: f64, overbought: f64, rsi_v: Vec<f64> }
impl RsiThreshold { pub fn new(period: usize, oversold: f64, overbought: f64) -> Self { Self { period, oversold, overbought, rsi_v: vec![] } } }
impl SignalGenerator for RsiThreshold {
    fn prepare(&mut self, bars: &[Bar]) {
        let o = to_ohlcv(bars);
        self.rsi_v = compute_rsi(&o, self.period).values.iter().map(|p| p.value).collect();
    }
    fn warmup(&self) -> usize { self.period + 1 }
    fn signal(&self, i: usize) -> Signal {
        if crossed_above_scalar(&self.rsi_v, self.oversold, i) { Signal::Buy }
        else if crossed_below_scalar(&self.rsi_v, self.overbought, i) { Signal::Sell }
        else { Signal::Hold }
    }
}

pub struct MacdCross { fast: usize, slow: usize, signal_p: usize, macd_v: Vec<f64>, sig_v: Vec<f64> }
impl MacdCross { pub fn new(fast: usize, slow: usize, signal: usize) -> Self { Self { fast, slow, signal_p: signal, macd_v: vec![], sig_v: vec![] } } }
impl SignalGenerator for MacdCross {
    fn prepare(&mut self, bars: &[Bar]) {
        let o = to_ohlcv(bars);
        let r = compute_macd(&o, self.fast, self.slow, self.signal_p);
        self.macd_v = r.values.iter().map(|p| p.value).collect();
        self.sig_v = r.values.iter().map(|p| p.secondary_value.unwrap_or(0.0)).collect();
    }
    fn warmup(&self) -> usize { self.slow + self.signal_p }
    fn signal(&self, i: usize) -> Signal {
        if crossed_above(&self.macd_v, &self.sig_v, i) { Signal::Buy }
        else if crossed_below(&self.macd_v, &self.sig_v, i) { Signal::Sell }
        else { Signal::Hold }
    }
}

pub struct BollingerReversion { period: usize, std_dev: f64, close_v: Vec<f64>, upper_v: Vec<f64>, lower_v: Vec<f64> }
impl BollingerReversion { pub fn new(period: usize, std_dev: f64) -> Self { Self { period, std_dev, close_v: vec![], upper_v: vec![], lower_v: vec![] } } }
impl SignalGenerator for BollingerReversion {
    fn prepare(&mut self, bars: &[Bar]) {
        let o = to_ohlcv(bars);
        let r = compute_bollinger(&o, self.period, self.std_dev);
        self.close_v = bars.iter().map(|b| b.close).collect();
        self.upper_v = r.values.iter().map(|p| p.secondary_value.unwrap_or(f64::INFINITY)).collect();
        self.lower_v = r.values.iter().map(|p| p.tertiary_value.unwrap_or(f64::NEG_INFINITY)).collect();
    }
    fn warmup(&self) -> usize { self.period }
    fn signal(&self, i: usize) -> Signal {
        // Reversion: price dipping below lower band = Buy; popping above upper = Sell.
        if crossed_below(&self.close_v, &self.lower_v, i) { Signal::Buy }
        else if crossed_above(&self.close_v, &self.upper_v, i) { Signal::Sell }
        else { Signal::Hold }
    }
}

/// Build a boxed signal generator from a strategy spec.
pub fn build_generator(spec: &StrategySpec) -> Box<dyn SignalGenerator> {
    match *spec {
        StrategySpec::SmaCrossover { fast, slow } => Box::new(SmaCrossover::new(fast, slow)),
        StrategySpec::EmaCrossover { fast, slow } => Box::new(EmaCrossover::new(fast, slow)),
        StrategySpec::RsiThreshold { period, oversold, overbought } => Box::new(RsiThreshold::new(period, oversold, overbought)),
        StrategySpec::MacdCross { fast, slow, signal } => Box::new(MacdCross::new(fast, slow, signal)),
        StrategySpec::BollingerReversion { period, std_dev } => Box::new(BollingerReversion::new(period, std_dev)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars_from_closes(closes: &[f64]) -> Vec<Bar> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Bar {
                timestamp: format!("2024-01-{:02}", i + 1),
                open: c,
                high: c,
                low: c,
                close: c,
                volume: 1000.0,
            })
            .collect()
    }

    #[test]
    fn sma_crossover_emits_buy_on_cross_up_and_sell_on_cross_down() {
        // Fast(2)/Slow(3). Closes engineered so fast SMA crosses slow up then down.
        let closes = [10.0, 10.0, 10.0, 12.0, 14.0, 8.0, 6.0];
        let bars = bars_from_closes(&closes);
        let mut gen = SmaCrossover::new(2, 3);
        gen.prepare(&bars);

        // Collect signals across all bars.
        let signals: Vec<Signal> = (0..bars.len()).map(|i| gen.signal(i)).collect();
        assert!(signals.contains(&Signal::Buy), "expected a Buy: {signals:?}");
        assert!(signals.contains(&Signal::Sell), "expected a Sell: {signals:?}");
        // A Buy must occur before the Sell.
        let buy = signals.iter().position(|s| *s == Signal::Buy).unwrap();
        let sell = signals.iter().position(|s| *s == Signal::Sell).unwrap();
        assert!(buy < sell);
    }

    #[test]
    fn rsi_threshold_buys_crossing_up_through_oversold() {
        // Falling then rising sharply -> RSI dips low then crosses up through 30.
        let closes = [50.0, 46.0, 42.0, 39.0, 37.0, 36.0, 44.0, 52.0, 60.0];
        let bars = bars_from_closes(&closes);
        let mut gen = RsiThreshold::new(3, 30.0, 70.0);
        gen.prepare(&bars);
        let signals: Vec<Signal> = (0..bars.len()).map(|i| gen.signal(i)).collect();
        assert!(signals.contains(&Signal::Buy), "{signals:?}");
    }

    #[test]
    fn ema_macd_bollinger_construct_and_produce_signals() {
        let closes: Vec<f64> = (0..60).map(|i| 100.0 + (i as f64 * 0.3).sin() * 10.0).collect();
        let bars = bars_from_closes(&closes);
        for mut g in [
            Box::new(EmaCrossover::new(5, 13)) as Box<dyn SignalGenerator>,
            Box::new(MacdCross::new(12, 26, 9)),
            Box::new(BollingerReversion::new(20, 2.0)),
        ] {
            g.prepare(&bars);
            // Must not panic and must return a valid signal for every index.
            for i in 0..bars.len() {
                let _ = g.signal(i);
            }
        }
    }
}
