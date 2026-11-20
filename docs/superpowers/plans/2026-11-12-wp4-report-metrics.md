# WP4 — Comparison Report Metrics and Health Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The client comparison PDF gains a Metrics movement (CAGR, volatility, Sharpe, Sortino, max drawdown, Calmar, best/worst month, VaR/CVaR, a monthly-returns grid, drawdown periods, Monte Carlo bust/goal odds) and a Health movement (a checklist adapted from Ghostfolio's rules), both computed by the app for the original and the recommended portfolio.

**Architecture:** Two new pure modules (`report_metrics.rs`, `portfolio_health.rs`) reuse `quant_service` where it already has the math. `scenario_comparison.rs` builds both movements from a trailing-365-day hypothetical series (today's positions held through the window, forward-filled FMP closes), prints their numbers into Claude's prompt, and asks Claude only for a `tempo`/`intro` per movement. `GoldmanReport.tsx` renders two new movement kinds. Monte Carlo is seeded so a re-run prints the same PDF.

**Tech Stack:** Rust (`quant_service`, `rand::rngs::StdRng`), react-pdf, vitest render test.

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md` (WP4 requirements; thresholds table rows "Metrics window" through "Health: account rules").

## Global Constraints

- Branch `wp4-report-metrics`; files below only (no anchors needed: no new commands or migrations).
- Claude never computes or invents a number: every figure in these movements is app-printed; the prose rules 1 and 9 of the comparison prompt stay.
- Window: trailing 365 calendar days ending at the report's price time. Risk-free rate 0% (labelled). Monte Carlo: 1,000 paths, 252 days, seed `20261112`, bust = max drawdown ≤ −20%, goal = terminal ≥ +10%.
- Health thresholds are the spec's table, held in one `const RULES` table in `portfolio_health.rs` so the user can change them in one place.
- The report must still generate when metrics cannot be computed (no prices, short history): the movement prints a note instead of failing.

## Review Focus

1. Fewer than 30 trading days of history for the window: metric rows show "n/a (insufficient history)", Monte Carlo is skipped with a note. (Task 1 test "short series yields n/a".)
2. A position with no closes at all: excluded from the series, named in the movement's note. (Task 2 test "unpriced symbol is noted".)
3. All-cash scenario (no positions): both movements print "No holdings to measure" and the report still builds. (Task 4 test.)
4. An ETF without an expense ratio: excluded from the weighted expense average and noted in the row's detail. (Task 3 test.)
5. FMP request failure during the report: the Metrics movement is replaced by its note; the rest of the report is unchanged. (Task 4 test "price fetch failure degrades to a note".)

---

### Task 1: Pure metrics

**Files:**
- Create: `src-tauri/src/services/report_metrics.rs`
- Modify: `src-tauri/src/services/mod.rs` (WP4 anchor: `pub mod report_metrics; pub mod portfolio_health;`)

**Interfaces:**
- Consumes: `quant_service::{compute_returns, sharpe_ratio, sortino_ratio, max_drawdown, annualized_return, annualized_volatility, value_at_risk, conditional_var, calmar_ratio}`.
- Produces:
  ```rust
  pub const MIN_DAYS: usize = 30;
  pub struct MetricRow { pub label: String, pub value: Option<f64>, pub format: MetricFormat /* Pct | Ratio | Days */ }
  pub fn metric_rows(values: &[f64]) -> Vec<MetricRow>  // CAGR, Volatility (ann.), Sharpe (rf 0%), Sortino (rf 0%), Max drawdown, Calmar, Best month, Worst month, VaR 95% (daily), CVaR 95% (daily); all None when values.len() < MIN_DAYS
  pub struct MonthlyRow { pub year: i32, pub months: [Option<f64>; 12], pub ytd: Option<f64> }
  pub fn monthly_returns(dates: &[NaiveDate], values: &[f64]) -> Vec<MonthlyRow>   // compounded within each calendar month; ytd compounds the year's months present
  pub struct DrawdownPeriod { pub start: NaiveDate, pub trough: NaiveDate, pub end: Option<NaiveDate>, pub depth_pct: f64, pub days: i64 }
  pub fn drawdown_periods(dates: &[NaiveDate], values: &[f64], top: usize) -> Vec<DrawdownPeriod>  // peak-to-recovery episodes, sorted by depth; `end` None while unrecovered
  pub struct MonteCarlo { pub p5: f64, pub p50: f64, pub p95: f64, pub bust_probability: f64, pub goal_probability: f64 }
  pub fn monte_carlo(daily_returns: &[f64], sims: usize, horizon: usize, seed: u64, bust: f64, goal: f64) -> Option<MonteCarlo>  // bootstrap with replacement; None when daily_returns.len() < MIN_DAYS
  ```

- [ ] **Step 1: Write the failing tests**: a 60-day series rising 1%/day → CAGR > 0, max drawdown 0, Sharpe > 0; "short series yields n/a" (29 values → every `value` None, `monte_carlo` None); `monthly_returns` on values spanning Jan 31 → Feb 28 with a 10% rise in Feb → February cell `0.10` (±1e-9); `drawdown_periods` on `[100, 90, 80, 100, 95]` → first period depth `−0.20`, `end` Some(day 3); `monte_carlo` is deterministic (same seed twice → identical struct) and probabilities lie in `[0, 1]`; with all-positive returns `bust_probability == 0.0`.
- [ ] **Step 2–4:** `cargo test --lib report_metrics` fail → implement → pass.
- [ ] **Step 5: Commit** `feat(reports): metrics, monthly grid, drawdown periods and seeded Monte Carlo`

---

### Task 2: Hypothetical series and price fetch

**Files:**
- Modify: `src-tauri/src/services/report_metrics.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct SeriesBuild { pub dates: Vec<NaiveDate>, pub values: Vec<f64>, pub skipped: Vec<String> }
  pub fn hypothetical_series(positions: &[(String /*symbol*/, f64 /*qty*/)], closes: &HashMap<String, Vec<(NaiveDate, f64)>>) -> SeriesBuild   // date axis = union of dates; forward fill; symbols absent from `closes` → skipped
  pub async fn fetch_closes(fmp: &FmpClient, symbols: &[String], from: NaiveDate, to: NaiveDate) -> Result<HashMap<String, Vec<(NaiveDate, f64)>>>   // concurrency 8; a symbol whose fetch fails is simply absent
  ```

- [ ] **Step 1: Write the failing tests**: two symbols with one differing holiday → forward-filled value on the missing day; "unpriced symbol is noted" → `skipped == ["ZZZ"]`.
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(reports): hypothetical holdings series over FMP closes`

---

### Task 3: Health rules

**Files:**
- Create: `src-tauri/src/services/portfolio_health.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)] pub enum AssetClass { Equity, FixedIncome, Commodity, Cash, Other }
  pub struct HealthHolding { pub symbol: String, pub value: f64, pub asset_class: Option<AssetClass>, pub sector: Option<String>, pub expense_ratio: Option<f64> /* fraction, ETFs only */ }
  pub struct HealthInput { pub holdings: Vec<HealthHolding>, pub cash: Option<f64>, pub trade_fees: Option<(f64 /*fees*/, f64 /*traded volume*/)>, pub unresolved_violations: Option<i64> }
  #[derive(Serialize)] pub enum HealthStatus { Pass, Warn, Fail, Na }
  pub struct HealthRow { pub key: &'static str, pub title: &'static str, pub status: HealthStatus, pub value_text: String, pub threshold_text: String, pub detail: Option<String> }
  pub struct Rule { pub key: &'static str, pub title: &'static str, pub source: &'static str }
  pub const RULES: &[Rule]   // equity_share, fixed_income_share, fee_ratio, fund_expenses, buying_power, single_position, sector_concentration, account_rules
  pub const EQUITY_BAND: (f64, f64) = (0.78, 0.82); pub const FIXED_INCOME_BAND: (f64, f64) = (0.18, 0.22); pub const FEE_RATIO_MAX: f64 = 0.01; pub const FUND_EXPENSE_MAX: f64 = 0.005; pub const SINGLE_POSITION_WARN: f64 = 0.10; pub const SINGLE_POSITION_FAIL: f64 = 0.25; pub const SECTOR_MAX: f64 = 0.30;
  pub fn evaluate(input: &HealthInput) -> Vec<HealthRow>   // one row per RULES entry, in order
  ```
  Semantics: shares are of invested value (cash excluded); `equity_share`/`fixed_income_share` → `Na` when more than 20% of invested value is unclassified; `fee_ratio` `Na` without `trade_fees`; `fund_expenses` weights by ETF value, `Na` when no ETF has a ratio, detail names ETFs without one; `buying_power` `Fail` when `cash < 0`, `Na` when `None`; `single_position` `Warn` above 10%, `Fail` above 25%; `sector_concentration` `Warn` above 30%, `Na` when sectors are unknown for > 20% of value; `account_rules` `Fail` when `unresolved_violations > 0`, `Na` when `None`.

- [ ] **Step 1: Write the failing tests**: equity 85% → `Warn` with `value_text` `85.0%`; equity 80% → `Pass`; fees 2% of volume → `Warn`; cash −1 → `Fail`; largest holding 30% → `Fail`; ETF without expense ratio → excluded and named in `detail`; unresolved 2 → `Fail`; every row has a non-empty `threshold_text`.
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(reports): portfolio health rules with Ghostfolio-derived thresholds`

---

### Task 4: Build the movements in the comparison service

**Files:**
- Modify: `src-tauri/src/services/scenario_comparison.rs`
- Modify: the Rust `GoldmanBrief` movement types where `MeasuresMovement` is defined (follow the import at `scenario_comparison.rs:20`)

**Interfaces:**
- Consumes: Tasks 1–3; `FmpClient::{get_company_profile, get_etf_info}` for sector/asset class/expense ratio (ETF with `asset_class` text mapped: contains "equity" → Equity, "fixed"/"bond" → FixedIncome, "commodit" → Commodity, else Other; a stock → Equity); `state.sqlite.get_trades(client_id)` for fees/volume; `count_unresolved_violations(client_id)`.
- Produces (serialized into `movements`):
  ```rust
  pub struct SideMetrics { pub label: String, pub rows: Vec<(String, Option<f64>, MetricFormat)>, pub monthly: Vec<MonthlyRow>, pub drawdowns: Vec<DrawdownPeriod>, pub monte_carlo: Option<MonteCarlo>, pub skipped: Vec<String> }
  pub struct MetricsMovement { pub kind: "metrics", pub numeral, tempo, title_main, title_accent, intro, pub window: String, pub rows: Vec<MeasureRow> /* label, original text, recommended text */, pub monthly: Vec<MonthlyTable /* label, years */>, pub drawdowns: Vec<DrawdownTable>, pub monte_carlo: Vec<MonteCarloLine /* label, p5, p50, p95, bust, goal */>, pub note: String }
  pub struct HealthMovement { pub kind: "health", …MovementBase, pub rows: Vec<HealthCompareRow { key, title, original: (status, value_text), recommended: (status, value_text), threshold: String }>, pub note: String }
  ```
  Movement order becomes: Changes, Measures, **Metrics**, **Health**, Why, Coda (renumber numerals where the app sets them). `ComparisonProse` gains `metrics: SectionProse` and `health: SectionProse`; `prose_schema()` adds them; the user prompt prints the metrics rows, both Monte Carlo lines and the health rows under headings `## Metrics (app-computed)` and `## Health (app-computed)`; the prompt rule list gains: "10. In the metrics and health intros, quote only figures printed in those sections."
  Degradation: `fetch_closes` error or no priced positions → `MetricsMovement` with empty tables and `note` = `"Price history was not available; metrics were skipped."` or `"No holdings to measure."`; the report still returns `Ok`.

- [ ] **Step 1: Write the failing tests**: the schema closure test extended for `metrics`/`health`; `metrics_movement_from(&SideMetrics, &SideMetrics)` renders `CAGR` as `12.3%` and `n/a` for None; "all-cash prints no holdings" (`build_metrics(&[], &[])` → note "No holdings to measure."); "price fetch failure degrades to a note" (a `DailyCloses`-style stub returning `Err` → note text, `Ok` result); health rows carry both sides' statuses.
- [ ] **Step 2–4:** fail → implement → pass; `cargo test --lib` all pass.
- [ ] **Step 5: Commit** `feat(comparison-report): metrics and health movements computed by the app`

---

### Task 5: PDF rendering

**Files:**
- Modify: `src/components/reports/goldman/types.ts` (add `MetricsMovement`, `HealthMovement` to `GoldmanMovement`)
- Modify: `src/components/reports/goldman/GoldmanReport.tsx` (`MetricsView`, `HealthView`)

**Interfaces:**
- `MetricsView`: `SideHeading` columns as in Measures; metrics table (label / original / recommended); for each side a monthly grid (Year + 12 months + YTD, font size 8, `tabular-nums`, header `fixed` on wrapping tables as in `ChangeTable`); drawdown table (Start, Trough, End, Depth, Days); Monte Carlo as three lines per side ("In 1,000 bootstrapped years: median +12.3%, 5th–95th −8.1% to +31.0%; odds of a −20% drawdown 23%; odds of +10% or better 68%"); `note` in muted italic; `window` under the heading.
- `HealthView`: table Title / Original / Recommended / Threshold with glyphs `✓` pass, `!` warn, `✗` fail, `–` n/a using the existing color tokens (`C.*`); legend line; `note`.

- [ ] **Step 1: Write the failing test**: extend `GoldmanReport.render.test.tsx` with fixture `src/components/reports/goldman/__fixtures__/comparison-with-metrics.json` (two sides, 13 metric rows, two years of monthly cells, two drawdowns, Monte Carlo lines, eight health rows) → the PDF renders, page count ≥ the fixture without the new movements + 1, extracted text contains "Monte Carlo" and "Health".
- [ ] **Step 2–4:** fail → implement → pass; `npm run build`, `npm run lint` → 0.
- [ ] **Step 5: Commit** `feat(comparison-report): render metrics and health movements`

---

### Task 6: Gates and sample render

- [ ] **Step 1:** Render a sample PDF with the fixture through the existing env-var path (`GOLDMAN_BRIEF_JSON`, `GOLDMAN_PDF_OUT`) and `pdftoppm -r 70 -png`; check the grid fits the page width and headers repeat on continuation pages.
- [ ] **Step 2: Gates**: `cd src-tauri && cargo test --lib`; `npx vitest run`; `npm run build`; `npm run lint`.
- [ ] **Step 3: Commit** `test(comparison-report): sample fixture with metrics and health`

**Definition of Done (user, after merge):** Clients → Casey → clone "Casey Borst Sch #0001" → **Comparison Report**. The PDF has two new movements after Measures: **Metrics** (both portfolios' trailing-year figures, a monthly grid per side, drawdown periods, Monte Carlo odds, a "prices as of" window) and **Health** (eight rows with ✓ / ! / ✗ / – for original and recommended and the threshold). Claude's intro paragraphs cite only numbers printed there. Running the report twice gives identical Monte Carlo numbers.
