//! The candle frame the indicator library computes over, and the readings
//! of a history frame it shares with the Flow data nodes (upstream
//! `agent/tools/market/candles.rs`: `candle_columns`, `candle_frame`,
//! `ist_timestamp`; the chart and summary helpers the Agent used are not
//! carried).

use crate::flow::contract::time::market_tz;
use crate::flow::support::numbers::{as_number, EPOCH_MILLISECOND_FLOOR};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

pub use crate::flow::support::numbers::epoch_seconds;

/// The candle fields, in the order a candle is emitted.
const CANDLE_FIELDS: [&str; 7] = ["timestamp", "open", "high", "low", "close", "volume", "oi"];

/// Which of a frame's own keys holds each candle field (built from the
/// first row; a frame's rows are uniform).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Columns(Vec<(&'static str, String)>);

impl Columns {
    /// The key holding `field`, when the frame has it.
    pub fn get(&self, field: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, k)| k.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn has(&self, field: &str) -> bool {
        self.get(field).is_some()
    }
}

/// The candle fields of one row, matched case-insensitively.
pub fn candle_columns(row: &Map<String, Value>) -> Columns {
    let mut lowered: Vec<(String, &String)> = Vec::new();
    for key in row.keys() {
        let low = key.trim().to_lowercase();
        lowered.retain(|(l, _)| *l != low);
        lowered.push((low, key));
    }
    Columns(
        CANDLE_FIELDS
            .iter()
            .filter_map(|f| {
                lowered
                    .iter()
                    .find(|(l, _)| l == f)
                    .map(|(_, k)| (*f, (*k).clone()))
            })
            .collect(),
    )
}

/// An instant from epoch seconds, microseconds rounded as Python's
/// `datetime.fromtimestamp` rounds them.
fn instant(seconds: f64) -> Option<DateTime<Utc>> {
    let micros = (seconds * 1_000_000.0).round_ties_even();
    if !micros.is_finite() || micros.abs() > 1e17 {
        return None;
    }
    let micros = micros as i64;
    DateTime::from_timestamp(
        micros.div_euclid(1_000_000),
        (micros.rem_euclid(1_000_000) * 1000) as u32,
    )
}

/// A candle timestamp as an ISO-8601 string in market time (epoch seconds
/// or milliseconds); anything that is not a number is passed through.
pub fn market_timestamp(value: &Value) -> Value {
    let Some(number) = value.as_f64().filter(|_| value.is_number()) else {
        return value.clone();
    };
    let seconds = if number.abs() >= EPOCH_MILLISECOND_FLOOR {
        number / 1000.0
    } else {
        number
    };
    match instant(seconds) {
        Some(t) => Value::String(iso_market(t)),
        None => value.clone(),
    }
}

/// Python's `datetime.isoformat()` of an instant in market time.
pub fn iso_market(t: DateTime<Utc>) -> String {
    let local = t.with_timezone(&market_tz());
    if local.timestamp_subsec_micros() == 0 {
        local.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
    } else {
        local.format("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string()
    }
}

fn field_of<'a>(row: &'a Map<String, Value>, columns: &Columns, field: &str) -> Option<&'a Value> {
    columns.get(field).and_then(|k| row.get(k))
}

fn number_of(row: &Map<String, Value>, columns: &Columns, field: &str) -> Option<f64> {
    field_of(row, columns, field).and_then(as_number)
}

/// The cleaned candles an indicator computes over: ascending unique
/// timestamps, the columns the rows supplied as floats, and no gap
/// anywhere. A column no row supplied is dropped rather than emptying the
/// frame (an index has no volume), then a row missing any remaining column
/// is dropped: one gap would poison every later value of a running sum.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CandleFrame {
    /// UTC epoch seconds, ascending, unique.
    pub time: Vec<i64>,
    /// The columns kept, each as long as `time`.
    pub columns: Vec<(&'static str, Vec<f64>)>,
}

impl CandleFrame {
    /// One column, when the frame kept it.
    pub fn column(&self, name: &str) -> Option<&[f64]> {
        self.columns
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_slice())
    }

    pub fn len(&self) -> usize {
        self.time.len()
    }

    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }
}

/// [`CandleFrame`] from a history frame's rows (any order); empty when the
/// rows carry no usable timestamp or no close column.
pub fn candle_frame(records: &[Map<String, Value>], columns: &Columns) -> CandleFrame {
    let fields: Vec<&'static str> = ["open", "high", "low", "close", "volume"]
        .into_iter()
        .filter(|f| columns.has(f))
        .collect();
    if records.is_empty() || !columns.has("timestamp") || !fields.contains(&"close") {
        return CandleFrame::default();
    }
    let rows: Vec<(i64, Vec<Option<f64>>)> = records
        .iter()
        .filter_map(|row| {
            let t = field_of(row, columns, "timestamp").and_then(epoch_seconds)?;
            Some((
                t,
                fields.iter().map(|f| number_of(row, columns, f)).collect(),
            ))
        })
        .collect();
    if rows.is_empty() {
        return CandleFrame::default();
    }
    let kept: Vec<usize> = (0..fields.len())
        .filter(|i| rows.iter().any(|(_, v)| v[*i].is_some()))
        .collect();
    let mut by_time: std::collections::BTreeMap<i64, Vec<f64>> = std::collections::BTreeMap::new();
    for (t, values) in rows {
        // A row with a gap is dropped first; among what is left the last
        // row of a repeated timestamp wins.
        let complete: Option<Vec<f64>> = kept.iter().map(|i| values[*i]).collect();
        if let Some(v) = complete {
            by_time.insert(t, v);
        }
    }
    let mut frame = CandleFrame {
        time: by_time.keys().copied().collect(),
        columns: kept.iter().map(|i| (fields[*i], Vec::new())).collect(),
    };
    for values in by_time.into_values() {
        for (slot, v) in frame.columns.iter_mut().zip(values) {
            slot.1.push(v);
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rows(v: Value) -> Vec<Map<String, Value>> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_object().unwrap().clone())
            .collect()
    }

    #[test]
    fn a_frame_keeps_complete_rows_in_time_order() {
        let records = rows(json!([
            {"Timestamp": 1_700_000_060, "Open": 2, "High": 3, "Low": 1, "Close": 2.5, "Volume": 10},
            {"Timestamp": 1_700_000_000, "Open": 1, "High": 2, "Low": 1, "Close": 1.5, "Volume": 5},
            {"Timestamp": 1_700_000_120, "Open": 2, "High": 3, "Low": 1, "Close": null, "Volume": 10},
        ]));
        let columns = candle_columns(&records[0]);
        assert_eq!(columns.get("close"), Some("Close"));
        let frame = candle_frame(&records, &columns);
        assert_eq!(frame.time, vec![1_700_000_000, 1_700_000_060]);
        assert_eq!(frame.column("close"), Some(&[1.5, 2.5][..]));
        assert_eq!(frame.len(), 2);
    }

    #[test]
    fn timestamps_render_in_market_time() {
        // 2026-10-30 13:30:00Z is 09:30 in New York (EDT).
        assert_eq!(
            market_timestamp(&json!(1_793_367_000)),
            json!("2026-10-30T09:30:00-04:00")
        );
        assert_eq!(market_timestamp(&json!("x")), json!("x"));
    }
}
