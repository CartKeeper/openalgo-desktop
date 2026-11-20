# WP2 — Portfolio Analytics (Returns, Tax Lots, Rebalancing) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clients get time- and money-weighted returns, tax lots with short/long-term gains, and allocation targets with a drift report and a whole-share rebalance plan that flows into Review Recommendations.

**Architecture:** A pure engine (`src-tauri/src/analytics/`: lots, returns, series, allocation; no I/O, fully unit-tested on fixtures) is fed by one service (`portfolio_analytics_service.rs`) that reads `client_trades`, `client_holdings`, scenarios and FMP daily closes. Six `analytics_*` commands expose it. Targets live per client in a new table. The frontend adds two tabs to Client Detail and one card to Scenario Detail; the plan's trade list converts to `OrderRecommendation[]` so the existing `ActionReviewModal` applies it (to a clone when the scenario is a baseline). Wealthfolio's `crates/core/src/portfolio/{performance,allocation_targets}` and `lots` are the reference reading; the implementation here is smaller and ours.

**Tech Stack:** Rust (chrono, serde), rusqlite migration `062_client_allocation_targets`, FMP `get_daily_closes`, React + shadcn Tabs/Card/Table, vitest.

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md` (WP2 requirements; thresholds table rows "Long-term holding" through "Performance periods"; UI additions table).

## Global Constraints

- Branch `wp2-portfolio-analytics`; files below plus WP2 anchors.
- Whole shares only in plans; never sell more than held; never produce a negative cash plan.
- Long term = holding days > 365. FIFO default. TWR with end-of-day flows. XIRR actual/365.
- Prices: current via `portfolio_service::enrich_positions_with_prices`; history via `FmpClient::get_daily_closes(symbol, from, to)` with the key from `state.sqlite.get_provider_key("fmp", &state.security)?`.
- Tables follow the existing `src/components/ui/table` pattern (spec open item 6).
- Numeric cells right-aligned, `tabular-nums`, monospace for numbers; period chips and buttons 32px.

## Review Focus

1. Trade history that does not explain the current holding (Schwab exports cover a window): an estimated opening lot is synthesized and labelled; an unexplainable sell is dropped with a warning. (Task 1 tests "opening lot is synthesized" and "oversell is warned and skipped".)
2. Same-day buy and sell of one symbol: ordering by `(trade_date, id)` keeps the buy first. (Task 1 test "same-day buy then sell matches the buy".)
3. A symbol with no closes (delisted, bad ticker): the series skips it and the report's `warnings` names it; nothing panics. (Task 3 test "missing closes are reported".)
4. Position with price 0 or missing at plan time: that line becomes `unresolved`, never a division by zero. (Task 4 test "unpriced holding is unresolved".)
5. Targets that sum above 100%: saving is refused with the sum in the message. (Task 5 test "targets over 100% are rejected".)

---

### Task 1: Lots engine

**Files:**
- Create: `src-tauri/src/analytics/mod.rs` (`pub mod lots; pub mod returns; pub mod series; pub mod allocation;`), `src-tauri/src/analytics/lots.rs`
- Modify: `src-tauri/src/lib.rs` (under `// [wp-anchor] WP2 analytics`: `pub mod analytics;`)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)] pub enum LotMethod { Fifo, Lifo, Hifo, Average }
  impl LotMethod { pub fn parse(s: &str) -> Option<Self> /* "fifo"|"lifo"|"hifo"|"average", case-insensitive */ }
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)] pub enum TradeSide { Buy, Sell }
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)] pub enum Term { Short, Long }
  pub struct TradeEvent { pub date: NaiveDate, pub seq: i64, pub symbol: String, pub side: TradeSide, pub quantity: f64, pub price: f64, pub fees: f64 }
  pub struct OpeningPosition { pub symbol: String, pub quantity: f64, pub avg_cost: f64 }
  pub struct OpenLot { pub symbol: String, pub opened: NaiveDate, pub quantity: f64, pub cost_per_share: f64, pub cost_basis: f64, pub estimated: bool, pub market_value: Option<f64>, pub unrealized_pnl: Option<f64>, pub term: Term }
  pub struct Disposal { pub symbol: String, pub opened: NaiveDate, pub closed: NaiveDate, pub quantity: f64, pub proceeds: f64, pub cost_basis: f64, pub realized_pnl: f64, pub holding_days: i64, pub term: Term, pub estimated: bool }
  pub struct LotTotals { pub cost_basis: f64, pub market_value: f64, pub unrealized_short: f64, pub unrealized_long: f64, pub realized_short_ytd: f64, pub realized_long_ytd: f64 }
  pub struct LotReport { pub method: LotMethod, pub as_of: NaiveDate, pub open_lots: Vec<OpenLot>, pub disposals: Vec<Disposal>, pub totals: LotTotals, pub warnings: Vec<String> }
  pub fn build_lots(trades: &[TradeEvent], openings: &[OpeningPosition], method: LotMethod, as_of: NaiveDate, prices: &HashMap<String, f64>) -> LotReport
  ```
- Rules: sort by `(date, seq)`. Buy fees add to cost; sell fees reduce proceeds. For each symbol, `opening_qty = opening.quantity − (Σbuys − Σsells)`; if > 0 create an opening lot dated `min(trade date) − 1 day` (or `as_of` with no trades) at `avg_cost`, `estimated: true`; if < 0 push a warning `"{SYMBOL}: trades sell {n} more shares than the history explains; those sells were ignored"` and drop the excess from the latest sells. `Term::Long` when `holding_days > 365` (open lots: `as_of − opened`). `Average`: one pooled lot per symbol, cost = running average. `realized_*_ytd` sum disposals closed in `as_of.year()`.

- [ ] **Step 1: Write the failing tests** (fixture: XYZ buy 10 @ 100 fee 1 on 2026-01-05; buy 10 @ 120 fee 1 on 2026-02-01; sell 5 @ 130 fee 1 on 2026-03-01; price 125; `as_of` 2026-11-12):
  - FIFO: one disposal, `proceeds 649.0`, `cost_basis 500.5`, `realized_pnl 148.5`, `holding_days 55`, `Term::Short`; open lots `[5 @ 100.1, 10 @ 120.1]`; `totals.cost_basis 1701.5`, `market_value 1875.0`, `unrealized_short 173.5`.
  - LIFO and HIFO: disposal `cost_basis 600.5`, `realized_pnl 48.5`, `holding_days 28`.
  - Average: disposal `cost_basis 550.5`, `realized_pnl 98.5`; one open lot `15 @ 110.1`.
  - "opening lot is synthesized": opening XYZ 20 @ 90 with the same trades → an extra lot `5 @ 90`, `estimated: true`, dated 2026-01-04.
  - "oversell is warned and skipped": only a sell of 5 with no buys and no opening → `disposals` empty, one warning containing `XYZ`.
  - "same-day buy then sell matches the buy": buy 5 @ 10 (seq 1) and sell 5 @ 12 (seq 2) on one date → realized 10.0 with zero fees.
  - "term at 366 days is long": opened 2025-11-11, as_of 2026-11-12 → `Long`; opened 2025-11-12 → `Short`.
- [ ] **Step 2: Run** `cd src-tauri && cargo test --lib analytics::lots` → FAIL (module missing).
- [ ] **Step 3: Implement** `build_lots` with the rules above (approach: per-symbol `VecDeque<OpenLot>`; pop front for FIFO, back for LIFO, max-cost for HIFO; pooled for Average).
- [ ] **Step 4: Run** → PASS. Floats compared with `(a − b).abs() < 1e-9`.
- [ ] **Step 5: Commit** `feat(analytics): tax-lot engine with FIFO/LIFO/HIFO/average and term classification`

---

### Task 2: Returns engine (TWR, annualization, XIRR, Modified Dietz)

**Files:**
- Create: `src-tauri/src/analytics/returns.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct ValuationPoint { pub date: NaiveDate, pub value: f64, pub flow: f64 }  // flow > 0: money into securities that day (buys incl. fees); flow < 0: out (sell proceeds net)
  pub fn time_weighted_return(points: &[ValuationPoint]) -> Option<f64>   // Π(1 + r_t) − 1 with r_t = (V_t − F_t)/V_{t−1} − 1; when V_{t−1} == 0: r_t = V_t/F_t − 1 if F_t > 0 else the day is skipped; None if no computable day
  pub fn annualize(total_return: f64, days: i64) -> Option<f64>           // (1+R)^(365/days) − 1; None when days < 365
  pub struct CashFlow { pub date: NaiveDate, pub amount: f64 }           // investor view: buys negative, sells/dividends positive, terminal value positive
  pub fn xirr(flows: &[CashFlow]) -> Option<f64>                          // Newton from 0.1 (≤100 iters, tol 1e-7), fallback bisection on [−0.99, 10]; None without a sign change; actual/365
  pub fn modified_dietz(start_value: f64, end_value: f64, flows: &[CashFlow], start: NaiveDate, end: NaiveDate) -> Option<f64>
  ```

- [ ] **Step 1: Write the failing tests**: TWR on `[(d0, 1000, 0), (d1, 1100, 0), (d2, 1650, 500)]` → `0.15` (±1e-9); TWR starting from zero with a first-day flow `[(d0, 0, 0), (d1, 500, 500), (d2, 550, 0)]` → `0.10`; `annualize(0.21, 730)` → `0.10` (±1e-6); `annualize(0.1, 100)` → `None`; XIRR `[−1000 @ 2025-01-01, +1100 @ 2026-01-01]` → `0.10` (±1e-6); XIRR with all-negative flows → `None`; Modified Dietz with no flows equals simple return.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(analytics): time-weighted, money-weighted and Dietz returns`

---

### Task 3: Valuation series from trades and daily closes

**Files:**
- Create: `src-tauri/src/analytics/series.rs`

**Interfaces:**
- Produces:
  ```rust
  #[async_trait] pub trait DailyCloses: Send + Sync { async fn closes(&self, symbol: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, f64)>>; }
  pub struct FmpCloses<'a> { pub client: &'a FmpClient }   // impl via get_daily_closes; dates parsed from DailyClose.date (YYYY-MM-DD)
  pub struct SeriesResult { pub points: Vec<ValuationPoint>, pub warnings: Vec<String> }
  pub async fn holdings_value_series(trades: &[TradeEvent], openings: &[OpeningPosition], from: NaiveDate, to: NaiveDate, closes: &dyn DailyCloses) -> Result<SeriesResult>
  ```
  Rules: the date axis is the union of close dates across symbols in `[from, to]`; quantity per symbol per date from openings + trades up to and including that date; close forward-filled from the last known; a symbol with no closes at all is skipped with warning `"{SYMBOL}: no price history"`; `flow` on a date = Σ buy cost (incl. fees) − Σ sell proceeds (net of fees) that date.

- [ ] **Step 1: Write the failing tests** with a `FakeCloses` (`HashMap<&str, Vec<(NaiveDate, f64)>>`): two symbols, one gap day forward-filled; a buy mid-window produces the flow on that date and the quantity from that date; "missing closes are reported".
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(analytics): holdings valuation series over daily closes`

---

### Task 4: Allocation targets, drift and rebalance plan

**Files:**
- Create: `src-tauri/src/analytics/allocation.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)] pub enum TargetScope { Symbol, Sector }
  pub struct AllocationTarget { pub id: Option<i64>, pub client_id: i64, pub scope: TargetScope, pub key: String, pub target_pct: f64, pub band_pct: f64 }
  pub struct Holding { pub symbol: String, pub exchange: String, pub quantity: f64, pub price: Option<f64>, pub sector: Option<String> }
  #[derive(Serialize)] pub enum DriftStatus { InBand, Over, Under }
  pub struct DriftRow { pub scope: TargetScope, pub key: String, pub target_pct: f64, pub actual_pct: f64, pub drift_pct: f64, pub actual_value: f64, pub status: DriftStatus }
  pub struct DriftReport { pub total_value: f64, pub cash: f64, pub rows: Vec<DriftRow>, pub untargeted_pct: f64 }
  pub fn drift(holdings: &[Holding], cash: f64, targets: &[AllocationTarget]) -> DriftReport
  pub struct PlanLine { pub symbol: String, pub exchange: String, pub side: TradeSide, pub quantity: f64, pub price: f64, pub value: f64, pub before_pct: f64, pub after_pct: f64, pub reason: String }
  pub struct RebalancePlan { pub lines: Vec<PlanLine>, pub cash_before: f64, pub cash_after: f64, pub unresolved: Vec<String>, pub note: String }
  pub fn rebalance(holdings: &[Holding], cash: f64, targets: &[AllocationTarget]) -> RebalancePlan
  pub fn validate_targets(targets: &[AllocationTarget]) -> Result<(), String>   // per scope: sum ≤ 100 (+1e-9), each 0..=100, band ≥ 0, keys unique
  ```
  Algorithm (pinned): `total = Σ value + cash`. Symbol gap = `target% × total − value`. Sector gap is spread over that sector's holdings in proportion to their value; a sector target with no holding → `unresolved` `"{sector}: no holding to adjust"`. Sells first (gap < 0): `qty = floor(|gap| / price)`; then buys by descending gap: `qty = floor(min(gap, cash_available) / price)`, `cash_available = cash + Σ sell values − Σ earlier buy values`. Lines with `qty == 0` are omitted. A holding with `price` `None` or `≤ 0` → `unresolved` `"{symbol}: no price"`. `reason` = `"Rebalance {key} from {before:.1}% to {target:.1}%"`. `note` = `"Whole shares; prices as of the plan time; cash never goes below zero."`

- [ ] **Step 1: Write the failing tests**
  - Drift: A 100 @ 50, B 50 @ 100, cash 1000, targets A 40 ±2, B 40 ±2 → `total 11000`, A `actual 45.4545`, `Over`; `untargeted_pct 9.0909`.
  - Rebalance (same): lines `A SELL 12 @ 50`, `B SELL 6 @ 100`, no buys, `cash_after 2200`.
  - Sector: A (Tech) and B (Tech) as above, targets Tech 60, Health 40 → `A SELL 34`, `B SELL 17`, `cash_after 4400`, `unresolved` contains `Health`.
  - Buy with cash cap: holdings A 10 @ 100, cash 250, target A 100% → `A BUY 2`, `cash_after 50`.
  - "unpriced holding is unresolved": price `None` → unresolved contains the symbol, no line.
  - `validate_targets`: 60 + 50 → `Err` containing `110`.
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(analytics): allocation drift and whole-share rebalance plan`

---

### Task 5: Targets table and DB functions

**Files:**
- Modify: `src-tauri/src/db/sqlite/migrations.rs` (WP2 anchors): migration `062_client_allocation_targets`:
  ```sql
  CREATE TABLE IF NOT EXISTS client_allocation_targets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('symbol','sector')),
    key TEXT NOT NULL,
    target_pct REAL NOT NULL,
    band_pct REAL NOT NULL DEFAULT 2.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (client_id, scope, key)
  );
  CREATE INDEX IF NOT EXISTS idx_allocation_targets_client ON client_allocation_targets(client_id);
  ```
- Create: `src-tauri/src/db/sqlite/allocation_targets.rs`; Modify: `src-tauri/src/db/sqlite/mod.rs` (WP2 anchor)

**Interfaces:**
- Produces (`impl SqliteDb`): `list_allocation_targets(&self, client_id: i64) -> Result<Vec<AllocationTarget>>`; `replace_allocation_targets(&self, client_id: i64, targets: &[AllocationTarget]) -> Result<Vec<AllocationTarget>>` (one transaction: delete the client's rows, insert the new ones, return them with ids; calls `validate_targets` first and maps `Err(msg)` to `AppError::Validation(msg)`).

- [ ] **Step 1: Write the failing tests**: replace twice → second set is what `list` returns; "targets over 100% are rejected" → `Err(AppError::Validation(..))` and the previous rows untouched.
- [ ] **Step 2–4:** fail → implement → pass (`cargo test --lib allocation_targets`).
- [ ] **Step 5: Commit** `feat(analytics): client allocation targets table`

---

### Task 6: Service and commands

**Files:**
- Create: `src-tauri/src/services/portfolio_analytics_service.rs`, `src-tauri/src/commands/analytics.rs`
- Modify: `src-tauri/src/services/mod.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (WP2 anchors)

**Interfaces:**
- Consumes: Tasks 1–5; `state.sqlite.get_trades(client_id)` (`ClientTrade` in `src-tauri/src/providers/types.rs:559`; only `trade_type` `buy`/`sell` become `TradeEvent`s, `seq = id`), `get_client_holdings(client_id)` (→ `OpeningPosition` from `quantity`, `avg_cost`), `get_client_scenario_by_id`, `get_scenario_positions`, `portfolio_service::enrich_positions_with_prices`, `FmpClient::get_company_profile(symbol).sector` (ETFs: `get_etf_info` non-empty → sector `"ETF"`), cached per call.
- Produces:
  ```rust
  pub struct PerformancePeriod { pub key: &'static str, pub start: NaiveDate, pub end: NaiveDate, pub start_value: f64, pub end_value: f64, pub net_flows: f64, pub twr: Option<f64>, pub twr_annualized: Option<f64>, pub mwr: Option<f64>, pub note: Option<String> }
  pub struct PerformanceReport { pub as_of: NaiveDate, pub prices_as_of: String /* America/New_York, 12-hour */, pub periods: Vec<PerformancePeriod>, pub warnings: Vec<String> }
  pub struct PortfolioAnalyticsService;
  impl PortfolioAnalyticsService {
    pub async fn client_lots(state: &AppState, client_id: i64, method: LotMethod) -> Result<LotReport>
    pub async fn client_performance(state: &AppState, client_id: i64) -> Result<PerformanceReport>   // periods 1M, 3M, 6M, YTD, 1Y, SI; one series fetch for SI, periods are slices
    pub async fn scenario_drift(state: &AppState, scenario_id: i64) -> Result<DriftReport>
    pub async fn rebalance_plan(state: &AppState, scenario_id: i64) -> Result<RebalancePlan>
  }
  ```
- Commands (all `Result<_, AppError>`): `analytics_client_lots(client_id: i64, method: String)`, `analytics_client_performance(client_id: i64)`, `analytics_list_targets(client_id: i64)`, `analytics_save_targets(client_id: i64, targets: Vec<AllocationTarget>)`, `analytics_scenario_drift(scenario_id: i64)`, `analytics_rebalance_plan(scenario_id: i64)`. Unknown `method` → `AppError::Validation("Unknown lot method: …")`.

- [ ] **Step 1: Write the failing unit tests** for the pure mapping helpers in the service (`trade_events_from(&[ClientTrade]) -> Vec<TradeEvent>` ignores `dividend`/`interest` rows and lowercases `trade_type`; `period_bounds(as_of) -> [(key, start)]` gives `YTD` start Jan 1 and `1M` start `as_of − 1 month`).
- [ ] **Step 2–4:** fail → implement service + commands (register under the WP2 handler anchor) → pass; `cargo test --lib` all pass.
- [ ] **Step 5: Commit** `feat(analytics): portfolio analytics service and analytics_* commands`

---

### Task 7: Frontend API and plan → actions conversion

**Files:**
- Create: `src/api/analytics.ts`, `src/lib/rebalancePlanToActions.ts`
- Test: `src/lib/rebalancePlanToActions.test.ts`

**Interfaces:**
- Produces: TS types mirroring Task 1–6 structs; `analyticsApi = { clientLots(clientId, method), clientPerformance(clientId), listTargets(clientId), saveTargets(clientId, targets), scenarioDrift(scenarioId), rebalancePlan(scenarioId) }` over `invoke`; `rebalancePlanToActions(plan: RebalancePlan): OrderRecommendation[]` with `side` `'BUY'|'SELL'`, `orderType 'MARKET'`, `product 'CNC'`, `price 0`, `rationale = line.reason`, `exchange = line.exchange`, and every other required field of `OrderRecommendation` (`src/types/actionQueue.ts`) filled the way `src/lib/parseActions.ts` fills it.

- [ ] **Step 1: Write the failing test**: a two-line plan → two recommendations with the fields above; an empty plan → `[]`.
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(clients): analytics API and rebalance plan to review actions`

---

### Task 8: Client Detail — Performance and Lots tabs

**Files:**
- Create: `src/components/clients/PerformanceCard.tsx`, `src/components/clients/LotsTable.tsx`
- Modify: `src/pages/clients/ClientDetail.tsx` (two `TabsTrigger`/`TabsContent` pairs: `performance`, `lots`)
- Test: `src/components/clients/LotsTable.test.tsx` (jsdom)

**Interfaces:**
- `PerformanceCard({ clientId })`: period chips (32px, one selected) and a two-column table: Time-weighted return, Annualized, Money-weighted (XIRR), Start value, End value, Net flows; `note` under the table; `prices_as_of` caption; loading skeleton, empty state ("No trades yet"), error with Retry.
- `LotsTable({ clientId })`: method `Select` (FIFO, LIFO, HIFO, Average); summary row (cost basis, market value, unrealized short/long, realized YTD short/long); table Open lots (Symbol, Opened, Qty, Cost/sh, Cost, Value, Unrealized, Term, "est." badge when `estimated`); table Realized (Symbol, Opened, Closed, Qty, Proceeds, Cost, P/L, Days, Term); `warnings` listed above the tables.

- [ ] **Step 1: Write the failing test**: with mocked `analyticsApi.clientLots` resolving the Task 1 FIFO fixture, the table shows `148.50` in the realized P/L cell and an `est.` badge when a lot is estimated; changing the method re-calls the API with `lifo`.
- [ ] **Step 2–4:** fail → implement → pass; `npm run build` → 0.
- [ ] **Step 5: Commit** `feat(clients): performance and lots tabs on client detail`

---

### Task 9: Scenario Detail — Allocation targets card

**Files:**
- Create: `src/components/clients/AllocationCard.tsx`
- Modify: `src/pages/clients/ScenarioDetail.tsx` (render the card below the positions card)
- Test: `src/components/clients/AllocationCard.test.tsx` (jsdom)

**Interfaces:**
- Consumes: `analyticsApi`, `rebalancePlanToActions`, `ActionReviewModal` (scenario mode; it already labels 'Create Clone & Apply' on a baseline and 'Apply to Scenario' otherwise).
- Card sections: (1) Targets editor: rows of `scope` Select (Symbol | Sector), `key` Input (symbol: datalist of the scenario's symbols; sector: datalist of the sectors seen in the drift report), `target %`, `band ±%` (default 2), remove; "Add target"; "Save" (disabled until changed; shows the server's validation message on error). (2) Drift table: Key, Target, Actual, Drift, Status badge (In band / Over / Under), with the untargeted share as the last line. (3) "Build rebalance plan" (secondary, 40px) → plan table (Symbol, Side, Qty, Price, Value, Before → After) + "Cash before / after" + unresolved list + `note`; "Review plan" (primary) opens `ActionReviewModal` with `rebalancePlanToActions(plan)`.

- [ ] **Step 1: Write the failing test**: with mocked APIs, saving a target calls `saveTargets` with the typed row; "Build rebalance plan" renders the lines and "Review plan" opens the modal with two actions.
- [ ] **Step 2–4:** fail → implement → pass; `npm run build`, `npm run lint` → 0.
- [ ] **Step 5: Commit** `feat(clients): allocation targets, drift and rebalance plan on scenario detail`

---

### Task 10: Help text and gates

- [ ] **Step 1:** Extend the `HelpButton` content on Client Detail (Performance, Lots: what TWR/MWR mean, "est." lots, long = over 365 days) and Scenario Detail (targets, bands, plan is whole shares, Review plan applies to a clone when this is the baseline).
- [ ] **Step 2: Gates**: `cd src-tauri && cargo test --lib`; `npx vitest run`; `npm run build`; `npm run lint`.
- [ ] **Step 3: Commit** `docs(clients): help text for analytics`

**Definition of Done (user, after merge):** Clients → Casey → **Performance** tab shows 1M…SI with TWR/MWR and a "prices as of" time in ET; **Lots** tab shows open lots (opening lots marked "est." where the Schwab history starts after the buy), realized rows with Short/Long, and switching to LIFO changes the numbers. Open the baseline scenario → **Allocation targets**: add `VOO 40 ±2`, Save, see the drift row; Build rebalance plan → lines in whole shares and cash after ≥ 0; Review plan → the existing review modal with "Create Clone & Apply".
