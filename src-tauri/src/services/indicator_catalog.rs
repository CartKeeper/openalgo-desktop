//! Indicator catalog
//!
//! The static list of indicators the server-side indicator service can compute,
//! grouped the way the Fundamentals → Analysis tab shows them. Every entry
//! except `VWAP` is a TA-Lib function (official `ta-lib` crate); its parameter
//! names are the app's snake_case names (`period`, `fast`, `slow`, `signal`,
//! `nb_dev_up`, …), listed in TA-Lib's call order, with TA-Lib's defaults and
//! ranges. The test module checks each entry against TA-Lib's own registry
//! (`ta_lib::abstract_api`) so the two cannot drift apart.

use serde::Serialize;

/// One parameter of an indicator.
#[derive(Debug, Clone, Serialize)]
pub struct ParamSpec {
    /// Request key (snake_case), e.g. `period`.
    pub name: &'static str,
    /// TA-Lib's default.
    pub default: f64,
    /// Smallest accepted value (inclusive).
    pub min: f64,
    /// Largest accepted value (inclusive).
    pub max: f64,
    /// `"int"`, `"float"` or `"matype"` (a moving-average type, 0 = SMA … 8 = T3).
    pub kind: &'static str,
}

/// One indicator the service can compute.
#[derive(Debug, Clone, Serialize)]
pub struct IndicatorSpec {
    /// TA-Lib name (upper case), e.g. `ADX`; also the request name.
    pub name: &'static str,
    /// Human label for the dropdown.
    pub label: &'static str,
    /// One of [`GROUPS`].
    pub group: &'static str,
    /// Which bar fields the indicator reads: a subset of open, high, low, close, volume.
    pub inputs: &'static [&'static str],
    /// Parameters in TA-Lib call order.
    pub params: &'static [ParamSpec],
    /// Output names in TA-Lib order; single-output indicators use `value`,
    /// candlestick patterns `pattern` (−100 / 0 / 100).
    pub outputs: &'static [&'static str],
}

/// Catalog groups, in display order.
pub const GROUPS: [&str; 7] = [
    "Overlap",
    "Momentum",
    "Volatility",
    "Volume",
    "Cycle",
    "Statistics",
    "Patterns",
];

/// The full catalog, in display order within each group.
pub fn catalog() -> &'static [IndicatorSpec] {
    CATALOG
}

/// Look an indicator up by name, case-insensitively.
pub fn find(name: &str) -> Option<&'static IndicatorSpec> {
    CATALOG.iter().find(|spec| spec.name.eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

/// TA-Lib's upper bound for every integer period.
const PERIOD_MAX: f64 = 100_000.0;
/// TA-Lib's `TA_REAL_MAX` / `-TA_REAL_MAX` bounds for unbounded reals.
const REAL_MAX: f64 = 3e37;
/// Moving-average types the app offers: TA-Lib's classic 0 = SMA … 8 = T3.
const MATYPE_MAX: f64 = 8.0;

const fn int(name: &'static str, default: f64, min: f64) -> ParamSpec {
    ParamSpec { name, default, min, max: PERIOD_MAX, kind: "int" }
}

const fn float(name: &'static str, default: f64, min: f64, max: f64) -> ParamSpec {
    ParamSpec { name, default, min, max, kind: "float" }
}

const fn matype(name: &'static str, default: f64) -> ParamSpec {
    ParamSpec { name, default, min: 0.0, max: MATYPE_MAX, kind: "matype" }
}

const CLOSE: &[&str] = &["close"];
const HL: &[&str] = &["high", "low"];
const HLC: &[&str] = &["high", "low", "close"];
const OHLC: &[&str] = &["open", "high", "low", "close"];
const HLCV: &[&str] = &["high", "low", "close", "volume"];
const CLOSE_VOLUME: &[&str] = &["close", "volume"];

const NO_PARAMS: &[ParamSpec] = &[];
const VALUE: &[&str] = &["value"];
const PATTERN: &[&str] = &["pattern"];

const fn spec(
    name: &'static str,
    label: &'static str,
    group: &'static str,
    inputs: &'static [&'static str],
    params: &'static [ParamSpec],
    outputs: &'static [&'static str],
) -> IndicatorSpec {
    IndicatorSpec { name, label, group, inputs, params, outputs }
}

/// A candlestick pattern: OHLC in, −100 / 0 / 100 out, optionally with
/// TA-Lib's `penetration` parameter.
macro_rules! cdl {
    ($name:literal, $label:literal) => {
        spec($name, $label, "Patterns", OHLC, NO_PARAMS, PATTERN)
    };
    ($name:literal, $label:literal, penetration = $default:literal) => {
        spec(
            $name,
            $label,
            "Patterns",
            OHLC,
            &[float("penetration", $default, 0.0, REAL_MAX)],
            PATTERN,
        )
    };
}

// ---------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------

static CATALOG: &[IndicatorSpec] = &[
    // Overlap
    spec("SMA", "Simple Moving Average", "Overlap", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec("EMA", "Exponential Moving Average", "Overlap", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec("WMA", "Weighted Moving Average", "Overlap", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec("DEMA", "Double Exponential Moving Average", "Overlap", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec("TEMA", "Triple Exponential Moving Average", "Overlap", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec("KAMA", "Kaufman Adaptive Moving Average", "Overlap", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec(
        "T3",
        "Triple Exponential Moving Average (T3)",
        "Overlap",
        CLOSE,
        &[int("period", 5.0, 1.0), float("v_factor", 0.7, 0.0, 1.0)],
        VALUE,
    ),
    spec(
        "BBANDS",
        "Bollinger Bands",
        "Overlap",
        CLOSE,
        &[
            int("period", 20.0, 2.0),
            float("nb_dev_up", 2.0, -REAL_MAX, REAL_MAX),
            float("nb_dev_dn", 2.0, -REAL_MAX, REAL_MAX),
            matype("matype", 0.0),
        ],
        &["upper", "middle", "lower"],
    ),
    spec(
        "SAR",
        "Parabolic SAR",
        "Overlap",
        HL,
        &[float("acceleration", 0.02, 0.0, REAL_MAX), float("maximum", 0.2, 0.0, REAL_MAX)],
        VALUE,
    ),
    spec("HT_TRENDLINE", "Hilbert Transform - Instantaneous Trendline", "Overlap", CLOSE, NO_PARAMS, VALUE),
    spec("MIDPOINT", "MidPoint over period", "Overlap", CLOSE, &[int("period", 14.0, 2.0)], VALUE),
    // Momentum
    spec("RSI", "Relative Strength Index", "Momentum", CLOSE, &[int("period", 14.0, 2.0)], VALUE),
    spec(
        "MACD",
        "Moving Average Convergence/Divergence",
        "Momentum",
        CLOSE,
        &[int("fast", 12.0, 2.0), int("slow", 26.0, 2.0), int("signal", 9.0, 1.0)],
        &["macd", "signal", "hist"],
    ),
    spec(
        "STOCH",
        "Stochastic",
        "Momentum",
        HLC,
        &[
            int("fast_k", 5.0, 1.0),
            int("slow_k", 3.0, 1.0),
            matype("slow_k_matype", 0.0),
            int("slow_d", 3.0, 1.0),
            matype("slow_d_matype", 0.0),
        ],
        &["slow_k", "slow_d"],
    ),
    spec(
        "STOCHRSI",
        "Stochastic Relative Strength Index",
        "Momentum",
        CLOSE,
        &[
            int("period", 14.0, 2.0),
            int("fast_k", 5.0, 1.0),
            int("fast_d", 3.0, 1.0),
            matype("fast_d_matype", 0.0),
        ],
        &["fast_k", "fast_d"],
    ),
    spec("CCI", "Commodity Channel Index", "Momentum", HLC, &[int("period", 14.0, 2.0)], VALUE),
    spec("MFI", "Money Flow Index", "Momentum", HLCV, &[int("period", 14.0, 2.0)], VALUE),
    spec("WILLR", "Williams' %R", "Momentum", HLC, &[int("period", 14.0, 2.0)], VALUE),
    spec("ROC", "Rate of Change", "Momentum", CLOSE, &[int("period", 10.0, 1.0)], VALUE),
    spec("MOM", "Momentum", "Momentum", CLOSE, &[int("period", 10.0, 1.0)], VALUE),
    spec("ADX", "Average Directional Movement Index", "Momentum", HLC, &[int("period", 14.0, 2.0)], VALUE),
    spec("ADXR", "Average Directional Movement Index Rating", "Momentum", HLC, &[int("period", 14.0, 2.0)], VALUE),
    spec("AROON", "Aroon", "Momentum", HL, &[int("period", 14.0, 2.0)], &["aroon_down", "aroon_up"]),
    spec("AROONOSC", "Aroon Oscillator", "Momentum", HL, &[int("period", 14.0, 2.0)], VALUE),
    spec(
        "PPO",
        "Percentage Price Oscillator",
        "Momentum",
        CLOSE,
        &[int("fast", 12.0, 2.0), int("slow", 26.0, 2.0), matype("matype", 1.0)],
        VALUE,
    ),
    spec(
        "APO",
        "Absolute Price Oscillator",
        "Momentum",
        CLOSE,
        &[int("fast", 12.0, 2.0), int("slow", 26.0, 2.0), matype("matype", 1.0)],
        VALUE,
    ),
    spec("CMO", "Chande Momentum Oscillator", "Momentum", CLOSE, &[int("period", 14.0, 2.0)], VALUE),
    spec("TRIX", "TRIX (1-day ROC of a Triple Smooth EMA)", "Momentum", CLOSE, &[int("period", 30.0, 1.0)], VALUE),
    spec(
        "ULTOSC",
        "Ultimate Oscillator",
        "Momentum",
        HLC,
        &[int("period1", 7.0, 1.0), int("period2", 14.0, 1.0), int("period3", 28.0, 1.0)],
        VALUE,
    ),
    spec("BOP", "Balance Of Power", "Momentum", OHLC, NO_PARAMS, VALUE),
    spec("DX", "Directional Movement Index", "Momentum", HLC, &[int("period", 14.0, 2.0)], VALUE),
    spec("MINUS_DI", "Minus Directional Indicator", "Momentum", HLC, &[int("period", 14.0, 1.0)], VALUE),
    spec("PLUS_DI", "Plus Directional Indicator", "Momentum", HLC, &[int("period", 14.0, 1.0)], VALUE),
    // Volatility
    spec("ATR", "Average True Range", "Volatility", HLC, &[int("period", 14.0, 1.0)], VALUE),
    spec("NATR", "Normalized Average True Range", "Volatility", HLC, &[int("period", 14.0, 1.0)], VALUE),
    spec("TRANGE", "True Range", "Volatility", HLC, NO_PARAMS, VALUE),
    // Volume
    spec("OBV", "On Balance Volume", "Volume", CLOSE_VOLUME, NO_PARAMS, VALUE),
    spec("AD", "Chaikin A/D Line", "Volume", HLCV, NO_PARAMS, VALUE),
    spec(
        "ADOSC",
        "Chaikin A/D Oscillator",
        "Volume",
        HLCV,
        &[int("fast", 3.0, 2.0), int("slow", 10.0, 2.0)],
        VALUE,
    ),
    // App-computed (cumulative VWAP over the loaded series), not TA-Lib.
    spec("VWAP", "Volume Weighted Average Price", "Volume", HLCV, NO_PARAMS, VALUE),
    // Cycle
    spec("HT_DCPERIOD", "Hilbert Transform - Dominant Cycle Period", "Cycle", CLOSE, NO_PARAMS, VALUE),
    // Statistics
    spec("LINEARREG", "Linear Regression", "Statistics", CLOSE, &[int("period", 14.0, 2.0)], VALUE),
    spec(
        "STDDEV",
        "Standard Deviation",
        "Statistics",
        CLOSE,
        &[int("period", 5.0, 2.0), float("nb_dev", 1.0, -REAL_MAX, REAL_MAX)],
        VALUE,
    ),
    spec(
        "VAR",
        "Variance",
        "Statistics",
        CLOSE,
        &[int("period", 5.0, 1.0), float("nb_dev", 1.0, -REAL_MAX, REAL_MAX)],
        VALUE,
    ),
    spec("TSF", "Time Series Forecast", "Statistics", CLOSE, &[int("period", 14.0, 2.0)], VALUE),
    // Patterns — all 61 TA-Lib candlestick recognizers, in TA-Lib order.
    cdl!("CDL2CROWS", "Two Crows"),
    cdl!("CDL3BLACKCROWS", "Three Black Crows"),
    cdl!("CDL3INSIDE", "Three Inside Up/Down"),
    cdl!("CDL3LINESTRIKE", "Three-Line Strike"),
    cdl!("CDL3OUTSIDE", "Three Outside Up/Down"),
    cdl!("CDL3STARSINSOUTH", "Three Stars In The South"),
    cdl!("CDL3WHITESOLDIERS", "Three Advancing White Soldiers"),
    cdl!("CDLABANDONEDBABY", "Abandoned Baby", penetration = 0.3),
    cdl!("CDLADVANCEBLOCK", "Advance Block"),
    cdl!("CDLBELTHOLD", "Belt-hold"),
    cdl!("CDLBREAKAWAY", "Breakaway"),
    cdl!("CDLCLOSINGMARUBOZU", "Closing Marubozu"),
    cdl!("CDLCONCEALBABYSWALL", "Concealing Baby Swallow"),
    cdl!("CDLCOUNTERATTACK", "Counterattack"),
    cdl!("CDLDARKCLOUDCOVER", "Dark Cloud Cover", penetration = 0.5),
    cdl!("CDLDOJI", "Doji"),
    cdl!("CDLDOJISTAR", "Doji Star"),
    cdl!("CDLDRAGONFLYDOJI", "Dragonfly Doji"),
    cdl!("CDLENGULFING", "Engulfing Pattern"),
    cdl!("CDLEVENINGDOJISTAR", "Evening Doji Star", penetration = 0.3),
    cdl!("CDLEVENINGSTAR", "Evening Star", penetration = 0.3),
    cdl!("CDLGAPSIDESIDEWHITE", "Up/Down-gap side-by-side white lines"),
    cdl!("CDLGRAVESTONEDOJI", "Gravestone Doji"),
    cdl!("CDLHAMMER", "Hammer"),
    cdl!("CDLHANGINGMAN", "Hanging Man"),
    cdl!("CDLHARAMI", "Harami Pattern"),
    cdl!("CDLHARAMICROSS", "Harami Cross Pattern"),
    cdl!("CDLHIGHWAVE", "High-Wave Candle"),
    cdl!("CDLHIKKAKE", "Hikkake Pattern"),
    cdl!("CDLHIKKAKEMOD", "Modified Hikkake Pattern"),
    cdl!("CDLHOMINGPIGEON", "Homing Pigeon"),
    cdl!("CDLIDENTICAL3CROWS", "Identical Three Crows"),
    cdl!("CDLINNECK", "In-Neck Pattern"),
    cdl!("CDLINVERTEDHAMMER", "Inverted Hammer"),
    cdl!("CDLKICKING", "Kicking"),
    cdl!("CDLKICKINGBYLENGTH", "Kicking - bull/bear determined by the longer marubozu"),
    cdl!("CDLLADDERBOTTOM", "Ladder Bottom"),
    cdl!("CDLLONGLEGGEDDOJI", "Long Legged Doji"),
    cdl!("CDLLONGLINE", "Long Line Candle"),
    cdl!("CDLMARUBOZU", "Marubozu"),
    cdl!("CDLMATCHINGLOW", "Matching Low"),
    cdl!("CDLMATHOLD", "Mat Hold", penetration = 0.5),
    cdl!("CDLMORNINGDOJISTAR", "Morning Doji Star", penetration = 0.3),
    cdl!("CDLMORNINGSTAR", "Morning Star", penetration = 0.3),
    cdl!("CDLONNECK", "On-Neck Pattern"),
    cdl!("CDLPIERCING", "Piercing Pattern"),
    cdl!("CDLRICKSHAWMAN", "Rickshaw Man"),
    cdl!("CDLRISEFALL3METHODS", "Rising/Falling Three Methods"),
    cdl!("CDLSEPARATINGLINES", "Separating Lines"),
    cdl!("CDLSHOOTINGSTAR", "Shooting Star"),
    cdl!("CDLSHORTLINE", "Short Line Candle"),
    cdl!("CDLSPINNINGTOP", "Spinning Top"),
    cdl!("CDLSTALLEDPATTERN", "Stalled Pattern"),
    cdl!("CDLSTICKSANDWICH", "Stick Sandwich"),
    cdl!("CDLTAKURI", "Takuri (Dragonfly Doji with very long lower shadow)"),
    cdl!("CDLTASUKIGAP", "Tasuki Gap"),
    cdl!("CDLTHRUSTING", "Thrusting Pattern"),
    cdl!("CDLTRISTAR", "Tristar Pattern"),
    cdl!("CDLUNIQUE3RIVER", "Unique 3 River"),
    cdl!("CDLUPSIDEGAP2CROWS", "Upside Gap Two Crows"),
    cdl!("CDLXSIDEGAP3METHODS", "Upside/Downside Gap Three Methods"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_has_at_least_a_hundred_entries() {
        assert!(catalog().len() >= 100, "catalog has {} entries", catalog().len());
    }

    #[test]
    fn catalog_names_are_unique() {
        let mut seen = HashSet::new();
        for spec in catalog() {
            assert!(seen.insert(spec.name), "duplicate catalog entry {}", spec.name);
        }
    }

    #[test]
    fn catalog_has_all_61_candlestick_patterns() {
        let patterns: Vec<_> = catalog().iter().filter(|s| s.group == "Patterns").collect();
        assert_eq!(patterns.len(), 61);
        for p in patterns {
            assert!(p.name.starts_with("CDL"), "{} is not a CDL function", p.name);
            assert_eq!(p.outputs, &["pattern"], "{}", p.name);
            assert_eq!(p.inputs, &["open", "high", "low", "close"], "{}", p.name);
        }
    }

    #[test]
    fn find_is_case_insensitive() {
        let adx = find("adx").expect("ADX is in the catalog");
        assert_eq!(adx.name, "ADX");
        assert_eq!(adx.group, "Momentum");
        assert!(find("no-such-indicator").is_none());
    }

    #[test]
    fn every_param_default_lies_inside_its_range() {
        for spec in catalog() {
            assert!(
                GROUPS.contains(&spec.group),
                "{} has unknown group {}",
                spec.name,
                spec.group
            );
            for p in spec.params {
                assert!(
                    p.min <= p.default && p.default <= p.max,
                    "{}.{}: {} is outside {}..={}",
                    spec.name,
                    p.name,
                    p.default,
                    p.min,
                    p.max
                );
                assert!(
                    matches!(p.kind, "int" | "float" | "matype"),
                    "{}.{} has kind {}",
                    spec.name,
                    p.name,
                    p.kind
                );
            }
        }
    }

    #[test]
    fn required_v1_entries_are_present() {
        for name in [
            "SMA", "EMA", "WMA", "DEMA", "TEMA", "KAMA", "T3", "BBANDS", "SAR", "HT_TRENDLINE",
            "MIDPOINT", "RSI", "MACD", "STOCH", "STOCHRSI", "CCI", "MFI", "WILLR", "ROC", "MOM",
            "ADX", "ADXR", "AROON", "AROONOSC", "PPO", "APO", "CMO", "TRIX", "ULTOSC", "BOP", "DX",
            "MINUS_DI", "PLUS_DI", "ATR", "NATR", "TRANGE", "OBV", "AD", "ADOSC", "LINEARREG",
            "STDDEV", "VAR", "TSF", "HT_DCPERIOD", "VWAP", "CDLDOJI", "CDLENGULFING",
        ] {
            assert!(find(name).is_some(), "{name} missing from the catalog");
        }
        let vwap = find("VWAP").unwrap();
        assert_eq!(vwap.group, "Volume");
        let macd = find("MACD").unwrap();
        assert_eq!(macd.outputs, &["macd", "signal", "hist"]);
        let names: Vec<_> = macd.params.iter().map(|p| p.name).collect();
        assert_eq!(names, ["fast", "slow", "signal"]);
    }

    /// Every TA-Lib entry agrees with TA-Lib's own registry: the function exists,
    /// sits in the mapped group, and its parameters (count, kind, default, range)
    /// and output count match, in call order.
    #[test]
    fn catalog_matches_the_talib_registry() {
        use ta_lib::abstract_api::{get_func_handle, Group, InputFlags, InputType, OptInputType};

        for spec in catalog() {
            if spec.name == "VWAP" {
                continue; // app-computed
            }
            let info = get_func_handle(spec.name)
                .unwrap_or_else(|| panic!("{} is not a TA-Lib function", spec.name))
                .info();

            let group = match info.group {
                Group::OverlapStudies => "Overlap",
                Group::MomentumIndicators => "Momentum",
                Group::VolatilityIndicators => "Volatility",
                Group::VolumeIndicators => "Volume",
                Group::CycleIndicators => "Cycle",
                Group::StatisticFunctions => "Statistics",
                Group::PatternRecognition => "Patterns",
                other => panic!("{}: unexpected TA-Lib group {:?}", spec.name, other),
            };
            assert_eq!(spec.group, group, "{} group", spec.name);

            assert_eq!(
                spec.params.len(),
                info.opt_inputs.len(),
                "{} parameter count",
                spec.name
            );
            for (p, oi) in spec.params.iter().zip(info.opt_inputs) {
                let what = format!("{}.{} (TA-Lib {})", spec.name, p.name, oi.param_name);
                #[allow(unreachable_patterns)]
                match oi.kind {
                    OptInputType::IntegerRange { min, max, default, .. } => {
                        assert_eq!(
                            (p.kind, p.min, p.max, p.default),
                            ("int", min as f64, max as f64, default as f64),
                            "{what}"
                        );
                    }
                    OptInputType::RealRange { min, max, default, .. } => {
                        assert_eq!(
                            (p.kind, p.min, p.max, p.default),
                            ("float", min, max, default),
                            "{what}"
                        );
                    }
                    OptInputType::IntegerList { values, default } => {
                        assert_eq!(p.kind, "matype", "{what}");
                        assert_eq!(p.default, default as f64, "{what}");
                        let listed = |v: f64| values.iter().any(|(x, _)| *x as f64 == v);
                        assert!(listed(p.min) && listed(p.max), "{what}: range not in TA-Lib's list");
                    }
                    other => panic!("{what}: unexpected TA-Lib parameter kind {other:?}"),
                }
            }

            assert_eq!(spec.outputs.len(), info.outputs.len(), "{} output count", spec.name);

            // Inputs: a `Real` input reads the close; a `Price` input reads the
            // OHLCV fields its flags name, in TA-Lib's field order.
            let mut expected: Vec<&str> = Vec::new();
            for input in info.inputs {
                #[allow(unreachable_patterns)]
                match input.kind {
                    InputType::Real => expected.push("close"),
                    InputType::Price => {
                        for (flag, field) in [
                            (InputFlags::PRICE_OPEN, "open"),
                            (InputFlags::PRICE_HIGH, "high"),
                            (InputFlags::PRICE_LOW, "low"),
                            (InputFlags::PRICE_CLOSE, "close"),
                            (InputFlags::PRICE_VOLUME, "volume"),
                        ] {
                            if input.flags.contains(flag) {
                                expected.push(field);
                            }
                        }
                    }
                    other => panic!("{}: unexpected TA-Lib input kind {other:?}", spec.name),
                }
            }
            assert_eq!(spec.inputs, expected.as_slice(), "{} inputs", spec.name);
        }
    }
}
