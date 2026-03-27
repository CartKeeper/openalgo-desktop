//! Technical Indicator Computation Service
//!
//! Computes technical indicators (SMA, EMA, RSI, MACD, Bollinger Bands, VWAP)
//! from OHLCV data using the `ta` crate.

use ta::indicators::{
    BollingerBands, ExponentialMovingAverage, MovingAverageConvergenceDivergence,
    RelativeStrengthIndex, SimpleMovingAverage,
};
use ta::Next;
use serde::{Deserialize, Serialize};

/// Result of computing a single indicator across a series of data points.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorResult {
    pub name: String,
    pub values: Vec<IndicatorPoint>,
}

/// A single computed indicator value at a point in time.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorPoint {
    pub timestamp: String,
    pub value: f64,
    /// Secondary value: MACD signal line, Bollinger upper band, etc.
    pub secondary_value: Option<f64>,
    /// Tertiary value: MACD histogram, Bollinger lower band, etc.
    pub tertiary_value: Option<f64>,
}

/// OHLCV data point used as input for indicator computation.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhlcvData {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Compute Simple Moving Average (SMA) over closing prices.
pub fn compute_sma(data: &[OhlcvData], period: usize) -> IndicatorResult {
    let mut sma = SimpleMovingAverage::new(period).expect("Invalid SMA period");
    let values = data
        .iter()
        .map(|bar| {
            let value = sma.next(bar.close);
            IndicatorPoint {
                timestamp: bar.timestamp.clone(),
                value,
                secondary_value: None,
                tertiary_value: None,
            }
        })
        .collect();

    IndicatorResult {
        name: format!("SMA_{}", period),
        values,
    }
}

/// Compute Exponential Moving Average (EMA) over closing prices.
pub fn compute_ema(data: &[OhlcvData], period: usize) -> IndicatorResult {
    let mut ema = ExponentialMovingAverage::new(period).expect("Invalid EMA period");
    let values = data
        .iter()
        .map(|bar| {
            let value = ema.next(bar.close);
            IndicatorPoint {
                timestamp: bar.timestamp.clone(),
                value,
                secondary_value: None,
                tertiary_value: None,
            }
        })
        .collect();

    IndicatorResult {
        name: format!("EMA_{}", period),
        values,
    }
}

/// Compute Relative Strength Index (RSI) over closing prices.
pub fn compute_rsi(data: &[OhlcvData], period: usize) -> IndicatorResult {
    let mut rsi = RelativeStrengthIndex::new(period).expect("Invalid RSI period");
    let values = data
        .iter()
        .map(|bar| {
            let value = rsi.next(bar.close);
            IndicatorPoint {
                timestamp: bar.timestamp.clone(),
                value,
                secondary_value: None,
                tertiary_value: None,
            }
        })
        .collect();

    IndicatorResult {
        name: format!("RSI_{}", period),
        values,
    }
}

/// Compute Moving Average Convergence Divergence (MACD).
///
/// - `value` = MACD line (fast EMA - slow EMA)
/// - `secondary_value` = signal line (EMA of MACD)
/// - `tertiary_value` = histogram (MACD - signal)
pub fn compute_macd(
    data: &[OhlcvData],
    fast: usize,
    slow: usize,
    signal: usize,
) -> IndicatorResult {
    let mut macd =
        MovingAverageConvergenceDivergence::new(fast, slow, signal).expect("Invalid MACD params");
    let values = data
        .iter()
        .map(|bar| {
            let output = macd.next(bar.close);
            IndicatorPoint {
                timestamp: bar.timestamp.clone(),
                value: output.macd,
                secondary_value: Some(output.signal),
                tertiary_value: Some(output.histogram),
            }
        })
        .collect();

    IndicatorResult {
        name: format!("MACD_{}_{}", fast, slow),
        values,
    }
}

/// Compute Bollinger Bands.
///
/// - `value` = middle band (SMA)
/// - `secondary_value` = upper band (SMA + multiplier * StdDev)
/// - `tertiary_value` = lower band (SMA - multiplier * StdDev)
pub fn compute_bollinger(
    data: &[OhlcvData],
    period: usize,
    multiplier: f64,
) -> IndicatorResult {
    let mut bb = BollingerBands::new(period, multiplier).expect("Invalid Bollinger params");
    let values = data
        .iter()
        .map(|bar| {
            let output = bb.next(bar.close);
            IndicatorPoint {
                timestamp: bar.timestamp.clone(),
                value: output.average,
                secondary_value: Some(output.upper),
                tertiary_value: Some(output.lower),
            }
        })
        .collect();

    IndicatorResult {
        name: format!("BOLLINGER_{}", period),
        values,
    }
}

/// Compute Volume Weighted Average Price (VWAP).
///
/// Manual calculation: cumulative(typical_price * volume) / cumulative(volume)
/// where typical_price = (high + low + close) / 3.
pub fn compute_vwap(data: &[OhlcvData]) -> IndicatorResult {
    let mut cumulative_tp_vol = 0.0_f64;
    let mut cumulative_vol = 0.0_f64;

    let values = data
        .iter()
        .map(|bar| {
            let typical_price = (bar.high + bar.low + bar.close) / 3.0;
            cumulative_tp_vol += typical_price * bar.volume;
            cumulative_vol += bar.volume;

            let vwap = if cumulative_vol > 0.0 {
                cumulative_tp_vol / cumulative_vol
            } else {
                typical_price
            };

            IndicatorPoint {
                timestamp: bar.timestamp.clone(),
                value: vwap,
                secondary_value: None,
                tertiary_value: None,
            }
        })
        .collect();

    IndicatorResult {
        name: "VWAP".to_string(),
        values,
    }
}

/// Dispatch indicator computation by name and params.
///
/// Supported indicator names (case-insensitive):
/// - `"SMA"` — params: `{ "period": usize }`
/// - `"EMA"` — params: `{ "period": usize }`
/// - `"RSI"` — params: `{ "period": usize }`
/// - `"MACD"` — params: `{ "fast": usize, "slow": usize, "signal": usize }`
/// - `"BOLLINGER"` — params: `{ "period": usize, "multiplier": f64 }`
/// - `"VWAP"` — no params required
pub fn compute_indicator(
    data: &[OhlcvData],
    indicator_name: &str,
    params: &serde_json::Value,
) -> Result<IndicatorResult, String> {
    match indicator_name.to_uppercase().as_str() {
        "SMA" => {
            let period = params
                .get("period")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| "SMA requires 'period' parameter (positive integer)".to_string())?;
            if period == 0 {
                return Err("SMA period must be greater than 0".to_string());
            }
            Ok(compute_sma(data, period))
        }
        "EMA" => {
            let period = params
                .get("period")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| "EMA requires 'period' parameter (positive integer)".to_string())?;
            if period == 0 {
                return Err("EMA period must be greater than 0".to_string());
            }
            Ok(compute_ema(data, period))
        }
        "RSI" => {
            let period = params
                .get("period")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| "RSI requires 'period' parameter (positive integer)".to_string())?;
            if period == 0 {
                return Err("RSI period must be greater than 0".to_string());
            }
            Ok(compute_rsi(data, period))
        }
        "MACD" => {
            let fast = params
                .get("fast")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(12);
            let slow = params
                .get("slow")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(26);
            let signal = params
                .get("signal")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(9);
            if fast == 0 || slow == 0 || signal == 0 {
                return Err("MACD fast, slow, and signal periods must be greater than 0".to_string());
            }
            Ok(compute_macd(data, fast, slow, signal))
        }
        "BOLLINGER" => {
            let period = params
                .get("period")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(20);
            let multiplier = params
                .get("multiplier")
                .and_then(|v| v.as_f64())
                .unwrap_or(2.0);
            if period == 0 {
                return Err("Bollinger period must be greater than 0".to_string());
            }
            Ok(compute_bollinger(data, period, multiplier))
        }
        "VWAP" => Ok(compute_vwap(data)),
        other => Err(format!(
            "Unknown indicator '{}'. Supported: SMA, EMA, RSI, MACD, BOLLINGER, VWAP",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> Vec<OhlcvData> {
        vec![
            OhlcvData { timestamp: "2024-01-01".into(), open: 100.0, high: 105.0, low: 98.0, close: 103.0, volume: 1000.0 },
            OhlcvData { timestamp: "2024-01-02".into(), open: 103.0, high: 107.0, low: 101.0, close: 106.0, volume: 1200.0 },
            OhlcvData { timestamp: "2024-01-03".into(), open: 106.0, high: 110.0, low: 104.0, close: 108.0, volume: 1100.0 },
            OhlcvData { timestamp: "2024-01-04".into(), open: 108.0, high: 112.0, low: 106.0, close: 110.0, volume: 900.0 },
            OhlcvData { timestamp: "2024-01-05".into(), open: 110.0, high: 115.0, low: 109.0, close: 114.0, volume: 1500.0 },
        ]
    }

    #[test]
    fn test_compute_sma() {
        let data = sample_data();
        let result = compute_sma(&data, 3);
        assert_eq!(result.name, "SMA_3");
        assert_eq!(result.values.len(), 5);
        // After 3 points: (103 + 106 + 108) / 3 = 105.666...
        assert!((result.values[2].value - 105.666).abs() < 0.01);
    }

    #[test]
    fn test_compute_ema() {
        let data = sample_data();
        let result = compute_ema(&data, 3);
        assert_eq!(result.name, "EMA_3");
        assert_eq!(result.values.len(), 5);
    }

    #[test]
    fn test_compute_rsi() {
        let data = sample_data();
        let result = compute_rsi(&data, 3);
        assert_eq!(result.name, "RSI_3");
        assert_eq!(result.values.len(), 5);
        // RSI values should be between 0 and 100
        for point in &result.values {
            assert!(point.value >= 0.0 && point.value <= 100.0);
        }
    }

    #[test]
    fn test_compute_macd() {
        let data = sample_data();
        let result = compute_macd(&data, 3, 5, 3);
        assert_eq!(result.name, "MACD_3_5");
        assert_eq!(result.values.len(), 5);
        for point in &result.values {
            assert!(point.secondary_value.is_some());
            assert!(point.tertiary_value.is_some());
        }
    }

    #[test]
    fn test_compute_bollinger() {
        let data = sample_data();
        let result = compute_bollinger(&data, 3, 2.0);
        assert_eq!(result.name, "BOLLINGER_3");
        assert_eq!(result.values.len(), 5);
        for point in &result.values {
            // Upper band should be above middle, lower below middle
            assert!(point.secondary_value.unwrap() >= point.value);
            assert!(point.tertiary_value.unwrap() <= point.value);
        }
    }

    #[test]
    fn test_compute_vwap() {
        let data = sample_data();
        let result = compute_vwap(&data);
        assert_eq!(result.name, "VWAP");
        assert_eq!(result.values.len(), 5);
        // First VWAP = typical_price of first bar
        let expected_first = (105.0 + 98.0 + 103.0) / 3.0;
        assert!((result.values[0].value - expected_first).abs() < 0.001);
    }

    #[test]
    fn test_compute_indicator_dispatcher() {
        let data = sample_data();
        let params = serde_json::json!({ "period": 3 });
        let result = compute_indicator(&data, "sma", &params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "SMA_3");
    }

    #[test]
    fn test_compute_indicator_unknown() {
        let data = sample_data();
        let params = serde_json::json!({});
        let result = compute_indicator(&data, "UNKNOWN", &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_indicator_missing_period() {
        let data = sample_data();
        let params = serde_json::json!({});
        let result = compute_indicator(&data, "SMA", &params);
        assert!(result.is_err());
    }
}
