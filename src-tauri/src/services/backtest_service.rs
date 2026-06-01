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
