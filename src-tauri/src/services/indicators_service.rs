//! Technical Indicator Computation Service
//!
//! Computes technical indicators from OHLCV data through the official TA-Lib
//! Rust crate (`ta-lib`), plus the app-computed VWAP, driven by the
//! [`indicator_catalog`]. Results are aligned to the input: the value for bar
//! `i` sits at index `i`, and bars before an indicator's lookback carry `None`
//! (`NaN` in the point form, which serializes as `null`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ta_lib::{Core, MAType, OutRange, RetCode};

use crate::error::{AppError, Result};
use crate::services::indicator_catalog::{self, IndicatorSpec};

/// Result of computing a single indicator across a series of data points.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorResult {
    /// Result id, e.g. `SMA_20`, `MACD_12_26`, `CDLDOJI`.
    pub name: String,
    /// One point per input bar carrying the first three outputs (see [`IndicatorPoint`]).
    pub values: Vec<IndicatorPoint>,
    /// Every output by name, in TA-Lib order, aligned to the input bars:
    /// `outputs[k].1[i]` is output `k` at bar `i`, `None` before the lookback.
    pub outputs: Vec<(String, Vec<Option<f64>>)>,
}

/// A single computed indicator value at a point in time.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorPoint {
    pub timestamp: String,
    /// Primary output (the middle band for BBANDS). `NaN` before the indicator's
    /// lookback, which serializes as `null`.
    pub value: f64,
    /// Secondary output: MACD signal line, Bollinger upper band, etc.
    pub secondary_value: Option<f64>,
    /// Tertiary output: MACD histogram, Bollinger lower band, etc.
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

// ---------------------------------------------------------------------------
// Catalog-driven entry points
// ---------------------------------------------------------------------------

/// Compute one catalog indicator over `data`.
///
/// `params` are keyed by the catalog's parameter names; a missing parameter
/// takes its catalog default, an unknown key or an out-of-range value is an
/// [`AppError::Validation`], and so is a non-finite input in any bar field the
/// indicator reads. A series shorter than the lookback is not an error: every
/// output is `None`.
pub fn compute(
    spec: &IndicatorSpec,
    data: &[OhlcvData],
    params: &HashMap<String, f64>,
) -> Result<IndicatorResult> {
    check_finite(spec, data)?;
    let args = resolve_params(spec, params)?;
    let outputs = if spec.name == "VWAP" {
        vec![vwap_series(data).into_iter().map(Some).collect()]
    } else {
        run_talib(spec, data, &args)?
    };
    Ok(assemble(spec, result_name(spec, &args), data, outputs))
}

/// Dispatch indicator computation by catalog name (case-insensitive).
///
/// `BOLLINGER` is kept as an alias of `BBANDS` for older callers: it accepts
/// `multiplier` (or `std_dev`) for both band widths and keeps the
/// `BOLLINGER_{period}` result id.
pub fn compute_indicator(
    data: &[OhlcvData],
    indicator_name: &str,
    params: &HashMap<String, f64>,
) -> Result<IndicatorResult> {
    let requested = indicator_name.trim().to_ascii_uppercase();
    if requested == "BOLLINGER" {
        let spec = indicator_catalog::find("BBANDS").expect("BBANDS is in the catalog");
        let mut result = compute(spec, data, params)?;
        let period = params.get("period").copied().unwrap_or(spec.params[0].default);
        result.name = format!("BOLLINGER_{}", fmt_num(period));
        return Ok(result);
    }
    let spec = indicator_catalog::find(&requested).ok_or_else(|| {
        AppError::Validation(format!(
            "Unknown indicator '{}'. Use a name from the indicator catalog.",
            indicator_name.trim()
        ))
    })?;
    compute(spec, data, params)
}

// ---------------------------------------------------------------------------
// Legacy entry points (backtester): fixed names and point layout
// ---------------------------------------------------------------------------

/// Simple Moving Average of the closes; result id `SMA_{period}`.
pub fn compute_sma(data: &[OhlcvData], period: usize) -> IndicatorResult {
    legacy("SMA", format!("SMA_{period}"), data, &[("period", period as f64)])
}

/// Exponential Moving Average of the closes (TA-Lib seeds it with the SMA of
/// the first period); result id `EMA_{period}`.
pub fn compute_ema(data: &[OhlcvData], period: usize) -> IndicatorResult {
    legacy("EMA", format!("EMA_{period}"), data, &[("period", period as f64)])
}

/// Relative Strength Index of the closes (Wilder smoothing); result id `RSI_{period}`.
pub fn compute_rsi(data: &[OhlcvData], period: usize) -> IndicatorResult {
    legacy("RSI", format!("RSI_{period}"), data, &[("period", period as f64)])
}

/// MACD: `value` = MACD line, `secondary_value` = signal, `tertiary_value` =
/// histogram; result id `MACD_{fast}_{slow}`.
pub fn compute_macd(data: &[OhlcvData], fast: usize, slow: usize, signal: usize) -> IndicatorResult {
    legacy(
        "MACD",
        format!("MACD_{fast}_{slow}"),
        data,
        &[("fast", fast as f64), ("slow", slow as f64), ("signal", signal as f64)],
    )
}

/// Bollinger Bands: `value` = middle band, `secondary_value` = upper,
/// `tertiary_value` = lower; result id `BOLLINGER_{period}`.
pub fn compute_bollinger(data: &[OhlcvData], period: usize, multiplier: f64) -> IndicatorResult {
    legacy(
        "BBANDS",
        format!("BOLLINGER_{period}"),
        data,
        &[("period", period as f64), ("nb_dev_up", multiplier), ("nb_dev_dn", multiplier)],
    )
}

/// Volume Weighted Average Price (app-computed, cumulative over the series):
/// cumulative(typical_price * volume) / cumulative(volume), where
/// typical_price = (high + low + close) / 3.
pub fn compute_vwap(data: &[OhlcvData]) -> IndicatorResult {
    let spec = indicator_catalog::find("VWAP").expect("VWAP is in the catalog");
    let series = vwap_series(data).into_iter().map(Some).collect();
    assemble(spec, "VWAP".to_string(), data, vec![series])
}

/// Run a catalog indicator for a legacy wrapper. The wrappers are infallible,
/// so a rejected parameter (e.g. an RSI period below TA-Lib's minimum of 2)
/// is logged and yields an all-`None` series instead of a panic.
fn legacy(catalog_name: &str, result_name: String, data: &[OhlcvData], params: &[(&str, f64)]) -> IndicatorResult {
    let spec = indicator_catalog::find(catalog_name).expect("legacy indicator is in the catalog");
    let params: HashMap<String, f64> = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    match compute(spec, data, &params) {
        Ok(mut result) => {
            result.name = result_name;
            result
        }
        Err(err) => {
            tracing::warn!("{result_name}: {err}; returning an empty series");
            let empty = vec![vec![None; data.len()]; spec.outputs.len()];
            assemble(spec, result_name, data, empty)
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Reject a non-finite value in any bar field the indicator reads.
fn check_finite(spec: &IndicatorSpec, data: &[OhlcvData]) -> Result<()> {
    for (i, bar) in data.iter().enumerate() {
        for field in spec.inputs {
            let value = match *field {
                "open" => bar.open,
                "high" => bar.high,
                "low" => bar.low,
                "close" => bar.close,
                "volume" => bar.volume,
                other => {
                    return Err(AppError::Internal(format!("{}: unknown input {other}", spec.name)))
                }
            };
            if !value.is_finite() {
                return Err(AppError::Validation(format!(
                    "Bar {i} has a non-finite value ({field} at {})",
                    bar.timestamp
                )));
            }
        }
    }
    Ok(())
}

/// Request keys accepted in place of a catalog parameter name (older callers).
fn aliases(param: &str) -> &'static [&'static str] {
    match param {
        "nb_dev_up" | "nb_dev_dn" => &["std_dev", "multiplier"],
        _ => &[],
    }
}

/// Resolve the request parameters to TA-Lib's call order, applying defaults
/// and the catalog's ranges.
fn resolve_params(spec: &IndicatorSpec, params: &HashMap<String, f64>) -> Result<Vec<f64>> {
    for key in params.keys() {
        let known = spec
            .params
            .iter()
            .any(|p| p.name == key || aliases(p.name).contains(&key.as_str()));
        if !known {
            let expected: Vec<&str> = spec.params.iter().map(|p| p.name).collect();
            let expected = if expected.is_empty() { "none".to_string() } else { expected.join(", ") };
            return Err(AppError::Validation(format!(
                "{}: unknown parameter '{key}' (expected: {expected})",
                spec.name
            )));
        }
    }
    spec.params
        .iter()
        .map(|p| {
            let value = params
                .get(p.name)
                .or_else(|| aliases(p.name).iter().find_map(|alias| params.get(*alias)))
                .copied()
                .unwrap_or(p.default);
            if !(value.is_finite() && value >= p.min && value <= p.max) {
                return Err(AppError::Validation(format!(
                    "{}: {} must be between {} and {}",
                    spec.name,
                    p.name,
                    fmt_num(p.min),
                    fmt_num(p.max)
                )));
            }
            if p.kind != "float" && value.fract() != 0.0 {
                return Err(AppError::Validation(format!(
                    "{}: {} must be a whole number",
                    spec.name, p.name
                )));
            }
            Ok(value)
        })
        .collect()
}

/// Compact number formatting for ids and messages: `14`, `0.3`, `3e37`.
fn fmt_num(value: f64) -> String {
    if value.abs() >= 1e15 {
        format!("{value:e}")
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

/// Result id: the catalog name followed by the resolved parameters, except
/// MACD, whose legacy id (`MACD_{fast}_{slow}`) the Analysis tab matches on.
fn result_name(spec: &IndicatorSpec, args: &[f64]) -> String {
    if spec.name == "MACD" {
        return format!("MACD_{}_{}", fmt_num(args[0]), fmt_num(args[1]));
    }
    let mut name = spec.name.to_string();
    for arg in args {
        name.push('_');
        name.push_str(&fmt_num(*arg));
    }
    name
}

// ---------------------------------------------------------------------------
// TA-Lib dispatch and alignment
// ---------------------------------------------------------------------------

/// Call the TA-Lib function behind `spec` and return its outputs aligned to the input.
fn run_talib(spec: &IndicatorSpec, data: &[OhlcvData], args: &[f64]) -> Result<Vec<Vec<Option<f64>>>> {
    let len = data.len();
    let n_out = spec.outputs.len();
    if len == 0 {
        return Ok(vec![Vec::new(); n_out]);
    }
    let last = len - 1;
    let open: Vec<f64> = data.iter().map(|b| b.open).collect();
    let high: Vec<f64> = data.iter().map(|b| b.high).collect();
    let low: Vec<f64> = data.iter().map(|b| b.low).collect();
    let close: Vec<f64> = data.iter().map(|b| b.close).collect();
    let volume: Vec<f64> = data.iter().map(|b| b.volume).collect();
    let (o, h, l, c, v) = (&open[..], &high[..], &low[..], &close[..], &volume[..]);

    let core = Core::new();
    let int = |k: usize| args[k] as i32;
    let real = |k: usize| args[k];
    let ma = |k: usize| MAType::try_from(args[k] as i32);

    let mut out0 = vec![0.0; len];
    let mut out1 = vec![0.0; len];
    let mut out2 = vec![0.0; len];
    let mut pattern = vec![0i32; len];

    let call: std::result::Result<OutRange, RetCode> = match spec.name {
        // Overlap
        "SMA" => core.sma(0, last, c, int(0), &mut out0),
        "EMA" => core.ema(0, last, c, int(0), &mut out0),
        "WMA" => core.wma(0, last, c, int(0), &mut out0),
        "DEMA" => core.dema(0, last, c, int(0), &mut out0),
        "TEMA" => core.tema(0, last, c, int(0), &mut out0),
        "KAMA" => core.kama(0, last, c, int(0), &mut out0),
        "T3" => core.t3(0, last, c, int(0), real(1), &mut out0),
        "BBANDS" => ma(3).and_then(|t| {
            core.bbands(0, last, c, int(0), real(1), real(2), t, &mut out0, &mut out1, &mut out2)
        }),
        "SAR" => core.sar(0, last, h, l, real(0), real(1), &mut out0),
        "HT_TRENDLINE" => core.ht_trendline(0, last, c, &mut out0),
        "MIDPOINT" => core.midpoint(0, last, c, int(0), &mut out0),
        // Momentum
        "RSI" => core.rsi(0, last, c, int(0), &mut out0),
        "MACD" => core.macd(0, last, c, int(0), int(1), int(2), &mut out0, &mut out1, &mut out2),
        "STOCH" => ma(2).and_then(|slow_k| {
            ma(4).and_then(|slow_d| {
                core.stoch(0, last, h, l, c, int(0), int(1), slow_k, int(3), slow_d, &mut out0, &mut out1)
            })
        }),
        "STOCHRSI" => ma(3).and_then(|fast_d| {
            core.stochrsi(0, last, c, int(0), int(1), int(2), fast_d, &mut out0, &mut out1)
        }),
        "CCI" => core.cci(0, last, h, l, c, int(0), &mut out0),
        "MFI" => core.mfi(0, last, h, l, c, v, int(0), &mut out0),
        "WILLR" => core.willr(0, last, h, l, c, int(0), &mut out0),
        "ROC" => core.roc(0, last, c, int(0), &mut out0),
        "MOM" => core.mom(0, last, c, int(0), &mut out0),
        "ADX" => core.adx(0, last, h, l, c, int(0), &mut out0),
        "ADXR" => core.adxr(0, last, h, l, c, int(0), &mut out0),
        "AROON" => core.aroon(0, last, h, l, int(0), &mut out0, &mut out1),
        "AROONOSC" => core.aroonosc(0, last, h, l, int(0), &mut out0),
        "PPO" => ma(2).and_then(|t| core.ppo(0, last, c, int(0), int(1), t, &mut out0)),
        "APO" => ma(2).and_then(|t| core.apo(0, last, c, int(0), int(1), t, &mut out0)),
        "CMO" => core.cmo(0, last, c, int(0), &mut out0),
        "TRIX" => core.trix(0, last, c, int(0), &mut out0),
        "ULTOSC" => core.ultosc(0, last, h, l, c, int(0), int(1), int(2), &mut out0),
        "BOP" => core.bop(0, last, o, h, l, c, &mut out0),
        "DX" => core.dx(0, last, h, l, c, int(0), &mut out0),
        "MINUS_DI" => core.minus_di(0, last, h, l, c, int(0), &mut out0),
        "PLUS_DI" => core.plus_di(0, last, h, l, c, int(0), &mut out0),
        // Volatility
        "ATR" => core.atr(0, last, h, l, c, int(0), &mut out0),
        "NATR" => core.natr(0, last, h, l, c, int(0), &mut out0),
        "TRANGE" => core.trange(0, last, h, l, c, &mut out0),
        // Volume
        "OBV" => core.obv(0, last, c, v, &mut out0),
        "AD" => core.ad(0, last, h, l, c, v, &mut out0),
        "ADOSC" => core.adosc(0, last, h, l, c, v, int(0), int(1), &mut out0),
        // Statistics
        "LINEARREG" => core.linearreg(0, last, c, int(0), &mut out0),
        "STDDEV" => core.stddev(0, last, c, int(0), real(1), &mut out0),
        "VAR" => core.var(0, last, c, int(0), real(1), &mut out0),
        "TSF" => core.tsf(0, last, c, int(0), &mut out0),
        // Cycle
        "HT_DCPERIOD" => core.ht_dcperiod(0, last, c, &mut out0),
        // Patterns
        name if name.starts_with("CDL") => {
            let penetration = args.first().copied().unwrap_or(0.0);
            match cdl(&core, name, last, o, h, l, c, penetration, &mut pattern) {
                Some(call) => call,
                None => return Err(not_dispatched(spec)),
            }
        }
        _ => return Err(not_dispatched(spec)),
    };
    let range = call.map_err(|rc| map_retcode(spec, rc))?;

    if spec.name.starts_with("CDL") {
        let as_real: Vec<f64> = pattern.iter().map(|&x| f64::from(x)).collect();
        return Ok(vec![align(&as_real, range, len)]);
    }
    Ok([out0, out1, out2]
        .iter()
        .take(n_out)
        .map(|out| align(out, range, len))
        .collect())
}

/// The 61 candlestick recognizers: OHLC in, −100 / 0 / 100 out.
#[allow(clippy::too_many_arguments)]
fn cdl(
    core: &Core,
    name: &str,
    last: usize,
    o: &[f64],
    h: &[f64],
    l: &[f64],
    c: &[f64],
    penetration: f64,
    out: &mut [i32],
) -> Option<std::result::Result<OutRange, RetCode>> {
    macro_rules! dispatch {
        (plain: [$($pn:literal => $pf:ident),* $(,)?],
         penetration: [$($qn:literal => $qf:ident),* $(,)?]) => {
            match name {
                $($pn => Some(core.$pf(0, last, o, h, l, c, out)),)*
                $($qn => Some(core.$qf(0, last, o, h, l, c, penetration, out)),)*
                _ => None,
            }
        };
    }
    dispatch!(
        plain: [
            "CDL2CROWS" => cdl2crows,
            "CDL3BLACKCROWS" => cdl3blackcrows,
            "CDL3INSIDE" => cdl3inside,
            "CDL3LINESTRIKE" => cdl3linestrike,
            "CDL3OUTSIDE" => cdl3outside,
            "CDL3STARSINSOUTH" => cdl3starsinsouth,
            "CDL3WHITESOLDIERS" => cdl3whitesoldiers,
            "CDLADVANCEBLOCK" => cdladvanceblock,
            "CDLBELTHOLD" => cdlbelthold,
            "CDLBREAKAWAY" => cdlbreakaway,
            "CDLCLOSINGMARUBOZU" => cdlclosingmarubozu,
            "CDLCONCEALBABYSWALL" => cdlconcealbabyswall,
            "CDLCOUNTERATTACK" => cdlcounterattack,
            "CDLDOJI" => cdldoji,
            "CDLDOJISTAR" => cdldojistar,
            "CDLDRAGONFLYDOJI" => cdldragonflydoji,
            "CDLENGULFING" => cdlengulfing,
            "CDLGAPSIDESIDEWHITE" => cdlgapsidesidewhite,
            "CDLGRAVESTONEDOJI" => cdlgravestonedoji,
            "CDLHAMMER" => cdlhammer,
            "CDLHANGINGMAN" => cdlhangingman,
            "CDLHARAMI" => cdlharami,
            "CDLHARAMICROSS" => cdlharamicross,
            "CDLHIGHWAVE" => cdlhighwave,
            "CDLHIKKAKE" => cdlhikkake,
            "CDLHIKKAKEMOD" => cdlhikkakemod,
            "CDLHOMINGPIGEON" => cdlhomingpigeon,
            "CDLIDENTICAL3CROWS" => cdlidentical3crows,
            "CDLINNECK" => cdlinneck,
            "CDLINVERTEDHAMMER" => cdlinvertedhammer,
            "CDLKICKING" => cdlkicking,
            "CDLKICKINGBYLENGTH" => cdlkickingbylength,
            "CDLLADDERBOTTOM" => cdlladderbottom,
            "CDLLONGLEGGEDDOJI" => cdllongleggeddoji,
            "CDLLONGLINE" => cdllongline,
            "CDLMARUBOZU" => cdlmarubozu,
            "CDLMATCHINGLOW" => cdlmatchinglow,
            "CDLONNECK" => cdlonneck,
            "CDLPIERCING" => cdlpiercing,
            "CDLRICKSHAWMAN" => cdlrickshawman,
            "CDLRISEFALL3METHODS" => cdlrisefall3methods,
            "CDLSEPARATINGLINES" => cdlseparatinglines,
            "CDLSHOOTINGSTAR" => cdlshootingstar,
            "CDLSHORTLINE" => cdlshortline,
            "CDLSPINNINGTOP" => cdlspinningtop,
            "CDLSTALLEDPATTERN" => cdlstalledpattern,
            "CDLSTICKSANDWICH" => cdlsticksandwich,
            "CDLTAKURI" => cdltakuri,
            "CDLTASUKIGAP" => cdltasukigap,
            "CDLTHRUSTING" => cdlthrusting,
            "CDLTRISTAR" => cdltristar,
            "CDLUNIQUE3RIVER" => cdlunique3river,
            "CDLUPSIDEGAP2CROWS" => cdlupsidegap2crows,
            "CDLXSIDEGAP3METHODS" => cdlxsidegap3methods,
        ],
        penetration: [
            "CDLABANDONEDBABY" => cdlabandonedbaby,
            "CDLDARKCLOUDCOVER" => cdldarkcloudcover,
            "CDLEVENINGDOJISTAR" => cdleveningdojistar,
            "CDLEVENINGSTAR" => cdleveningstar,
            "CDLMATHOLD" => cdlmathold,
            "CDLMORNINGDOJISTAR" => cdlmorningdojistar,
            "CDLMORNINGSTAR" => cdlmorningstar,
        ]
    )
}

fn not_dispatched(spec: &IndicatorSpec) -> AppError {
    AppError::Internal(format!("{} is in the catalog but has no dispatcher", spec.name))
}

/// Catalog ranges are checked before the call, so TA-Lib's own `BadParam` is
/// unexpected; still, it is reported as a validation problem, not a crash.
fn map_retcode(spec: &IndicatorSpec, rc: RetCode) -> AppError {
    match rc {
        RetCode::BadParam => AppError::Validation(format!(
            "{}: a parameter is outside TA-Lib's accepted range",
            spec.name
        )),
        other => AppError::Internal(format!("{}: TA-Lib returned {other:?}", spec.name)),
    }
}

/// Place TA-Lib's packed output (`count` values starting at `beg_idx`) at the
/// input indices it belongs to; everything else is `None`.
fn align(out: &[f64], range: OutRange, len: usize) -> Vec<Option<f64>> {
    let mut aligned = vec![None; len];
    for (k, value) in out.iter().take(range.count).enumerate() {
        if let Some(slot) = aligned.get_mut(range.beg_idx + k) {
            *slot = Some(*value);
        }
    }
    aligned
}

/// Build the result from aligned outputs.
///
/// The point layout (`value`, `secondary_value`, `tertiary_value`) follows
/// TA-Lib's output order, except BBANDS, whose primary line is the middle band
/// (the legacy layout: value = middle, secondary = upper, tertiary = lower).
fn assemble(
    spec: &IndicatorSpec,
    name: String,
    data: &[OhlcvData],
    outputs: Vec<Vec<Option<f64>>>,
) -> IndicatorResult {
    let order: [usize; 3] = if spec.name == "BBANDS" { [1, 0, 2] } else { [0, 1, 2] };
    let at = |slot: usize, i: usize| outputs.get(order[slot]).and_then(|out| out[i]);
    let values = data
        .iter()
        .enumerate()
        .map(|(i, bar)| IndicatorPoint {
            timestamp: bar.timestamp.clone(),
            value: at(0, i).unwrap_or(f64::NAN),
            secondary_value: at(1, i),
            tertiary_value: at(2, i),
        })
        .collect();
    let outputs = spec
        .outputs
        .iter()
        .map(|n| n.to_string())
        .zip(outputs)
        .collect();
    IndicatorResult { name, values, outputs }
}

/// Cumulative VWAP: cumulative(typical_price * volume) / cumulative(volume).
fn vwap_series(data: &[OhlcvData]) -> Vec<f64> {
    let mut cumulative_tp_vol = 0.0_f64;
    let mut cumulative_vol = 0.0_f64;
    data.iter()
        .map(|bar| {
            let typical_price = (bar.high + bar.low + bar.close) / 3.0;
            cumulative_tp_vol += typical_price * bar.volume;
            cumulative_vol += bar.volume;
            if cumulative_vol > 0.0 {
                cumulative_tp_vol / cumulative_vol
            } else {
                typical_price
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars_from_closes(closes: &[f64]) -> Vec<OhlcvData> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &c)| OhlcvData {
                timestamp: format!("2024-01-{:02}", i + 1),
                open: c,
                high: c + 1.0,
                low: c - 1.0,
                close: c,
                volume: 1000.0,
            })
            .collect()
    }

    fn params(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn spec(name: &str) -> &'static IndicatorSpec {
        indicator_catalog::find(name).expect("catalog entry")
    }

    fn sample_data() -> Vec<OhlcvData> {
        vec![
            OhlcvData { timestamp: "2024-01-01".into(), open: 100.0, high: 105.0, low: 98.0, close: 103.0, volume: 1000.0 },
            OhlcvData { timestamp: "2024-01-02".into(), open: 103.0, high: 107.0, low: 101.0, close: 106.0, volume: 1200.0 },
            OhlcvData { timestamp: "2024-01-03".into(), open: 106.0, high: 110.0, low: 104.0, close: 108.0, volume: 1100.0 },
            OhlcvData { timestamp: "2024-01-04".into(), open: 108.0, high: 112.0, low: 106.0, close: 110.0, volume: 900.0 },
            OhlcvData { timestamp: "2024-01-05".into(), open: 110.0, high: 115.0, low: 109.0, close: 114.0, volume: 1500.0 },
        ]
    }

    // --- alignment -------------------------------------------------------

    #[test]
    fn sma_is_aligned_to_the_input_with_none_before_the_lookback() {
        let data = bars_from_closes(&(1..=10).map(f64::from).collect::<Vec<_>>());
        let r = compute(spec("SMA"), &data, &params(&[("period", 3.0)])).unwrap();
        assert_eq!(r.name, "SMA_3");
        assert_eq!(r.outputs.len(), 1);
        assert_eq!(r.outputs[0].0, "value");
        let v = &r.outputs[0].1;
        assert_eq!(v.len(), 10);
        assert_eq!(&v[0..2], &[None, None]);
        assert_eq!(v[2], Some(2.0));
        assert_eq!(v[9], Some(9.0));
        // The point form mirrors it: NaN before the lookback, the value after.
        assert!(r.values[0].value.is_nan());
        assert!(r.values[1].value.is_nan());
        assert_eq!(r.values[2].value, 2.0);
        assert_eq!(r.values[2].timestamp, "2024-01-03");
    }

    #[test]
    fn ema_seeds_with_the_sma_of_the_first_period() {
        let data = bars_from_closes(&(1..=10).map(f64::from).collect::<Vec<_>>());
        let r = compute(spec("EMA"), &data, &params(&[("period", 3.0)])).unwrap();
        let v = &r.outputs[0].1;
        assert_eq!(&v[0..2], &[None, None]);
        assert_eq!(v[2], Some(2.0)); // (1 + 2 + 3) / 3
        assert_eq!(v[3], Some(3.0)); // 2 + (4 - 2) * 2/(3+1)
        assert_eq!(v[4], Some(4.0));
    }

    #[test]
    fn rsi_on_a_monotonic_rise_is_100_after_the_lookback() {
        let data = bars_from_closes(&(1..=30).map(f64::from).collect::<Vec<_>>());
        let r = compute(spec("RSI"), &data, &params(&[("period", 14.0)])).unwrap();
        assert_eq!(r.name, "RSI_14");
        let v = &r.outputs[0].1;
        assert!(v[..14].iter().all(Option::is_none));
        assert!(v[14..].iter().all(|x| *x == Some(100.0)), "{:?}", &v[14..]);
    }

    // --- multi-output ----------------------------------------------------

    #[test]
    fn macd_returns_three_named_outputs_of_equal_length() {
        let closes: Vec<f64> = (0..40).map(|i| 100.0 + (i as f64 * 0.3).sin() * 10.0).collect();
        let data = bars_from_closes(&closes);
        let r = compute(spec("MACD"), &data, &params(&[("fast", 12.0), ("slow", 26.0), ("signal", 9.0)])).unwrap();
        assert_eq!(r.name, "MACD_12_26");
        let names: Vec<&str> = r.outputs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["macd", "signal", "hist"]);
        for (_, v) in &r.outputs {
            assert_eq!(v.len(), 40);
        }
        // lookback = (26 - 1) + (9 - 1) = 33
        assert!(r.outputs[0].1[32].is_none());
        assert!(r.outputs[0].1[33].is_some());
        let i = 39;
        let (macd, signal, hist) = (r.outputs[0].1[i].unwrap(), r.outputs[1].1[i].unwrap(), r.outputs[2].1[i].unwrap());
        assert!((macd - signal - hist).abs() < 1e-9);
        assert_eq!(r.values[i].value, macd);
        assert_eq!(r.values[i].secondary_value, Some(signal));
        assert_eq!(r.values[i].tertiary_value, Some(hist));
        assert!(r.values[0].value.is_nan());
        assert_eq!(r.values[0].secondary_value, None);
    }

    #[test]
    fn bbands_keeps_the_middle_band_in_value_and_the_bands_in_talib_order() {
        let data = sample_data();
        let r = compute(spec("BBANDS"), &data, &params(&[("period", 3.0), ("std_dev", 2.0)])).unwrap();
        let names: Vec<&str> = r.outputs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["upper", "middle", "lower"]);
        assert!(r.values[0].value.is_nan() && r.values[0].secondary_value.is_none());
        for i in 2..5 {
            let (upper, middle, lower) = (r.outputs[0].1[i].unwrap(), r.outputs[1].1[i].unwrap(), r.outputs[2].1[i].unwrap());
            assert!(upper >= middle && middle >= lower);
            assert_eq!(r.values[i].value, middle);
            assert_eq!(r.values[i].secondary_value, Some(upper));
            assert_eq!(r.values[i].tertiary_value, Some(lower));
        }
        // index 2: middle = (103 + 106 + 108) / 3
        assert!((r.outputs[1].1[2].unwrap() - 105.6667).abs() < 1e-3);
    }

    #[test]
    fn stoch_and_aroon_return_two_aligned_outputs() {
        let closes: Vec<f64> = (0..40).map(|i| 100.0 + (i as f64 * 0.5).sin() * 5.0).collect();
        let data = bars_from_closes(&closes);
        let s = compute(spec("STOCH"), &data, &HashMap::new()).unwrap();
        assert_eq!(s.outputs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["slow_k", "slow_d"]);
        let a = compute(spec("AROON"), &data, &HashMap::new()).unwrap();
        assert_eq!(a.outputs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["aroon_down", "aroon_up"]);
        for r in [&s, &a] {
            for (_, v) in &r.outputs {
                assert_eq!(v.len(), 40);
                assert!(v.last().unwrap().is_some());
            }
        }
    }

    // --- edge cases --------------------------------------------------------

    #[test]
    fn a_series_shorter_than_the_lookback_is_all_none_not_an_error() {
        let data = sample_data(); // 5 bars
        let r = compute(spec("ADX"), &data, &params(&[("period", 14.0)])).unwrap();
        assert_eq!(r.values.len(), 5);
        assert!(r.values.iter().all(|p| p.value.is_nan()));
        assert!(r.outputs[0].1.iter().all(Option::is_none));
        let m = compute(spec("MACD"), &data, &HashMap::new()).unwrap();
        assert!(m.outputs.iter().all(|(_, v)| v.len() == 5 && v.iter().all(Option::is_none)));
        assert!(m.values.iter().all(|p| p.secondary_value.is_none() && p.tertiary_value.is_none()));
    }

    #[test]
    fn an_empty_series_yields_empty_outputs() {
        let r = compute(spec("RSI"), &[], &HashMap::new()).unwrap();
        assert!(r.values.is_empty());
        assert_eq!(r.outputs.len(), 1);
        assert!(r.outputs[0].1.is_empty());
    }

    #[test]
    fn a_non_finite_bar_is_rejected_naming_the_bar() {
        let mut data = bars_from_closes(&(1..=10).map(f64::from).collect::<Vec<_>>());
        data[3].close = f64::NAN;
        let err = compute(spec("SMA"), &data, &params(&[("period", 3.0)])).unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(msg.starts_with("Bar 3 has a non-finite value"), "{msg}"),
            other => panic!("expected a validation error, got {other:?}"),
        }
        // A bad field the indicator does not read is not a reason to refuse.
        data[3].close = 4.0;
        data[3].volume = f64::INFINITY;
        assert!(compute(spec("SMA"), &data, &params(&[("period", 3.0)])).is_ok());
        assert!(compute(spec("OBV"), &data, &HashMap::new()).is_err());
    }

    #[test]
    fn a_parameter_outside_talibs_range_is_rejected_with_the_range() {
        let data = bars_from_closes(&(1..=30).map(f64::from).collect::<Vec<_>>());
        let err = compute(spec("RSI"), &data, &params(&[("period", 1.0)])).unwrap_err();
        match err {
            AppError::Validation(msg) => assert_eq!(msg, "RSI: period must be between 2 and 100000"),
            other => panic!("expected a validation error, got {other:?}"),
        }
        let err = compute(spec("T3"), &data, &params(&[("v_factor", 1.5)])).unwrap_err();
        assert!(matches!(err, AppError::Validation(ref m) if m == "T3: v_factor must be between 0 and 1"), "{err:?}");
        let err = compute(spec("SMA"), &data, &params(&[("period", 2.5)])).unwrap_err();
        assert!(matches!(err, AppError::Validation(ref m) if m == "SMA: period must be a whole number"), "{err:?}");
        let err = compute(spec("SMA"), &data, &params(&[("perod", 3.0)])).unwrap_err();
        assert!(matches!(err, AppError::Validation(ref m) if m.contains("unknown parameter 'perod'")), "{err:?}");
    }

    #[test]
    fn missing_parameters_take_the_catalog_defaults() {
        let data = bars_from_closes(&(1..=40).map(f64::from).collect::<Vec<_>>());
        let r = compute(spec("SMA"), &data, &HashMap::new()).unwrap();
        assert_eq!(r.name, "SMA_30");
        assert!(r.outputs[0].1[28].is_none() && r.outputs[0].1[29].is_some());
    }

    // --- candlestick patterns ------------------------------------------------

    /// 20 bars with one-point bodies and two-point ranges; bar 19 is a doji
    /// (open == close). TA-Lib's doji needs a body <= 10% of the average
    /// high-low range of the previous 10 bars, so it is the only match.
    fn doji_fixture() -> Vec<OhlcvData> {
        (0..20)
            .map(|i| {
                let open = 100.0 + i as f64;
                let close = if i == 19 { open } else { open + 1.0 };
                OhlcvData {
                    timestamp: format!("2024-02-{:02}", i + 1),
                    open,
                    high: open.max(close) + 0.5,
                    low: open.min(close) - 0.5,
                    close,
                    volume: 1000.0,
                }
            })
            .collect()
    }

    #[test]
    fn a_pattern_on_fewer_bars_than_its_lookback_is_all_none() {
        let data = &doji_fixture()[..5];
        let r = compute(spec("CDLDOJI"), data, &HashMap::new()).unwrap();
        assert_eq!(r.name, "CDLDOJI");
        assert_eq!(r.outputs[0].0, "pattern");
        assert_eq!(r.outputs[0].1.len(), 5);
        assert!(r.outputs[0].1.iter().all(Option::is_none));
    }

    #[test]
    fn a_doji_is_flagged_only_on_the_matching_bar() {
        let data = doji_fixture();
        let r = compute(spec("CDLDOJI"), &data, &HashMap::new()).unwrap();
        let v = &r.outputs[0].1;
        assert!(v[..10].iter().all(Option::is_none), "lookback is 10");
        assert!(v[10..19].iter().all(|x| *x == Some(0.0)), "{:?}", &v[10..19]);
        assert_eq!(v[19], Some(100.0));
        assert_eq!(r.values[19].value, 100.0);
    }

    #[test]
    fn a_pattern_with_a_penetration_parameter_computes() {
        let data = doji_fixture();
        let r = compute(spec("CDLMORNINGSTAR"), &data, &params(&[("penetration", 0.3)])).unwrap();
        assert_eq!(r.name, "CDLMORNINGSTAR_0.3");
        assert_eq!(r.outputs[0].1.len(), 20);
    }

    #[test]
    fn every_catalog_entry_dispatches() {
        // 200 bars: the longest default lookbacks (TRIX 30 → 88, TEMA 30 → 87) fit.
        let closes: Vec<f64> = (0..200).map(|i| 100.0 + (i as f64 * 0.37).sin() * 8.0 + i as f64 * 0.1).collect();
        let data = bars_from_closes(&closes);
        for s in indicator_catalog::catalog() {
            let r = compute(s, &data, &HashMap::new()).unwrap_or_else(|e| panic!("{}: {e:?}", s.name));
            assert_eq!(r.outputs.len(), s.outputs.len(), "{}", s.name);
            for (name, v) in &r.outputs {
                assert_eq!(v.len(), 200, "{}.{name}", s.name);
            }
            assert_eq!(r.values.len(), 200, "{}", s.name);
            // Something must come out of 200 bars for every indicator and the
            // patterns (patterns legitimately report 0 everywhere).
            assert!(r.outputs[0].1.iter().any(Option::is_some), "{} produced nothing", s.name);
        }
    }

    // --- VWAP and the legacy entry points --------------------------------------

    #[test]
    fn vwap_is_app_computed_and_starts_at_the_first_typical_price() {
        let data = sample_data();
        let r = compute(spec("VWAP"), &data, &HashMap::new()).unwrap();
        assert_eq!(r.name, "VWAP");
        let expected_first = (105.0 + 98.0 + 103.0) / 3.0;
        assert!((r.values[0].value - expected_first).abs() < 0.001);
        assert_eq!(r.outputs[0].1[0], Some(r.values[0].value));
        let legacy = compute_vwap(&data);
        assert_eq!(legacy.values[4].value, r.values[4].value);
    }

    #[test]
    fn legacy_wrappers_keep_their_names_and_layout() {
        let data = sample_data();
        let sma = compute_sma(&data, 3);
        assert_eq!(sma.name, "SMA_3");
        assert_eq!(sma.values.len(), 5);
        assert!((sma.values[2].value - 105.666).abs() < 0.01);
        assert!(sma.values[0].value.is_nan());

        let ema = compute_ema(&data, 3);
        assert_eq!(ema.name, "EMA_3");
        assert!((ema.values[2].value - 105.6667).abs() < 1e-3); // seeded with the SMA
        assert!((ema.values[3].value - 107.8333).abs() < 1e-3);

        let rsi = compute_rsi(&data, 3);
        assert_eq!(rsi.name, "RSI_3");
        assert!(rsi.values[..3].iter().all(|p| p.value.is_nan()));
        assert!(rsi.values[3..].iter().all(|p| (0.0..=100.0).contains(&p.value)));

        let macd = compute_macd(&data, 3, 5, 3);
        assert_eq!(macd.name, "MACD_3_5");
        assert_eq!(macd.values.len(), 5); // lookback 6 > 4: nothing yet
        assert!(macd.values.iter().all(|p| p.value.is_nan() && p.secondary_value.is_none()));

        let bb = compute_bollinger(&data, 3, 2.0);
        assert_eq!(bb.name, "BOLLINGER_3");
        assert!(bb.values[1].secondary_value.is_none());
        for p in &bb.values[2..] {
            assert!(p.secondary_value.unwrap() >= p.value);
            assert!(p.tertiary_value.unwrap() <= p.value);
        }
    }

    #[test]
    fn legacy_wrappers_do_not_panic_on_bad_parameters() {
        let data = sample_data();
        let r = compute_rsi(&data, 1); // below TA-Lib's minimum of 2
        assert_eq!(r.values.len(), 5);
        assert!(r.values.iter().all(|p| p.value.is_nan()));
    }

    #[test]
    fn compute_indicator_dispatches_by_name_case_insensitively() {
        let data = sample_data();
        let r = compute_indicator(&data, "sma", &params(&[("period", 3.0)])).unwrap();
        assert_eq!(r.name, "SMA_3");
        let r = compute_indicator(&data, "cdlDoji", &HashMap::new()).unwrap();
        assert_eq!(r.name, "CDLDOJI");
    }

    #[test]
    fn compute_indicator_keeps_the_legacy_bollinger_alias() {
        let data = sample_data();
        let r = compute_indicator(&data, "BOLLINGER", &params(&[("period", 3.0), ("multiplier", 2.0)])).unwrap();
        assert_eq!(r.name, "BOLLINGER_3");
        let via_bbands = compute(spec("BBANDS"), &data, &params(&[("period", 3.0), ("nb_dev_up", 2.0), ("nb_dev_dn", 2.0)])).unwrap();
        assert_eq!(r.values[4].value, via_bbands.values[4].value);
        assert_eq!(r.values[4].secondary_value, via_bbands.values[4].secondary_value);
    }

    #[test]
    fn compute_indicator_rejects_unknown_names() {
        let data = sample_data();
        let err = compute_indicator(&data, "UNKNOWN", &HashMap::new()).unwrap_err();
        assert!(matches!(err, AppError::Validation(ref m) if m.contains("Unknown indicator 'UNKNOWN'")), "{err:?}");
    }

    #[test]
    fn compute_indicator_uses_defaults_when_params_are_missing() {
        // Previously an error ("SMA requires 'period'"); the catalog supplies TA-Lib's default now.
        let data = bars_from_closes(&(1..=40).map(f64::from).collect::<Vec<_>>());
        let r = compute_indicator(&data, "SMA", &HashMap::new()).unwrap();
        assert_eq!(r.name, "SMA_30");
    }

    /// Smoke test for the official TA-Lib crate: the batch API's documented shape.
    /// SMA(3) over 11..=20 produces its first value at input index 2 (the lookback)
    /// and eight values in total; the first is (11 + 12 + 13) / 3 = 12.
    #[test]
    fn test_talib_crate_sma_smoke() {
        let close: Vec<f64> = (11..=20).map(|v| v as f64).collect();
        let mut out = vec![0.0; close.len()];
        let range = ta_lib::Core::new()
            .sma(0, close.len() - 1, &close, 3, &mut out)
            .expect("TA-Lib SMA succeeds on a plain series");
        assert_eq!(out[0], 12.0);
        assert_eq!(range.beg_idx, 2);
        assert_eq!(range.count, 8);
    }
}
