# Ports Program — Design

Date: 2026-11-12. Owner: Jason Borst. Written from the research session of the same day (GitHub survey of stock/portfolio projects, upstream `marketcalls/openalgo-desktop` comparison).

## Intent

Make OpenAlgo Desktop a better app for managing the user's own and clients' US portfolios by bringing in proven open-source pieces instead of building from scratch. Eight work packages (WPs), each built by its own agent on its own branch off `feat/historify-desktop-backend`, then merged in a fixed order.

The app stays what it is: Tauri 2 (React 19 + Rust), AGPL-3.0, US brokers (Alpaca live, Schwab/Tradier/IBKR), client sandbox scenarios, Claude copilot, Goldman-style PDF reports.

## Goals

1. **WP1** Drawing tools and a large indicator library on the Historify candlestick chart, persisted per symbol.
2. **WP2** Real portfolio analytics for clients: time- and money-weighted returns, tax lots (FIFO/LIFO/HIFO/average) with short/long-term gains, allocation targets with drift and a whole-share rebalance plan that feeds the existing Review Recommendations flow.
3. **WP3** "Analyze with Claude" with substance: a staged pipeline (deterministic dossier → four analysts → bull/bear → research manager → risk panel → portfolio manager → app-side validation), emitting the same `ACTIONS_JSON` the app already parses.
4. **WP4** The comparison PDF gains a metrics movement (CAGR, volatility, Sharpe, Sortino, max drawdown, Calmar, monthly-returns grid, drawdown periods, Monte Carlo bust/goal odds) and a health checklist adapted from Ghostfolio's rules.
5. **WP5** American option pricing (binomial, OptionStratLib) beside the existing Black-Scholes greeks.
6. **WP6** 200+ server-side indicators via the official TA-Lib Rust crate, replacing the 6 hand-written ones.
7. **WP7** FinanceDatabase symbol metadata (sector, industry, country, ISIN/CUSIP) loaded into DuckDB, searchable in the app.
8. **WP8** Upstream OpenAlgo Desktop's **Flow** (visual workflow builder, 61 node types, schedule/price-alert/order-update/webhook triggers), carried from `upstream/master` and adapted to this app (US exchanges, Eastern time, this app's broker/API layer).

## Non-goals

- Replacing the app with any surveyed project. None does live US execution + client sandboxes + Claude analysis + PDF reports.
- Upstream's Agent, OpenScript, scalping terminal, openalgo-charts engine, ngrok built-in tunnel, WhatsApp. (Flow's `whatsappAlert` node ships as "not available".)
- Multi-currency, Indian exchanges beyond what Flow's carried validator already accepts.
- Running the Tauri app from a worktree (vite `strictPort 4379` forbids it). Agents prove work with tests and `npm run build`; the user verifies in the UI after each merge.

## Anti-goals (would make this a failure even if it "works")

- Any change to the baseline-scenario protection (`15f3a9b`): chat never applies trades to a baseline.
- Analysis prose that quotes numbers the app did not print (comparison report rule 1 stands).
- A feature with no UI entry point. Every WP's Definition of Done names what the user clicks and sees.
- Silent deviation from the design standards in `CLAUDE.md` (fixed button heights, help icon on every page, `America/New_York` display in 12-hour format, no hardcoded colors).

## Constraints (project-wide)

| Constraint | Value |
|---|---|
| Base branch | `feat/historify-desktop-backend` (master is 85 commits behind it and has nothing extra) |
| Toolchain | rustc 1.93.1, Node 24.15, React 19.2, Vite 7, Vitest 4, Tailwind 4.1, lightweight-charts 5.x, axum 0.7, rusqlite 0.31, duckdb 1.0 |
| Rust MSRV | `Cargo.toml` says `rust-version = "1.77"`; WP5 raises it to **1.89** (optionstratlib); WP6 needs ≥ 1.88 |
| Licenses | App is AGPL-3.0. AGPL code (upstream Flow, Wealthfolio, Ghostfolio ideas) may be used. MIT/BSD/Apache pieces keep their notices under `docs/third-party/` |
| Timezone | All scheduling and display in `America/New_York` via `chrono_tz`; never a fixed offset. Flow's upstream IST assumption is replaced (see WP8) |
| Money | Whole shares only (Alpaca memory). Buy budgets are cumulative and re-validated |
| Secrets | Never logged, never in test fixtures, never in commit messages |
| Data | Never delete/drop/truncate tables. New tables only via numbered migrations |
| Tests | `cd src-tauri && cargo test --lib` (234 pass today), `npx vitest run` (16 files today), `npm run build`, `npm run lint` must all pass before a WP is merge-ready |

## Decisions made in this design (the user may override any of them)

### Branching and merging
- One branch per WP, named `wp1-chart-tools`, `wp2-portfolio-analytics`, `wp3-staged-analyzer`, `wp4-report-metrics`, `wp5-american-options`, `wp6-talib-indicators`, `wp7-symbol-metadata`, `wp8a-flow-core`, `wp8b-flow-ui`, `wp8c-flow-triggers`, `wp8d-flow-parity`.
- Worktrees under `.claude/worktrees/<branch>`.
- **Task 0** (done once on the base branch before any agent starts) adds `// [wp-anchor] …` comment lines at the shared insertion points so parallel additions merge cleanly. See the program plan.
- Merge order: WP1 → WP6 → WP5 → WP7 → WP2 → WP4 → WP3 → WP8a → WP8b → WP8c → WP8d. After each merge: full test suite, then the user's UI check, then the next merge.

### Shared conventions
- Rust: new modules under `src-tauri/src/services/` or a new top-level module; commands in `src-tauri/src/commands/<wp>.rs`; one `pub mod` line per WP under the anchors.
- Frontend: invoke wrappers in `src/api/<feature>.ts` (new file per WP, not in `tauri-client.ts`), components under `src/components/<area>/`, tests colocated `*.test.ts(x)`, `// @vitest-environment jsdom` on component tests.
- Prices: current prices through `portfolio_service::enrich_positions_with_prices`; daily history through `FmpClient::get_daily_closes` (the path `ride_report` already uses), key from `state.sqlite.get_provider_key("fmp", &state.security)`.
- Claude: structured outputs through `AnthropicClient::send_structured` with closed schemas (`additionalProperties: false`, every property required, nullable via `"type": ["number","null"]`), and the `stop_reason` handling in `ClientBriefService::ask_claude_json`.

### UI additions (explicit, for approval)
| WP | Where | What |
|---|---|---|
| 1 | `/historify/charts` | A vertical drawing rail left of the chart (32×32 icon buttons), an **Indicators** button in the chart header opening a searchable picker, a chips row of active indicators (× to remove, gear for parameters). Help text updated. |
| 2 | `/clients/:id` | Two new tabs: **Performance**, **Lots**. |
| 2 | `/clients/:id/scenarios/:scenarioId` | New card **Allocation targets**: targets editor, drift table, **Build rebalance plan** → plan table → **Review plan** (opens the existing ActionReviewModal in scenario mode). |
| 3 | Analyze with Claude dialog | A second button beside Send: **Run full analysis**; while it runs, a stage list (Dossier, Analysts, Bull, Bear, Research manager, Risk panel, Portfolio manager, Checks). |
| 4 | Comparison PDF | Two new movements after Measures: **Metrics** and **Health**. |
| 5 | `/options` | A two-way model switch: **Black-Scholes** / **Binomial (American)**. |
| 6 | Fundamentals → Analysis tab | The indicator dropdown lists the full catalog, grouped. |
| 7 | `/search/token` | A **Download symbol metadata** button with a status line, and a "Company data" results section. |
| 8 | Sidebar → Strategy | **Flow** entry (all regions). Routes `/flow`, `/flow/editor/:id`, `/flow/shortcuts`. Telegram page gains token/chat-id fields, Save, **Send test message**. |

### Thresholds and defaults (constants; user-adjustable before build)
| Key | Default | Source |
|---|---|---|
| Long-term holding | > 365 days | US tax convention |
| Lot method default | FIFO | — |
| TWR flow convention | end-of-day flows: `r_t = (V_t − F_t)/V_{t−1} − 1` | standard |
| XIRR day count | actual/365 | standard |
| Performance periods | 1M, 3M, 6M, YTD, 1Y, SI | — |
| Metrics window | trailing 365 days to the report's price time | — |
| Risk-free rate for Sharpe/Sortino | 0% (label says so) | quantstats default |
| Monte Carlo | 1,000 bootstrap paths, 252-day horizon, seed 20261112, bust = max drawdown ≤ −20%, goal = terminal ≥ +10% | quantstats semantics |
| Health: equity share | 78–82% of invested (pass inside) | Ghostfolio `AssetClassClusterRiskEquity` |
| Health: fixed-income share | 18–22% | Ghostfolio `AssetClassClusterRiskFixedIncome` |
| Health: trade fee ratio | ≤ 1% of traded volume | Ghostfolio `FeeRatioTotalInvestmentVolume` |
| Health: buying power | cash ≥ 0 | Ghostfolio `BuyingPower` |
| Health: fund expenses (weighted ETF expense ratio) | ≤ 0.50% | app default |
| Health: single position | ≤ 10% warn, ≤ 25% fail | app default |
| Health: sector concentration | ≤ 30% | app default |
| Health: account rules | 0 unresolved compliance violations | existing `client_compliance_violations` |
| Binomial steps | 200 | — |
| Flow market time | `America/New_York`; market hours 09:30–16:00 ET from `market_status_service` | CLAUDE.md |
| Flow price-alert poll | every 5 s (as upstream) | upstream |
| Flow order-update poll | every 5 s while a trigger is active | app (no order stream) |
| Flow default product | `CNC` on US exchanges | — |
| FinanceDatabase pin | commit `1ac8809da480` (2026-11-08) | — |

## Per-WP requirements (what the plan must deliver)

Each WP plan carries the detailed tasks. The requirements below are the contract.

**WP1** Drawings persist per `exchange:symbol` in SQLite (`chart_preferences` gains a `drawings` column); indicators render through the library's `IndicatorRenderer` (overlays on the main pane, oscillators in a new pane); the page keeps working when persistence fails (toast, no crash). Libraries: `lightweight-charts-drawing@0.9.0`, `lightweight-charts-indicators@0.20.3`, `oakscriptjs@0.17.1`, `lightweight-charts` bumped to `^5.3.0` (peer requirement ≥ 5.2).

**WP2** Pure Rust engine (`src-tauri/src/analytics/`) with no I/O, fed by a service that reads `client_trades`, `client_holdings`, scenarios and FMP closes. Opening lots are synthesized when trade history does not explain current holdings, flagged "estimated". Targets are stored per client (`client_allocation_targets`), drift computed per scenario. The rebalance plan converts to `OrderRecommendation[]` for `ActionReviewModal`.

**WP3** No change to the chat path or the ACTIONS_JSON contract. The staged run is a separate command that returns a `CopilotResponse` whose `response_text` ends with an ACTIONS_JSON block already validated by the app (whole shares, no oversell, cumulative cash cap, 401(k) flags). Progress via Tauri events. Cost: ~9 Claude calls; the plan documents it.

**WP4** Metrics reuse `quant_service` functions where they exist. New movements are app-computed; Claude writes only `tempo`/`intro` for them. The render test proves the PDF builds with the new movements.

**WP5** `optionstratlib` with `default-features = false, features = ["pricing"]`. Greeks for the binomial model by central finite differences with the same units as `services/greeks.rs`. Existing Black-Scholes results unchanged.

**WP6** TA-Lib results are authoritative; existing indicator tests are updated where TA-Lib's seeding differs (EMA/RSI) with the reason in the commit. The `ta` crate is removed if nothing else uses it.

**WP7** Downloads pinned to the FinanceDatabase commit, loaded transactionally into DuckDB `symbol_metadata` (columns added by migration), attribution file under `docs/third-party/`. OTC (PNK, 14.5 MB) behind a checkbox.

**WP8** Carry, don't rewrite: `git checkout upstream/master -- <paths>` for `src-tauri/src/flow/**`, `src-tauri/src/clock.rs`, `src-tauri/src/security/secret.rs`, `src-tauri/src/agent/indicators/**` (as `indicators_lib`), the goldens under `tests/fixtures/web/flow/**`, and the frontend Flow tree. Adapt at the documented seams only. `/flow/api/*` becomes 22 Tauri commands; the public webhook stays an axum route. Four sequential phases, each merge-ready.

## Open items — decided 2026-11-12

The user delegated these to the planner's recommendation ("go with what your recommendation is"). Decisions, binding for the agents:

1. **Health thresholds:** keep all eight rules with the defaults in the table above. They are informational rows in a PDF; each lives in one constant so an advisor can retune later.
2. **WP3 cost:** accepted. The full analysis is an explicit button, never the default chat path, and shows its stages while it runs.
3. **WP7 OTC:** behind the checkbox, default off. One click adds the 14.5 MB pink-sheet file when a client holds OTC names.
4. **WP8 Telegram settings:** on the existing Telegram page (it is a stub today and already sits in the navigation).
5. **WP1 toolbar:** the seven primary tools (pointer, trend line, horizontal line, ray, rectangle, Fibonacci retracement, text) plus a "More tools" menu with the rest.
6. **Table standard:** option (a) for this program — the new read-only tables follow the existing `src/components/ui/table` pattern. A separate app-wide table work package (implement the selection/action-bar spec once, retrofit every table) is the right way to meet `CLAUDE.md`'s table-interaction spec and is deferred, not dropped.
