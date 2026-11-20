//! A client-facing Goldman Sax & Violins report comparing a cloned scenario with
//! the scenario it was cloned from: what the recommendations changed, and both
//! portfolios priced at the same live quotes on the report date.
//!
//! The app computes every number — the cover panel, the change table, the
//! measures, the trailing-year metrics and the health checklist. Claude
//! writes only prose: the subtitle, intros, one reason per change, the case
//! for the changes, and the closing checklist.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Duration, NaiveDate, Utc};
use chrono_tz::America::New_York;
use futures_util::{future::join, stream, StreamExt};
use serde::Deserialize;
use tracing::warn;

use crate::error::{AppError, Result};
use crate::importers::schwab::rules_401k::holding_flag;
use crate::providers::fmp::FmpClient;
use crate::providers::ProviderError;
use crate::providers::types::{
    BriefComparison, ChangeRow, ChangesMovement, ChecklistGroup, ClientTrade, CodaMovement,
    CompanyProfile, ComparisonSide, CoverTitle, DiagnosisIssue, DiagnosisMovement, DrawdownRow,
    DrawdownTable, EtfInfo, GoldmanBrief, GoldmanMovement, HeadlineNote, HealthCell,
    HealthCompareRow, HealthMovement, MeasureRow, MeasuresMovement, MetricsMovement,
    MonteCarloLine, MonthlyTable, MonthlyYear, ScenarioPosition,
};
use crate::providers::yahoo::YahooClient;
use crate::services::client_brief_service::{
    generation_times, live_quotes_with_names, price_rows, push_header, push_positions,
    symbol_is_stranded, AccountContext, ClientBriefService, PricedRow,
};
use crate::services::copilot_service::CopilotService;
use crate::services::portfolio_health::{evaluate, AssetClass, HealthHolding, HealthInput, HealthRow};
use crate::services::quant_service::compute_returns;
use crate::services::report_metrics::{
    common_start, drawdown_periods, fetch_closes, hypothetical_series_on, metric_rows, monte_carlo,
    monthly_returns, shared_axis, DrawdownPeriod, MetricFormat, MetricRow, MonteCarlo, MonthlyRow,
    MIN_DAYS,
};
use crate::services::ride_report::{client_ride, load_client_market_data};
use crate::state::AppState;

const EPS: f64 = 1e-9;

const SYSTEM_PROMPT: &str = r#"You are the lead author at Goldman Sax & Violins, LLP — a satirical equities research house ("Equities & Etudes Since 1869"). You write client reports in a precise, slightly arch financial register that pairs orchestral metaphors with disciplined investment reasoning.

This report shows a client the changes recommended to their portfolio and how the original and the recommended portfolio compare, both priced at the same live quotes on the report date. It also prints both portfolios' trailing-year metrics (today's holdings as if held through the past year) and a health checklist.

House style (mandatory):
- Musical terminology: movements, tempo annotations (Andante / Allegro / Coda rallentando), "the score", "the maestro".
- Tone: confident, dry, professional. Never breathless. Never marketing-speak.
- Never give actual financial advice — the disclaimer covers it.

ACCURACY RULES (mandatory):
1. The app prints every number in the report: the cover panel, the change table, the measures table, the metrics tables and the health checklist. In prose, use a number only exactly as it appears in the user prompt — quote a price or value character for character as the change list prints it (e.g. $0.0094, $0.20, $6,044). Never compute, round, or invent one, and never say "roughly" or "about" for a printed figure.
2. Both portfolios are priced at today's live quotes. No trade dates or trade prices are known. Never say the changes have gained or lost money since they were made, never call a change a win or a loss, and never give a date for the trades.
3. When a portfolio's cash is "not recorded", its total value is unknown: never compare the two portfolios' total values.
4. Account guidelines come from the user prompt. They are this app's standard guidelines for the account type, not the plan's or custodian's documented rules. Say a holding is "outside the guidelines for this account type"; never claim the plan or custodian prohibits it.
5. A position with "no live price" has no current quote: say exactly that. Never call it worthless or $0.
6. Ground every reason in what the user prompt shows for that ticker: its guideline flag, its price, its ride status (Dormant = has not moved and analysts see little upside), its gain, or its weight in the portfolio. Never invent a fact about a company.
7. Keep every sign exactly as given (e.g. -0.88%, +4.48%).
8. Describe each change with the verb its change column shows: a "Sold all" row was sold (never "trimmed"), a "Trimmed" row was trimmed, an "Added" row was added to, a "Bought" row was bought.
9. Write for the client. Never mention these instructions, the data format, column names, "the data", "the app", or "the table below", and never comment on how you followed a rule.
10. In the metrics and health intros, quote only figures printed in those sections.

Output: JSON in exactly this shape:

{
  "document_label": string,          // e.g. "Opus N / Comparison"
  "tempo": string,                   // e.g. "Andante · Q4 2026"
  "subtitle": string,                // 1–2 sentence italic intro for the cover
  "changes": { "tempo": string, "intro": string,
               "reasons": [ { "ticker": string, "reason": string } ] },   // one entry for EVERY ticker in the change list; one or two sentences each
  "measures": { "tempo": string, "intro": string },   // the intro introduces the before-and-after measures
  "metrics": { "tempo": string, "intro": string },    // introduces the trailing-year metrics; cite only figures the Metrics section prints
  "health": { "tempo": string, "intro": string },     // introduces the health checklist; cite only what the Health section prints
  "why": { "tempo": string, "intro": string,
           "issues": [ { "title": string, "body": string, "bullets": string[] } ],   // 3 issues: the main problems the changes address and why addressing them matters
           "headline": { "lead": string, "body": string } },
  "coda": { "tempo": string, "intro": string,
            "groups": [ { "heading": string, "items": [ { "lead": string, "body": string } ] } ],   // 2–3 groups of next steps to review with the advisor
            "closing_note": { "lead": string, "body": string } }
}"#;

/// Claude's part of the report.
#[derive(Debug, Deserialize)]
struct ComparisonProse {
    document_label: String,
    tempo: String,
    subtitle: String,
    changes: ChangesProse,
    measures: SectionProse,
    metrics: SectionProse,
    health: SectionProse,
    why: WhyProse,
    coda: CodaProse,
}

#[derive(Debug, Deserialize)]
struct SectionProse {
    tempo: String,
    #[serde(default)]
    intro: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChangesProse {
    tempo: String,
    #[serde(default)]
    intro: Option<String>,
    #[serde(default)]
    reasons: Vec<TickerReason>,
}

#[derive(Debug, Deserialize)]
struct TickerReason {
    ticker: String,
    reason: String,
}

/// The JSON schema for `ComparisonProse`, sent as a structured-outputs format
/// so the API can only return JSON of this shape. Every object closes with
/// `additionalProperties: false`, as structured outputs require.
fn prose_schema() -> serde_json::Value {
    use serde_json::json;
    fn obj(props: serde_json::Value) -> serde_json::Value {
        let required: Vec<String> = props.as_object().unwrap().keys().cloned().collect();
        json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
    }
    let s = || json!({ "type": "string" });
    let note = || obj(json!({ "lead": s(), "body": s() }));
    obj(json!({
        "document_label": s(),
        "tempo": s(),
        "subtitle": s(),
        "changes": obj(json!({
            "tempo": s(),
            "intro": s(),
            "reasons": { "type": "array", "items": obj(json!({ "ticker": s(), "reason": s() })) },
        })),
        "measures": obj(json!({ "tempo": s(), "intro": s() })),
        "metrics": obj(json!({ "tempo": s(), "intro": s() })),
        "health": obj(json!({ "tempo": s(), "intro": s() })),
        "why": obj(json!({
            "tempo": s(),
            "intro": s(),
            "issues": { "type": "array", "items": obj(json!({
                "title": s(), "body": s(), "bullets": { "type": "array", "items": s() },
            })) },
            "headline": note(),
        })),
        "coda": obj(json!({
            "tempo": s(),
            "intro": s(),
            "groups": { "type": "array", "items": obj(json!({
                "heading": s(),
                "items": { "type": "array", "items": note() },
            })) },
            "closing_note": note(),
        })),
    }))
}

#[derive(Debug, Deserialize)]
struct WhyProse {
    tempo: String,
    #[serde(default)]
    intro: Option<String>,
    issues: Vec<DiagnosisIssue>,
    #[serde(default)]
    headline: Option<HeadlineNote>,
}

#[derive(Debug, Deserialize)]
struct CodaProse {
    tempo: String,
    #[serde(default)]
    intro: Option<String>,
    groups: Vec<ChecklistGroup>,
    #[serde(default)]
    closing_note: Option<HeadlineNote>,
}

// ---------------------------------------------------------------------------
// Computation (pure)
// ---------------------------------------------------------------------------

/// Net shares per uppercase symbol: long positive, short negative.
fn net_shares(positions: &[ScenarioPosition]) -> BTreeMap<String, f64> {
    let mut out: BTreeMap<String, f64> = BTreeMap::new();
    for p in positions {
        let signed = if p.side.eq_ignore_ascii_case("short") { -p.quantity } else { p.quantity };
        *out.entry(p.symbol.to_uppercase()).or_default() += signed;
    }
    out
}

/// Priced rows for a portfolio, one per symbol (long positions only carry a cost).
fn priced(positions: &[ScenarioPosition], prices: &HashMap<String, f64>) -> Vec<PricedRow> {
    let raw: Vec<(String, f64, f64)> =
        positions.iter().map(|p| (p.symbol.to_uppercase(), p.quantity, p.avg_price)).collect();
    price_rows(&raw, prices)
}

/// Whole digits with thousands separators: 15000 -> "15,000".
fn group_thousands(whole: u64) -> String {
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    grouped
}

/// A share count, unsigned: 15000 -> "15,000", 2.5 -> "2.5".
fn shares(n: f64) -> String {
    let n = n.abs();
    if (n - n.round()).abs() < EPS {
        group_thousands(n.round() as u64)
    } else {
        let frac = format!("{:.4}", n.fract());
        let frac = frac.trim_start_matches('0').trim_end_matches('0');
        format!("{}{}", group_thousands(n.trunc() as u64), frac)
    }
}

/// Every symbol whose shares differ, sold first, then trimmed, added, bought;
/// within each, the largest change by today's value first.
pub(crate) fn change_rows(
    original: &[ScenarioPosition],
    recommended: &[ScenarioPosition],
    prices: &HashMap<String, f64>,
) -> Vec<ChangeRow> {
    let before = net_shares(original);
    let after = net_shares(recommended);
    let symbols: HashSet<&String> = before.keys().chain(after.keys()).collect();
    let mut rows: Vec<ChangeRow> = symbols
        .into_iter()
        .filter_map(|symbol| {
            let b = before.get(symbol).copied().unwrap_or(0.0);
            let a = after.get(symbol).copied().unwrap_or(0.0);
            let delta = a - b;
            if delta.abs() < EPS {
                return None;
            }
            let (kind, change) = if b.abs() < EPS {
                ("bought", format!("Bought {}", shares(a)))
            } else if a.abs() < EPS {
                ("sold", format!("Sold all {}", shares(b)))
            } else if a.abs() > b.abs() {
                ("added", format!("Added {}", shares(delta)))
            } else {
                ("trimmed", format!("Trimmed {}", shares(delta)))
            };
            let price = prices.get(symbol).copied().filter(|p| *p > 0.0);
            Some(ChangeRow {
                ticker: symbol.clone(),
                kind: kind.to_string(),
                change,
                before: b,
                after: a,
                price,
                value_today: price.map(|p| delta.abs() * p),
                reason: String::new(),
            })
        })
        .collect();
    let rank = |k: &str| match k {
        "sold" => 0,
        "trimmed" => 1,
        "added" => 2,
        _ => 3,
    };
    rows.sort_by(|x, y| {
        rank(&x.kind)
            .cmp(&rank(&y.kind))
            .then_with(|| y.value_today.unwrap_or(-1.0).total_cmp(&x.value_today.unwrap_or(-1.0)))
            .then_with(|| x.ticker.cmp(&y.ticker))
    });
    rows
}

pub(crate) fn comparison_side(name: &str, rows: &[PricedRow], cash: Option<f64>) -> ComparisonSide {
    let holdings_value: f64 = rows.iter().filter_map(|r| r.market_value).sum();
    ComparisonSide {
        name: name.to_string(),
        positions: rows.len(),
        holdings_value,
        cash,
        total_value: cash.map(|c| holdings_value + c),
        unpriced: rows.iter().filter(|r| r.price.is_none()).map(|r| r.symbol.clone()).collect(),
    }
}

fn usd(n: f64) -> String {
    format!("{}${}", if n < 0.0 { "-" } else { "" }, group_thousands(n.abs().round() as u64))
}

/// Dollars and cents with separators: 6129.4 -> "$6,129.40".
fn usd_cents(n: f64) -> String {
    let cents = (n.abs() * 100.0).round() as u64;
    format!("{}${}.{:02}", if n < 0.0 { "-" } else { "" }, group_thousands(cents / 100), cents % 100)
}

/// A share price exactly as the PDF prints it: "$6,129.40", and four decimals
/// under $1 ("$0.0094"). Keep in step with `price` in GoldmanReport.tsx.
fn price_text(p: f64) -> String {
    if p < 1.0 {
        format!("${p:.4}")
    } else {
        usd_cents(p)
    }
}

/// A change's value today exactly as the PDF prints it: whole dollars from
/// $10, cents below ("$7.23", "$0.20"), and "under $0.01" for a sliver.
/// Keep in step with `valueToday` in GoldmanReport.tsx.
fn value_text(v: f64) -> String {
    if v >= 10.0 {
        usd(v)
    } else if v > 0.0 && v < 0.005 {
        "under $0.01".to_string()
    } else {
        usd_cents(v)
    }
}

fn pct(part: f64, whole: f64) -> String {
    if whole > 0.0 {
        format!("{:.1}%", part / whole * 100.0)
    } else {
        "—".to_string()
    }
}

/// A portfolio's figures for the measures table.
struct SideStats<'a> {
    side: &'a ComparisonSide,
    rows: &'a [PricedRow],
}

impl SideStats<'_> {
    fn value(&self, r: &PricedRow) -> f64 {
        r.market_value.unwrap_or(0.0)
    }

    fn by_value(&self) -> Vec<&PricedRow> {
        let mut v: Vec<&PricedRow> = self.rows.iter().filter(|r| r.market_value.is_some()).collect();
        v.sort_by(|a, b| self.value(b).total_cmp(&self.value(a)));
        v
    }

    /// "N · $X (Y%)" for the holdings matching `pred`, or "None".
    fn group(&self, base: f64, pred: impl Fn(&PricedRow) -> bool) -> String {
        let hits: Vec<&PricedRow> = self.rows.iter().filter(|r| pred(r)).collect();
        if hits.is_empty() {
            return "None".to_string();
        }
        let value: f64 = hits.iter().map(|r| self.value(r)).sum();
        format!("{} · {} ({})", hits.len(), usd(value), pct(value, base))
    }
}

/// Side-by-side measures. Shares are of account value (holdings + cash) when
/// both portfolios' cash is recorded, otherwise of holdings value.
pub(crate) fn measure_rows(
    original: (&ComparisonSide, &[PricedRow]),
    recommended: (&ComparisonSide, &[PricedRow]),
    flags: &HashMap<String, &'static str>,
    dormant: &HashSet<String>,
) -> (Vec<MeasureRow>, String) {
    let o = SideStats { side: original.0, rows: original.1 };
    let r = SideStats { side: recommended.0, rows: recommended.1 };
    let both_cash = o.side.cash.is_some() && r.side.cash.is_some();
    let base = |s: &SideStats| if both_cash { s.side.total_value.unwrap_or(0.0) } else { s.side.holdings_value };
    let mut rows: Vec<MeasureRow> = Vec::new();
    let mut row = |label: &str, f: &dyn Fn(&SideStats) -> String| {
        rows.push(MeasureRow { label: label.to_string(), original: f(&o), recommended: f(&r) });
    };

    row("Account value (holdings + cash)", &|s| match s.side.total_value {
        Some(t) => usd(t),
        None => "Cash not recorded".to_string(),
    });
    row("Holdings value", &|s| usd(s.side.holdings_value));
    row("Cash", &|s| s.side.cash.map(usd).unwrap_or_else(|| "Not recorded".to_string()));
    row("Positions", &|s| s.side.positions.to_string());
    row("Largest holding", &|s| match s.by_value().first() {
        Some(top) => format!("{} · {}", top.symbol, pct(s.value(top), base(s))),
        None => "—".to_string(),
    });
    row("Five largest holdings", &|s| {
        let v: f64 = s.by_value().iter().take(5).map(|r| s.value(r)).sum();
        pct(v, base(s))
    });
    row("Leveraged, inverse or other flagged funds", &|s| {
        s.group(base(s), |r| flags.contains_key(&r.symbol.to_uppercase()))
    });
    row("Priced under $1", &|s| s.group(base(s), |r| r.price.is_some_and(|p| p < 1.0)));
    row("Dormant holdings", &|s| {
        let n = s.rows.iter().filter(|r| dormant.contains(&r.symbol.to_uppercase())).count();
        if n == 0 { "None".to_string() } else { n.to_string() }
    });
    row("No live price", &|s| {
        let n = s.rows.iter().filter(|r| r.price.is_none()).count();
        if n == 0 { "None".to_string() } else { n.to_string() }
    });

    let note = if both_cash {
        "Percentages are shares of account value (holdings plus cash) at today's prices.".to_string()
    } else {
        "Percentages are shares of holdings value at today's prices; cash is not recorded for both portfolios, so it is left out.".to_string()
    };
    (rows, note)
}

/// Fill each change's reason from Claude's answer (matched by ticker, any case).
fn apply_reasons(rows: &mut [ChangeRow], reasons: &[TickerReason]) {
    let by_ticker: HashMap<String, &String> =
        reasons.iter().map(|r| (r.ticker.trim().to_uppercase(), &r.reason)).collect();
    for row in rows {
        if let Some(reason) = by_ticker.get(&row.ticker) {
            row.reason = reason.trim().to_string();
        }
    }
}

// ---------------------------------------------------------------------------
// Metrics and health (app-computed)
// ---------------------------------------------------------------------------

/// The metrics window: calendar days of daily closes before the report's price time.
const METRICS_WINDOW_DAYS: i64 = 365;
/// Monte Carlo: bootstrapped one-year paths with a fixed seed, so a re-run prints the same odds.
const MONTE_CARLO_PATHS: usize = 1_000;
const MONTE_CARLO_HORIZON: usize = 252;
const MONTE_CARLO_SEED: u64 = 20261112;
/// A path busts when its drawdown reaches this fraction.
const MONTE_CARLO_BUST: f64 = -0.20;
/// A path meets the goal when it ends at or above this return.
const MONTE_CARLO_GOAL: f64 = 0.10;
/// Drawdown episodes listed per portfolio.
const TOP_DRAWDOWNS: usize = 5;
/// Symbols whose profile and fund info are looked up at once.
const LOOKUP_CONCURRENCY: usize = 8;

pub(crate) const NOTE_NO_PRICES: &str = "Price history was not available; metrics were skipped.";
pub(crate) const NOTE_NO_HOLDINGS: &str = "No holdings to measure.";

/// One portfolio's figures over the window.
pub(crate) struct SideMetrics {
    pub(crate) rows: Vec<MetricRow>,
    pub(crate) monthly: Vec<MonthlyRow>,
    pub(crate) drawdowns: Vec<DrawdownPeriod>,
    pub(crate) monte_carlo: Option<MonteCarlo>,
    /// Held symbols with no price history, left out of the series.
    pub(crate) skipped: Vec<String>,
    /// Trading days in the series (0 when none of the side's holdings is priced).
    pub(crate) days: usize,
}

/// The days both portfolios are measured over.
pub(crate) struct MetricsWindow {
    pub(crate) first: NaiveDate,
    pub(crate) last: NaiveDate,
    pub(crate) days: usize,
    /// The holding whose first close opened the window later than the others'.
    pub(crate) limited_by: Option<String>,
}

/// Both sides' figures over one window, or the one reason there are none.
pub(crate) struct MetricsBuild {
    pub(crate) original: Option<SideMetrics>,
    pub(crate) recommended: Option<SideMetrics>,
    pub(crate) window: Option<MetricsWindow>,
    pub(crate) note: Option<&'static str>,
}

/// Net shares per symbol, shorts negative, zero holdings dropped.
fn holdings_of(positions: &[ScenarioPosition]) -> Vec<(String, f64)> {
    net_shares(positions).into_iter().filter(|(_, q)| q.abs() > EPS).collect()
}

fn side_metrics(
    positions: &[(String, f64)],
    closes: &HashMap<String, Vec<(NaiveDate, f64)>>,
    axis: &[NaiveDate],
) -> SideMetrics {
    let built = hypothetical_series_on(positions, closes, axis);
    let enough = built.values.len() >= MIN_DAYS;
    let returns = compute_returns(&built.values);
    SideMetrics {
        rows: metric_rows(&built.dates, &built.values),
        monthly: if enough { monthly_returns(&built.dates, &built.values) } else { Vec::new() },
        drawdowns: if enough { drawdown_periods(&built.dates, &built.values, TOP_DRAWDOWNS) } else { Vec::new() },
        monte_carlo: monte_carlo(
            &returns,
            MONTE_CARLO_PATHS,
            MONTE_CARLO_HORIZON,
            MONTE_CARLO_SEED,
            MONTE_CARLO_BUST,
            MONTE_CARLO_GOAL,
        ),
        days: built.values.len(),
        skipped: built.skipped,
    }
}

/// Metrics for both portfolios from their holdings and the fetched closes,
/// measured over one shared window so their figures compare like with like.
/// Nothing held, a failed fetch, or closes covering none of the holdings give
/// no figures and a note saying so; the report goes on without them.
pub(crate) fn build_metrics(
    original: &[(String, f64)],
    recommended: &[(String, f64)],
    closes: Result<HashMap<String, Vec<(NaiveDate, f64)>>>,
) -> MetricsBuild {
    let none =
        |note: &'static str| MetricsBuild { original: None, recommended: None, window: None, note: Some(note) };
    if original.is_empty() && recommended.is_empty() {
        return none(NOTE_NO_HOLDINGS);
    }
    let closes = match closes {
        Ok(closes) => closes,
        Err(e) => {
            warn!("comparison report: metrics skipped: {}", e);
            return none(NOTE_NO_PRICES);
        }
    };
    let Some((start, limited_by)) = common_start(&[original, recommended], &closes) else {
        return none(NOTE_NO_PRICES);
    };
    let axis = shared_axis(&[original, recommended], &closes, start);
    let (Some(first), Some(last)) = (axis.first().copied(), axis.last().copied()) else {
        return none(NOTE_NO_PRICES);
    };
    let side = |positions: &[(String, f64)]| {
        (!positions.is_empty()).then(|| side_metrics(positions, &closes, &axis))
    };
    MetricsBuild {
        original: side(original),
        recommended: side(recommended),
        window: Some(MetricsWindow { first, last, days: axis.len(), limited_by }),
        note: None,
    }
}

/// A fraction as a percentage with one decimal; a rounded zero never shows a sign.
fn pct_text(v: f64) -> String {
    let x = (v * 1000.0).round() / 10.0;
    format!("{:.1}%", if x == 0.0 { 0.0 } else { x })
}

/// `pct_text` with an explicit plus on a gain: "+12.3%".
fn signed_pct_text(v: f64) -> String {
    let x = (v * 1000.0).round() / 10.0;
    if x > 0.0 {
        format!("+{x:.1}%")
    } else {
        format!("{:.1}%", if x == 0.0 { 0.0 } else { x })
    }
}

/// A threshold as a whole percentage with its sign: "-20%", "+10%".
fn signed_whole_pct(v: f64) -> String {
    let x = (v * 100.0).round();
    if x > 0.0 { format!("+{x:.0}%") } else { format!("{x:.0}%") }
}

/// A probability as a whole percentage: 0.234 -> "23%".
fn odds_text(p: f64) -> String {
    format!("{:.0}%", (p * 100.0).round())
}

/// A metric's cell as the PDF prints it: "12.3%", "1.23" or "n/a".
fn metric_text(value: Option<f64>, format: MetricFormat) -> String {
    match (value, format) {
        (None, _) => "n/a".to_string(),
        (Some(v), MetricFormat::Pct) => pct_text(v),
        (Some(v), MetricFormat::Ratio) => {
            let x = (v * 100.0).round() / 100.0;
            format!("{:.2}", if x == 0.0 { 0.0 } else { x })
        }
    }
}

/// "November 12, 2025".
fn long_date(d: NaiveDate) -> String {
    d.format("%B %-d, %Y").to_string()
}

/// A Monte Carlo line exactly as the PDF prints it. Keep in step with
/// `monteCarloText` in GoldmanReport.tsx.
pub(crate) fn monte_carlo_text(line: &MonteCarloLine) -> String {
    format!(
        "In {} bootstrapped years ({} trading days each): median {}; 5th to 95th percentile {} to {}; odds of a {} drawdown {}; odds of {} or better {}.",
        group_thousands(line.paths as u64),
        line.horizon_days,
        signed_pct_text(line.p50),
        signed_pct_text(line.p5),
        signed_pct_text(line.p95),
        signed_whole_pct(line.bust_threshold),
        odds_text(line.bust_probability),
        signed_whole_pct(line.goal_threshold),
        odds_text(line.goal_probability),
    )
}

/// The movement's note: what each side lacks, then how the figures were made.
fn metrics_note(sides: &[(&str, Option<&SideMetrics>)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut short = false;
    for (label, side) in sides {
        let Some(s) = side else {
            parts.push(format!("{label}: no holdings to measure."));
            continue;
        };
        if s.days == 0 {
            parts.push(format!("{label}: no price history."));
        }
        short |= s.days > 0 && s.days < MIN_DAYS;
        if !s.skipped.is_empty() {
            parts.push(format!("No price history for {}; left out of {label}.", s.skipped.join(", ")));
        }
    }
    if short {
        parts.push(format!(
            "Fewer than {MIN_DAYS} trading days in the window, so the figures read n/a (insufficient history)."
        ));
    }
    parts.push(format!(
        "Each portfolio is today's holdings carried through the window at daily closes: price returns, excluding dividends; the first and last months, and an earlier year's total, cover only the days inside the window. Ratios use a 0% risk-free rate. Monte Carlo odds come from {} bootstrapped years of each portfolio's own daily returns, drawn with a fixed seed so a re-run prints the same odds.",
        group_thousands(MONTE_CARLO_PATHS as u64)
    ));
    parts.join(" ")
}

/// The window line under the heading: the days measured, and the holding that
/// set the opening day when it cut the window short; without figures, the
/// window that was asked for.
fn window_text(window: Option<&MetricsWindow>, to: NaiveDate) -> String {
    let Some(w) = window else {
        return format!("Trailing {METRICS_WINDOW_DAYS} days of daily closes to {}", long_date(to));
    };
    let mut text = format!("Daily closes, {} to {} ({} trading days)", long_date(w.first), long_date(w.last), w.days);
    if let Some(symbol) = &w.limited_by {
        text.push_str(&format!(
            "; the window opens at {symbol}'s first close, so both portfolios are measured over the same days"
        ));
    }
    text
}

/// The Metrics movement from both sides' figures; `to` is the report's price
/// date. Prose (`tempo`, `intro`) is filled in from Claude's answer afterwards.
pub(crate) fn metrics_movement_from(
    build: &MetricsBuild,
    original_label: &str,
    recommended_label: &str,
    to: NaiveDate,
) -> MetricsMovement {
    let window = window_text(build.window.as_ref(), to);
    let (o, r) = (build.original.as_ref(), build.recommended.as_ref());
    let cell = |side: Option<&SideMetrics>, i: usize| {
        side.and_then(|s| s.rows.get(i)).map_or_else(|| "n/a".to_string(), |m| metric_text(m.value, m.format))
    };
    let rows: Vec<MeasureRow> = o
        .or(r)
        .map(|s| {
            s.rows
                .iter()
                .enumerate()
                .map(|(i, m)| MeasureRow { label: m.label.clone(), original: cell(o, i), recommended: cell(r, i) })
                .collect()
        })
        .unwrap_or_default();

    let sides = [(original_label, o), (recommended_label, r)];
    let (mut monthly, mut drawdowns, mut lines) = (Vec::new(), Vec::new(), Vec::new());
    for (label, side) in &sides {
        let Some(s) = side else { continue };
        if !s.monthly.is_empty() {
            let years = s.monthly.iter().map(|y| MonthlyYear { year: y.year, months: y.months, ytd: y.ytd }).collect();
            monthly.push(MonthlyTable { label: label.to_string(), years });
        }
        if !s.drawdowns.is_empty() {
            let rows = s
                .drawdowns
                .iter()
                .map(|d| DrawdownRow {
                    start: d.start.to_string(),
                    trough: d.trough.to_string(),
                    end: d.end.map(|e| e.to_string()),
                    depth_pct: d.depth_pct,
                    days: d.days,
                })
                .collect();
            drawdowns.push(DrawdownTable { label: label.to_string(), rows });
        }
        if let Some(mc) = &s.monte_carlo {
            lines.push(MonteCarloLine {
                label: label.to_string(),
                paths: MONTE_CARLO_PATHS,
                horizon_days: MONTE_CARLO_HORIZON,
                p5: mc.p5,
                p50: mc.p50,
                p95: mc.p95,
                bust_threshold: MONTE_CARLO_BUST,
                bust_probability: mc.bust_probability,
                goal_threshold: MONTE_CARLO_GOAL,
                goal_probability: mc.goal_probability,
            });
        }
    }
    let note = match build.note {
        Some(reason) => reason.to_string(),
        None => metrics_note(&sides),
    };
    MetricsMovement {
        numeral: "III".into(),
        tempo: String::new(),
        title_main: "Portfolio".into(),
        title_accent: "Metrics".into(),
        intro: None,
        original_label: original_label.to_string(),
        recommended_label: recommended_label.to_string(),
        window,
        rows,
        monthly,
        drawdowns,
        monte_carlo: lines,
        note,
    }
}

/// Class, sector and fund facts for one symbol, from FMP's fund info and
/// company profile. The default is "nothing known".
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Classification {
    pub(crate) asset_class: Option<AssetClass>,
    pub(crate) sector: Option<String>,
    /// `None` when fund status could not be looked up.
    pub(crate) is_fund: Option<bool>,
    /// Annual expense ratio as a fraction.
    pub(crate) expense_ratio: Option<f64>,
}

/// What an FMP lookup gave for a symbol.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Lookup<T> {
    Found(T),
    /// FMP answered and has no record.
    Absent,
    /// The request failed, so nothing is known either way.
    Failed,
}

/// A fund is classed by FMP's asset-class text and carries its expense ratio
/// (FMP reports it in percent: 0.03 = 0.03%); a fund has no sector of its own.
/// A symbol FMP has no fund record for but a profile is a stock: equity in
/// its profile's sector. A failed fund lookup cannot tell a bond fund from a
/// stock, and nothing on record says nothing: both stay unknown.
pub(crate) fn classify(fund: Lookup<&EtfInfo>, profile: Lookup<&CompanyProfile>) -> Classification {
    match (fund, profile) {
        (Lookup::Found(fund), _) => {
            let asset_class = fund.asset_class.as_deref().map(|text| {
                let t = text.to_lowercase();
                if t.contains("equity") {
                    AssetClass::Equity
                } else if t.contains("fixed") || t.contains("bond") {
                    AssetClass::FixedIncome
                } else if t.contains("commodit") {
                    AssetClass::Commodity
                } else {
                    AssetClass::Other
                }
            });
            Classification {
                asset_class,
                sector: None,
                is_fund: Some(true),
                expense_ratio: fund.expense_ratio.map(|percent| percent / 100.0),
            }
        }
        (Lookup::Absent, Lookup::Found(profile)) => {
            let sector = profile.sector.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
            Classification { asset_class: Some(AssetClass::Equity), sector, is_fund: Some(false), expense_ratio: None }
        }
        _ => Classification::default(),
    }
}

/// An FMP answer as a `Lookup`, logging a failure (never the key).
fn lookup<'a, T>(answer: &'a std::result::Result<Option<T>, ProviderError>, what: &str, symbol: &str) -> Lookup<&'a T> {
    match answer {
        Ok(Some(found)) => Lookup::Found(found),
        Ok(None) => Lookup::Absent,
        Err(e) => {
            warn!("comparison report: {} for {} failed: {}", what, symbol, e);
            Lookup::Failed
        }
    }
}

/// Class, sector and fund facts per uppercase symbol: two FMP calls each,
/// eight symbols at a time. Without an FMP client the map is empty, and a
/// symbol whose lookup failed stays unknown (never guessed to be a stock).
async fn lookup_classes(fmp: Option<&FmpClient>, symbols: &[String]) -> HashMap<String, Classification> {
    let Some(fmp) = fmp else { return HashMap::new() };
    stream::iter(symbols.iter().cloned())
        .map(|symbol| async move {
            let (fund, profile) = join(fmp.get_etf_info(&symbol), fmp.get_company_profile(&symbol)).await;
            let fund = fund.map(|funds| funds.into_iter().next());
            let class = classify(lookup(&fund, "fund info", &symbol), lookup(&profile, "profile", &symbol));
            (symbol.to_uppercase(), class)
        })
        .buffer_unordered(LOOKUP_CONCURRENCY)
        .collect()
        .await
}

/// Fees paid and volume traded over the client's trade history; `None` without one.
pub(crate) fn trade_fees(trades: &[ClientTrade]) -> Option<(f64, f64)> {
    if trades.is_empty() {
        return None;
    }
    let fees = trades.iter().map(|t| t.fees).sum();
    let volume = trades.iter().map(|t| t.quantity.abs() * t.price).sum();
    Some((fees, volume))
}

/// A portfolio's health input from its priced rows (an unpriced position has
/// no value to weigh).
fn health_input(
    rows: &[PricedRow],
    cash: Option<f64>,
    classes: &HashMap<String, Classification>,
    trade_fees: Option<(f64, f64)>,
    unresolved_violations: Option<i64>,
) -> HealthInput {
    let holdings = rows
        .iter()
        .filter_map(|r| {
            let value = r.market_value?;
            let c = classes.get(&r.symbol.to_uppercase()).cloned().unwrap_or_default();
            Some(HealthHolding {
                symbol: r.symbol.clone(),
                value,
                asset_class: c.asset_class,
                sector: c.sector,
                is_fund: c.is_fund,
                expense_ratio: c.expense_ratio,
            })
        })
        .collect();
    HealthInput { holdings, cash, trade_fees, unresolved_violations }
}

const HEALTH_NOTE: &str = "Informational checks adapted from Ghostfolio's portfolio rules and this app's defaults, not advice. Shares are of holdings value at today's prices; cash is judged only by buying power. A check reads n/a when the data to judge it is not on record.";

/// The Health movement: both portfolios' checklist rows side by side, one per
/// rule. Prose (`tempo`, `intro`) is filled in from Claude's answer afterwards.
pub(crate) fn health_movement_from(
    original: &HealthInput,
    recommended: &HealthInput,
    original_label: &str,
    recommended_label: &str,
) -> HealthMovement {
    let cell = |r: &HealthRow| HealthCell {
        status: r.status.as_str().to_string(),
        value_text: r.value_text.clone(),
        detail: r.detail.clone(),
    };
    let (o_rows, r_rows) = (evaluate(original), evaluate(recommended));
    let rows = o_rows
        .iter()
        .zip(&r_rows)
        .map(|(o, r)| HealthCompareRow {
            key: o.key.to_string(),
            title: o.title.to_string(),
            original: cell(o),
            recommended: cell(r),
            threshold: o.threshold_text.clone(),
        })
        .collect();
    let note = if original.holdings.is_empty() && recommended.holdings.is_empty() {
        format!("{NOTE_NO_HOLDINGS} {HEALTH_NOTE}")
    } else {
        HEALTH_NOTE.to_string()
    };
    HealthMovement {
        numeral: "IV".into(),
        tempo: String::new(),
        title_main: "Portfolio".into(),
        title_accent: "Health".into(),
        intro: None,
        original_label: original_label.to_string(),
        recommended_label: recommended_label.to_string(),
        rows,
        note,
    }
}

// ---------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------

/// The Metrics section of the user prompt: the same figures the PDF prints.
fn push_metrics(s: &mut String, m: &MetricsMovement) {
    s.push_str("\n## Metrics (app-computed)\n");
    s.push_str(&format!("Window: {}\n", m.window));
    if !m.rows.is_empty() {
        s.push_str(&format!("metric: {} | {}\n", m.original_label, m.recommended_label));
        for r in &m.rows {
            s.push_str(&format!("{}: {} | {}\n", r.label, r.original, r.recommended));
        }
    }
    for line in &m.monte_carlo {
        s.push_str(&format!("{}: {}\n", line.label, monte_carlo_text(line)));
    }
    s.push_str(&format!("({})\n", m.note));
}

/// A status as the prompt spells it: "pass", "warn", "fail", "n/a".
fn status_word(status: &str) -> &str {
    if status == "na" { "n/a" } else { status }
}

/// The Health section of the user prompt: the same rows the PDF prints.
fn push_health(s: &mut String, h: &HealthMovement) {
    s.push_str("\n## Health (app-computed)\n");
    s.push_str("check: original | recommended | threshold\n");
    for r in &h.rows {
        s.push_str(&format!(
            "{}: {} {} | {} {} | {}\n",
            r.title,
            status_word(&r.original.status),
            r.original.value_text,
            status_word(&r.recommended.status),
            r.recommended.value_text,
            r.threshold
        ));
    }
    s.push_str(&format!("({})\n", h.note));
}

fn cash_line(cash: Option<f64>) -> String {
    match cash {
        Some(c) => format!("Cash: {}\n", usd(c)),
        None => "Cash: not recorded\n".to_string(),
    }
}

fn change_line(
    row: &ChangeRow,
    flags: &HashMap<String, &'static str>,
    ride: &HashMap<String, &'static str>,
    names: &HashMap<String, String>,
) -> String {
    format!(
        "{} | {} | {} | {} → {} shares | {} | {} | {} | {}\n",
        row.ticker,
        names.get(&row.ticker).map(String::as_str).unwrap_or("—"),
        row.change,
        shares(row.before),
        shares(row.after),
        row.price.map(price_text).unwrap_or_else(|| "no live price".to_string()),
        row.value_today.map(value_text).unwrap_or_else(|| "—".to_string()),
        ride.get(&row.ticker).copied().unwrap_or("—"),
        flags.get(&row.ticker).map(|f| f.replace('_', " ")).unwrap_or_else(|| "none".to_string()),
    )
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub struct ScenarioComparisonService;

impl ScenarioComparisonService {
    /// Compare a cloned scenario with the scenario it was cloned from.
    pub async fn generate(state: &AppState, scenario_id: i64) -> Result<GoldmanBrief> {
        let api_key = ClientBriefService::api_key(state)?;
        let recommended = state.sqlite.get_client_scenario_by_id(scenario_id)?;
        let source_id = recommended.cloned_from_id.ok_or_else(|| {
            AppError::Validation("This scenario wasn't cloned from another, so there's nothing to compare it with.".into())
        })?;
        let original = state.sqlite.get_client_scenario_by_id(source_id).map_err(|_| {
            AppError::Validation("The scenario this one was cloned from no longer exists.".into())
        })?;
        let client = state.sqlite.get_client_by_id(recommended.client_id)?;
        let original_positions = state.sqlite.get_scenario_positions(source_id)?;
        let recommended_positions = state.sqlite.get_scenario_positions(scenario_id)?;

        // Price both portfolios at the same quotes.
        let mut symbols: Vec<String> = original_positions
            .iter()
            .chain(&recommended_positions)
            .map(|p| p.symbol.to_uppercase())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        symbols.sort();
        let quotes = live_quotes_with_names(state, &symbols).await;
        let prices: HashMap<String, f64> = quotes.iter().map(|(k, (p, _))| (k.clone(), *p)).collect();
        let names: HashMap<String, String> =
            quotes.iter().filter_map(|(k, (_, n))| n.clone().map(|n| (k.clone(), n))).collect();

        let mut changes = change_rows(&original_positions, &recommended_positions, &prices);
        if changes.is_empty() {
            return Err(AppError::Validation(format!(
                "\"{}\" holds the same positions as \"{}\", so there are no changes to report.",
                recommended.name, original.name
            )));
        }

        let original_rows = priced(&original_positions, &prices);
        let recommended_rows = priced(&recommended_positions, &prices);
        let (generated_date, prices_as_of) = generation_times();
        let account_type = recommended.account_type.clone().or_else(|| client.account_type.clone());
        let account = AccountContext::new(account_type.as_deref());
        let original_side = comparison_side(&original.name, &original_rows, original.cash);
        let recommended_side = comparison_side(&recommended.name, &recommended_rows, recommended.cash);

        let flags: HashMap<String, &'static str> = symbols
            .iter()
            .filter(|s| !symbol_is_stranded(s))
            .filter_map(|s| holding_flag(s, names.get(s).map(String::as_str)).map(|f| (s.clone(), f)))
            .collect();
        let ride = Self::ride_statuses(state, &original_positions, &recommended_positions, &prices).await;
        let dormant: HashSet<String> =
            ride.iter().filter(|(_, s)| **s == "Dormant").map(|(k, _)| k.clone()).collect();
        let (measures, measures_note) = measure_rows(
            (&original_side, &original_rows),
            (&recommended_side, &recommended_rows),
            &flags,
            &dormant,
        );

        // ---- Metrics and health (app-computed; Claude writes only their intros)
        let fmp = CopilotService::get_fmp_client(state).ok();
        let today = Utc::now().with_timezone(&New_York).date_naive();
        let from = today - Duration::days(METRICS_WINDOW_DAYS);
        let tradable: Vec<String> = symbols.iter().filter(|s| !symbol_is_stranded(s)).cloned().collect();
        let closes = match fmp.as_ref() {
            Some(fmp) => fetch_closes(fmp, &tradable, from, today).await,
            None => Err(AppError::Provider("FMP API key not configured".to_string())),
        };
        let metrics = build_metrics(&holdings_of(&original_positions), &holdings_of(&recommended_positions), closes);
        let mut metrics_movement = metrics_movement_from(&metrics, &original.name, &recommended.name, today);
        let classes = lookup_classes(fmp.as_ref(), &tradable).await;
        let fees = trade_fees(&state.sqlite.get_client_trades(recommended.client_id).unwrap_or_default());
        let violations = state.sqlite.count_unresolved_violations(recommended.client_id).ok();
        let mut health_movement = health_movement_from(
            &health_input(&original_rows, original.cash, &classes, fees, violations),
            &health_input(&recommended_rows, recommended.cash, &classes, fees, violations),
            &original.name,
            &recommended.name,
        );

        // ---- Prompt
        let mut s = String::new();
        push_header(&mut s, &client, &account, &prices_as_of);
        s.push_str(&format!("# Original portfolio: \"{}\"\n", original.name));
        s.push_str(&cash_line(original.cash));
        push_positions(&mut s, &original_rows, &HashMap::new());
        s.push_str(&format!("\n# Recommended portfolio: \"{}\"\n", recommended.name));
        s.push_str(&cash_line(recommended.cash));
        push_positions(&mut s, &recommended_rows, &HashMap::new());
        s.push_str(&format!(
            "\n# Change list ({}) — write one reason per ticker\nticker | name | change | shares before → after | price today | value of the change today | ride status | guideline flag\n",
            changes.len()
        ));
        for row in &changes {
            s.push_str(&change_line(row, &flags, &ride, &names));
        }
        s.push_str("\n# Measures (original | recommended)\n");
        for m in &measures {
            s.push_str(&format!("{}: {} | {}\n", m.label, m.original, m.recommended));
        }
        s.push_str(&format!("({measures_note})\n"));
        push_metrics(&mut s, &metrics_movement);
        push_health(&mut s, &health_movement);
        s.push_str("\n# Task\nWrite the report's prose: why each change was recommended, why the recommended portfolio is better suited to this account, and short introductions to the metrics and health sections that cite only figures printed there, grounded only in what is above.");

        let prose: ComparisonProse =
            ClientBriefService::ask_claude_json(state, &api_key, SYSTEM_PROMPT, s, Some(prose_schema()))
                .await?;
        apply_reasons(&mut changes, &prose.changes.reasons);
        metrics_movement.tempo = prose.metrics.tempo;
        metrics_movement.intro = prose.metrics.intro;
        health_movement.tempo = prose.health.tempo;
        health_movement.intro = prose.health.intro;

        Ok(GoldmanBrief {
            client_name: client.name.clone(),
            document_label: prose.document_label,
            tempo: prose.tempo,
            generated_date,
            subtitle: prose.subtitle,
            confidential_label: Some(format!("Confidential client report — {}", account.label)),
            facts: None,
            comparison: Some(BriefComparison {
                account_type: account.label.to_string(),
                prices_as_of,
                original: original_side,
                recommended: recommended_side,
            }),
            cover_title: Some(CoverTitle {
                main: "The Recommendations".to_string(),
                accent: "Before & After".to_string(),
            }),
            movements: vec![
                GoldmanMovement::Changes(ChangesMovement {
                    numeral: "I".into(),
                    tempo: prose.changes.tempo,
                    title_main: "The".into(),
                    title_accent: "Recommendations".into(),
                    intro: prose.changes.intro,
                    rows: changes,
                }),
                GoldmanMovement::Measures(MeasuresMovement {
                    numeral: "II".into(),
                    tempo: prose.measures.tempo,
                    title_main: "Before &".into(),
                    title_accent: "After".into(),
                    intro: prose.measures.intro,
                    original_label: original.name.clone(),
                    recommended_label: recommended.name.clone(),
                    rows: measures,
                    note: Some(measures_note),
                }),
                GoldmanMovement::Metrics(metrics_movement),
                GoldmanMovement::Health(health_movement),
                GoldmanMovement::Diagnosis(DiagnosisMovement {
                    numeral: "V".into(),
                    tempo: prose.why.tempo,
                    title_main: "Why It".into(),
                    title_accent: "Matters".into(),
                    intro: prose.why.intro,
                    issues: prose.why.issues,
                    headline: prose.why.headline,
                }),
                GoldmanMovement::Coda(CodaMovement {
                    numeral: "VI".into(),
                    tempo: prose.coda.tempo,
                    title_main: "The".into(),
                    title_accent: "Coda".into(),
                    intro: prose.coda.intro,
                    groups: prose.coda.groups,
                    closing_note: prose.coda.closing_note,
                }),
            ],
        })
    }

    /// The briefing's ride status for every long holding in either portfolio.
    async fn ride_statuses(
        state: &AppState,
        original: &[ScenarioPosition],
        recommended: &[ScenarioPosition],
        prices: &HashMap<String, f64>,
    ) -> HashMap<String, &'static str> {
        let longs: Vec<&ScenarioPosition> = original
            .iter()
            .chain(recommended)
            .filter(|p| p.quantity > 0.0 && !p.side.eq_ignore_ascii_case("short"))
            .collect();
        let symbols: Vec<String> =
            longs.iter().map(|p| p.symbol.to_uppercase()).collect::<HashSet<_>>().into_iter().collect();
        let yahoo = YahooClient::new((*state.http_client).clone());
        let fmp = CopilotService::get_fmp_client(state).ok();
        let today = Utc::now().with_timezone(&New_York).date_naive();
        let market = load_client_market_data(fmp.as_ref(), &yahoo, symbols, today).await;
        // A symbol held in both keeps the original's cost, the basis the change was judged on.
        let mut out = HashMap::new();
        for p in longs {
            let key = p.symbol.to_uppercase();
            if out.contains_key(&key) {
                continue;
            }
            let data = market.get(&key);
            let closes = data.map_or(&[][..], |d| d.closes.as_slice());
            let ride = client_ride(prices.get(&key).copied(), p.avg_price, closes, data.and_then(|d| d.target.as_ref()));
            out.insert(key, ride.status);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ClientTrade, CompanyProfile, EtfInfo};
    use crate::services::portfolio_health::{AssetClass, HealthHolding, HealthInput, RULES};
    use crate::services::report_metrics::{MetricFormat, MetricRow};
    use chrono::{Duration, NaiveDate};

    fn pos(symbol: &str, qty: f64, avg: f64) -> ScenarioPosition {
        ScenarioPosition {
            id: None,
            scenario_id: 1,
            symbol: symbol.to_string(),
            exchange: "GENERIC".to_string(),
            quantity: qty,
            avg_price: avg,
            side: "long".to_string(),
            notes: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn prices() -> HashMap<String, f64> {
        [("TQQQ", 60.0), ("VOO", 700.0), ("MSFT", 400.0), ("AAPL", 250.0), ("SMME", 0.05)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn change_rows_label_and_order_every_difference() {
        let original = [pos("TQQQ", 10.0, 50.0), pos("MSFT", 4.0, 300.0), pos("AAPL", 2.0, 200.0), pos("SMME", 1000.0, 0.2)];
        let recommended = [pos("MSFT", 2.0, 300.0), pos("AAPL", 3.0, 210.0), pos("VOO", 1.0, 700.0)];
        let rows = change_rows(&original, &recommended, &prices());
        let summary: Vec<(&str, &str)> = rows.iter().map(|r| (r.ticker.as_str(), r.change.as_str())).collect();
        assert_eq!(
            summary,
            vec![
                ("TQQQ", "Sold all 10"),
                ("SMME", "Sold all 1,000"),
                ("MSFT", "Trimmed 2"),
                ("AAPL", "Added 1"),
                ("VOO", "Bought 1"),
            ]
        );
        assert_eq!(rows[0].value_today, Some(600.0));
        assert_eq!(rows[2].value_today, Some(800.0));
    }

    #[test]
    fn unchanged_holdings_are_not_changes() {
        let both = [pos("VOO", 3.0, 600.0)];
        assert!(change_rows(&both, &both, &prices()).is_empty());
    }

    #[test]
    fn a_change_without_a_live_price_has_no_value() {
        let rows = change_rows(&[pos("ZZZZ", 5.0, 1.0)], &[], &prices());
        assert_eq!((rows[0].price, rows[0].value_today), (None, None));
    }

    #[test]
    fn measures_use_account_value_only_when_both_cash_balances_are_known() {
        let p = prices();
        let orig_rows = price_rows(&[("TQQQ".into(), 10.0, 50.0), ("VOO".into(), 1.0, 600.0)], &p);
        let rec_rows = price_rows(&[("VOO".into(), 2.0, 650.0)], &p);
        let flags: HashMap<String, &'static str> = [("TQQQ".to_string(), "leveraged_etf")].into();
        let dormant: HashSet<String> = HashSet::new();

        let o = comparison_side("Current", &orig_rows, Some(100.0));
        let r = comparison_side("Rebalanced", &rec_rows, Some(0.0));
        assert_eq!(o.total_value, Some(1400.0));
        let (rows, note) = measure_rows((&o, &orig_rows), (&r, &rec_rows), &flags, &dormant);
        let get = |label: &str| rows.iter().find(|m| m.label == label).unwrap().clone();
        assert_eq!(get("Account value (holdings + cash)").original, "$1,400");
        assert_eq!(get("Leveraged, inverse or other flagged funds").original, "1 · $600 (42.9%)");
        assert_eq!(get("Leveraged, inverse or other flagged funds").recommended, "None");
        assert_eq!(get("Largest holding").recommended, "VOO · 100.0%");
        assert!(note.contains("account value"));

        let unknown = comparison_side("Rebalanced", &rec_rows, None);
        let (rows, note) = measure_rows((&o, &orig_rows), (&unknown, &rec_rows), &flags, &dormant);
        let get = |label: &str| rows.iter().find(|m| m.label == label).unwrap().clone();
        assert_eq!(get("Cash").recommended, "Not recorded");
        assert_eq!(get("Account value (holdings + cash)").recommended, "Cash not recorded");
        // Without both cash balances, shares are of holdings value.
        assert_eq!(get("Leveraged, inverse or other flagged funds").original, "1 · $600 (46.2%)");
        assert!(note.contains("holdings value"));
    }

    #[test]
    fn reasons_match_tickers_in_any_case() {
        let mut rows = change_rows(&[pos("TQQQ", 1.0, 50.0)], &[], &prices());
        let reasons = [TickerReason { ticker: "tqqq".to_string(), reason: " Leveraged. ".to_string() }];
        apply_reasons(&mut rows, &reasons);
        assert_eq!(rows[0].reason, "Leveraged.");
    }

    /// Structured outputs reject open objects: every object in the schema must
    /// list all its properties as required and close with additionalProperties.
    fn assert_closed(v: &serde_json::Value, path: &str) {
        if let Some(o) = v.as_object() {
            if o.get("type").and_then(|t| t.as_str()) == Some("object") {
                assert_eq!(o["additionalProperties"], serde_json::json!(false), "{path} is open");
                let props = o["properties"].as_object().unwrap();
                let req: Vec<&str> = o["required"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
                for k in props.keys() {
                    assert!(req.contains(&k.as_str()), "{path}.{k} not required");
                }
            }
            for (k, child) in o {
                assert_closed(child, &format!("{path}.{k}"));
            }
        }
    }

    #[test]
    fn the_prose_schema_is_closed_and_matches_the_struct() {
        let schema = prose_schema();
        assert_closed(&schema, "$");
        for section in ["measures", "metrics", "health"] {
            assert!(schema["properties"][section].is_object(), "schema lacks {section}");
        }
        // An answer shaped by the schema deserializes into ComparisonProse.
        let answer = serde_json::json!({
            "document_label": "Opus 46 / Comparison", "tempo": "Andante", "subtitle": "s",
            "changes": { "tempo": "t", "intro": "i", "reasons": [{ "ticker": "TQQQ", "reason": "r" }] },
            "measures": { "tempo": "t", "intro": "i" },
            "metrics": { "tempo": "tm", "intro": "im" },
            "health": { "tempo": "th", "intro": "ih" },
            "why": { "tempo": "t", "intro": "i",
                     "issues": [{ "title": "a", "body": "b", "bullets": ["c"] }],
                     "headline": { "lead": "l", "body": "b" } },
            "coda": { "tempo": "t", "intro": "i",
                      "groups": [{ "heading": "h", "items": [{ "lead": "l", "body": "b" }] }],
                      "closing_note": { "lead": "l", "body": "b" } },
        });
        let prose: ComparisonProse = serde_json::from_value(answer).unwrap();
        assert_eq!(prose.changes.reasons[0].ticker, "TQQQ");
        assert_eq!((prose.metrics.tempo.as_str(), prose.health.intro.as_deref()), ("tm", Some("ih")));
        assert!(SYSTEM_PROMPT.contains("\"metrics\": {") && SYSTEM_PROMPT.contains("\"health\": {"));
        assert!(SYSTEM_PROMPT.contains("10. In the metrics and health intros, quote only figures printed in those sections."));
    }

    // ----- Metrics and health movements -----

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    const REQUESTED: (NaiveDate, NaiveDate) = match (NaiveDate::from_ymd_opt(2025, 11, 12), NaiveDate::from_ymd_opt(2026, 11, 11)) {
        (Some(from), Some(to)) => (from, to),
        _ => unreachable!(),
    };

    fn side(rows: Vec<(&str, Option<f64>, MetricFormat)>) -> SideMetrics {
        SideMetrics {
            rows: rows.into_iter().map(|(l, v, f)| MetricRow { label: l.to_string(), value: v, format: f }).collect(),
            monthly: Vec::new(),
            drawdowns: Vec::new(),
            monte_carlo: None,
            skipped: Vec::new(),
            days: 60,
        }
    }

    #[test]
    fn metric_cells_read_as_the_pdf_prints_them() {
        use MetricFormat::{Pct, Ratio};
        let o = side(vec![("CAGR", Some(0.123), Pct), ("Sharpe ratio (rf 0%)", Some(1.234), Ratio), ("Max drawdown", Some(0.0), Pct)]);
        let r = side(vec![("CAGR", None, Pct), ("Sharpe ratio (rf 0%)", Some(-0.001), Ratio), ("Max drawdown", Some(-0.2), Pct)]);
        let window = MetricsWindow { first: REQUESTED.0, last: REQUESTED.1, days: 60, limited_by: None };
        let build = MetricsBuild { original: Some(o), recommended: Some(r), window: Some(window), note: None };
        let m = metrics_movement_from(&build, "Current", "Rebalanced", REQUESTED.1);
        assert_eq!(m.rows[0], MeasureRow { label: "CAGR".into(), original: "12.3%".into(), recommended: "n/a".into() });
        assert_eq!((m.rows[1].original.as_str(), m.rows[1].recommended.as_str()), ("1.23", "0.00"));
        assert_eq!((m.rows[2].original.as_str(), m.rows[2].recommended.as_str()), ("0.0%", "-20.0%"));
        assert_eq!((m.original_label.as_str(), m.recommended_label.as_str()), ("Current", "Rebalanced"));
        assert_eq!(m.window, "Daily closes, November 12, 2025 to November 11, 2026 (60 trading days)");
        assert!(m.note.contains("price returns, excluding dividends"), "{}", m.note);
        assert!(m.note.contains("cover only the days inside the window"), "{}", m.note);
        assert!(m.monthly.is_empty() && m.drawdowns.is_empty() && m.monte_carlo.is_empty());
    }

    #[test]
    fn all_cash_prints_no_holdings() {
        let build = build_metrics(&[], &[], Ok(HashMap::new()));
        assert_eq!(build.note, Some(NOTE_NO_HOLDINGS));
        let m = metrics_movement_from(&build, "Current", "Rebalanced", REQUESTED.1);
        assert_eq!(m.note, "No holdings to measure.");
        assert_eq!(m.window, "Trailing 365 days of daily closes to November 11, 2026");
        assert!(m.rows.is_empty() && m.monthly.is_empty() && m.drawdowns.is_empty() && m.monte_carlo.is_empty());
        let h = health_movement_from(&HealthInput::default(), &HealthInput::default(), "Current", "Rebalanced");
        assert!(h.note.starts_with("No holdings to measure."), "{}", h.note);
        assert_eq!(h.rows.len(), 8);
    }

    #[test]
    fn price_fetch_failure_degrades_to_a_note() {
        let held = [("VOO".to_string(), 1.0)];
        let build = build_metrics(&held, &held, Err(AppError::Provider("FMP down".into())));
        assert!(build.original.is_none() && build.recommended.is_none());
        assert_eq!(build.note, Some(NOTE_NO_PRICES));
        let m = metrics_movement_from(&build, "a", "b", REQUESTED.1);
        assert_eq!(m.note, "Price history was not available; metrics were skipped.");
        assert!(m.rows.is_empty() && m.monte_carlo.is_empty());
        // Closes that cover none of the held symbols read the same way.
        assert_eq!(build_metrics(&held, &held, Ok(HashMap::new())).note, Some(NOTE_NO_PRICES));
    }

    fn closes_for(symbol: &str, from: NaiveDate, days: i64) -> (String, Vec<(NaiveDate, f64)>) {
        let points = (0..days)
            .map(|i| {
                let dip = if i % 7 == 3 { 0.98 } else { 1.0 };
                (from + Duration::days(i), 100.0 * (1.0 + 0.0005 * i as f64) * dip)
            })
            .collect();
        (symbol.to_string(), points)
    }

    #[test]
    fn metrics_from_closes_fill_every_table_and_name_unpriced_symbols() {
        let closes: HashMap<String, Vec<(NaiveDate, f64)>> = [closes_for("VOO", REQUESTED.0, 300)].into();
        let build = build_metrics(
            &[("VOO".to_string(), 1.0)],
            &[("VOO".to_string(), 2.0), ("ZZZ".to_string(), 1.0)],
            Ok(closes),
        );
        assert_eq!(build.note, None);
        let m = metrics_movement_from(&build, "Current", "Rebalanced", REQUESTED.1);
        assert_eq!(m.rows.len(), 10);
        assert_eq!((m.monthly.len(), m.drawdowns.len(), m.monte_carlo.len()), (2, 2, 2));
        assert_eq!((m.monthly[1].label.as_str(), m.monthly[0].years[0].year), ("Rebalanced", 2025));
        // Five episodes, deepest first, dated the way the PDF formats them.
        let episodes = &m.drawdowns[0].rows;
        assert_eq!(episodes.len(), 5);
        assert!(episodes.windows(2).all(|w| w[0].depth_pct <= w[1].depth_pct), "{episodes:?}");
        assert!(NaiveDate::parse_from_str(&episodes[0].start, "%Y-%m-%d").is_ok(), "{}", episodes[0].start);
        assert!(episodes[0].end.is_some() && episodes[0].days > 0);
        assert_eq!((m.monte_carlo[0].paths, m.monte_carlo[0].horizon_days), (1000, 252));
        // Twice the shares of the same fund is the same return series: identical figures and odds.
        assert_eq!(m.rows[0].original, m.rows[0].recommended);
        assert_eq!(m.monte_carlo[0].p50, m.monte_carlo[1].p50);
        assert!(m.note.contains("No price history for ZZZ; left out of Rebalanced."), "{}", m.note);
        assert_eq!(m.window, "Daily closes, November 12, 2025 to September 7, 2026 (300 trading days)");
    }

    #[test]
    fn a_young_holding_on_one_side_shortens_both_windows_and_is_named() {
        // XYZ first traded on June 1, 2026: both portfolios are measured from that day, and the
        // window line says so, so the side-by-side figures stay comparable.
        let closes: HashMap<String, Vec<(NaiveDate, f64)>> =
            [closes_for("VOO", REQUESTED.0, 365), closes_for("XYZ", d(2026, 6, 1), 164)].into();
        let build = build_metrics(
            &[("VOO".to_string(), 1.0)],
            &[("VOO".to_string(), 1.0), ("XYZ".to_string(), 3.0)],
            Ok(closes),
        );
        let (o, r) = (build.original.as_ref().unwrap(), build.recommended.as_ref().unwrap());
        assert_eq!((o.days, r.days), (164, 164));
        let m = metrics_movement_from(&build, "Current", "Rebalanced", REQUESTED.1);
        assert_eq!(
            m.window,
            "Daily closes, June 1, 2026 to November 11, 2026 (164 trading days); the window opens at XYZ's first close, so both portfolios are measured over the same days"
        );
        assert!(!m.note.contains("No price history"), "{}", m.note);
    }

    #[test]
    fn a_short_history_prints_n_a_and_says_why() {
        let points: Vec<(NaiveDate, f64)> = (0..10).map(|i| (d(2026, 11, 1) + Duration::days(i), 100.0 + i as f64)).collect();
        let closes: HashMap<String, Vec<(NaiveDate, f64)>> = [("VOO".to_string(), points)].into();
        let build = build_metrics(&[("VOO".to_string(), 1.0)], &[], Ok(closes));
        let m = metrics_movement_from(&build, "Current", "Rebalanced", REQUESTED.1);
        assert!(m.rows.iter().all(|r| r.original == "n/a" && r.recommended == "n/a"), "{:?}", m.rows);
        assert_eq!(m.window, "Daily closes, November 1, 2026 to November 10, 2026 (10 trading days)");
        assert!(m.note.contains("Fewer than 30 trading days in the window"), "{}", m.note);
        assert!(m.note.contains("insufficient history"), "{}", m.note);
        assert!(m.note.contains("Rebalanced: no holdings to measure."), "{}", m.note);
        assert!(m.monte_carlo.is_empty());
    }

    #[test]
    fn monte_carlo_lines_read_as_the_pdf_prints_them() {
        let line = MonteCarloLine {
            label: "Current".into(),
            paths: 1000,
            horizon_days: 252,
            p5: -0.081,
            p50: 0.123,
            p95: 0.31,
            bust_threshold: -0.20,
            bust_probability: 0.23,
            goal_threshold: 0.10,
            goal_probability: 0.68,
        };
        assert_eq!(
            monte_carlo_text(&line),
            "In 1,000 bootstrapped years (252 trading days each): median +12.3%; 5th to 95th percentile -8.1% to +31.0%; odds of a -20% drawdown 23%; odds of +10% or better 68%."
        );
    }

    fn health_holding(symbol: &str, value: f64) -> HealthHolding {
        HealthHolding {
            symbol: symbol.to_string(),
            value,
            asset_class: Some(AssetClass::Equity),
            sector: None,
            is_fund: Some(false),
            expense_ratio: None,
        }
    }

    #[test]
    fn health_rows_carry_both_sides_statuses() {
        let o = HealthInput {
            holdings: vec![health_holding("AAPL", 100.0)],
            cash: Some(-5.0),
            trade_fees: None,
            unresolved_violations: Some(1),
        };
        let r = HealthInput {
            holdings: (0..10).map(|i| health_holding(&format!("S{i}"), 10.0)).collect(),
            cash: Some(50.0),
            trade_fees: None,
            unresolved_violations: Some(0),
        };
        let h = health_movement_from(&o, &r, "Current", "Rebalanced");
        let row = |key: &str| h.rows.iter().find(|r| r.key == key).unwrap();
        assert_eq!((row("buying_power").original.status.as_str(), row("buying_power").recommended.status.as_str()), ("fail", "pass"));
        assert_eq!((row("single_position").original.status.as_str(), row("single_position").recommended.status.as_str()), ("fail", "pass"));
        assert_eq!(row("single_position").original.value_text, "AAPL · 100.0%");
        assert_eq!((row("account_rules").original.status.as_str(), row("account_rules").original.value_text.as_str()), ("fail", "1 unresolved"));
        assert_eq!(row("buying_power").threshold, "cash at or above $0");
        assert_eq!(h.rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), RULES.iter().map(|r| r.key).collect::<Vec<_>>());
        assert_eq!((h.original_label.as_str(), h.recommended_label.as_str()), ("Current", "Rebalanced"));
        assert!(!h.note.starts_with("No holdings"), "{}", h.note);
    }

    #[test]
    fn classification_follows_fmp_data() {
        let etf = |class: Option<&str>, ratio: Option<f64>| EtfInfo {
            symbol: "X".into(),
            name: None,
            asset_class: class.map(String::from),
            avg_volume: None,
            description: None,
            etf_company: None,
            expense_ratio: ratio,
            inception_date: None,
            holdings_count: None,
            nav: None,
            net_assets: None,
        };
        let profile = |sector: Option<&str>| CompanyProfile {
            symbol: "X".into(),
            company_name: "X".into(),
            exchange: String::new(),
            industry: None,
            sector: sector.map(String::from),
            country: None,
            market_cap: None,
            description: None,
            website: None,
            ceo: None,
            employees: None,
            ipo_date: None,
            currency: None,
            price: None,
            beta: None,
            volume_avg: None,
            last_dividend: None,
            image: None,
        };
        use Lookup::{Absent, Failed, Found};
        // A fund: FMP's expense ratio is in percent (0.03 = 0.03%); a fund has no sector of its own.
        let fund = classify(Found(&etf(Some("Equity"), Some(0.03))), Found(&profile(Some("Financial Services"))));
        assert_eq!((fund.asset_class, fund.sector, fund.is_fund), (Some(AssetClass::Equity), None, Some(true)));
        assert!((fund.expense_ratio.unwrap() - 0.0003).abs() < 1e-12);
        assert_eq!(classify(Found(&etf(Some("Fixed Income"), None)), Absent).asset_class, Some(AssetClass::FixedIncome));
        assert_eq!(classify(Found(&etf(Some("Bond"), None)), Absent).asset_class, Some(AssetClass::FixedIncome));
        assert_eq!(classify(Found(&etf(Some("Commodities"), None)), Absent).asset_class, Some(AssetClass::Commodity));
        assert_eq!(classify(Found(&etf(Some("Real Estate"), None)), Absent).asset_class, Some(AssetClass::Other));
        assert_eq!(classify(Found(&etf(None, None)), Absent).asset_class, None);
        // A stock: FMP has no fund record and a profile — equity, with its sector.
        let stock = classify(Absent, Found(&profile(Some("Technology"))));
        assert_eq!(
            stock,
            Classification {
                asset_class: Some(AssetClass::Equity),
                sector: Some("Technology".into()),
                is_fund: Some(false),
                expense_ratio: None,
            }
        );
        assert_eq!(classify(Absent, Found(&profile(Some("  ")))).sector, None);
        // A failed fund lookup cannot tell a bond fund from a stock: unknown, never equity.
        assert_eq!(classify(Failed, Found(&profile(Some("Technology")))), Classification::default());
        assert_eq!(classify(Absent, Failed), Classification::default());
        assert_eq!(classify(Absent, Absent), Classification::default());
        assert_eq!(Classification::default().is_fund, None);
    }

    #[test]
    fn trade_fees_sum_fees_over_traded_volume() {
        let trade = |qty: f64, price: f64, fees: f64| ClientTrade {
            id: None,
            client_id: 1,
            import_batch_id: None,
            symbol: "VOO".into(),
            exchange: String::new(),
            trade_date: "2026-01-02".into(),
            trade_type: "buy".into(),
            quantity: qty,
            price,
            fees,
            order_id: None,
            notes: None,
            created_at: None,
        };
        assert_eq!(trade_fees(&[trade(10.0, 100.0, 1.0), trade(5.0, 200.0, 0.5)]), Some((1.5, 2000.0)));
        assert_eq!(trade_fees(&[]), None);
    }

    #[test]
    fn the_prompt_prints_the_metrics_and_health_sections() {
        let build = build_metrics(&[], &[], Ok(HashMap::new()));
        let m = metrics_movement_from(&build, "Current", "Rebalanced", REQUESTED.1);
        let mut s = String::new();
        push_metrics(&mut s, &m);
        assert!(s.contains("## Metrics (app-computed)\nWindow: Trailing 365 days of daily closes to November 11, 2026\n"), "{s}");
        assert!(s.contains("(No holdings to measure.)"), "{s}");
        let h = health_movement_from(&HealthInput::default(), &HealthInput::default(), "Current", "Rebalanced");
        let mut s = String::new();
        push_health(&mut s, &h);
        assert!(s.contains("## Health (app-computed)\ncheck: original | recommended | threshold\n"), "{s}");
        assert!(s.contains("Buying power: n/a Cash not recorded | n/a Cash not recorded | cash at or above $0\n"), "{s}");
    }

    #[test]
    fn share_counts_group_thousands() {
        assert_eq!(shares(15000.0), "15,000");
        assert_eq!(shares(-3.0), "3");
        assert_eq!(shares(1234.5), "1,234.5");
    }

    #[test]
    fn prices_and_values_read_as_the_pdf_prints_them() {
        assert_eq!(price_text(6129.4), "$6,129.40");
        assert_eq!(price_text(60.44), "$60.44");
        assert_eq!(price_text(0.7239), "$0.7239");
        assert_eq!(price_text(0.0001), "$0.0001");
        assert_eq!(value_text(6044.0), "$6,044");
        assert_eq!(value_text(10.0), "$10");
        assert_eq!(value_text(7.2346), "$7.23");
        assert_eq!(value_text(0.2), "$0.20");
        assert_eq!(value_text(0.015), "$0.02");
        assert_eq!(value_text(0.004), "under $0.01");
        assert_eq!(value_text(0.0), "$0.00");
    }

    #[test]
    fn usd_groups_thousands_and_keeps_the_sign() {
        assert_eq!(usd(1234567.4), "$1,234,567");
        assert_eq!(usd(-950.0), "-$950");
        assert_eq!(usd(0.0), "$0");
    }
}
