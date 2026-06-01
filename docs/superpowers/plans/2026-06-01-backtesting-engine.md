# Backtesting Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic single-symbol backtester for US/Alpaca equities with built-in strategies, realistic costs + capital-gains tax drag, metrics, and a results page.

**Architecture:** A pure Rust simulation core (`backtest_service.rs`) built around a `SignalGenerator` trait (pluggable so a rule builder can slot in later). It reuses `indicators_service` for signals and `quant_service` for metrics, reads candles from DuckDB via `HistoryService`, exposes a Tauri command, persists run summaries to SQLite, and renders in a new `Backtest.tsx` page.

**Tech Stack:** Rust (Tauri 2.0, rusqlite), React/TypeScript, `lightweight-charts` v5, existing `indicators_service`/`quant_service`/`history_service`.

**Spec:** `docs/superpowers/specs/2026-06-01-backtesting-engine-design.md`

---

## Reference: exact signatures of code we reuse

```rust
// src/services/history_service.rs
pub struct CandleData { pub timestamp: String, pub open: f64, pub high: f64, pub low: f64, pub close: f64, pub volume: i64 }
pub struct HistoryResult { pub success: bool, pub symbol: String, pub exchange: String, pub interval: String, pub candles: Vec<CandleData> }
HistoryService::get_history(state: &AppState, symbol, exchange, interval, from_date, to_date: &str, api_key: Option<&str>) -> Result<HistoryResult>

// src/services/indicators_service.rs
pub struct OhlcvData { pub timestamp: String, pub open: f64, pub high: f64, pub low: f64, pub close: f64, pub volume: f64 }
pub struct IndicatorPoint { pub timestamp: String, pub value: f64, pub secondary_value: Option<f64>, pub tertiary_value: Option<f64> }
pub struct IndicatorResult { pub name: String, pub values: Vec<IndicatorPoint> }
compute_sma(&[OhlcvData], period) -> IndicatorResult            // value = SMA
compute_ema(&[OhlcvData], period) -> IndicatorResult            // value = EMA
compute_rsi(&[OhlcvData], period) -> IndicatorResult            // value = RSI
compute_macd(&[OhlcvData], fast, slow, signal) -> IndicatorResult   // value=macd line, secondary=signal line
compute_bollinger(&[OhlcvData], period, std_dev) -> IndicatorResult // value=middle, secondary=upper, tertiary=lower

// src/services/quant_service.rs   (all consume LOG returns from compute_returns)
compute_returns(&[f64]) -> Vec<f64>          // log returns of a price series
sharpe_ratio(&[f64], risk_free_rate: f64) -> f64
sortino_ratio(&[f64], risk_free_rate: f64) -> f64
max_drawdown(&[f64]) -> (f64, usize, usize)  // (depth_fraction, peak_idx, trough_idx)
calmar_ratio(returns: &[f64], prices: &[f64]) -> f64
annualized_volatility(&[f64]) -> f64
```

> **Note on `compute_macd` arity:** verify the exact parameter order in `indicators_service.rs` line ~116 before Task 4; if it differs, adapt the call. The plan assumes `compute_macd(data, fast, slow, signal)`.

---

## File Structure

- **Create** `src-tauri/src/services/backtest_service.rs` — types, `SignalGenerator` trait + 5 generators, cost/tax helpers, simulation core, metrics, benchmark, top-level `run_backtest`. (One file; it is cohesive and ~600 lines. If it grows past ~800, split generators into `backtest/signals.rs`.)
- **Modify** `src-tauri/src/services/mod.rs` — `pub mod backtest_service;` + re-exports.
- **Create** `src-tauri/src/commands/backtest.rs` — Tauri commands `run_backtest`, `save_backtest_run`, `list_backtest_runs`, `get_backtest_run`.
- **Modify** `src-tauri/src/commands/mod.rs` — `pub mod backtest;`.
- **Create** `src-tauri/src/db/sqlite/backtest.rs` — `backtest_runs` CRUD.
- **Modify** `src-tauri/src/db/sqlite/mod.rs` and `migrations.rs` — register module + migration.
- **Modify** `src-tauri/src/lib.rs` — register the 4 commands in `generate_handler!`.
- **Create** `src/api/backtest.ts` — typed bindings + TS types mirroring the Rust result.
- **Modify** `src/api/tauri-client.ts` — add `backtestCommands`.
- **Create** `src/pages/Backtest.tsx` — config form + results.
- **Modify** `src/App.tsx` (route) and `src/config/navigation.ts` (nav item).

---

## Task 1: Scaffold module + core types

**Files:**
- Create: `src-tauri/src/services/backtest_service.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Create the file with types and a compile anchor**

```rust
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
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/services/mod.rs`, add alongside the other `pub mod` lines:
```rust
pub mod backtest_service;
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Finishes with no errors (dead-code warnings for unused types are fine at this stage).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs src-tauri/src/services/mod.rs
git commit -m "feat(backtest): scaffold service module and core types"
```

---

## Task 2: Crossover helpers + `SignalGenerator` trait + SMA crossover

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

- [ ] **Step 1: Write the failing test**

Append at the bottom of `backtest_service.rs`:
```rust
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
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd src-tauri && cargo test --lib backtest_service::tests::sma_crossover`
Expected: FAIL to compile — `SmaCrossover` / `prepare` / `signal` not defined.

- [ ] **Step 3: Implement helpers, trait, and SMA generator**

Add above the `#[cfg(test)]` block:
```rust
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
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd src-tauri && cargo test --lib backtest_service::tests::sma_crossover`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): SignalGenerator trait + SMA crossover with crossover helpers"
```

---

## Task 3: Remaining signal generators (EMA, RSI, MACD, Bollinger)

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module:
```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test --lib backtest_service::tests`
Expected: FAIL — `RsiThreshold`/`EmaCrossover`/`MacdCross`/`BollingerReversion` undefined.

- [ ] **Step 3: Implement the four generators**

Add after `SmaCrossover` (and extend the `use` line):
```rust
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cd src-tauri && cargo test --lib backtest_service::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): EMA/RSI/MACD/Bollinger generators + build_generator"
```

---

## Task 4: Cost helpers (slippage + fees)

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

- [ ] **Step 1: Write failing tests**

Add to `tests`:
```rust
#[test]
fn slippage_moves_buy_up_and_sell_down() {
    // 100.0 with 50 bps slippage -> buy 100.5, sell 99.5
    assert!((fill_price(100.0, 50.0, Signal::Buy) - 100.5).abs() < 1e-9);
    assert!((fill_price(100.0, 50.0, Signal::Sell) - 99.5).abs() < 1e-9);
}

#[test]
fn reg_fees_only_apply_to_sells_when_enabled() {
    let costs = Costs { commission_per_trade: 1.0, slippage_bps: 0.0, reg_fees_enabled: true };
    // Buy: commission only.
    assert!((entry_fees(&costs) - 1.0).abs() < 1e-9);
    // Sell on $10,000 notional: commission + SEC + TAF, all > 0 and small.
    let sell = exit_fees(&costs, 10_000.0, 100.0);
    assert!(sell > 1.0 && sell < 5.0, "sell fees {sell}");
    // Disabled: sell = commission only.
    let costs_off = Costs { reg_fees_enabled: false, ..costs };
    assert!((exit_fees(&costs_off, 10_000.0, 100.0) - 1.0).abs() < 1e-9);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test --lib backtest_service::tests::slippage`
Expected: FAIL — `fill_price`/`entry_fees`/`exit_fees` undefined.

- [ ] **Step 3: Implement cost helpers**

Add near the other free functions:
```rust
/// SEC fee rate on sells (per $ of notional). Approximate; user can disable.
const SEC_FEE_RATE: f64 = 0.0000278; // $27.80 per $1,000,000 (2024-era)
/// FINRA Trading Activity Fee per share sold, capped per trade.
const TAF_PER_SHARE: f64 = 0.000166;
const TAF_CAP: f64 = 8.30;

/// Apply slippage to a reference price for the given side.
fn fill_price(reference: f64, slippage_bps: f64, side: Signal) -> f64 {
    let adj = slippage_bps / 10_000.0;
    match side {
        Signal::Buy => reference * (1.0 + adj),
        Signal::Sell => reference * (1.0 - adj),
        Signal::Hold => reference,
    }
}

/// Fees charged on an entry (buy): commission only.
fn entry_fees(costs: &Costs) -> f64 {
    costs.commission_per_trade
}

/// Fees charged on an exit (sell): commission + optional SEC + FINRA TAF.
fn exit_fees(costs: &Costs, notional: f64, shares: f64) -> f64 {
    let mut f = costs.commission_per_trade;
    if costs.reg_fees_enabled {
        f += notional * SEC_FEE_RATE;
        f += (shares * TAF_PER_SHARE).min(TAF_CAP);
    }
    f
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cd src-tauri && cargo test --lib backtest_service::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): slippage + entry/exit fee model (US/Alpaca)"
```

---

## Task 5: Tax helper (bucket netting, loss offset within bucket)

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

- [ ] **Step 1: Write failing test**

Add to `tests`:
```rust
#[test]
fn tax_nets_losses_within_holding_buckets() {
    let tax = TaxConfig { st_rate: 0.35, lt_rate: 0.15 };
    let trades = vec![
        mk_trade(1000.0, 100, false), // ST +1000
        mk_trade(-400.0, 50, false),  // ST -400  -> ST net 600
        mk_trade(2000.0, 400, true),  // LT +2000 -> LT net 2000
    ];
    let (st, lt) = compute_tax(&trades, &tax);
    assert!((st - 600.0 * 0.35).abs() < 1e-6, "st {st}");
    assert!((lt - 2000.0 * 0.15).abs() < 1e-6, "lt {lt}");
}

#[test]
fn tax_is_zero_when_bucket_net_is_a_loss() {
    let tax = TaxConfig { st_rate: 0.35, lt_rate: 0.15 };
    let trades = vec![mk_trade(-1000.0, 30, false), mk_trade(200.0, 30, false)];
    let (st, lt) = compute_tax(&trades, &tax);
    assert_eq!(st, 0.0);
    assert_eq!(lt, 0.0);
}
```

Add this test-only helper inside `tests`:
```rust
fn mk_trade(pnl: f64, holding_days: i64, long_term: bool) -> Trade {
    Trade {
        entry_time: "2024-01-01".into(),
        exit_time: "2024-01-02".into(),
        entry_price: 0.0,
        exit_price: 0.0,
        shares: 0.0,
        pnl_after_fees: pnl,
        fees: 0.0,
        holding_days,
        long_term,
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test --lib backtest_service::tests::tax`
Expected: FAIL — `compute_tax` undefined.

- [ ] **Step 3: Implement**

```rust
/// Capital-gains tax via per-bucket netting (losses offset gains within the
/// same holding bucket; no cross-bucket offset, no carryforward). Returns
/// (short_term_tax, long_term_tax).
fn compute_tax(trades: &[Trade], tax: &TaxConfig) -> (f64, f64) {
    let st_net: f64 = trades.iter().filter(|t| !t.long_term).map(|t| t.pnl_after_fees).sum();
    let lt_net: f64 = trades.iter().filter(|t| t.long_term).map(|t| t.pnl_after_fees).sum();
    (st_net.max(0.0) * tax.st_rate, lt_net.max(0.0) * tax.lt_rate)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cd src-tauri && cargo test --lib backtest_service::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): capital-gains tax via per-bucket netting"
```

---

## Task 6: Simulation core (`simulate`)

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

**Semantics (must match the test):** iterate `i` from `warmup..n`. At each `i`: first execute any order pending from bar `i-1` at `bars[i].open`; then compute `signal(i)` and store it as pending if Buy/Sell; then record gross equity at `bars[i].close`. After the loop, force-close any open position at the last bar's close (so all P&L is realized for the trade list and tax). Long-only: Buy opens if flat; Sell closes if long; otherwise Hold.

- [ ] **Step 1: Write failing test**

Add to `tests`:
```rust
#[test]
fn simulate_executes_at_next_open_and_realizes_pnl() {
    // 6 bars. Force a deterministic buy then sell using a stub generator.
    // open == close == value here, slippage 0, no fees, AllIn, whole shares.
    let closes = [10.0, 10.0, 20.0, 20.0, 30.0, 30.0];
    let bars = bars_from_closes(&closes);

    struct Stub; // Buy at i=1 (fills at open of i=2 = 20), Sell at i=3 (fills at open i=4 = 30)
    impl SignalGenerator for Stub {
        fn prepare(&mut self, _b: &[Bar]) {}
        fn warmup(&self) -> usize { 1 }
        fn signal(&self, i: usize) -> Signal {
            match i { 1 => Signal::Buy, 3 => Signal::Sell, _ => Signal::Hold }
        }
    }

    let cfg = BacktestConfig {
        symbol: "T".into(), exchange: "NASDAQ".into(), interval: "D".into(),
        from_date: "2024-01-01".into(), to_date: "2024-01-06".into(),
        starting_capital: 1000.0,
        sizing: Sizing::AllIn,
        costs: Costs { commission_per_trade: 0.0, slippage_bps: 0.0, reg_fees_enabled: false },
        tax: TaxConfig { st_rate: 0.0, lt_rate: 0.0 },
        fractional: false,
        risk_free_rate: 0.0,
        strategy: StrategySpec::SmaCrossover { fast: 1, slow: 1 }, // unused; Stub drives it
    };

    let (equity, trades) = BacktestService::simulate(&bars, &mut Stub, &cfg);
    assert_eq!(trades.len(), 1);
    let t = &trades[0];
    // $1000 / $20 = 50 shares (whole). Sell at 30 -> pnl 50*(30-20)=500.
    assert!((t.entry_price - 20.0).abs() < 1e-9);
    assert!((t.exit_price - 30.0).abs() < 1e-9);
    assert!((t.shares - 50.0).abs() < 1e-9);
    assert!((t.pnl_after_fees - 500.0).abs() < 1e-9);
    // Final equity = 1000 + 500 = 1500 (last point).
    assert!((equity.last().unwrap().equity - 1500.0).abs() < 1e-6);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test --lib backtest_service::tests::simulate`
Expected: FAIL — `BacktestService::simulate` undefined.

- [ ] **Step 3: Implement `simulate`**

Add an `impl BacktestService` block:
```rust
impl BacktestService {
    /// Run the deterministic simulation. Returns the gross equity curve and the
    /// list of completed (round-trip) trades. Long-only. See module semantics.
    pub fn simulate(
        bars: &[Bar],
        generator: &mut dyn SignalGenerator,
        cfg: &BacktestConfig,
    ) -> (Vec<EquityPoint>, Vec<Trade>) {
        let n = bars.len();
        let warmup = generator.warmup().max(1).min(n);

        let mut cash = cfg.starting_capital;
        let mut shares = 0.0f64;
        let mut entry_price = 0.0f64;
        let mut entry_time = String::new();
        let mut pending: Option<Signal> = None;

        let mut trades: Vec<Trade> = Vec::new();
        let mut equity: Vec<EquityPoint> = Vec::with_capacity(n);
        let mut peak = cfg.starting_capital;

        let mut push_equity = |ts: &str, eq: f64, peak: &mut f64, out: &mut Vec<EquityPoint>| {
            if eq > *peak { *peak = eq; }
            let dd = if *peak > 0.0 { (eq - *peak) / *peak } else { 0.0 };
            out.push(EquityPoint { timestamp: ts.to_string(), equity: eq, drawdown: dd });
        };

        for i in warmup..n {
            // 1) Execute pending order from previous bar at THIS bar's open.
            if let Some(sig) = pending.take() {
                let open = bars[i].open;
                match sig {
                    Signal::Buy if shares == 0.0 => {
                        let px = fill_price(open, cfg.costs.slippage_bps, Signal::Buy);
                        let budget = match cfg.sizing {
                            Sizing::AllIn => cash,
                            Sizing::FixedFraction(p) => (cash + shares * open) * p,
                            Sizing::FixedShares(s) => (s * px).min(cash),
                        };
                        let mut qty = budget / px;
                        if !cfg.fractional { qty = qty.floor(); }
                        if qty > 0.0 {
                            let cost = qty * px + entry_fees(&cfg.costs);
                            if cost <= cash {
                                cash -= cost;
                                shares = qty;
                                entry_price = px;
                                entry_time = bars[i].timestamp.clone();
                            }
                        }
                    }
                    Signal::Sell if shares > 0.0 => {
                        let px = fill_price(open, cfg.costs.slippage_bps, Signal::Sell);
                        trades.push(close_position(
                            &cfg.costs, shares, entry_price, &entry_time, px, &bars[i].timestamp,
                        ));
                        cash += shares * px - exit_fees(&cfg.costs, shares * px, shares);
                        shares = 0.0;
                    }
                    _ => {}
                }
            }

            // 2) Compute this bar's signal; queue actionable ones.
            match generator.signal(i) {
                s @ (Signal::Buy | Signal::Sell) => pending = Some(s),
                Signal::Hold => {}
            }

            // 3) Record gross mark-to-market equity at close.
            let eq = cash + shares * bars[i].close;
            push_equity(&bars[i].timestamp, eq, &mut peak, &mut equity);
        }

        // Force-close any open position at the final bar's close.
        if shares > 0.0 && n > 0 {
            let last = &bars[n - 1];
            let px = fill_price(last.close, cfg.costs.slippage_bps, Signal::Sell);
            trades.push(close_position(
                &cfg.costs, shares, entry_price, &entry_time, px, &last.timestamp,
            ));
            cash += shares * px - exit_fees(&cfg.costs, shares * px, shares);
            shares = 0.0;
            if let Some(p) = equity.last_mut() {
                p.equity = cash; // realized; reflect final cash
            }
        }

        (equity, trades)
    }
}

/// Build a completed Trade, computing pnl_after_fees, fees, and holding period.
fn close_position(
    costs: &Costs,
    shares: f64,
    entry_price: f64,
    entry_time: &str,
    exit_price: f64,
    exit_time: &str,
) -> Trade {
    let entry_cost = shares * entry_price + entry_fees(costs);
    let exit_value = shares * exit_price - exit_fees(costs, shares * exit_price, shares);
    let fees = entry_fees(costs) + exit_fees(costs, shares * exit_price, shares);
    let holding_days = holding_days_between(entry_time, exit_time);
    Trade {
        entry_time: entry_time.to_string(),
        exit_time: exit_time.to_string(),
        entry_price,
        exit_price,
        shares,
        pnl_after_fees: exit_value - entry_cost,
        fees,
        holding_days,
        long_term: holding_days >= 365,
    }
}

/// Whole days between two `YYYY-MM-DD` (or RFC3339-prefixed) timestamps.
fn holding_days_between(entry: &str, exit: &str) -> i64 {
    use chrono::NaiveDate;
    let parse = |s: &str| NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d").ok();
    match (parse(entry), parse(exit)) {
        (Some(a), Some(b)) => (b - a).num_days(),
        _ => 0,
    }
}
```

> `chrono` is already a dependency (used in `manager.rs`/`quant_service`). Confirm it's in `Cargo.toml`; it is.

- [ ] **Step 4: Run to verify pass**

Run: `cd src-tauri && cargo test --lib backtest_service::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): deterministic simulation core (next-open fills, long-only)"
```

---

## Task 7: Metrics + benchmark + top-level `run` assembly

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

- [ ] **Step 1: Write failing test**

Add to `tests`:
```rust
#[test]
fn metrics_and_benchmark_assemble_from_a_run() {
    let closes = [10.0, 10.0, 20.0, 20.0, 30.0, 30.0];
    let bars = bars_from_closes(&closes);
    struct Stub;
    impl SignalGenerator for Stub {
        fn prepare(&mut self, _b: &[Bar]) {}
        fn warmup(&self) -> usize { 1 }
        fn signal(&self, i: usize) -> Signal { match i { 1 => Signal::Buy, 3 => Signal::Sell, _ => Signal::Hold } }
    }
    let cfg = BacktestConfig {
        symbol: "T".into(), exchange: "NASDAQ".into(), interval: "D".into(),
        from_date: "2024-01-01".into(), to_date: "2024-01-06".into(),
        starting_capital: 1000.0, sizing: Sizing::AllIn,
        costs: Costs { commission_per_trade: 0.0, slippage_bps: 0.0, reg_fees_enabled: false },
        tax: TaxConfig { st_rate: 0.35, lt_rate: 0.15 }, fractional: false,
        risk_free_rate: 0.0, strategy: StrategySpec::SmaCrossover { fast: 1, slow: 1 },
    };
    let (equity, trades) = BacktestService::simulate(&bars, &mut Stub, &cfg);
    let m = BacktestService::compute_metrics(&equity, &trades, &cfg);
    // total return = 1500/1000 - 1 = 0.5
    assert!((m.total_return - 0.5).abs() < 1e-6);
    assert_eq!(m.num_trades, 1);
    assert!((m.win_rate - 1.0).abs() < 1e-9);
    // ST gain 500 -> tax 175 -> net final 1325 -> net return 0.325
    assert!((m.total_tax - 175.0).abs() < 1e-6);
    assert!((m.net_total_return - 0.325).abs() < 1e-6);

    let b = BacktestService::benchmark(&bars, 1, cfg.starting_capital);
    // Buy&hold from bar 1 open (10) to last close (30): 200% return.
    assert!((b.total_return - 2.0).abs() < 1e-6);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd src-tauri && cargo test --lib backtest_service::tests::metrics`
Expected: FAIL — `compute_metrics`/`benchmark` undefined.

- [ ] **Step 3: Implement metrics + benchmark + `run`**

Extend the `impl BacktestService` block:
```rust
use crate::services::quant_service;

impl BacktestService {
    pub fn compute_metrics(
        equity: &[EquityPoint],
        trades: &[Trade],
        cfg: &BacktestConfig,
    ) -> BacktestMetrics {
        let values: Vec<f64> = equity.iter().map(|e| e.equity).collect();
        let start = cfg.starting_capital;
        let final_eq = *values.last().unwrap_or(&start);
        let total_return = if start > 0.0 { final_eq / start - 1.0 } else { 0.0 };

        let returns = quant_service::compute_returns(&values);
        let volatility = quant_service::annualized_volatility(&returns);
        let sharpe = quant_service::sharpe_ratio(&returns, cfg.risk_free_rate);
        let sortino = quant_service::sortino_ratio(&returns, cfg.risk_free_rate);
        let calmar = quant_service::calmar_ratio(&returns, &values);
        let (dd_depth, peak_idx, trough_idx) = quant_service::max_drawdown(&values);
        let max_dd_peak = equity.get(peak_idx).map(|e| e.timestamp.clone()).unwrap_or_default();
        let max_dd_trough = equity.get(trough_idx).map(|e| e.timestamp.clone()).unwrap_or_default();

        // Calendar-time CAGR (interval-agnostic).
        let days = holding_days_between(&cfg.from_date, &cfg.to_date).max(1) as f64;
        let years = days / 365.0;
        let cagr = if start > 0.0 && years > 0.0 { (final_eq / start).powf(1.0 / years) - 1.0 } else { 0.0 };

        // Trade stats.
        let num_trades = trades.len();
        let wins: Vec<f64> = trades.iter().filter(|t| t.pnl_after_fees > 0.0).map(|t| t.pnl_after_fees).collect();
        let losses: Vec<f64> = trades.iter().filter(|t| t.pnl_after_fees < 0.0).map(|t| t.pnl_after_fees).collect();
        let win_rate = if num_trades > 0 { wins.len() as f64 / num_trades as f64 } else { 0.0 };
        let avg_win = if !wins.is_empty() { wins.iter().sum::<f64>() / wins.len() as f64 } else { 0.0 };
        let avg_loss = if !losses.is_empty() { losses.iter().sum::<f64>() / losses.len() as f64 } else { 0.0 };
        let gross_win: f64 = wins.iter().sum();
        let gross_loss: f64 = losses.iter().sum::<f64>().abs();
        let profit_factor = if gross_loss > 0.0 { gross_win / gross_loss } else if gross_win > 0.0 { f64::INFINITY } else { 0.0 };
        let mut max_consecutive_losses = 0usize;
        let mut run = 0usize;
        for t in trades {
            if t.pnl_after_fees < 0.0 { run += 1; max_consecutive_losses = max_consecutive_losses.max(run); } else { run = 0; }
        }
        let avg_holding_days = if num_trades > 0 {
            trades.iter().map(|t| t.holding_days as f64).sum::<f64>() / num_trades as f64
        } else { 0.0 };
        let bars_in_market: usize = trades.iter().map(|t| t.holding_days.max(0) as usize).sum();
        let time_in_market = if !equity.is_empty() { (bars_in_market as f64 / equity.len() as f64).min(1.0) } else { 0.0 };

        let total_fees: f64 = trades.iter().map(|t| t.fees).sum();
        let (st_tax, lt_tax) = compute_tax(trades, &cfg.tax);
        let total_tax = st_tax + lt_tax;
        let net_final = final_eq - total_tax;
        let net_total_return = if start > 0.0 { net_final / start - 1.0 } else { 0.0 };
        let net_cagr = if start > 0.0 && years > 0.0 { (net_final / start).max(0.0).powf(1.0 / years) - 1.0 } else { 0.0 };

        BacktestMetrics {
            total_return, cagr, volatility, sharpe, sortino, calmar,
            max_drawdown: dd_depth, max_dd_peak, max_dd_trough,
            num_trades, win_rate, avg_win, avg_loss, profit_factor,
            max_consecutive_losses, avg_holding_days, time_in_market,
            total_fees, st_tax, lt_tax, total_tax, net_total_return, net_cagr,
        }
    }

    /// Buy & hold from `start_idx` open to the last bar close, whole shares.
    pub fn benchmark(bars: &[Bar], start_idx: usize, capital: f64) -> BenchmarkResult {
        if bars.is_empty() || start_idx >= bars.len() {
            return BenchmarkResult { total_return: 0.0, cagr: 0.0, max_drawdown: 0.0 };
        }
        let entry = bars[start_idx].open;
        let qty = (capital / entry).floor();
        let curve: Vec<f64> = bars[start_idx..].iter().map(|b| (capital - qty * entry) + qty * b.close).collect();
        let final_eq = *curve.last().unwrap();
        let total_return = if capital > 0.0 { final_eq / capital - 1.0 } else { 0.0 };
        let days = holding_days_between(&bars[start_idx].timestamp, &bars[bars.len() - 1].timestamp).max(1) as f64;
        let years = days / 365.0;
        let cagr = if capital > 0.0 && years > 0.0 { (final_eq / capital).powf(1.0 / years) - 1.0 } else { 0.0 };
        let (dd, _, _) = quant_service::max_drawdown(&curve);
        BenchmarkResult { total_return, cagr, max_drawdown: dd }
    }

    /// Run a full backtest over already-loaded bars (no I/O).
    pub fn run(bars: &[Bar], cfg: &BacktestConfig, warnings: Vec<String>) -> BacktestResult {
        let mut generator = build_generator(&cfg.strategy);
        generator.prepare(bars);
        let warmup = generator.warmup().max(1).min(bars.len());
        let (equity_curve, trades) = Self::simulate(bars, generator.as_mut(), cfg);
        let metrics = Self::compute_metrics(&equity_curve, &trades, cfg);
        let benchmark = Self::benchmark(bars, warmup.min(bars.len().saturating_sub(1)), cfg.starting_capital);
        BacktestResult { config: cfg.clone(), equity_curve, trades, metrics, benchmark, warnings }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cd src-tauri && cargo test --lib backtest_service`
Expected: PASS (all backtest tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): metrics, buy&hold benchmark, and run assembly"
```

---

## Task 8: Load candles + validate (`run_for_config`)

**Files:**
- Modify: `src-tauri/src/services/backtest_service.rs`

- [ ] **Step 1: Implement the history-loading wrapper (no unit test; covered by command smoke test)**

Add to `impl BacktestService`:
```rust
use crate::error::{AppError, Result};
use crate::services::history_service::HistoryService;
use crate::state::AppState;

impl BacktestService {
    /// Load candles from DuckDB, validate, and run. Returns a validation error
    /// if there is not enough history for the strategy warmup.
    pub async fn run_for_config(state: &AppState, cfg: BacktestConfig) -> Result<BacktestResult> {
        let hist = HistoryService::get_history(
            state, &cfg.symbol, &cfg.exchange, &cfg.interval, &cfg.from_date, &cfg.to_date, None,
        )
        .await?;

        if hist.candles.is_empty() {
            return Err(AppError::Validation(format!(
                "No historical data for {}:{} ({}). Download history first (Historify).",
                cfg.exchange, cfg.symbol, cfg.interval
            )));
        }

        let bars: Vec<Bar> = hist.candles.iter().map(|c| Bar {
            timestamp: c.timestamp.clone(),
            open: c.open, high: c.high, low: c.low, close: c.close,
            volume: c.volume as f64,
        }).collect();

        let probe = build_generator(&cfg.strategy);
        let warmup = probe.warmup();
        if bars.len() <= warmup + 1 {
            return Err(AppError::Validation(format!(
                "Not enough bars ({}) for this strategy's warmup ({}). Use a longer date range.",
                bars.len(), warmup
            )));
        }

        let warnings = Vec::new();
        Ok(Self::run(&bars, &cfg, warnings))
    }
}
```

- [ ] **Step 2: Verify compile**

Run: `cd src-tauri && cargo check`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/backtest_service.rs
git commit -m "feat(backtest): history loading + warmup validation (run_for_config)"
```

---

## Task 9: SQLite persistence (`backtest_runs`)

**Files:**
- Create: `src-tauri/src/db/sqlite/backtest.rs`
- Modify: `src-tauri/src/db/sqlite/mod.rs`, `src-tauri/src/db/sqlite/migrations.rs`

- [ ] **Step 1: Add migration**

In `migrations.rs`, add a const near the others:
```rust
const CREATE_BACKTEST_RUNS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS backtest_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    interval TEXT NOT NULL,
    from_date TEXT NOT NULL,
    to_date TEXT NOT NULL,
    strategy_kind TEXT NOT NULL,
    config_json TEXT NOT NULL,
    summary_json TEXT NOT NULL
);";
```
And register it at the end of the `run_migrations` list (use the next free number; e.g.):
```rust
run_migration(conn, "056_backtest_runs", CREATE_BACKTEST_RUNS_TABLE)?;
```
> Verify the highest existing migration number first and use the next integer.

- [ ] **Step 2: Create the CRUD module**

`src-tauri/src/db/sqlite/backtest.rs`:
```rust
//! backtest_runs persistence (run config + summary metrics only).

use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRunRecord {
    pub id: i64,
    pub created_at: String,
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
    pub from_date: String,
    pub to_date: String,
    pub strategy_kind: String,
    pub config_json: String,
    pub summary_json: String,
}

/// Insert a run; returns the new row id.
pub fn insert_run(
    conn: &Connection,
    created_at: &str,
    symbol: &str,
    exchange: &str,
    interval: &str,
    from_date: &str,
    to_date: &str,
    strategy_kind: &str,
    config_json: &str,
    summary_json: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO backtest_runs
         (created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json],
    )?;
    Ok(conn.last_insert_rowid())
}

/// List runs, newest first.
pub fn list_runs(conn: &Connection) -> Result<Vec<BacktestRunRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json
         FROM backtest_runs ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(BacktestRunRecord {
            id: r.get(0)?, created_at: r.get(1)?, symbol: r.get(2)?, exchange: r.get(3)?,
            interval: r.get(4)?, from_date: r.get(5)?, to_date: r.get(6)?,
            strategy_kind: r.get(7)?, config_json: r.get(8)?, summary_json: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Fetch one run's config_json by id (for reload).
pub fn get_run(conn: &Connection, id: i64) -> Result<Option<BacktestRunRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, symbol, exchange, interval, from_date, to_date, strategy_kind, config_json, summary_json
         FROM backtest_runs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |r| {
        Ok(BacktestRunRecord {
            id: r.get(0)?, created_at: r.get(1)?, symbol: r.get(2)?, exchange: r.get(3)?,
            interval: r.get(4)?, from_date: r.get(5)?, to_date: r.get(6)?,
            strategy_kind: r.get(7)?, config_json: r.get(8)?, summary_json: r.get(9)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}
```

- [ ] **Step 3: Register the module**

In `src-tauri/src/db/sqlite/mod.rs`, add `pub mod backtest;` alongside the other db submodules.

- [ ] **Step 4: Verify compile**

Run: `cd src-tauri && cargo check`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/sqlite/backtest.rs src-tauri/src/db/sqlite/mod.rs src-tauri/src/db/sqlite/migrations.rs
git commit -m "feat(backtest): backtest_runs table + CRUD"
```

---

## Task 10: Tauri commands

**Files:**
- Create: `src-tauri/src/commands/backtest.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Create the command module**

`src-tauri/src/commands/backtest.rs`:
```rust
//! Backtest commands.

use crate::db::sqlite::backtest::{self as bt_db, BacktestRunRecord};
use crate::error::Result;
use crate::services::backtest_service::{BacktestConfig, BacktestResult, BacktestService};
use crate::state::AppState;
use tauri::State;

/// Run a backtest from a config (no persistence).
#[tauri::command]
pub async fn run_backtest(state: State<'_, AppState>, config: BacktestConfig) -> Result<BacktestResult> {
    BacktestService::run_for_config(&state, config).await
}

/// Persist a completed run's config + summary; returns the new id.
#[tauri::command]
pub async fn save_backtest_run(
    state: State<'_, AppState>,
    config: BacktestConfig,
    summary_json: String,
) -> Result<i64> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let strategy_kind = match &config.strategy {
        crate::services::backtest_service::StrategySpec::SmaCrossover { .. } => "SmaCrossover",
        crate::services::backtest_service::StrategySpec::EmaCrossover { .. } => "EmaCrossover",
        crate::services::backtest_service::StrategySpec::RsiThreshold { .. } => "RsiThreshold",
        crate::services::backtest_service::StrategySpec::MacdCross { .. } => "MacdCross",
        crate::services::backtest_service::StrategySpec::BollingerReversion { .. } => "BollingerReversion",
    };
    let config_json = serde_json::to_string(&config)
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    let conn = state.sqlite.conn();
    let id = bt_db::insert_run(
        &conn, &created_at, &config.symbol, &config.exchange, &config.interval,
        &config.from_date, &config.to_date, strategy_kind, &config_json, &summary_json,
    )?;
    Ok(id)
}

/// List saved runs (newest first).
#[tauri::command]
pub async fn list_backtest_runs(state: State<'_, AppState>) -> Result<Vec<BacktestRunRecord>> {
    let conn = state.sqlite.conn();
    bt_db::list_runs(&conn)
}

/// Fetch one saved run by id.
#[tauri::command]
pub async fn get_backtest_run(state: State<'_, AppState>, id: i64) -> Result<Option<BacktestRunRecord>> {
    let conn = state.sqlite.conn();
    bt_db::get_run(&conn, id)
}
```

> **Verify** how other commands obtain a `Connection` from `state.sqlite` (e.g. `state.sqlite.conn()` vs a pool guard). Match the existing pattern exactly — grep `state.sqlite.` in `src/commands/`.

- [ ] **Step 2: Register the module + commands**

In `src-tauri/src/commands/mod.rs` add `pub mod backtest;`.
In `src-tauri/src/lib.rs` `generate_handler!`, add:
```rust
commands::backtest::run_backtest,
commands::backtest::save_backtest_run,
commands::backtest::list_backtest_runs,
commands::backtest::get_backtest_run,
```

- [ ] **Step 3: Verify compile + run full test suite**

Run: `cd src-tauri && cargo test --lib`
Expected: all tests pass (including the backtest tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/backtest.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(backtest): Tauri commands (run/save/list/get)"
```

---

## Task 11: Frontend API bindings + types

**Files:**
- Create: `src/api/backtest.ts`
- Modify: `src/api/tauri-client.ts`

- [ ] **Step 1: Create typed bindings**

`src/api/backtest.ts`:
```ts
import { invoke } from '@tauri-apps/api/core'

export type StrategySpec =
  | { kind: 'SmaCrossover'; fast: number; slow: number }
  | { kind: 'EmaCrossover'; fast: number; slow: number }
  | { kind: 'RsiThreshold'; period: number; oversold: number; overbought: number }
  | { kind: 'MacdCross'; fast: number; slow: number; signal: number }
  | { kind: 'BollingerReversion'; period: number; std_dev: number }

export type Sizing =
  | { kind: 'AllIn' }
  | { kind: 'FixedFraction'; value: number }
  | { kind: 'FixedShares'; value: number }

export interface Costs { commission_per_trade: number; slippage_bps: number; reg_fees_enabled: boolean }
export interface TaxConfig { st_rate: number; lt_rate: number }

export interface BacktestConfig {
  symbol: string
  exchange: string
  interval: string
  from_date: string
  to_date: string
  starting_capital: number
  sizing: Sizing
  costs: Costs
  tax: TaxConfig
  fractional: boolean
  risk_free_rate: number
  strategy: StrategySpec
}

export interface Trade {
  entry_time: string; exit_time: string; entry_price: number; exit_price: number
  shares: number; pnl_after_fees: number; fees: number; holding_days: number; long_term: boolean
}
export interface EquityPoint { timestamp: string; equity: number; drawdown: number }
export interface BenchmarkResult { total_return: number; cagr: number; max_drawdown: number }
export interface BacktestMetrics {
  total_return: number; cagr: number; volatility: number; sharpe: number; sortino: number
  calmar: number; max_drawdown: number; max_dd_peak: string; max_dd_trough: string
  num_trades: number; win_rate: number; avg_win: number; avg_loss: number; profit_factor: number
  max_consecutive_losses: number; avg_holding_days: number; time_in_market: number
  total_fees: number; st_tax: number; lt_tax: number; total_tax: number
  net_total_return: number; net_cagr: number
}
export interface BacktestResult {
  config: BacktestConfig; equity_curve: EquityPoint[]; trades: Trade[]
  metrics: BacktestMetrics; benchmark: BenchmarkResult; warnings: string[]
}

export interface BacktestRunRecord {
  id: number; created_at: string; symbol: string; exchange: string; interval: string
  from_date: string; to_date: string; strategy_kind: string; config_json: string; summary_json: string
}

export const backtestApi = {
  run: (config: BacktestConfig) => invoke<BacktestResult>('run_backtest', { config }),
  save: (config: BacktestConfig, summaryJson: string) =>
    invoke<number>('save_backtest_run', { config, summaryJson }),
  list: () => invoke<BacktestRunRecord[]>('list_backtest_runs'),
  get: (id: number) => invoke<BacktestRunRecord | null>('get_backtest_run', { id }),
}
```

- [ ] **Step 2: (Optional) re-export from tauri-client**

If the app centralizes commands in `src/api/tauri-client.ts`, add `export { backtestApi } from './backtest'` for consistency. Otherwise import `backtestApi` directly in the page.

- [ ] **Step 3: Typecheck**

Run: `npx tsc --noEmit -p tsconfig.json`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src/api/backtest.ts src/api/tauri-client.ts
git commit -m "feat(backtest): frontend API bindings + types"
```

---

## Task 12: Backtest page (config + results)

**Files:**
- Create: `src/pages/Backtest.tsx`
- Modify: `src/App.tsx`, `src/config/navigation.ts`

- [ ] **Step 1: Build the page**

Create `src/pages/Backtest.tsx`. Follow the existing page patterns (see `WebSocketTest.tsx` / `quant/QuantPage.tsx`) for the shell, design-standard spacing, and the bare `help-circle-outline` icon. The page must:
- Render a **config panel**: strategy `<select>` (5 options) with strategy-specific param inputs; symbol + exchange; from/to date; starting capital; sizing select; slippage bps; reg-fees checkbox; tax ST/LT % inputs; fractional checkbox; a **Run** button.
- On Run: build a `BacktestConfig` (convert percent inputs to fractions: tax 35 → 0.35), call `backtestApi.run`, set result/loading/error state.
- Render a **results panel** when a result exists:
  - **Equity curve** using `lightweight-charts` (reuse the chart setup pattern from `HistorifyCharts.tsx`): a line series for strategy equity + a second line series for a buy&hold curve derived from `benchmark` (or just annotate benchmark return in the metrics table if a full B&H curve isn't returned — the engine returns only benchmark summary, so plot strategy equity and show benchmark figures in the metrics table).
  - **Drawdown** as an area series from `equity_curve[].drawdown`.
  - **Metrics table** (two columns: gross vs net-of-tax where applicable) with `tabular-nums`.
  - **Trade list** table: entry/exit time, shares, entry/exit price, P&L, holding days, term (ST/LT). Apply the project table interaction spec (selection, etc.) per `table-interaction-spec.md`.
  - **Export CSV** button → build CSV from `result.trades` and call `saveTextFile('backtest-trades.csv', csv)` from `src/lib/exportFile.ts`.
  - **Save run** button → `backtestApi.save(config, JSON.stringify(result.metrics))`, toast the new id.
- Render **loading** (skeleton matching layout), **empty** ("Configure a backtest to begin"), and **error** (inline message + retry) states.
- Include the contextual help icon.

> Keep the file focused. If it exceeds ~400 lines, split the results visualization into `src/components/backtest/` subcomponents (EquityChart, MetricsTable, TradeTable).

- [ ] **Step 2: Register the route**

In `src/App.tsx`: add `const Backtest = lazy(() => import('@/pages/Backtest'))` and a `<Route path="/backtest" element={<Backtest />} />` inside the authenticated `<Routes>` block (match where `WebSocketTest` is routed).

- [ ] **Step 3: Add the nav item**

In `src/config/navigation.ts`, add a `NavItem` (e.g. `{ href: '/backtest', label: 'Backtest', icon: FlaskConical }`) in the appropriate section. Reuse an already-imported icon (`FlaskConical` is imported) to avoid touching the import list unnecessarily.

- [ ] **Step 4: Typecheck + build**

Run: `npx tsc --noEmit -p tsconfig.json`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src/pages/Backtest.tsx src/App.tsx src/config/navigation.ts
git commit -m "feat(backtest): results page (config, equity/drawdown charts, trades, CSV)"
```

---

## Task 13: End-to-end verification

**Files:** none (verification only)

- [ ] **Step 1: Full backend test suite**

Run: `cd src-tauri && cargo test --lib`
Expected: all pass, including `backtest_service::tests::*`.

- [ ] **Step 2: Frontend typecheck**

Run: `npx tsc --noEmit -p tsconfig.json`
Expected: no errors.

- [ ] **Step 3: Manual smoke (document results)**

Launch the app, download daily history for a US symbol via Historify (e.g. AAPL on NASDAQ), open **Backtest**, run **SMA Crossover 20/50** over a multi-year range. Confirm: equity curve renders, metrics populate, trade list shows round trips, gross vs net-of-tax differ, CSV export writes a file, Save run + reload works. Note anything off.

- [ ] **Step 4: Final commit (if any fixups)**

```bash
git add -A
git commit -m "test(backtest): end-to-end verification fixups"
```

---

## Self-Review Notes (author)

- **Spec coverage:** strategies (Task 2–3), no-look-ahead next-open fills (Task 6), US costs + reg fees (Task 4), capital-gains bucket-netted tax (Task 5, refines the spec's "per-trade" wording to a consistent, testable bucket model — gross vs net still reported), metrics reusing quant_service + benchmark (Task 7), validation/errors (Task 8), persistence (Task 9), commands (Task 10), API (Task 11), page with charts/CSV/states/help icon (Task 12). All covered.
- **Tax model clarification vs spec:** the spec says "tax computed per closed trade" and "losses offset gains within the run." The plan implements this as **per-holding-bucket netting** (ST and LT buckets, losses offset gains within a bucket, no cross-bucket offset/carryforward), which is the consistent, testable realization of that intent. Flag for the user during spec review if cross-bucket offsetting is desired instead.
- **Interval note:** Sharpe/Sortino/volatility annualize assuming ~daily bars (quant_service uses `TRADING_DAYS_PER_YEAR`). CAGR uses real calendar time and is interval-agnostic. Documented limitation; daily is the default interval.
- **Verify-before-use flags:** `compute_macd` arity (Task 3), `state.sqlite` connection accessor (Task 10), next free migration number (Task 9), exact `generate_handler!` location (Task 10).
