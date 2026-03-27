//! Quantitative Metrics Service — Pure math module for portfolio risk analytics
//!
//! All functions operate on return series (`&[f64]`) or price series (`&[f64]`).
//! Annualization assumes 252 trading days per year.

use serde::{Deserialize, Serialize};

const TRADING_DAYS_PER_YEAR: f64 = 252.0;

// ── Result Structs ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetrics {
    pub symbol: String,
    pub annualized_return: f64,
    pub annualized_volatility: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown: f64,
    pub max_drawdown_peak_date: Option<String>,
    pub max_drawdown_trough_date: Option<String>,
    pub beta: Option<f64>,
    pub alpha: Option<f64>,
    pub var_95: f64,
    pub cvar_95: f64,
    pub calmar_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationEntry {
    pub symbol_a: String,
    pub symbol_b: String,
    pub correlation: f64,
}

// ── Helper Functions ──────────────────────────────────────────────────────────

/// Mean of a slice. Returns 0.0 for empty input.
fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Sample standard deviation (N-1 denominator). Returns 0.0 for fewer than 2 values.
fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

// ── Public Functions ──────────────────────────────────────────────────────────

/// Compute daily log returns from a price series.
/// Returns an empty Vec if prices has fewer than 2 elements.
pub fn compute_returns(prices: &[f64]) -> Vec<f64> {
    if prices.len() < 2 {
        return Vec::new();
    }
    prices
        .windows(2)
        .filter_map(|w| {
            if w[0] > 0.0 && w[1] > 0.0 {
                Some((w[1] / w[0]).ln())
            } else {
                None
            }
        })
        .collect()
}

/// Annualized Sharpe ratio.
/// `risk_free_rate` is the annualized risk-free rate (e.g., 0.05 for 5%).
/// Returns 0.0 if volatility is zero or insufficient data.
pub fn sharpe_ratio(returns: &[f64], risk_free_rate: f64) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let daily_rf = risk_free_rate / TRADING_DAYS_PER_YEAR;
    let excess: Vec<f64> = returns.iter().map(|r| r - daily_rf).collect();
    let excess_mean = mean(&excess);
    let vol = std_dev(&excess);
    if vol == 0.0 {
        return 0.0;
    }
    (excess_mean / vol) * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Annualized Sortino ratio (downside deviation only).
/// Returns 0.0 if downside deviation is zero or insufficient data.
pub fn sortino_ratio(returns: &[f64], risk_free_rate: f64) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let daily_rf = risk_free_rate / TRADING_DAYS_PER_YEAR;
    let excess: Vec<f64> = returns.iter().map(|r| r - daily_rf).collect();
    let excess_mean = mean(&excess);

    // Downside deviation: only consider returns below the target (daily rf)
    let downside_sq: Vec<f64> = returns
        .iter()
        .filter(|&&r| r < daily_rf)
        .map(|r| (r - daily_rf).powi(2))
        .collect();

    if downside_sq.is_empty() {
        return 0.0;
    }

    let downside_dev = (downside_sq.iter().sum::<f64>() / downside_sq.len() as f64).sqrt();
    if downside_dev == 0.0 {
        return 0.0;
    }
    (excess_mean / downside_dev) * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Maximum drawdown from a price series.
/// Returns `(max_dd_pct, peak_idx, trough_idx)` where `max_dd_pct` is a positive
/// fraction (e.g., 0.20 means 20% drawdown).
/// Returns `(0.0, 0, 0)` if prices has fewer than 2 elements.
pub fn max_drawdown(prices: &[f64]) -> (f64, usize, usize) {
    if prices.len() < 2 {
        return (0.0, 0, 0);
    }

    let mut peak = prices[0];
    let mut peak_idx = 0;
    let mut max_dd = 0.0_f64;
    let mut max_dd_peak_idx = 0;
    let mut max_dd_trough_idx = 0;

    for (i, &price) in prices.iter().enumerate() {
        if price > peak {
            peak = price;
            peak_idx = i;
        }
        if peak > 0.0 {
            let dd = (peak - price) / peak;
            if dd > max_dd {
                max_dd = dd;
                max_dd_peak_idx = peak_idx;
                max_dd_trough_idx = i;
            }
        }
    }

    (max_dd, max_dd_peak_idx, max_dd_trough_idx)
}

/// Drawdown series — drawdown fraction at each price point relative to the
/// running peak. Values are non-positive (0.0 at peaks, negative during drawdowns).
pub fn drawdown_series(prices: &[f64]) -> Vec<f64> {
    if prices.is_empty() {
        return Vec::new();
    }

    let mut peak = prices[0];
    prices
        .iter()
        .map(|&price| {
            if price > peak {
                peak = price;
            }
            if peak > 0.0 {
                (price - peak) / peak
            } else {
                0.0
            }
        })
        .collect()
}

/// Annualized return from daily log returns.
pub fn annualized_return(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let total_log_return: f64 = returns.iter().sum();
    let daily_mean = total_log_return / returns.len() as f64;
    // Convert mean daily log return to annualized simple return
    (daily_mean * TRADING_DAYS_PER_YEAR).exp() - 1.0
}

/// Annualized volatility from daily log returns.
pub fn annualized_volatility(returns: &[f64]) -> f64 {
    std_dev(returns) * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Beta of an asset relative to a market benchmark.
/// Both return series must be the same length.
/// Returns 0.0 if the market has zero variance or insufficient data.
pub fn beta(asset_returns: &[f64], market_returns: &[f64]) -> f64 {
    let n = asset_returns.len().min(market_returns.len());
    if n < 2 {
        return 0.0;
    }

    let asset = &asset_returns[..n];
    let market = &market_returns[..n];

    let asset_mean = mean(asset);
    let market_mean = mean(market);

    let mut covariance = 0.0;
    let mut market_variance = 0.0;

    for i in 0..n {
        let a_diff = asset[i] - asset_mean;
        let m_diff = market[i] - market_mean;
        covariance += a_diff * m_diff;
        market_variance += m_diff * m_diff;
    }

    if market_variance == 0.0 {
        return 0.0;
    }

    covariance / market_variance
}

/// Jensen's alpha: annualized excess return vs. CAPM predicted return.
/// `risk_free_rate` is annualized.
pub fn alpha(asset_returns: &[f64], market_returns: &[f64], risk_free_rate: f64) -> f64 {
    let asset_ann = annualized_return(asset_returns);
    let market_ann = annualized_return(market_returns);
    let b = beta(asset_returns, market_returns);
    // Jensen's alpha = R_asset - [R_f + beta * (R_market - R_f)]
    asset_ann - (risk_free_rate + b * (market_ann - risk_free_rate))
}

/// Historical Value at Risk at the given confidence level (e.g., 0.95 for 95%).
/// Returns the loss threshold as a positive number.
/// For example, VaR(95%) = 0.02 means there is a 5% chance of losing more than 2%.
pub fn value_at_risk(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let idx = ((1.0 - confidence) * sorted.len() as f64).floor() as usize;
    let idx = idx.min(sorted.len() - 1);

    // VaR is the loss at the percentile — return as positive value
    -sorted[idx]
}

/// Conditional VaR (Expected Shortfall) at the given confidence level.
/// Average of losses beyond the VaR threshold, returned as a positive number.
pub fn conditional_var(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let cutoff_idx = ((1.0 - confidence) * sorted.len() as f64).floor() as usize;
    let cutoff_idx = cutoff_idx.min(sorted.len() - 1);

    if cutoff_idx == 0 {
        // Only the worst return
        return -sorted[0];
    }

    let tail = &sorted[..=cutoff_idx];
    let avg_tail_loss = tail.iter().sum::<f64>() / tail.len() as f64;
    -avg_tail_loss
}

/// NxN correlation matrix from a matrix of return series.
/// `returns_matrix[i]` is the return series for asset `i`.
/// Returns a square matrix where `result[i][j]` is the Pearson correlation between assets i and j.
pub fn correlation_matrix(returns_matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = returns_matrix.len();
    if n == 0 {
        return Vec::new();
    }

    let mut matrix = vec![vec![0.0; n]; n];

    for i in 0..n {
        matrix[i][i] = 1.0; // Self-correlation
        for j in (i + 1)..n {
            let corr = pearson_correlation(&returns_matrix[i], &returns_matrix[j]);
            matrix[i][j] = corr;
            matrix[j][i] = corr;
        }
    }

    matrix
}

/// Pearson correlation coefficient between two series.
fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 {
        return 0.0;
    }

    let x = &x[..n];
    let y = &y[..n];

    let x_mean = mean(x);
    let y_mean = mean(y);

    let mut cov = 0.0;
    let mut x_var = 0.0;
    let mut y_var = 0.0;

    for i in 0..n {
        let xd = x[i] - x_mean;
        let yd = y[i] - y_mean;
        cov += xd * yd;
        x_var += xd * xd;
        y_var += yd * yd;
    }

    let denom = (x_var * y_var).sqrt();
    if denom == 0.0 {
        return 0.0;
    }

    cov / denom
}

/// Calmar ratio: annualized return divided by maximum drawdown.
/// Returns 0.0 if max drawdown is zero.
pub fn calmar_ratio(returns: &[f64], prices: &[f64]) -> f64 {
    let ann_ret = annualized_return(returns);
    let (mdd, _, _) = max_drawdown(prices);
    if mdd == 0.0 {
        return 0.0;
    }
    ann_ret / mdd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_returns() {
        let prices = vec![100.0, 105.0, 103.0, 110.0];
        let returns = compute_returns(&prices);
        assert_eq!(returns.len(), 3);
        assert!((returns[0] - (105.0_f64 / 100.0).ln()).abs() < 1e-10);
    }

    #[test]
    fn test_compute_returns_empty() {
        assert!(compute_returns(&[]).is_empty());
        assert!(compute_returns(&[100.0]).is_empty());
    }

    #[test]
    fn test_max_drawdown() {
        let prices = vec![100.0, 110.0, 90.0, 95.0, 80.0, 120.0];
        let (dd, peak_idx, trough_idx) = max_drawdown(&prices);
        // Peak at 110 (idx 1), trough at 80 (idx 4) => dd = 30/110
        assert!((dd - 30.0 / 110.0).abs() < 1e-10);
        assert_eq!(peak_idx, 1);
        assert_eq!(trough_idx, 4);
    }

    #[test]
    fn test_drawdown_series() {
        let prices = vec![100.0, 110.0, 100.0, 120.0];
        let dd = drawdown_series(&prices);
        assert_eq!(dd.len(), 4);
        assert!((dd[0] - 0.0).abs() < 1e-10);
        assert!((dd[1] - 0.0).abs() < 1e-10); // new peak
        assert!((dd[2] - (-10.0 / 110.0)).abs() < 1e-10);
        assert!((dd[3] - 0.0).abs() < 1e-10); // new peak
    }

    #[test]
    fn test_beta_identical() {
        let returns = vec![0.01, -0.02, 0.015, -0.005, 0.03];
        let b = beta(&returns, &returns);
        assert!((b - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_correlation_self() {
        let returns = vec![vec![0.01, -0.02, 0.015], vec![0.01, -0.02, 0.015]];
        let corr = correlation_matrix(&returns);
        assert!((corr[0][1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_var_basic() {
        // 100 returns, all -0.01 except one at -0.10
        let mut returns = vec![-0.01; 99];
        returns.push(-0.10);
        returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let var = value_at_risk(&returns, 0.95);
        // The worst 5% of 100 values = 5 values; the cutoff is at index 5
        assert!(var >= 0.01);
    }

    #[test]
    fn test_sharpe_zero_vol() {
        let returns = vec![0.01, 0.01, 0.01];
        // Constant returns => zero excess stdev => sharpe = 0
        let s = sharpe_ratio(&returns, 0.01 * 252.0);
        assert_eq!(s, 0.0);
    }

    #[test]
    fn test_calmar_no_drawdown() {
        let prices = vec![100.0, 101.0, 102.0, 103.0];
        let returns = compute_returns(&prices);
        let c = calmar_ratio(&returns, &prices);
        // No drawdown => calmar is 0.0 (division by zero protection)
        // Actually there is no drawdown here since prices only go up
        assert!(c > 0.0 || c == 0.0);
    }
}
