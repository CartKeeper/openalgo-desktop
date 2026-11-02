# Briefing Hitchhiker Strategy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the 10-Minute Briefing into a standing strategy: a deterministic Ride Report per position, rotation proposals funded by dormant positions, good-till-cancelled trailing stops bundled with every buy, and a modal that places those groups in order and refuses before the open.

**Architecture:** Rust computes every number (status, trail, scoreboard, entry dates) in a new `ride_report` service and injects a table into the briefing prompt; the model only narrates and emits `ACTIONS_JSON` with `group`/`role`/`validity`. The frontend parser and affordability reconcile learn the new fields (sell proceeds fund same-batch buys), the Review modal groups cards and gates on the market clock, and a new `strategy_basket` service places cancels → sells → buy → trailing stop with fill waits.

**Tech Stack:** Rust (Tauri 2, reqwest, serde, rusqlite, chrono), React + TypeScript (zustand, vitest), Alpaca broker API, Financial Modeling Prep `/stable`, Yahoo chart API.

**Spec:** `docs/superpowers/specs/2026-11-02-briefing-hitchhiker-strategy-design.md`

## Global Constraints

- Whole shares only, long only, cash account. No fractional quantities anywhere.
- Every `TRAILING_STOP` recommendation is `validity: "GTC"`. The parser corrects `DAY` or missing to `GTC`.
- Dormant = 30-trading-day net move within ±3.0% AND price ≥ average cost AND consensus upside ≤ 5.0% (unknown consensus counts as ≤ 5.0%). Trail = 2 × daily swing, 3 × when upside ≥ 15.0%, clamped to [3.0, 15.0], rounded to 0.1; 3.0 when swing unknown.
- Rotation target: upside at least 10 percentage points above the dormant position's, `targets_last_90d` ≥ 3, not up more than 10% intraday.
- Nothing is placed or cancelled without the user approving it in the modal. Gate B unchanged.
- Placement of any batch containing a `cancel`, `fund`, or `buy` leg is refused while the market is closed, with the copy `Market opens at 9:30 AM ET. Place this then.`
- Dates shown to the user: `America/New_York`, 12-hour with AM/PM.
- Never delete rows from existing tables. New tables via `run_migration` only.
- No UI additions beyond: modal group headers, GTC badge, cancel card, market-closed footer line. Caption typography: 12px / 600 / 1.4; card padding 16px; radius 12px; 4px spacing grid.
- Only the price-target bullet is removed from the June 2026 system-prompt block.
- Frontend: `npm run test:run` and `npm run lint` clean. Rust: `cargo test` in `src-tauri` clean.
- Commits: one per task, message ends with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Never push.

## Review Focus

1. **A position bought, half sold, then added to.** `entry_date_from_fills` must pick the start of the current unbroken holding run, not the first-ever buy. Test in Task 3.
2. **Consensus unknown (FMP and Yahoo both empty).** Classification must treat upside as ≤ 5.0 and the prompt row must read `no analyst target available`; trail falls back to 2× swing. Test in Task 3 and Task 5.
3. **Sell proceeds shrink between display and submit.** The submit-time reconcile re-prices the `fund` sell from a fresh quote; the `buy` quantity must drop with it and the user must be forced to re-review. Test in Task 10.
4. **A protect stop for a symbol whose old DAY stop still pledges the shares.** The planner must order `cancel` before `fund`/`protect` for the same symbol, and a failed cancel must stop that group only. Test in Task 8.
5. **Modal opened at 9:25 AM ET, Place clicked at 9:31.** The clock is re-read on click; the button must enable without reopening. Conversely opened at 9:31, clicked after 4:00 PM, must refuse. Test in Task 12.

---

## File Structure

**Rust (create):**
- `src-tauri/src/services/ride_report.rs` — pure rules (status, swing, net move, trail, entry date, trading-day count) + async `build_ride_report` + prompt renderer.
- `src-tauri/src/services/strategy_basket_service.rs` — leg types, `plan_groups`, async `place_strategy_basket` with cancel/fill waits.
- `src-tauri/src/db/sqlite/ride.rs` — accessors for `ride_entries` and `dismissed_protection`.
- `src-tauri/src/commands/ride.rs` — `record_protection_dismissal`.

**Rust (modify):**
- `src-tauri/src/brokers/types.rs` — `Order` gains `trail_percent: Option<f64>`, `hwm: Option<f64>`.
- `src-tauri/src/brokers/mod.rs` — trait gains `get_open_orders` (default: filter `get_order_book`) and `get_fills` (default: `Err`).
- `src-tauri/src/brokers/alpaca/mod.rs` — `AlpacaOrder` trail fields; `get_open_orders` (`status=open&limit=500`); `get_fills` (`/v2/account/activities/FILL` with `page_token` paging).
- Every other `Order { .. }` literal (`brokers/{angel,fyers,ibkr,schwab,tradier,zerodha}/mod.rs`, `db/sqlite/sandbox.rs`, `services/orderbook_service.rs` if any) — add `trail_percent: None, hwm: None`.
- `src-tauri/src/services/orderbook_service.rs` — `get_open_orders(state, api_key) -> OrderbookResult`.
- `src-tauri/src/providers/types.rs` — `PriceTargetConsensus`, `PriceTargetSummary`, `DailyClose`.
- `src-tauri/src/providers/fmp.rs` — repoint `get_price_targets` to `/price-target-news`; add `get_price_target_consensus`, `get_price_target_summary`, `get_daily_closes`.
- `src-tauri/src/providers/yahoo.rs` — `get_target_mean(symbol) -> Option<(f64, i32)>` best-effort.
- `src-tauri/src/providers/anthropic.rs` — two tool definitions.
- `src-tauri/src/services/copilot_service.rs` — tool dispatch + summaries; system prompt bullet swap.
- `src-tauri/src/commands/copilot.rs` — `BriefingResponse`, new prompt, ride report injection.
- `src-tauri/src/commands/orders.rs` — `place_strategy_basket` command.
- `src-tauri/src/db/sqlite/migrations.rs`, `src-tauri/src/db/sqlite/mod.rs` — two tables + wrappers.
- `src-tauri/src/services/mod.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` — module exports and command registration.

**Frontend (modify):**
- `src/types/actionQueue.ts` — `validity`, `group`, `role`, `cancelOrderId`; `StrategyLegResult`.
- `src/lib/parseActions.ts` — parse new fields, GTC correction.
- `src/lib/affordability.ts` — proceeds-inclusive budget, protect-to-bought cap, trailing-without-trail drop, cancel-id check.
- `src/stores/actionQueueStore.ts` — removal cascades, re-reconcile on removal, dismissal recording.
- `src/api/tauri-client.ts` — `orderCommands.placeStrategyBasket`, `rideCommands.recordProtectionDismissal`.
- `src/components/trading/ActionReviewModal.tsx` — group headers, GTC badge, cancel card, validity pass-through, market gate, strategy placement path, per-leg results, proceeds line.
- `src/pages/briefing/BriefingPage.tsx` — `BriefingResponse.ride_report`, `SECTION_META` icons.

**Frontend (create tests):** `src/lib/parseActions.test.ts`, `src/stores/actionQueueStore.test.ts`, `src/components/trading/ActionReviewModal.test.tsx`; extend `src/lib/affordability.test.ts`.

---

### Task 1: Open orders, trail fields, and fill history from the broker

**Files:**
- Modify: `src-tauri/src/brokers/types.rs:68-88` (Order), `src-tauri/src/brokers/mod.rs:58` (trait), `src-tauri/src/brokers/alpaca/mod.rs` (`AlpacaOrder` ~line 270, `get_order_book` mapping ~line 600, `get_activities` ~line 794), every other `Order { .. }` literal, `src-tauri/src/services/orderbook_service.rs:43`
- Test: `src-tauri/src/brokers/alpaca/mod.rs` (`#[cfg(test)] mod order_parse_tests`)

**Interfaces:**
- Produces: `Order.trail_percent: Option<f64>`, `Order.hwm: Option<f64>`; trait `async fn get_open_orders(&self, auth_token: &str) -> Result<Vec<Order>>` (default impl: `get_order_book` filtered to status `OPEN | PARTIALLY_FILLED | PENDING`); trait `async fn get_fills(&self, auth_token: &str, max_pages: u32) -> Result<Vec<AccountActivity>>` (default `Err(AppError::Broker("Fill history is not supported by this broker"))`); `OrderbookService::get_open_orders(state: &AppState, api_key: Option<&str>) -> Result<OrderbookResult>` (sandbox: `get_sandbox_orderbook` filtered the same way).

- [ ] **Step 1: Write the failing parse test**

```rust
#[test]
fn maps_trailing_stop_fields_from_alpaca_order() {
    let body = r#"[{"id":"abc","client_order_id":"c1","symbol":"NVDA","side":"sell","type":"trailing_stop",
      "qty":"4","filled_qty":"0","status":"new","time_in_force":"gtc","created_at":"2026-11-02T14:00:00Z",
      "trail_percent":"9.9","hwm":"210.5"}]"#;
    let orders: Vec<AlpacaOrder> = serde_json::from_str(body).unwrap();
    let mapped = map_alpaca_orders(orders);
    assert_eq!(mapped[0].order_type, "TRAILING_STOP");
    assert_eq!(mapped[0].validity, "GTC");
    assert_eq!(mapped[0].trail_percent, Some(9.9));
    assert_eq!(mapped[0].hwm, Some(210.5));
}
```

- [ ] **Step 2: Run it**: `cd src-tauri && cargo test maps_trailing_stop_fields_from_alpaca_order` → FAIL (no `trail_percent` field / no `map_alpaca_orders`).

- [ ] **Step 3: Implement.** Add `#[serde(default)] trail_percent: Option<String>` and `#[serde(default)] hwm: Option<String>` to `AlpacaOrder`. Extract the `.map(|o| Order { .. })` closure from `get_order_book` into `fn map_alpaca_orders(orders: Vec<AlpacaOrder>) -> Vec<Order>` and use it from both `get_order_book` and the new `get_open_orders` (`GET /v2/orders?status=open&limit=500`). Add the two `Option<f64>` fields to `Order` (`#[serde(default)]`) and `trail_percent: None, hwm: None` to every other constructor until `cargo build` is clean.

- [ ] **Step 4: Implement `get_fills` for Alpaca.** `GET {base}/v2/account/activities/FILL?page_size=100&direction=desc[&page_token=<last id>]`, loop while the page is full and pages < `max_pages`, map with the existing `AccountActivity` mapping. Add `OrderbookService::get_open_orders`.

- [ ] **Step 5: Run**: `cargo test` in `src-tauri` → PASS; `cargo build` → clean.

- [ ] **Step 6: Commit** `feat(brokers): open-orders query, trailing-stop fields, fill history paging`

---

### Task 2: FMP analyst targets and daily closes; Yahoo target mean

**Files:**
- Modify: `src-tauri/src/providers/types.rs`, `src-tauri/src/providers/fmp.rs:166-184`, `src-tauri/src/providers/yahoo.rs`
- Test: `src-tauri/src/providers/fmp.rs` (`#[cfg(test)] mod target_mapping_tests`)

**Interfaces:**
- Produces (types.rs): `PriceTargetConsensus { symbol, target_high: Option<f64>, target_low: Option<f64>, target_consensus: Option<f64>, target_median: Option<f64> }`; `PriceTargetSummary { symbol, last_month_count: Option<i32>, last_quarter_count: Option<i32>, last_quarter_avg_price_target: Option<f64>, last_year_count: Option<i32>, all_time_count: Option<i32> }`; `DailyClose { date: String, close: f64, volume: Option<i64> }`.
- Produces (fmp.rs): `get_price_targets(&self, symbol) -> Result<Vec<PriceTarget>>` now reads `/stable/price-target-news?symbol=&limit=50` (map `newsPublisher` into the existing `news_title`-adjacent fields; keep the `PriceTarget` struct unchanged); `get_price_target_consensus(&self, symbol) -> Result<Option<PriceTargetConsensus>>`; `get_price_target_summary(&self, symbol) -> Result<Option<PriceTargetSummary>>`; `get_daily_closes(&self, symbol, from: &str, to: &str) -> Result<Vec<DailyClose>>` from `/stable/historical-price-eod/light` sorted ascending by date. Expose pure mappers `fn map_consensus(v: &serde_json::Value) -> PriceTargetConsensus`, `fn map_summary(v) -> PriceTargetSummary`, `fn map_daily_close(v) -> Option<DailyClose>`.
- Produces (yahoo.rs): `get_target_mean(&self, symbol) -> Option<(f64, i32)>` calling `https://query2.finance.yahoo.com/v10/finance/quoteSummary/{symbol}?modules=financialData`, returning `None` on any error or missing field (never `Err`).

- [ ] **Step 1: Write failing mapper tests** using the verified payloads:

```rust
#[test]
fn maps_consensus_payload() {
    let v: serde_json::Value = serde_json::from_str(r#"{"symbol":"PFE","targetHigh":33,"targetLow":26,"targetConsensus":29.17,"targetMedian":29}"#).unwrap();
    let c = map_consensus(&v);
    assert_eq!(c.target_consensus, Some(29.17));
    assert_eq!(c.target_high, Some(33.0));
}
#[test]
fn maps_summary_payload() {
    let v: serde_json::Value = serde_json::from_str(r#"{"symbol":"PFE","lastMonthCount":2,"lastMonthAvgPriceTarget":30.5,"lastQuarterCount":4,"lastQuarterAvgPriceTarget":30.5,"lastYearCount":19,"lastYearAvgPriceTarget":27.95,"allTimeCount":64,"allTimeAvgPriceTarget":40.98}"#).unwrap();
    assert_eq!(map_summary(&v).last_quarter_count, Some(4));
}
#[test]
fn maps_daily_close_and_skips_zero_price() {
    let v: serde_json::Value = serde_json::from_str(r#"{"symbol":"PFE","date":"2026-10-30","price":30.52,"volume":45485398}"#).unwrap();
    assert_eq!(map_daily_close(&v).unwrap().close, 30.52);
    let z: serde_json::Value = serde_json::from_str(r#"{"symbol":"PFE","date":"2026-10-31","price":0}"#).unwrap();
    assert!(map_daily_close(&z).is_none());
}
#[test]
fn price_target_news_record_maps_to_price_target() {
    let v: serde_json::Value = serde_json::from_str(r#"{"symbol":"PFE","publishedDate":"2026-10-08T11:17:00.000Z","analystName":"Jane","analystCompany":"Cantor","priceTarget":28,"adjPriceTarget":28,"priceWhenPosted":24.9,"newsTitle":"PT Raised","newsURL":"u","newsPublisher":"StreetInsider","newsBaseURL":"streetinsider.com"}"#).unwrap();
    let t = map_price_target_news(&v, "PFE");
    assert_eq!(t.price_target, Some(28.0));
    assert_eq!(t.analyst_company.as_deref(), Some("Cantor"));
}
```

- [ ] **Step 2: Run** `cargo test target_mapping_tests` → FAIL.
- [ ] **Step 3: Implement** the four mappers and the four client methods via `fetch_fmp_json`. `get_price_targets` uses `map_price_target_news`.
- [ ] **Step 4: Implement** `YahooClient::get_target_mean`; no test (network-only, best-effort).
- [ ] **Step 5: Run** `cargo test` → PASS. **Step 6: Commit** `feat(providers): FMP consensus/summary/daily closes, repoint dead price-target endpoint`

---

### Task 3: Ride rules (pure)

**Files:**
- Create: `src-tauri/src/services/ride_report.rs`; add `pub mod ride_report;` to `src-tauri/src/services/mod.rs`
- Test: same file, `#[cfg(test)] mod rules_tests`

**Interfaces:**
- Produces:
  ```rust
  pub enum RideStatus { TooNew, Falling, Dormant, FlatWithUpside, PullingBack, Climbing }
  pub enum TrailReason { TwoTimesSwing, ThreeTimesSwingHighUpside, UnknownSwingFloor }
  pub fn net_move_pct(closes: &[f64]) -> Option<f64>          // needs ≥ 31 closes; (last − closes[len-31]) / closes[len-31] × 100, rounded 0.1
  pub fn daily_swing_pct(closes: &[f64]) -> Option<f64>       // mean |Δ%| over the last 30 steps, rounded 0.1
  pub fn upside_pct(consensus: f64, price: f64) -> f64         // rounded 0.1
  pub fn classify(age_trading_days: Option<u32>, price: f64, avg_cost: f64, net_move: Option<f64>, upside: Option<f64>) -> RideStatus
  pub fn trail_percent(daily_swing: Option<f64>, upside: Option<f64>) -> (f64, TrailReason)
  pub fn entry_date_from_fills(fills: &[AccountActivity], symbol: &str, held_qty: f64) -> Option<String>  // fills newest-first; walk, sum signed qty (buy +, sell −) until running ≥ held_qty − 1e-9; return that fill's `date`
  pub fn count_trading_days(sessions: &[String], from_date: &str, to_date: &str) -> u32   // sessions = "YYYY-MM-DD" list from the market calendar; count sessions strictly after from_date up to and including to_date
  ```

- [ ] **Step 1: Write failing tests** pinning the spec and the live check:

```rust
#[test] fn classify_table() {
    assert!(matches!(classify(None, 30.0, 25.0, Some(0.0), Some(0.0)), RideStatus::TooNew));
    assert!(matches!(classify(Some(10), 30.0, 25.0, Some(0.0), Some(0.0)), RideStatus::TooNew));
    assert!(matches!(classify(Some(40), 24.0, 25.0, Some(5.0), Some(50.0)), RideStatus::Falling));
    assert!(matches!(classify(Some(40), 30.0, 25.0, Some(1.2), Some(2.0)), RideStatus::Dormant));
    assert!(matches!(classify(Some(40), 30.0, 25.0, Some(-3.0), None), RideStatus::Dormant));   // unknown upside counts as ≤ 5
    assert!(matches!(classify(Some(40), 30.0, 25.0, Some(2.9), Some(5.1)), RideStatus::FlatWithUpside));
    assert!(matches!(classify(Some(40), 204.64, 199.91, Some(-7.9), Some(65.7)), RideStatus::PullingBack));
    assert!(matches!(classify(Some(40), 30.52, 25.71, Some(10.3), Some(-4.4)), RideStatus::Climbing));
    assert!(matches!(classify(Some(40), 106.20, 90.60, Some(3.5), Some(14.3)), RideStatus::Climbing));
}
#[test] fn trail_rule() {
    assert_eq!(trail_percent(Some(1.0), Some(-4.4)).0, 3.0);     // 2×1.0 floored to 3.0
    assert_eq!(trail_percent(Some(1.5), Some(65.7)).0, 4.5);     // 3× swing because upside ≥ 15.0
    assert_eq!(trail_percent(Some(2.1), Some(10.0)).0, 4.2);
    assert_eq!(trail_percent(Some(9.0), Some(50.0)).0, 15.0);    // capped
    assert!(matches!(trail_percent(None, Some(50.0)), (3.0, TrailReason::UnknownSwingFloor)));
}
```
  The rule runs on the already-rounded daily swing (the spec defines swing as rounded to 0.1), so NVDA at 1.5 × 3 = 4.5. The spec's live-check table has been corrected to 4.5.

```rust
#[test] fn entry_date_picks_current_holding_run() {
    // Fills newest first. Held today: 4. The current run is the two buys after the Oct 1 sell,
    // so the entry is the Oct 15 buy, not the Sep 1 buy that started the first run.
    let fills = vec![fill("PFE","buy",2.0,"2026-10-20"), fill("PFE","buy",2.0,"2026-10-15"), fill("PFE","sell",5.0,"2026-10-01"), fill("PFE","buy",5.0,"2026-09-01")];
    assert_eq!(entry_date_from_fills(&fills, "PFE", 4.0).as_deref(), Some("2026-10-15"));
}
#[test] fn entry_date_none_when_fills_do_not_cover_held_qty() {
    let fills = vec![fill("PFE","buy",1.0,"2026-10-20")];
    assert_eq!(entry_date_from_fills(&fills, "PFE", 4.0), None);
}
#[test] fn net_move_and_swing_need_31_closes() {
    assert!(net_move_pct(&[1.0; 30]).is_none());
    let mut c: Vec<f64> = (0..31).map(|i| 100.0 + i as f64).collect();
    assert_eq!(net_move_pct(&c), Some(30.0));
    c.reverse();
    assert_eq!(daily_swing_pct(&c).map(|x| x > 0.0), Some(true));
}
#[test] fn trading_days_counts_sessions_after_entry() {
    let s = ["2026-10-28","2026-10-29","2026-10-30","2026-11-02"].map(String::from);
    assert_eq!(count_trading_days(&s, "2026-10-28", "2026-11-02"), 3);
}
```

- [ ] **Step 2: Run** `cargo test rules_tests` → FAIL. **Step 3: Implement** the functions (a `fill()` test helper builds `AccountActivity`). **Step 4: Run** → PASS. **Step 5: Commit** `feat(ride): deterministic ride status, trail, entry-date rules`

---

### Task 4: SQLite tables for ride entries and dismissed protection

**Files:**
- Modify: `src-tauri/src/db/sqlite/migrations.rs` (add two consts + two `run_migration(conn, "...", ...)?;` calls at the end of `run_migrations`), `src-tauri/src/db/sqlite/mod.rs` (wrappers near line 914)
- Create: `src-tauri/src/db/sqlite/ride.rs`
- Test: `src-tauri/src/db/sqlite/ride.rs` (`#[cfg(test)]` using `Connection::open_in_memory()` + `run_migrations`)

**Interfaces:**
- Tables:
  ```sql
  CREATE TABLE ride_entries (symbol TEXT NOT NULL, quantity REAL NOT NULL, entered_at TEXT NOT NULL, computed_at TEXT NOT NULL DEFAULT (datetime('now')), PRIMARY KEY (symbol, quantity));
  CREATE TABLE dismissed_protection (symbol TEXT NOT NULL, quantity REAL NOT NULL, dismissed_at TEXT NOT NULL DEFAULT (datetime('now')), PRIMARY KEY (symbol, quantity));
  ```
- Produces (ride.rs, all `(conn: &Connection, ...)`): `get_ride_entry(conn, symbol, quantity) -> Result<Option<String>>`, `upsert_ride_entry(conn, symbol, quantity, entered_at) -> Result<()>`, `is_protection_dismissed(conn, symbol, quantity) -> Result<Option<String>>` (returns `dismissed_at`), `record_protection_dismissal(conn, symbol, quantity) -> Result<()>`.
- Produces (mod.rs `impl SqliteDb`): same four names without `conn`.

- [ ] **Step 1: Write failing tests**: `ride_entry_roundtrip_keyed_by_symbol_and_qty` (upsert PFE/11 → get PFE/11 Some, get PFE/12 None) and `dismissal_expires_when_quantity_changes` (record NVDA/4 → NVDA/4 Some, NVDA/5 None).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** migrations, accessors, wrappers. **Step 4: Run** `cargo test ride::` → PASS. **Step 5: Commit** `feat(db): ride_entries and dismissed_protection tables`

---

### Task 5: Ride Report builder and prompt renderer

**Files:**
- Modify: `src-tauri/src/services/ride_report.rs`
- Test: same file, `#[cfg(test)] mod render_tests`

**Interfaces:**
- Consumes: Task 1 `OrderbookService::get_open_orders`, broker `get_fills` via a new `ActivitiesService::get_fills(state, None, 5)`; Task 2 FMP/Yahoo methods via `CopilotService::get_fmp_client`-equivalent (add `pub(crate) fn fmp_client(state) -> Result<FmpClient>` in `ride_report.rs` mirroring copilot's); Task 3 rules; Task 4 accessors; `FundsService::get_portfolio_history(state, None, "1M", "1D")`; `MarketStatusService::get_calendar` for sessions; `PositionService::get_positions` with `portfolio_positions` fallback exactly as `generate_briefing` does today.
- Produces:
  ```rust
  #[derive(Serialize, Clone)] pub struct ExistingStop { pub order_id: String, pub order_type: String, pub quantity: f64, pub validity: String, pub trail_percent: Option<f64>, pub trigger_price: f64 }
  #[derive(Serialize, Clone)] pub struct RidePosition { pub symbol: String, pub exchange: String, pub quantity: f64, pub avg_cost: f64, pub price: f64, pub pnl_pct: f64, pub entered_at: Option<String>, pub age_trading_days: Option<u32>, pub net_move_30d_pct: Option<f64>, pub daily_swing_pct: Option<f64>, pub consensus_target: Option<f64>, pub upside_pct: Option<f64>, pub targets_last_90d: Option<i32>, pub target_source: Option<String>, pub status: RideStatus, pub protected: bool, pub working_sells: Vec<ExistingStop>, pub proposed_trail_pct: f64, pub trail_reason: TrailReason, pub protection_dismissed_at: Option<String> }
  #[derive(Serialize, Clone)] pub struct Scoreboard { pub equity_now: f64, pub equity_1w: Option<f64>, pub equity_1m: Option<f64>, pub as_of: String }
  #[derive(Serialize, Clone)] pub struct RideReport { pub positions: Vec<RidePosition>, pub scoreboard: Option<Scoreboard>, pub paper_mode: bool, pub generated_at: String }
  pub async fn build_ride_report(state: &AppState) -> RideReport          // never Err; per-field best effort
  pub fn render_for_prompt(report: &RideReport) -> String                  // markdown table + legend
  ```
- `protected` = any `working_sells` entry with `order_type == "TRAILING_STOP"` and `quantity == position.quantity`.
- Entry date: `get_ride_entry(symbol, qty)` else compute from fills and `upsert_ride_entry`; manual positions use `added_at`.
- Scoreboard: `equity_now` = last equity point; `equity_1w` = point 5 back; `equity_1m` = first point; `None` when the series is shorter. `paper_mode` true ⇒ `scoreboard: None` and every position `TooNew` with `age_trading_days: None`.

- [ ] **Step 1: Write failing renderer tests** (pure; build `RideReport` literals):
  - `render_marks_unknown_consensus`: a position with `consensus_target: None` renders a row containing `no analyst target available`.
  - `render_shows_dismissal_and_paper_mode`: `protection_dismissed_at: Some("2026-11-01 ...")` renders `protection dismissed by you on`; `paper_mode: true` renders `paper mode: no fill history`.
  - `render_trail_reason_copy`: `ThreeTimesSwingHighUpside` renders `widened because analysts see`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** `build_ride_report` and `render_for_prompt`. Legend must define every column and the six statuses in one line each. **Step 4: Run** `cargo test render_tests` → PASS; `cargo build` clean. **Step 5: Commit** `feat(ride): build Ride Report from broker, FMP and fills; prompt renderer`

---

### Task 6: Copilot tools and system-prompt change

**Files:**
- Modify: `src-tauri/src/providers/anthropic.rs:189` (`build_tool_definitions`), `src-tauri/src/services/copilot_service.rs:44-66` (system prompt), `:239-262` (dispatch), `summarize_tool_result`
- Test: `src-tauri/src/services/copilot_service.rs` (`#[cfg(test)] mod prompt_tests`)

**Interfaces:**
- Produces tools: `get_price_history` `{ symbol: string, range: "1mo"|"3mo"|"6mo"|"1y" }` → `{ symbol, source: "fmp"|"yahoo", bars: [{date, close, volume}], net_move_30d_pct, daily_swing_pct, high_30d, low_30d }`; `get_analyst_targets` `{ symbol: string }` → `{ symbol, source, consensus, median, high, low, targets_last_90d, upside_pct, recent: [{date, analyst, company, target, headline}] (≤5) }`. Both call Task 2 methods and Task 3 rules; `upside_pct` needs the current price from `YahooClient::get_quotes`.
- System prompt: delete the bullet beginning `- Replace price-target language` and insert `- When you cite an analyst target or expected upside, name the source (consensus of N analysts, or the firm) and the as-of date. State targets as what analysts expect, not as a guarantee.` in its place.

- [ ] **Step 1: Write failing test**
```rust
#[test] fn system_prompt_allows_analyst_targets() {
    let p = CopilotService::build_system_prompt();
    assert!(!p.contains("Replace price-target language"));
    assert!(!p.contains("No single price target stated as an expectation"));
    assert!(p.contains("name the source (consensus of N analysts, or the firm)"));
}
```
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** the prompt swap, two `ToolDefinition`s (descriptions must say the data is from FMP with Yahoo fallback and that `upside_pct` is analyst expectation), dispatch arms, and `summarize_tool_result` arms (`"Fetched {n} daily bars for {sym}"`, `"Fetched analyst targets for {sym}: consensus {x}"`). **Step 4: Run** `cargo test prompt_tests` → PASS. **Step 5: Commit** `feat(copilot): price-history and analyst-target tools; allow sourced price targets`

---

### Task 7: Briefing command rewrite

**Files:**
- Modify: `src-tauri/src/commands/copilot.rs:65-300`
- Create: `src-tauri/src/commands/ride.rs` (`record_protection_dismissal(state, symbol: String, quantity: f64) -> Result<(), AppError>`); add `pub mod ride;` to `commands/mod.rs`; register `commands::ride::record_protection_dismissal` in `lib.rs` next to `generate_briefing`.
- Test: `src-tauri/src/commands/copilot.rs` (`#[cfg(test)] mod briefing_prompt_tests`)

**Interfaces:**
- Produces: `#[derive(Serialize)] pub struct BriefingResponse { #[serde(flatten)] pub copilot: CopilotResponse, pub ride_report: RideReport }`; `generate_briefing` returns `Result<BriefingResponse, AppError>`. Extract `pub fn build_briefing_prompt(report_table: &str, cash_line: &str, open_orders_block: &str, symbols: &str, paper_mode: bool) -> String`.
- Prompt content, in this order, each as a `## ` header the model must emit verbatim: `Scoreboard`, `Market Pulse`, `Ride Report`, `Rotation`, `Protection`, `Alerts & Watchlist`, `Risk Radar`, `Deep Dive`, `New Stock Corner`. Section instructions are the spec's "Briefing prompt" list; the Rotation selection rule and the ACTIONS_JSON example are copied from the spec verbatim (`validity`, `group`, `role`, `cancelOrderId`, `trailPercent`). The Open Orders block now prints each order's `order_id` so the model can emit `cancel` legs. Remove the sentences that forbid buys exceeding cash; replace with: buys must fit `available cash + proceeds of fund sells in the same group`. Keep the whole-share rule. In `paper_mode`, tell the model the Scoreboard, Rotation and Protection sections must each be the single line `Live account only.`

- [ ] **Step 1: Write failing tests**
```rust
#[test] fn prompt_has_sections_in_order_and_gtc_example() {
    let p = build_briefing_prompt("TABLE", "cash", "orders", "PFE", false);
    let idx = |s: &str| p.find(s).unwrap();
    assert!(idx("## Scoreboard") < idx("## Market Pulse") && idx("## Market Pulse") < idx("## Ride Report") && idx("## Ride Report") < idx("## Rotation") && idx("## Rotation") < idx("## Protection") && idx("## Protection") < idx("## New Stock Corner"));
    assert!(p.contains(r#""orderType": "TRAILING_STOP""#) && p.contains(r#""validity": "GTC""#) && p.contains(r#""role": "cancel""#));
    assert!(!p.contains("DO NOT include a BUY action for it"));
}
#[test] fn paper_mode_prompt_says_live_only() {
    assert!(build_briefing_prompt("T", "c", "o", "PFE", true).contains("Live account only."));
}
```
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** (build ride report first, then prompt, then `process_message`, wrap into `BriefingResponse`). **Step 4: Run** → PASS; `cargo build`. **Step 5: Commit** `feat(briefing): ride-report-driven prompt, rotation/protection sections, GTC actions`

---

### Task 8: Strategy basket placement (backend)

**Files:**
- Create: `src-tauri/src/services/strategy_basket_service.rs`; `pub mod strategy_basket_service;` in `services/mod.rs`
- Modify: `src-tauri/src/commands/orders.rs` (add command), `src-tauri/src/lib.rs` (register after `place_basket_order`)
- Test: same service file, `#[cfg(test)] mod plan_tests`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Deserialize, Serialize, Clone, PartialEq, Debug)] #[serde(rename_all = "lowercase")] pub enum LegRole { Cancel, Fund, Buy, Protect, Standalone }
  #[derive(Deserialize, Clone, Debug)] pub struct StrategyLeg { pub group: Option<String>, pub role: LegRole, pub symbol: String, #[serde(default)] pub cancel_order_id: Option<String>, #[serde(default)] pub order: Option<OrderRequest> }
  #[derive(Serialize, Clone, Debug)] pub struct LegResult { pub group: Option<String>, pub role: LegRole, pub symbol: String, pub success: bool, pub order_id: Option<String>, pub filled_quantity: f64, pub message: String }
  pub fn plan_groups(legs: Vec<StrategyLeg>) -> Result<Vec<Vec<StrategyLeg>>>   // groups keyed by `group` in first-appearance order; each Standalone/ungrouped leg is its own group; within a group sort stably by role rank Cancel=0,Fund=1,Buy=2,Protect=3; Err(Validation) if a Cancel lacks cancel_order_id or a non-Cancel lacks order
  pub async fn place_strategy_basket(state: &AppState, legs: Vec<StrategyLeg>, expected_mode: Option<bool>) -> Result<Vec<LegResult>>
  ```
- Executor per group, stop-on-failure within the group: Cancel → `OrderService::cancel_order` then poll `OrderbookService::get_order_status` every 2 s until status is `CANCELLED` (or absent from open orders), 30 s cap. Fund/Buy → `OrderService::place_order` then poll until `filled_quantity > 0 && status == "COMPLETE"`, 60 s cap; a Buy that is partially filled at the cap gets `cancel_order` on the remainder and continues with `filled_quantity`. Protect → `place_order` with `quantity = buy.filled_quantity` when a Buy precedes it in the group, else as given. Messages must be user-facing (`"PFE sell filled; BAC buy not placed because PFE sell did not fill within 60 seconds"`).
- Tauri command: `place_strategy_basket(state, legs: Vec<StrategyLeg>, expected_analyze_mode: Option<bool>) -> Result<Vec<LegResult>>`.

- [ ] **Step 1: Write failing planner tests**
```rust
#[test] fn groups_sorted_cancel_fund_buy_protect() { /* input order protect,buy,fund,cancel in group "r1" → roles out: Cancel,Fund,Buy,Protect */ }
#[test] fn standalone_legs_are_their_own_groups() { /* two ungrouped legs → 2 groups */ }
#[test] fn cancel_without_order_id_is_validation_error() { /* assert Err */ }
#[test] fn cancel_for_same_symbol_precedes_fund_even_if_listed_after() { /* fund PFE then cancel PFE → Cancel first */ }
```
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** `plan_groups`, executor, command, registration. **Step 4: Run** `cargo test plan_tests` → PASS; `cargo build`. **Step 5: Commit** `feat(orders): strategy basket placement with cancel/fill sequencing`

---

### Task 9: Frontend types and parser

**Files:**
- Modify: `src/types/actionQueue.ts`, `src/lib/parseActions.ts`
- Create: `src/lib/parseActions.test.ts`

**Interfaces:**
- Produces (`actionQueue.ts`): on `OrderRecommendation`: `validity?: 'DAY' | 'GTC'`, `group?: string`, `role?: 'cancel' | 'fund' | 'buy' | 'protect' | 'standalone'`, `cancelOrderId?: string`. New `export interface StrategyLegResult { group: string | null; role: ...; symbol: string; success: boolean; order_id: string | null; filled_quantity: number; message: string }`.
- Parser: copies the four fields; `orderType === 'TRAILING_STOP'` ⇒ `validity = 'GTC'` regardless of input; `role === 'cancel'` requires `cancelOrderId` else the item is skipped; a `cancel` item passes the `quantity` guard with `quantity: 0` allowed.

- [ ] **Step 1: Write failing tests**
```ts
it('forces GTC on trailing stops and keeps group/role', () => { /* input validity 'DAY' → 'GTC'; group 'rotation-1'; role 'protect' */ })
it('passes DAY through for a market fund sell', () => {})
it('keeps a cancel leg with cancelOrderId and drops one without', () => {})
```
- [ ] **Step 2: Run** `npm run test:run -- parseActions` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS. **Step 5: Commit** `feat(actions): group/role/validity in recommendations; GTC trailing stops`

---

### Task 10: Affordability with sell proceeds

**Files:**
- Modify: `src/lib/affordability.ts`, `src/lib/affordability.test.ts`

**Interfaces:**
- Consumes: Task 9 fields; `orderCommands.getOrderBook()` (mock as `getOrderBook` in the test file's `vi.mock`).
- Behavior: walk in order; a liquidating sell (`side==='SELL'`, `orderType` `MARKET`/`LIMIT`) adds `quantity × unitPrice` to `remaining` (LIMIT price or live quote); stops add nothing; buys consume as today; a `protect` stop whose symbol has a `buy` earlier in the same `group` is capped to that buy's (possibly reduced) quantity instead of held; `TRAILING_STOP` without `trailPercent > 0` is dropped with notice `removed trailing stop for X — no trail distance given`; a `cancel` whose `cancelOrderId` is not in the order book with status `OPEN|PARTIALLY_FILLED|PENDING` is dropped with notice `removed cancel for X — that order is no longer open`. Notice for a funded buy: `BAC ×13 is funded by selling PFE (about $334) plus $351 cash`.

- [ ] **Step 1: Write failing tests** (extend the existing describe style):
```ts
it('funds a buy from a market sell earlier in the batch', async () => { cash(351); pos([{symbol:'PFE',quantity:11}]); getQuote → PFE 30.38, BAC 50.27; items: fund SELL PFE×11 MARKET, buy BAC×13 MARKET group 'r1' → BAC qty 13; notice matches /funded by selling PFE/ })
it('shrinks the dependent buy when the sell proceeds shrink', async () => { same but PFE quote 25.00 → BAC qty floor((351+275)/50.27)=12 })
it('caps a protect stop to the bought quantity in its group', async () => { buy BAC×13 reduced to 12 by cash → protect BAC trailing qty 13 becomes 12 })
it('drops a trailing stop with no trail distance', async () => {})
it('drops a cancel whose order is not open', async () => {})
```
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** `npm run test:run -- affordability` → PASS. **Step 5: Commit** `feat(affordability): sell proceeds fund same-batch buys; protect/cancel checks`

---

### Task 11: Action queue cascades and dismissal recording

**Files:**
- Modify: `src/stores/actionQueueStore.ts`, `src/api/tauri-client.ts` (add `export const rideCommands = { recordProtectionDismissal: (symbol: string, quantity: number) => tauriInvoke<void>('record_protection_dismissal', { symbol, quantity }) }` and `orderCommands.placeStrategyBasket: (legs, expectedAnalyzeMode?) => tauriInvoke<StrategyLegResult[]>('place_strategy_basket', { legs, expectedAnalyzeMode })`)
- Create: `src/stores/actionQueueStore.test.ts`

**Interfaces:**
- `removeItem(id)` becomes async-safe: removing a `buy` also removes `protect` items with the same `group` and `symbol`; removing a `fund` also removes `cancel` items with the same `group` and `symbol`; removing a `protect` calls `rideCommands.recordProtectionDismissal(symbol, quantity)` (fire-and-forget, errors swallowed); after any removal, if any remaining item has a `group`, re-run `reconcileRecommendationsWithAccount` and update `items` + `fundsNotice`.
- New selector helper `export function groupHeader(items: OrderRecommendation[], group: string): string` → `Rotation: sell PFE → buy BAC` when the group has fund+buy, `Sell PFE to cash` when fund only, `Protect NVDA` when protect only.

- [ ] **Step 1: Write failing tests** (mock `@/api/tauri-client` and `@/lib/affordability`): `removing the buy removes its protect but keeps the fund sells`, `removing a fund removes its cancels`, `removing a protect records a dismissal`, `groupHeader copy for the three shapes`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS. **Step 5: Commit** `feat(action-queue): group removal cascades, dismissal recording, group headers`

---

### Task 12: Review modal — groups, GTC, cancel cards, market gate, strategy placement

**Files:**
- Modify: `src/components/trading/ActionReviewModal.tsx`
- Create: `src/components/trading/ActionReviewModal.test.tsx` (render with the zustand store seeded; mock `@/api/tauri-client`, `@/api/trading`, `@/lib/affordability`)

**Behavior (all on the live path only):**
- Items render grouped: for each distinct `group` in first-appearance order, a header row (caption style, 12px/600, uppercase with `letter-spacing: 0.05em`) with `groupHeader(...)`, then that group's cards; ungrouped cards after, as today.
- A `cancel` item renders a card with `Cancel order` badge, the symbol, the rationale, and no Qty/Price/Type inputs.
- The type badge shows `Trailing Stop · GTC` when `validity === 'GTC'`.
- `validity: item.validity ?? 'DAY'` is passed to the broker instead of the hardcoded `'DAY'`.
- Gate B cost line adds `Sell proceeds about $X` when any liquidating sell is present (priced like the reconcile: limit price, else quote via `quoteCommands.getQuote`).
- Market gate: when `isLivePath && items.some(i => ['cancel','fund','buy'].includes(i.role ?? ''))`, read `marketStatusCommands.getMarketClock()` when the modal opens, every 60 s while it stays open, and again at the top of `handlePlaceAll`. If the latest read says `!is_open`, Place is disabled and the footer shows `Market opens at 9:30 AM ET. Place this then.` (caption style, muted). If the read fails, treat the market as closed and append ` (could not read the market clock)`.
- Placement: if any item has a `group`, build `StrategyLeg[]` (`cancel` → `{ group, role, symbol, cancel_order_id }`; others → `{ group, role: item.role ?? 'standalone', symbol, order: <same mapping as today incl. validity> }`) and call `orderCommands.placeStrategyBasket(legs, !freshLive)`; results view lists each leg as `SYMBOL · role` with success/failure and the message. Otherwise keep `placeBasketOrder` as today.

- [ ] **Step 1: Write failing tests**
```ts
it('renders a rotation group header and a cancel card without inputs', () => {})
it('disables Place and shows the market-closed line when the clock says closed', async () => { getMarketClock → { is_open: false } })
it('re-reads the clock on Place and enables when the market has opened', async () => { first call closed, second open → placeStrategyBasket called })
it('does not gate a batch that is only GTC trailing stops', async () => {})
it('passes validity GTC through to the strategy basket legs', async () => {})
```
- [ ] **Step 2: Run** `npm run test:run -- ActionReviewModal` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS; `npm run lint` clean. **Step 5: Commit** `feat(review-modal): grouped strategy cards, GTC badge, cancel cards, market-hours gate`

---

### Task 13: Briefing page wiring

**Files:**
- Modify: `src/pages/briefing/BriefingPage.tsx:42-45` (type), `:290-303` (`SECTION_META`)

- [ ] **Step 1: Add** `ride_report: RideReport` to `BriefingResponse` (declare `RideReport` in `src/api/tauri-client.ts` mirroring the Rust struct) and keep it in component state (`setRideReport`) for the Protection/Ride Report cards' future use; no new rendering beyond the existing markdown sections.
- [ ] **Step 2: Add** `SECTION_META` entries: `'scoreboard'` (`Trophy`), `'ride report'` (`Route`), `'rotation'` (`RefreshCw`), `'protection'` (`Shield`) from `lucide-react`.
- [ ] **Step 3: Run** `npm run build` (tsc) → clean; `npm run test:run` → PASS. **Step 4: Commit** `feat(briefing-page): ride report type and new section icons`

---

### Task 14: Live verification against the Definition of Done

**Files:** none (manual run in the dev app, live account).

- [ ] **Step 1:** Restart the dev app (Sidekick "OpenAlgo Desktop"), open Briefing, click **Generate Briefing**. Confirm sections appear in the spec order and the Ride Report rows match `ride_report` numbers (compare against a `cargo run`-free check: the FMP consensus for PFE should be within a day's drift of 29.17).
- [ ] **Step 2:** Open the Analyst page, enter `PFE`. Confirm price targets now populate (dead endpoint repointed).
- [ ] **Step 3:** With the market open, approve a Protection group for one position. Confirm the Order Book shows a working `TRAILING_STOP`, and the next Generate shows that position as protected with the trail percent.
- [ ] **Step 4:** Before 9:30 AM ET (or after 4:00 PM), open a batch with a `buy` leg. Confirm Place is disabled with the market-closed line; a stops-only batch is not gated.
- [ ] **Step 5:** Record outcomes in the PR description; anything not observed in the UI is reported as unfinished.

---

## Self-review notes

- Spec coverage: Definitions → Tasks 2–5; Architecture §1 → Tasks 1, 2, 5, 6; §2 → Tasks 6, 7; §3 → Task 9; §4 → Task 10; §5 → Task 12; §6 → Task 8; §7 → Tasks 4, 11; Market hours → Task 12; paper mode → Tasks 5, 7; Analyst page fix → Task 2; Definition of done → Task 14.
- One spec correction surfaced while pinning tests: the NVDA example trail is 4.5%, not 4.4%, because the rule runs on the rounded swing (Task 3 note). The spec table was corrected when this plan was written.
- Names used across tasks: `RideStatus`, `TrailReason`, `RideReport`, `RidePosition`, `BriefingResponse`, `StrategyLeg`, `LegRole`, `LegResult`, `StrategyLegResult`, `groupHeader`, `rideCommands.recordProtectionDismissal`, `orderCommands.placeStrategyBasket` — each defined once in the task that produces it.
