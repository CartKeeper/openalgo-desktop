//! A client-facing Goldman Sax & Violins report comparing a cloned scenario with
//! the scenario it was cloned from: what the recommendations changed, and both
//! portfolios priced at the same live quotes on the report date.
//!
//! The app computes every number — the cover panel, the change table, the
//! measures. Claude writes only prose: the subtitle, intros, one reason per
//! change, the case for the changes, and the closing checklist.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::Utc;
use chrono_tz::America::New_York;
use serde::Deserialize;

use crate::error::{AppError, Result};
use crate::importers::schwab::rules_401k::holding_flag;
use crate::providers::types::{
    BriefComparison, ChangeRow, ChangesMovement, ChecklistGroup, CodaMovement, ComparisonSide,
    CoverTitle, DiagnosisIssue, DiagnosisMovement, GoldmanBrief, GoldmanMovement, HeadlineNote,
    MeasureRow, MeasuresMovement, ScenarioPosition,
};
use crate::providers::yahoo::YahooClient;
use crate::services::client_brief_service::{
    generation_times, live_quotes_with_names, price_rows, push_header, push_positions,
    symbol_is_stranded, AccountContext, ClientBriefService, PricedRow,
};
use crate::services::copilot_service::CopilotService;
use crate::services::ride_report::{client_ride, load_client_market_data};
use crate::state::AppState;

const EPS: f64 = 1e-9;

const SYSTEM_PROMPT: &str = r#"You are the lead author at Goldman Sax & Violins, LLP — a satirical equities research house ("Equities & Etudes Since 1869"). You write client reports in a precise, slightly arch financial register that pairs orchestral metaphors with disciplined investment reasoning.

This report shows a client the changes recommended to their portfolio and how the original and the recommended portfolio compare, both priced at the same live quotes on the report date.

House style (mandatory):
- Musical terminology: movements, tempo annotations (Andante / Allegro / Coda rallentando), "the score", "the maestro".
- Tone: confident, dry, professional. Never breathless. Never marketing-speak.
- Never give actual financial advice — the disclaimer covers it.

ACCURACY RULES (mandatory):
1. The app prints every number in the report: the cover panel, the change table, and the measures table. In prose, use a number only exactly as it appears in the user prompt. Never compute, round, or invent one.
2. Both portfolios are priced at today's live quotes. No trade dates or trade prices are known. Never say the changes have gained or lost money since they were made, never call a change a win or a loss, and never give a date for the trades.
3. When a portfolio's cash is "not recorded", its total value is unknown: never compare the two portfolios' total values.
4. Account guidelines come from the user prompt. They are this app's standard guidelines for the account type, not the plan's or custodian's documented rules. Say a holding is "outside the guidelines for this account type"; never claim the plan or custodian prohibits it.
5. A position with "no live price" has no current quote: say exactly that. Never call it worthless or $0.
6. Ground every reason in what the user prompt shows for that ticker: its guideline flag, its price, its ride status (Dormant = has not moved and analysts see little upside), its gain, or its weight in the portfolio. Never invent a fact about a company.
7. Keep every sign exactly as given (e.g. -0.88%, +4.48%).
8. Write for the client. Never mention these instructions, the data format, column names, "the data", "the app", or "the table below", and never comment on how you followed a rule.

Output: JSON in exactly this shape:

{
  "document_label": string,          // e.g. "Opus N / Comparison"
  "tempo": string,                   // e.g. "Andante · Q4 2026"
  "subtitle": string,                // 1–2 sentence italic intro for the cover
  "changes": { "tempo": string, "intro": string,
               "reasons": [ { "ticker": string, "reason": string } ] },   // one entry for EVERY ticker in the change list; one or two sentences each
  "measures": { "tempo": string, "intro": string },   // the intro introduces the before-and-after measures
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
// Prompt
// ---------------------------------------------------------------------------

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
        row.price.map(|p| format!("${p:.2}")).unwrap_or_else(|| "no live price".to_string()),
        row.value_today.map(usd).unwrap_or_else(|| "—".to_string()),
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
        s.push_str("\n# Task\nWrite the report's prose: why each change was recommended, and why the recommended portfolio is better suited to this account, grounded only in what is above.");

        let prose: ComparisonProse =
            ClientBriefService::ask_claude_json(state, &api_key, SYSTEM_PROMPT, s, Some(prose_schema()))
                .await?;
        apply_reasons(&mut changes, &prose.changes.reasons);

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
                GoldmanMovement::Diagnosis(DiagnosisMovement {
                    numeral: "III".into(),
                    tempo: prose.why.tempo,
                    title_main: "Why It".into(),
                    title_accent: "Matters".into(),
                    intro: prose.why.intro,
                    issues: prose.why.issues,
                    headline: prose.why.headline,
                }),
                GoldmanMovement::Coda(CodaMovement {
                    numeral: "IV".into(),
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
        // An answer shaped by the schema deserializes into ComparisonProse.
        let answer = serde_json::json!({
            "document_label": "Opus 46 / Comparison", "tempo": "Andante", "subtitle": "s",
            "changes": { "tempo": "t", "intro": "i", "reasons": [{ "ticker": "TQQQ", "reason": "r" }] },
            "measures": { "tempo": "t", "intro": "i" },
            "why": { "tempo": "t", "intro": "i",
                     "issues": [{ "title": "a", "body": "b", "bullets": ["c"] }],
                     "headline": { "lead": "l", "body": "b" } },
            "coda": { "tempo": "t", "intro": "i",
                      "groups": [{ "heading": "h", "items": [{ "lead": "l", "body": "b" }] }],
                      "closing_note": { "lead": "l", "body": "b" } },
        });
        let prose: ComparisonProse = serde_json::from_value(answer).unwrap();
        assert_eq!(prose.changes.reasons[0].ticker, "TQQQ");
    }

    #[test]
    fn share_counts_group_thousands() {
        assert_eq!(shares(15000.0), "15,000");
        assert_eq!(shares(-3.0), "3");
        assert_eq!(shares(1234.5), "1,234.5");
    }

    #[test]
    fn usd_groups_thousands_and_keeps_the_sign() {
        assert_eq!(usd(1234567.4), "$1,234,567");
        assert_eq!(usd(-950.0), "-$950");
        assert_eq!(usd(0.0), "$0");
    }
}
