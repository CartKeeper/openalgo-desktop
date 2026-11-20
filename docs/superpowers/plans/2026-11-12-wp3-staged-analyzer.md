# WP3 — Staged Analyzer (Analyze with Claude) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** "Analyze with Claude" can run a full, staged analysis — a deterministic dossier, four analysts, a bull/bear debate, a research manager, a risk panel, a portfolio manager, then app-side checks — and hand the result to the existing Review Recommendations flow unchanged.

**Architecture:** A new `staged_analysis` service runs the pipeline with structured outputs (closed JSON schemas) and emits one Tauri progress event per stage. The chat path (`copilot_send_message`, tools, ACTIONS_JSON) is untouched; the staged result is returned as a normal `CopilotResponse` whose markdown ends with an ACTIONS_JSON block the app has already validated (whole shares, no oversell, cumulative cash cap, 401(k) flags). Stage structure follows TradingAgents (`analysts → researchers → research_manager → risk_mgmt → portfolio_manager`); the "LLM proposes, plain code decides" rule follows ai-hedge-fund.

**Tech Stack:** Rust (`AnthropicClient::send_structured`, `ClientBriefService::ask_claude_json`, `futures::stream::buffer_unordered`), Tauri 2 events (`AppHandle::emit`), React (`@tauri-apps/api/event` `listen`), vitest.

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md` (WP3 requirements; UI additions; open item 2 on cost).

## Global Constraints

- Branch `wp3-staged-analyzer`; files below plus WP3 anchors.
- Every schema is closed: every object has `additionalProperties: false` and lists all properties as required; optional values are nullable types. The schema-closure test from `scenario_comparison.rs` is the pattern.
- The ACTIONS_JSON block format is exactly the one documented in `copilot_service.rs` ("## Actionable Recommendations"): fields `symbol, exchange, side, quantity, orderType, product, price` plus `triggerPrice`/`trailPercent` when relevant, `rationale`.
- Never applies trades; never mentions the user's own broker cash in a client scenario (reuse `budget_section(CopilotContext::ClientScenario)` text).
- One full run ≈ 9 Claude calls (4 analysts in parallel, bull, bear, research manager, risk panel, portfolio manager). Whole-run timeout 10 minutes; the error names the stage.
- Times shown to the user in `America/New_York`, 12-hour.

## Review Focus

1. Claude stops with `max_tokens` or `refusal` mid-pipeline: the run fails at that stage with the stage name, earlier stages' progress stays "done", nothing is applied. (Task 6 test "a failing stage is named".)
2. FMP is down or the key is missing: the dossier still builds from scenario data alone and says what is missing; analysts are told. (Task 2 test "dossier degrades without market data".)
3. A scenario with no priced positions: the pipeline refuses before any Claude call with a clear message. (Task 6 test "no priced positions refuses early".)
4. Portfolio manager proposes a sell the client does not hold, a fractional quantity, or more buys than cash: dropped or resized with a reason the user can read. (Task 4 tests.)
5. More than 20 holdings: full dossier for the top 20 by value, the rest summarized; the prompt stays under a size cap. (Task 2 test "dossier caps detail at 20 holdings".)

---

### Task 1: Schemas and closure test

**Files:**
- Create: `src-tauri/src/services/staged_analysis/mod.rs` (`pub mod schemas; pub mod dossier; pub mod stages; pub mod validate; pub mod render;`), `src-tauri/src/services/staged_analysis/schemas.rs`
- Modify: `src-tauri/src/services/mod.rs` (WP3 anchor: `pub mod staged_analysis;`)

**Interfaces:**
- Produces (all `Deserialize + Serialize + Debug`):
  ```rust
  pub struct Finding { pub subject: String /* ticker or "PORTFOLIO" */, pub point: String, pub evidence: String, pub stance: String /* "bullish"|"bearish"|"neutral" */, pub weight: u8 /* 1..=5 */ }
  pub struct AnalystReport { pub summary: String, pub findings: Vec<Finding> }
  pub struct Argument { pub subject: String, pub claim: String, pub evidence_ref: String }
  pub struct Case { pub thesis: String, pub arguments: Vec<Argument>, pub rebuttals: Vec<String> }
  pub struct HoldingCall { pub ticker: String, pub rating: String /* Buy|Overweight|Hold|Underweight|Sell */, pub rationale: String }
  pub struct ResearchPlan { pub overall: String, pub holdings: Vec<HoldingCall>, pub strategic_actions: Vec<String>, pub what_would_change_it: Vec<String> }
  pub struct Stance { pub view: String, pub adjustments: Vec<String> }
  pub struct RiskPanel { pub aggressive: Stance, pub conservative: Stance, pub neutral: Stance, pub constraints: Vec<String> }
  pub struct ProposedTrade { pub symbol: String, pub exchange: String, pub side: String /* BUY|SELL */, pub quantity: f64, pub order_type: String /* MARKET|LIMIT|SL|SL-M|TRAILING_STOP */, pub price: f64, pub trigger_price: Option<f64>, pub trail_percent: Option<f64>, pub rationale: String }
  pub struct Decision { pub executive_summary: String, pub thesis: String, pub rating: String, pub trades: Vec<ProposedTrade>, pub risks: Vec<String> }
  pub fn analyst_schema() -> Value; pub fn case_schema() -> Value; pub fn research_plan_schema() -> Value; pub fn risk_panel_schema() -> Value; pub fn decision_schema() -> Value
  ```
  `rating`, `stance`, `side`, `order_type` use `"enum"` in the schema. Nullable: `{"type": ["number","null"]}`.

- [ ] **Step 1: Write the failing tests**: `assert_closed` (copy the helper from `scenario_comparison.rs` tests) on all five schemas; a sample JSON per schema deserializes into its struct; `decision_schema()` has `enum` for `side` with exactly `["BUY","SELL"]`.
- [ ] **Step 2–4:** `cargo test --lib staged_analysis::schemas` fail → implement → pass.
- [ ] **Step 5: Commit** `feat(analysis): closed schemas for the staged pipeline`

---

### Task 2: Dossier (deterministic data gathering)

**Files:**
- Create: `src-tauri/src/services/staged_analysis/dossier.rs`

**Interfaces:**
- Consumes: `state.sqlite.get_client_scenario_by_id`, `get_scenario_positions`, `get_client_by_id`, `get_client_compliance_violations`, `portfolio_service::enrich_positions_with_prices`, `FmpClient::{get_company_profile, get_key_metrics, get_ratios, get_price_target_consensus, get_stock_news, get_daily_closes, get_sector_performance}`, `providers/fred.rs` (10-year yield, fed funds, CPI series as that provider exposes them; read the file).
- Produces:
  ```rust
  pub struct HoldingDossier { pub symbol: String, pub exchange: String, pub quantity: f64, pub price: Option<f64>, pub value: Option<f64>, pub weight_pct: Option<f64>, pub sector: Option<String>, pub industry: Option<String>, pub beta: Option<f64>, pub market_cap: Option<f64>, pub pe: Option<f64>, pub debt_to_equity: Option<f64>, pub consensus_target: Option<f64>, pub change_1m_pct: Option<f64>, pub change_3m_pct: Option<f64>, pub change_1y_pct: Option<f64>, pub above_sma50: Option<bool>, pub above_sma200: Option<bool>, pub pct_from_52w_high: Option<f64>, pub headlines: Vec<String> /* ≤5 */, pub detailed: bool }
  pub struct Dossier { pub scenario_id: i64, pub scenario_name: String, pub is_baseline: bool, pub client_name: String, pub account_type: Option<String>, pub cash: Option<f64>, pub total_value: Option<f64>, pub holdings: Vec<HoldingDossier> /* sorted by value desc */, pub unpriced: Vec<String>, pub compliance: Vec<String>, pub sector_performance: Vec<(String, f64)>, pub macro_lines: Vec<String>, pub missing: Vec<String> /* data sources that failed */, pub as_of: String /* ET, 12-hour */ }
  pub async fn build(state: &AppState, scenario_id: i64) -> Result<Dossier>   // top 20 by value get `detailed: true` and the FMP calls; concurrency 8 via buffer_unordered; any provider error → entry in `missing`, never an Err
  pub fn render_for_prompt(d: &Dossier) -> String                               // deterministic; one line per holding; ≤ 24,000 characters
  ```

- [ ] **Step 1: Write the failing tests** on a hand-built `Dossier`: `render_for_prompt` contains each ticker and `weight` with one decimal, the cash line `Cash: $142,569.15` for `cash = Some(142569.15)`, `Cash: not recorded` for `None`, the account type, every compliance line; "dossier caps detail at 20 holdings" (build the holdings vector with 25 entries through the pure `mark_detailed(&mut Vec<HoldingDossier>)` helper → exactly 20 `detailed`); "dossier degrades without market data" (`missing` non-empty → the rendering starts with a `Data not available: …` line).
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(analysis): deterministic scenario dossier`

---

### Task 3: Stage runners and prompts

**Files:**
- Create: `src-tauri/src/services/staged_analysis/stages.rs`

**Interfaces:**
- Consumes: `ClientBriefService::ask_claude_json::<T>(state, api_key, system_prompt, user_prompt, Some(schema))` (it is `pub(crate)`).
- Produces:
  ```rust
  pub const SYSTEM_COMMON: &str   // AI-generated disclaimer, client-scenario budget text, never sell what isn't held, whole shares, quote dossier numbers only
  pub async fn run_analysts(state, api_key, dossier_text: &str) -> Result<[AnalystReport; 4]>   // fundamentals, market (price/technicals), news, macro; tokio::join! of four calls
  pub async fn run_bull(state, api_key, dossier_text, analysts: &[AnalystReport; 4]) -> Result<Case>
  pub async fn run_bear(state, api_key, dossier_text, analysts, bull: &Case) -> Result<Case>
  pub async fn run_research_manager(state, api_key, dossier_text, bull, bear) -> Result<ResearchPlan>
  pub async fn run_risk_panel(state, api_key, dossier_text, plan: &ResearchPlan) -> Result<RiskPanel>
  pub async fn run_portfolio_manager(state, api_key, dossier_text, plan, risk: &RiskPanel, focus: Option<&str>) -> Result<Decision>
  ```
  Prompt texts: the research manager and portfolio manager prompts carry TradingAgents' rating scale (Buy / Overweight / Hold / Underweight / Sell) and its rule that conflict alone is never a reason to Hold; the portfolio manager must size trades to the scenario's cash plus proposed sell proceeds, whole shares, and must not sell more than held.

- [ ] **Step 1: Write the failing tests** (prompt assertions, no network): `SYSTEM_COMMON` contains "whole shares", "never recommend selling", "AI-generated"; the portfolio-manager user prompt built by `pm_prompt(dossier_text, plan, risk, focus)` contains the rating scale words and `Overweight`; `focus = Some("tax efficiency")` appears verbatim.
- [ ] **Step 2–4:** fail → implement (prompt builders as pure `fn` so they are testable; the `run_*` wrappers call Claude) → pass.
- [ ] **Step 5: Commit** `feat(analysis): analyst, debate, manager and risk stages`

---

### Task 4: App-side validation of proposed trades

**Files:**
- Create: `src-tauri/src/services/staged_analysis/validate.rs`

**Interfaces:**
- Consumes: `importers::schwab::rules_401k::holding_flag(symbol, name) -> Option<&'static str>`.
- Produces:
  ```rust
  pub struct PricedPosition { pub symbol: String, pub exchange: String, pub quantity: f64, pub price: Option<f64>, pub name: Option<String> }
  pub struct Dropped { pub trade: ProposedTrade, pub reason: String }
  pub struct Validated { pub kept: Vec<ProposedTrade>, pub dropped: Vec<Dropped> }
  pub fn validate_trades(trades: Vec<ProposedTrade>, positions: &[PricedPosition], cash: Option<f64>, account_type: Option<&str>) -> Validated
  ```
  Rules, in order: (1) `quantity = quantity.floor()`; 0 → dropped "Less than one whole share". (2) SELL of a symbol not held → dropped "Not held in this scenario"; SELL above held → resized to held, reason recorded in `kept`'s rationale suffix `" (reduced to the N held)"`. (3) BUY when `account_type` is a 401(k) rule set and `holding_flag(symbol, name).is_some()` → dropped with that flag text. (4) Cumulative cash: `budget = cash.unwrap_or(0) + Σ kept SELL qty × price`; BUYs in listed order: cost = qty × (price if LIMIT else live price); if cost > remaining, `qty = floor(remaining / unit)`; 0 → dropped "Over the cash budget". BUY with no live price and `price 0` → dropped "No price to size this buy".

- [ ] **Step 1: Write the failing tests**: oversell resized; unheld sell dropped; 1.7 shares → 1; flagged 401(k) buy dropped with the flag text; budget: cash 1,000, buy 20 @ 60 → kept as 16; second buy after the budget is spent → dropped "Over the cash budget"; `cash None` with a sell 10 @ 50 funds a buy of 5 @ 100.
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(analysis): deterministic trade validation (whole shares, holdings, cash, 401k flags)`

---

### Task 5: Rendering the result

**Files:**
- Create: `src-tauri/src/services/staged_analysis/render.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct PipelineOutput { pub analysts: [AnalystReport; 4], pub bull: Case, pub bear: Case, pub plan: ResearchPlan, pub risk: RiskPanel, pub decision: Decision, pub validated: Validated }
  pub fn render_markdown(d: &Dossier, out: &PipelineOutput) -> String
  ```
  Sections in order: `## Executive summary`, `**Rating:**`, `## Thesis`, `## What the analysts found` (four short lists), `## Bull vs bear`, `## Risk panel`, `## Trades` (table: Symbol, Side, Qty, Type, Price, Why), `## Dropped by the app's checks` (only if any), a closing line that the trades are ready under **Review Recommendations** (and, on a baseline, that Review applies them to a new clone), the AI disclaimer, then the `<!-- ACTIONS_JSON [...] -->` block built from `validated.kept` (omitted when empty).

- [ ] **Step 1: Write the failing tests**: output ends with a block whose JSON parses to an array with one object per kept trade and the documented keys; no block when `kept` is empty; a baseline dossier produces the "new clone" sentence; a 401(k)-dropped trade appears under "Dropped".
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(analysis): render the staged analysis with an ACTIONS_JSON block`

---

### Task 6: Orchestrator, command, progress events

**Files:**
- Modify: `src-tauri/src/services/staged_analysis/mod.rs`
- Create: `src-tauri/src/commands/staged_analysis.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (WP3 anchors)

**Interfaces:**
- Produces:
  ```rust
  pub const STAGES: [&str; 9] = ["dossier","analysts","bull","bear","research_manager","risk_panel","portfolio_manager","checks","render"];
  #[derive(Clone, Serialize)] pub struct StageProgress { pub scenario_id: i64, pub stage: String, pub status: String /* running|done|failed */, pub detail: Option<String> }
  pub async fn run(state: &AppState, api_key: &str, scenario_id: i64, focus: Option<String>, on_progress: &(dyn Fn(StageProgress) + Sync)) -> Result<CopilotResponse>
  ```
  `run`: builds the dossier; if no holding has a price → `Err(AppError::Validation("No priced positions in this scenario; add positions or wait for prices."))` before any Claude call; wraps the whole pipeline in `tokio::time::timeout(600 s)`; on a stage error returns `Err(AppError::Provider(format!("Full analysis failed at stage '{stage}': {e}")))` after emitting `failed`; the `CopilotResponse` has `response_text = render_markdown(..)`, `tool_calls_made = vec![]`, `conversation = [user: focus or "Run a full analysis of this scenario.", assistant: response_text]`.
- Command: `staged_run_analysis(app: tauri::AppHandle, state: State<'_, AppState>, scenario_id: i64, focus: Option<String>) -> Result<CopilotResponse, AppError>`; emits `staged-analysis-progress` with `StageProgress` via `app.emit(...)` (Tauri 2 `Emitter`). API key as in `copilot_send_message`.

- [ ] **Step 1: Write the failing tests** (pure parts): "a failing stage is named" (`stage_error("bull", err)` message contains `'bull'`); "no priced positions refuses early" (`refuse_if_unpriced(&dossier)` → `Err` for a dossier whose holdings all have `price None`); `STAGES` has 9 unique entries.
- [ ] **Step 2–4:** fail → implement → pass; `cargo test --lib` all pass.
- [ ] **Step 5: Commit** `feat(analysis): staged_run_analysis command with progress events`

---

### Task 7: Dialog — Run full analysis and progress

**Files:**
- Create: `src/api/stagedAnalysis.ts`, `src/components/clients/StagedProgress.tsx`
- Modify: `src/pages/clients/AnalyzeWithClaudeDialog.tsx`
- Test: `src/components/clients/StagedProgress.test.tsx`, `src/lib/stagedAnalysisSample.test.ts`

**Interfaces:**
- `runStagedAnalysis(scenarioId: number, focus?: string): Promise<CopilotResponse>`; `onStagedProgress(scenarioId, cb: (p: StageProgress) => void): Promise<UnlistenFn>`.
- `StagedProgress({ stages: { key: string; label: string; status: 'pending'|'running'|'done'|'failed' }[] })`: a compact list with an icon per status; labels: Dossier, Analysts, Bull case, Bear case, Research manager, Risk panel, Portfolio manager, Checks, Report.
- Dialog: a second button beside Send labelled **Run full analysis** (40px, secondary); disabled while a chat message or a run is in flight; `focus` = the current input text (sent as the focus, input cleared). While running, `StagedProgress` renders above the input. The response is appended to `messages` and `conversationHistory` exactly as a chat response is, so the existing Review Recommendations button appears when the text carries actions.

- [ ] **Step 1: Write the failing tests**: `StagedProgress` renders nine rows and the right icon per status; `stagedAnalysisSample.test.ts` feeds a markdown sample in the Task 5 format to `parseActionsFromMarkdown(sample, 'copilot')` and expects two actions with `side` and `quantity`.
- [ ] **Step 2–4:** fail → implement → pass; `npm run build`, `npm run lint` → 0.
- [ ] **Step 5: Commit** `feat(clients): run a full staged analysis from the Analyze dialog`

---

### Task 8: Help text and gates

- [ ] **Step 1:** Help content for the dialog/page: what the stages are, that it takes several minutes and about nine Claude calls, that trades are checked by the app before they are offered, and that nothing is applied until Review Recommendations.
- [ ] **Step 2: Gates**: `cd src-tauri && cargo test --lib`; `npx vitest run`; `npm run build`; `npm run lint`.
- [ ] **Step 3: Commit** `docs(clients): help text for the staged analysis`

**Definition of Done (user, after merge):** Clients → Casey → baseline scenario → Analyze with Claude → **Run full analysis**. The stage list ticks through nine stages (minutes). The result reads as a report: executive summary, rating, analyst findings, bull vs bear, risk panel, a trades table, and any trades the app dropped with the reason. The **Review Recommendations** button appears and opens the review modal with those trades, "Create Clone & Apply" on the baseline. The baseline's positions and cash are unchanged. A follow-up chat question about the result gets an answer that refers to it.
