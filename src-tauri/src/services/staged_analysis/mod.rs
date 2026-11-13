//! Staged "Analyze with Claude" pipeline (WP3).
//!
//! A deterministic dossier, four analysts, a bull/bear debate, a research
//! manager, a risk panel and a portfolio manager, followed by app-side checks.
//! The chat path (`copilot_send_message`, tools, ACTIONS_JSON) is untouched; the
//! staged result is a normal `CopilotResponse` whose markdown ends with an
//! ACTIONS_JSON block the app has already validated.

pub mod dossier;
pub mod render;
pub mod schemas;
pub mod stages;
pub mod validate;

use std::collections::{BTreeSet, HashSet};
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

use crate::error::{AppError, Result};
use crate::providers::anthropic::{Message, MessageContent};
use crate::services::client_brief_service::live_quotes_with_names;
use crate::services::copilot_service::CopilotResponse;
use crate::state::AppState;

use dossier::{redact_secrets, Dossier};
use render::PipelineOutput;
use schemas::{AnalystReport, Case, Decision, ProposedTrade, ResearchPlan, RiskPanel};
use validate::{validate_trades, PricedPosition};

/// The stages, in order, as the progress events name them.
pub const STAGES: [&str; 9] = [
    "dossier",
    "analysts",
    "bull",
    "bear",
    "research_manager",
    "risk_panel",
    "portfolio_manager",
    "checks",
    "render",
];

/// The whole run, dossier to report, must finish inside this.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(600);

/// The user turn recorded in the conversation when no focus is given.
pub const DEFAULT_REQUEST: &str = "Run a full analysis of this scenario.";

/// One progress event; emitted as `staged-analysis-progress`.
#[derive(Debug, Clone, Serialize)]
pub struct StageProgress {
    pub scenario_id: i64,
    /// One of `STAGES`.
    pub stage: String,
    /// `running` | `done` | `failed`
    pub status: String,
    pub detail: Option<String>,
}

/// The error for a stage that failed, naming the stage.
pub fn stage_error(stage: &str, err: AppError) -> AppError {
    AppError::Provider(format!("Full analysis failed at stage '{stage}': {err}"))
}

/// Refuse, before any Claude call, a scenario in which nothing has a price.
pub fn refuse_if_unpriced(d: &Dossier) -> Result<()> {
    if d.holdings.iter().any(|h| h.price.is_some()) {
        Ok(())
    } else {
        Err(AppError::Validation(
            "No priced positions in this scenario; add positions or wait for prices.".to_string(),
        ))
    }
}

/// The user turn recorded for the run: the focus, or the default request.
pub(crate) fn request_text(focus: Option<&str>) -> String {
    focus
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_REQUEST.to_string())
}

/// The scenario's holdings as the checks see them.
pub(crate) fn scenario_positions(d: &Dossier) -> Vec<PricedPosition> {
    d.holdings
        .iter()
        .map(|h| PricedPosition {
            symbol: h.symbol.clone(),
            exchange: h.exchange.clone(),
            quantity: h.quantity,
            price: h.price,
            name: h.name.clone(),
        })
        .collect()
}

/// The holdings plus a quote (price and name, quantity 0) for every proposed
/// name the scenario does not hold, so a buy of a new name can be sized and
/// checked against the account guidelines.
async fn priced_positions(state: &AppState, dossier: &Dossier, trades: &[ProposedTrade]) -> Vec<PricedPosition> {
    let mut positions = scenario_positions(dossier);
    let held: HashSet<String> = positions.iter().map(|p| p.symbol.to_uppercase()).collect();
    let new_names: Vec<String> = trades
        .iter()
        .map(|t| t.symbol.trim().to_uppercase())
        .filter(|s| !s.is_empty() && !held.contains(s))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !new_names.is_empty() {
        let quotes = live_quotes_with_names(state, &new_names).await;
        for symbol in new_names {
            let (price, name) = match quotes.get(&symbol) {
                Some((price, name)) => (Some(*price), name.clone()),
                None => (None, None),
            };
            positions.push(PricedPosition { symbol, exchange: String::new(), quantity: 0.0, price, name });
        }
    }
    positions
}

/// Progress reporting for one run: the sink plus the stage currently running
/// (so a timeout can name it).
struct Progress<'a> {
    scenario_id: i64,
    sink: &'a (dyn Fn(StageProgress) + Sync),
    current: Mutex<&'static str>,
}

impl Progress<'_> {
    fn emit(&self, stage: &str, status: &str, detail: Option<String>) {
        (self.sink)(StageProgress {
            scenario_id: self.scenario_id,
            stage: stage.to_string(),
            status: status.to_string(),
            detail,
        });
    }

    fn running(&self, stage: &'static str) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = stage;
        self.emit(stage, "running", None);
    }

    fn done(&self, stage: &str, detail: Option<String>) {
        self.emit(stage, "done", detail);
    }

    fn failed(&self, stage: &str, detail: &str) {
        self.emit(stage, "failed", Some(redact_secrets(detail)));
    }

    fn current(&self) -> &'static str {
        *self.current.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Run one stage: `running` before, `done` (with `detail` of the result) after,
/// or `failed` and an error naming the stage.
async fn step<T, F>(
    progress: &Progress<'_>,
    stage: &'static str,
    work: F,
    detail: impl Fn(&T) -> Option<String>,
) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    progress.running(stage);
    match work.await {
        Ok(value) => {
            progress.done(stage, detail(&value));
            Ok(value)
        }
        Err(err) => {
            progress.failed(stage, &err.to_string());
            Err(stage_error(stage, err))
        }
    }
}

/// Run the whole pipeline for a scenario and return the report as a normal
/// `CopilotResponse` (no tool calls; the conversation holds the request and the
/// report). Nothing is applied to the scenario.
pub async fn run(
    state: &AppState,
    api_key: &str,
    scenario_id: i64,
    focus: Option<String>,
    on_progress: &(dyn Fn(StageProgress) + Sync),
) -> Result<CopilotResponse> {
    let progress = Progress { scenario_id, sink: on_progress, current: Mutex::new(STAGES[0]) };
    match tokio::time::timeout(RUN_TIMEOUT, pipeline(state, api_key, scenario_id, focus.as_deref(), &progress)).await {
        Ok(result) => result,
        Err(_elapsed) => {
            let stage = progress.current();
            progress.failed(stage, "timed out");
            Err(AppError::Provider(format!(
                "Full analysis timed out after {} minutes at stage '{stage}'",
                RUN_TIMEOUT.as_secs() / 60
            )))
        }
    }
}

async fn pipeline(
    state: &AppState,
    api_key: &str,
    scenario_id: i64,
    focus: Option<&str>,
    progress: &Progress<'_>,
) -> Result<CopilotResponse> {
    progress.running("dossier");
    let dossier = match dossier::build(state, scenario_id).await {
        Ok(dossier) => dossier,
        Err(err) => {
            progress.failed("dossier", &err.to_string());
            return Err(stage_error("dossier", err));
        }
    };
    if let Err(err) = refuse_if_unpriced(&dossier) {
        progress.failed("dossier", &err.to_string());
        return Err(err);
    }
    let priced = dossier.holdings.iter().filter(|h| h.price.is_some()).count();
    let gaps = if dossier.missing.is_empty() { String::new() } else { format!("; {} data gap(s)", dossier.missing.len()) };
    progress.done("dossier", Some(format!("{} holdings, {} priced{}", dossier.holdings.len(), priced, gaps)));
    let dossier_text = dossier::render_for_prompt(&dossier);

    let analysts = step(
        progress,
        "analysts",
        stages::run_analysts(state, api_key, &dossier_text),
        |reports: &[AnalystReport; 4]| Some(format!("{} findings", reports.iter().map(|r| r.findings.len()).sum::<usize>())),
    )
    .await?;
    let bull = step(progress, "bull", stages::run_bull(state, api_key, &dossier_text, &analysts), |case: &Case| {
        Some(format!("{} arguments", case.arguments.len()))
    })
    .await?;
    let bear = step(
        progress,
        "bear",
        stages::run_bear(state, api_key, &dossier_text, &analysts, &bull),
        |case: &Case| Some(format!("{} arguments", case.arguments.len())),
    )
    .await?;
    let plan = step(
        progress,
        "research_manager",
        stages::run_research_manager(state, api_key, &dossier_text, &bull, &bear),
        |plan: &ResearchPlan| Some(format!("{} holdings rated", plan.holdings.len())),
    )
    .await?;
    let risk = step(
        progress,
        "risk_panel",
        stages::run_risk_panel(state, api_key, &dossier_text, &plan),
        |risk: &RiskPanel| Some(format!("{} constraints", risk.constraints.len())),
    )
    .await?;
    let decision = step(
        progress,
        "portfolio_manager",
        stages::run_portfolio_manager(state, api_key, &dossier_text, &plan, &risk, focus),
        |decision: &Decision| Some(format!("{} · {} trades proposed", decision.rating, decision.trades.len())),
    )
    .await?;

    progress.running("checks");
    let positions = priced_positions(state, &dossier, &decision.trades).await;
    let validated =
        validate_trades(decision.trades.clone(), &positions, dossier.cash, dossier.account_type.as_deref());
    progress.done("checks", Some(format!("{} kept, {} dropped", validated.kept.len(), validated.dropped.len())));

    progress.running("render");
    let output = PipelineOutput { analysts, bull, bear, plan, risk, decision, validated };
    let response_text = render::render_markdown(&dossier, &output);
    progress.done("render", None);

    Ok(CopilotResponse {
        response_text: response_text.clone(),
        tool_calls_made: Vec::new(),
        conversation: vec![
            Message { role: "user".to_string(), content: MessageContent::Text(request_text(focus)) },
            Message { role: "assistant".to_string(), content: MessageContent::Text(response_text) },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::services::staged_analysis::dossier::{Dossier, HoldingDossier};
    use std::collections::HashSet;

    fn holding(symbol: &str, quantity: f64, price: Option<f64>) -> HoldingDossier {
        HoldingDossier {
            symbol: symbol.to_string(),
            exchange: "GENERIC".to_string(),
            quantity,
            name: Some(format!("{symbol} Inc")),
            price,
            value: price.map(|p| p * quantity),
            weight_pct: None,
            sector: None,
            industry: None,
            beta: None,
            market_cap: None,
            pe: None,
            debt_to_equity: None,
            consensus_target: None,
            change_1m_pct: None,
            change_3m_pct: None,
            change_1y_pct: None,
            above_sma50: None,
            above_sma200: None,
            pct_from_52w_high: None,
            headlines: Vec::new(),
            detailed: false,
        }
    }

    fn dossier(holdings: Vec<HoldingDossier>) -> Dossier {
        Dossier {
            scenario_id: 12,
            scenario_name: "Casey baseline".to_string(),
            is_baseline: true,
            client_name: "Casey Borst".to_string(),
            account_type: Some("401k".to_string()),
            cash: Some(100.0),
            total_value: None,
            holdings,
            unpriced: Vec::new(),
            compliance: Vec::new(),
            sector_performance: Vec::new(),
            macro_lines: Vec::new(),
            missing: Vec::new(),
            as_of: "Nov 12, 2026 4:05 PM ET".to_string(),
        }
    }

    #[test]
    fn a_failing_stage_is_named() {
        let err = stage_error("bull", AppError::Provider("Claude declined to write this report.".to_string()));
        let text = err.to_string();
        assert!(text.contains("'bull'"), "{text}");
        assert!(text.contains("Claude declined to write this report."), "{text}");
        assert!(matches!(err, AppError::Provider(_)));
    }

    #[test]
    fn no_priced_positions_refuses_early() {
        let unpriced = dossier(vec![holding("AAA", 1.0, None), holding("BBB", 2.0, None)]);
        let err = refuse_if_unpriced(&unpriced).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err}");
        assert!(err.to_string().contains("No priced positions in this scenario"), "{err}");
        assert!(refuse_if_unpriced(&dossier(Vec::new())).is_err());
        assert!(refuse_if_unpriced(&dossier(vec![holding("AAA", 1.0, None), holding("BBB", 2.0, Some(10.0))])).is_ok());
    }

    #[test]
    fn the_stages_are_nine_and_unique() {
        assert_eq!(STAGES.len(), 9);
        let unique: HashSet<&str> = STAGES.iter().copied().collect();
        assert_eq!(unique.len(), 9);
        assert_eq!((STAGES[0], STAGES[8]), ("dossier", "render"));
    }

    #[test]
    fn progress_events_serialize_with_snake_case_fields() {
        let event = StageProgress { scenario_id: 12, stage: "bull".to_string(), status: "running".to_string(), detail: None };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["scenario_id"], 12);
        assert_eq!(value["stage"], "bull");
        assert_eq!(value["status"], "running");
        assert!(value["detail"].is_null());
    }

    #[test]
    fn scenario_positions_carry_quantity_price_name_and_exchange() {
        let d = dossier(vec![holding("VOO", 10.0, Some(500.0)), holding("XYZ", -3.0, None)]);
        let positions = scenario_positions(&d);
        assert_eq!(positions.len(), 2);
        assert_eq!((positions[0].symbol.as_str(), positions[0].quantity, positions[0].price), ("VOO", 10.0, Some(500.0)));
        assert_eq!(positions[0].exchange, "GENERIC");
        assert_eq!(positions[0].name.as_deref(), Some("VOO Inc"));
        assert_eq!((positions[1].quantity, positions[1].price), (-3.0, None));
    }

    #[tokio::test]
    async fn a_failed_stage_is_marked_failed_after_the_finished_ones() {
        let events: Mutex<Vec<StageProgress>> = Mutex::new(Vec::new());
        let sink = |p: StageProgress| events.lock().unwrap().push(p);
        let progress = Progress { scenario_id: 12, sink: &sink, current: Mutex::new(STAGES[0]) };

        let n = step(&progress, "analysts", async { Ok::<usize, AppError>(4) }, |n| Some(format!("{n} findings")))
            .await
            .unwrap();
        assert_eq!(n, 4);
        let cut_off = AppError::Provider("Claude's answer was cut off at the length limit before it finished.".to_string());
        let err = step(&progress, "bull", async { Err::<usize, _>(cut_off) }, |_| None).await.unwrap_err();
        assert!(err.to_string().contains("'bull'"), "{err}");
        assert_eq!(progress.current(), "bull");

        let seen: Vec<(String, String, Option<String>)> =
            events.lock().unwrap().iter().map(|e| (e.stage.clone(), e.status.clone(), e.detail.clone())).collect();
        let expected_start = vec![
            ("analysts".to_string(), "running".to_string(), None),
            ("analysts".to_string(), "done".to_string(), Some("4 findings".to_string())),
            ("bull".to_string(), "running".to_string(), None),
        ];
        assert_eq!(seen[..3].to_vec(), expected_start);
        assert_eq!((seen[3].0.as_str(), seen[3].1.as_str()), ("bull", "failed"));
        assert!(seen[3].2.as_deref().unwrap_or_default().contains("cut off"), "{seen:?}");
        assert_eq!(seen.len(), 4, "nothing re-marks the finished stage");
    }

    #[test]
    fn the_request_text_is_the_focus_or_the_default() {
        assert_eq!(request_text(None), DEFAULT_REQUEST);
        assert_eq!(request_text(Some("   ")), DEFAULT_REQUEST);
        assert_eq!(request_text(Some(" tax efficiency ")), "tax efficiency");
    }
}
