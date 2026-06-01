# Backtesting Engine — Design Spec

**Date:** 2026-06-01
**Status:** Approved (design); pending implementation plan
**Scope:** Single-symbol, built-in-strategy backtester for US/Alpaca equities, with brokerage + capital-gains tax modeling and a results UI.

---

## Goal

Let the user run a deterministic historical backtest of a built-in strategy on a single symbol over a date range, with realistic US/Alpaca costs and capital-gains tax drag, and see equity curve, drawdown, metrics, and a trade list — compared against buy & hold.

## Non-Goals (v1) — documented future work

- **Rule builder** (indicator entry/exit condition composer). The engine is designed with a pluggable signal boundary so this can be added later with no engine rework.
- **Multi-symbol portfolio** (shared capital pool, concurrent positions, rebalancing).
- **Intraday microstructure** fill modeling (bid/ask ladder, partial fills). Daily (or whatever interval the history holds) is supported, but fills use bar open, not tick data.
- **Short selling** (margin/borrow modeling). v1 is long-only.
- **Indian-broker cost stack** (STT/GST/stamp/etc.). v1 models US/Alpaca only.

---

## Architecture

Three layers, following existing project patterns (Rust service → Tauri command → React page):

1. **`src-tauri/src/services/backtest_service.rs`** — pure, deterministic simulation core. No I/O inside the loop; candles are fetched once up front via `HistoryService`.
2. **Tauri command `run_backtest`** in `src-tauri/src/commands/backtest.rs` — validates config, loads candles, runs the core, returns the full result. Registered in `commands/mod.rs` + `lib.rs`.
3. **`src/pages/Backtest.tsx`** — config form + results visualization. Frontend API binding in `src/api/` and tauri-client.

### The pluggable signal boundary

```rust
/// Per-bar trading signal.
enum Signal { Buy, Sell, Hold }

/// Produces a signal for bar `i` given all bars up to and including `i`.
/// Implementors MUST NOT read bars[j] for j > i (no look-ahead).
trait SignalGenerator {
    fn signal(&self, bars: &[Candle], i: usize) -> Signal;
}
```

Built-in implementors (each uses `indicators_service`):
- `SmaCrossover { fast, slow }` — Buy when fast SMA crosses above slow, Sell on cross below.
- `EmaCrossover { fast, slow }` — same with EMA.
- `RsiThreshold { period, oversold, overbought }` — Buy when RSI crosses up through `oversold`, Sell when crossing down through `overbought`.
- `MacdCross { fast, slow, signal }` — Buy on MACD line crossing above signal line, Sell on cross below.
- `BollingerReversion { period, std_dev }` — Buy when close crosses below lower band, Sell when close crosses above upper band.

The rule builder (future) becomes just another `SignalGenerator` — the engine, costs, taxes, metrics, and UI are unaffected.

---

## Simulation Loop

Inputs: ordered `Vec<Candle>` (ascending time), a `SignalGenerator`, and a `BacktestConfig`.

For each bar `i` from `warmup` to `n-1`:
1. Compute `signal(bars, i)` from the **close** of bar `i`.
2. If a Buy/Sell is generated, the order **fills at the open of bar `i+1`** (the last bar generates no actionable order). This is the standard look-ahead-free convention.
3. Apply **slippage** to the fill price (buys fill higher, sells fill lower) and **fees** (see Costs).
4. Update cash, position, and realized P&L; record the trade leg.
5. Mark-to-market **equity** = cash + position × close, recorded every bar for the equity curve and drawdown series.

Position model: **long-only**. A Buy with no position opens one; a Sell with a position closes it (flat). A Buy while already long, or a Sell while flat, is a no-op (Hold). Position size = configurable (see Sizing). Whole shares by default (matches the live whole-share constraint); a `fractional` flag allows fractional shares for the backtest.

### Position sizing

`BacktestConfig.sizing`:
- `AllIn` — use all available cash (default).
- `FixedFraction(pct)` — use `pct`% of current equity per entry.
- `FixedShares(n)` — fixed share count per entry.

Whole-share rounding (floor) unless `fractional = true`.

---

## Cost & Tax Model (US / Alpaca)

`BacktestConfig.costs`:
- `commission_per_trade` — default **$0** (Alpaca commission-free).
- `slippage_bps` — default **5 bps**, applied to every fill price.
- `reg_fees_enabled` — when true, apply on **sells**: SEC fee (rate × notional) + FINRA TAF (per-share, capped). Rates configurable with current defaults; documented as approximate and user-overridable.

`BacktestConfig.tax`:
- Capital-gains drag on **realized** gains, computed per closed round-trip trade.
- Holding period determines rate: **short-term** (held < 365 days) at `st_rate` (default 35%), **long-term** (≥ 365 days) at `lt_rate` (default 15%).
- Losses offset gains within the run (net realized gain taxed; net loss = $0 tax, no carryforward modeling in v1).
- Reported as **gross** (pre-tax) and **net-of-tax** equity/return, so the tax drag is explicit and never silently baked into a single number.

---

## Metrics

Computed from the equity curve and trade list, reusing `quant_service`:
- Total return, CAGR (annualized return), volatility (annualized).
- Sharpe, Sortino, Calmar.
- Max drawdown (value + peak/trough dates), drawdown series.
- Trade stats: number of trades, win rate, average win, average loss, profit factor, max consecutive losses, average holding period.
- Time-in-market (% of bars holding a position).
- **Buy & Hold benchmark**: same symbol, full capital deployed at first bar open, held to last bar — its return, CAGR, and max drawdown for side-by-side comparison.

All metrics computed on both gross and net-of-tax equity where tax applies.

---

## Results Page (`Backtest.tsx`)

Layout (fixed config panel + scrollable results):

```
┌ Config ─────────────────────────────┐  ┌ Results ───────────────────────────┐
│ Strategy ▾  (params per strategy)    │  │ Equity curve  (strategy vs B&H)    │
│ Symbol      Date range               │  │ Drawdown chart                     │
│ Starting capital                     │  │ Metrics table  (gross / net-of-tax)│
│ Sizing ▾    Slippage bps             │  │ Trade list (entry/exit/P&L/fees)   │
│ Reg fees ☐  Tax ST% LT%   [▸ Run]    │  │  CSV export ⤓                      │
└──────────────────────────────────────┘  └────────────────────────────────────┘
```

- Charts via the existing charting stack used elsewhere in the app (match the patterns in the Historify/Quant pages).
- CSV export of the trade list via the `saveTextFile` helper (`src/lib/exportFile.ts`).
- Help icon (`help-circle-outline`, bare, no filled bubble) with contextual guidance, per project rules.
- Loading (skeleton matching layout), empty ("Configure a backtest to begin"), and error (inline retry) states.
- Design-standard spacing/typography/buttons; metrics use `tabular-nums`.

---

## Persistence (lightweight)

New SQLite table `backtest_runs` (named, idempotent migration following the existing `run_migration` pattern):
- Columns: id, created_at (UTC), symbol, exchange, interval, start/end date, strategy key, params (JSON), config (JSON: capital, sizing, costs, tax), summary metrics (JSON: total return, CAGR, Sharpe, max DD, # trades, net-of-tax return).
- The full trade list / equity curve is **not** persisted (recomputable; cheap to rerun). Only config + summary, so the user can list past runs and compare summaries.
- A "saved runs" list on the Backtest page lets the user reload a config and rerun.

---

## Error Handling

- Insufficient history for the requested warmup/period → clear validation error before running.
- Symbol not in DuckDB history → prompt to download history first (link to existing Historify download flow — surfaced as guidance, not auto-run).
- No trades generated → valid result with a "no trades" empty state, not an error.
- All monetary math in `f64`; document that this is a backtest estimate, not a tax document.

---

## Testing

- **Simulation core unit tests** with hand-computed fixtures: e.g., a 6-bar series with a known SMA crossover → assert exact entry/exit bars, fill prices (with slippage), fees, realized P&L, and short-vs-long-term tax classification.
- **No-look-ahead test**: assert the signal for bar `i` is unaffected by mutating bars after `i`.
- **Cost/tax tests**: a single round-trip with known prices → assert gross vs net-of-tax figures.
- Metric calculations are already covered in `quant_service` tests; add a small integration test asserting the backtest wires them correctly.

---

## File Inventory

- Create: `src-tauri/src/services/backtest_service.rs` (core + signal generators + tests)
- Create: `src-tauri/src/commands/backtest.rs` (Tauri command)
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (register command)
- Modify: `src-tauri/src/db/sqlite/migrations.rs` (+ a `backtest.rs` db module for run CRUD)
- Create: `src/pages/Backtest.tsx`, `src/api/backtest.ts` (+ tauri-client binding, route registration)
- Reuse: `indicators_service`, `quant_service`, `history_service`, `src/lib/exportFile.ts`
