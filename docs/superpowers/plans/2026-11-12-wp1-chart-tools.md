# WP1 — Chart Drawing Tools and Indicator Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Historify candlestick chart TradingView-style drawing tools and a searchable library of 1,800+ indicators, both saved per symbol.

**Architecture:** Two MIT libraries built for lightweight-charts v5 do the work: `lightweight-charts-drawing` (`DrawingManager`, 86 tools, `exportJSON`/`importJSON`) and `lightweight-charts-indicators` (`indicatorRegistry`, `IndicatorRenderer`). Two hooks own their lifecycles; two small components present them; persistence goes through a new `drawings` column on the existing `chart_preferences` table (unique per `exchange, symbol`) via two Tauri commands.

**Tech Stack:** React 19, lightweight-charts `^5.3.0` (bumped from `^5.1.0`; the indicators package needs ≥ 5.2), `lightweight-charts-drawing@0.9.0`, `lightweight-charts-indicators@0.20.3`, `oakscriptjs@0.17.1` (peer of the indicators package), rusqlite, vitest + jsdom.

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md` (WP1 requirements, UI additions table, open item 5).

## Global Constraints

- Branch `wp1-chart-tools`; files listed below plus the WP1 anchor lines only.
- Pin exact versions for the three new packages; bump `lightweight-charts` to `^5.3.0` and confirm the four existing chart sites still build (`EquityChart.tsx`, `PortfolioChartCard.tsx`, `PnLTracker.tsx`, `HistorifyCharts.tsx`).
- Icon-only buttons are 32×32 (`h-8 w-8`), radius 8px, `transition-colors`; chips use radius 6px; no hardcoded colors (tokens only); works in light and dark.
- Persistence failures never break the chart: toast once per symbol, keep drawing.
- Help icon content for the page describes the rail, Escape/Delete, per-symbol saving, and the Indicators button.

## Review Focus

1. Switching symbol while a tool is half-placed: the pending save must be cancelled and the half drawing discarded, not written to the new symbol. (Task 4 test "pending save is cancelled on symbol change".)
2. Interval change reloads bars: indicators must recompute on the same renderer instances, never duplicate panes. (Task 5 test "second bars update reuses the renderer".)
3. The page re-creates the chart when the theme flips: drawings must re-import from the last exported JSON, not refetch from the DB. (Task 4 test "re-created manager re-imports the last JSON".)
4. Saved JSON from another library version fails `importJSON`: catch, toast, continue with an empty canvas. (Task 6 try/catch; Task 4 test "import error is reported, not thrown".)
5. Rapid bar updates with several oscillators: recompute is debounced 100 ms. (Task 5 test "ten bar updates in 50 ms render once".)

---

### Task 1: Dependencies and import smoke test

**Files:**
- Modify: `package.json` (`dependencies`)
- Create: `src/lib/chartTools/libs.test.ts`

**Interfaces:**
- Produces: the four packages installed; `lightweight-charts` at 5.3.x.

- [ ] **Step 1: Write the failing test**

```ts
// src/lib/chartTools/libs.test.ts
import { DrawingManager } from 'lightweight-charts-drawing'
import { SMA } from 'lightweight-charts-indicators'

const bars = [1, 2, 3, 4, 5].map((c, i) => ({ time: i + 1, open: c, high: c, low: c, close: c, volume: 1 }))

test('chart tool libraries load and compute', () => {
  expect(typeof DrawingManager).toBe('function')
  expect(SMA.calculate(bars, { len: 3 })).toBeTruthy()
})
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/chartTools/libs.test.ts` → FAIL, module not found.

- [ ] **Step 3: Add the dependencies**

In `package.json` dependencies: `"lightweight-charts": "^5.3.0"`, `"lightweight-charts-drawing": "0.9.0"`, `"lightweight-charts-indicators": "0.20.3"`, `"oakscriptjs": "0.17.1"`. Run `npm install`.

- [ ] **Step 4: Run the test and the build**

Run: `npx vitest run src/lib/chartTools/libs.test.ts` → PASS. Run: `npm run build` → exit 0 (proves the 5.3 bump compiles against the four existing chart sites).

- [ ] **Step 5: Commit**

```bash
git add package.json package-lock.json src/lib/chartTools/libs.test.ts
git commit -m "feat(charts): add drawing and indicator libraries for lightweight-charts 5"
```

---

### Task 2: Indicator catalog (pure module)

**Files:**
- Create: `src/lib/chartTools/indicatorCatalog.ts`
- Test: `src/lib/chartTools/indicatorCatalog.test.ts`

**Interfaces:**
- Produces:
  ```ts
  export interface CatalogInput { id: string; label: string; type: 'int' | 'float' | 'bool' | 'string' | 'source'; default: number | boolean | string }
  export interface CatalogEntry { id: string; name: string; group: string; overlay: boolean; inputs: CatalogInput[] }
  export function listIndicators(): CatalogEntry[]                 // from indicatorRegistry; sorted by group, then name
  export function searchIndicators(q: string, limit?: number): CatalogEntry[]  // case-insensitive over id and name; limit default 50
  export function isOverlay(entry: Pick<CatalogEntry, 'id'> & Partial<CatalogEntry>): boolean
  export const OVERLAY_IDS: ReadonlySet<string>
  ```
- Decision: `overlay` comes from the registry's own metadata when the exported indicator type carries a pane/overlay flag (read the package's `.d.ts`); otherwise from `OVERLAY_IDS` = normalized (lowercase, no spaces/dashes) ids `{sma, ema, wma, vwap, bollingerbands, keltnerchannels, donchianchannels, ichimokucloud, parabolicsar, supertrend, pivotpoints}`.

- [ ] **Step 1: Write the failing tests**

```ts
test('catalog is large and well-formed', () => {
  const all = listIndicators()
  expect(all.length).toBeGreaterThan(1000)
  expect(all.every((e) => e.id && e.name && e.group)).toBe(true)
})
test('search finds RSI first', () => {
  expect(searchIndicators('rsi')[0].id).toMatch(/rsi/i)
})
test('overlay classification', () => {
  expect(isOverlay({ id: 'SMA' })).toBe(true)
  expect(isOverlay({ id: 'RSI' })).toBe(false)
})
```

- [ ] **Step 2: Run to verify they fail** → FAIL, module missing.
- [ ] **Step 3: Implement** the four exports using `indicatorRegistry` (read its API from `node_modules/lightweight-charts-indicators/dist/*.d.ts`; it is the only place the registry shape is defined).
- [ ] **Step 4: Run the tests** → PASS.
- [ ] **Step 5: Commit** `feat(charts): indicator catalog over lightweight-charts-indicators`

---

### Task 3: Drawings persistence (table column, DB functions, commands, API)

**Files:**
- Modify: `src-tauri/src/db/sqlite/migrations.rs` (under `// [wp-anchor] WP1 migrations` and `// [wp-anchor] WP1 sql`)
- Create: `src-tauri/src/db/sqlite/chart_preferences.rs`
- Modify: `src-tauri/src/db/sqlite/mod.rs` (under the WP1 anchor: `pub mod chart_preferences;`)
- Create: `src-tauri/src/commands/chart_preferences.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (WP1 anchors)
- Create: `src/api/chartPreferences.ts`

**Interfaces:**
- Produces (Rust, in `impl SqliteDb`, following the style of `db/sqlite/clients.rs`):
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct ChartPreference { pub symbol: String, pub exchange: String, pub timeframe: String, pub chart_type: String, pub indicators: Option<String>, pub drawings: Option<String>, pub updated_at: String }
  pub fn get_chart_preference(&self, exchange: &str, symbol: &str) -> Result<Option<ChartPreference>>
  pub fn save_chart_preference(&self, exchange: &str, symbol: &str, indicators: Option<&str>, drawings: Option<&str>) -> Result<()>  // upsert on (exchange, symbol); leaves timeframe/chart_type at their defaults when inserting
  ```
- Commands: `chart_get_preference(exchange: String, symbol: String) -> Result<Option<ChartPreference>, AppError>`, `chart_save_preference(exchange: String, symbol: String, indicators: Option<String>, drawings: Option<String>) -> Result<(), AppError>`.
- TS (`src/api/chartPreferences.ts`): `getChartPreference(exchange, symbol): Promise<ChartPreference | null>`, `saveChartPreference(exchange, symbol, data: { indicators: string | null; drawings: string | null }): Promise<void>`.
- Migration: name `061_chart_drawings`, SQL `ALTER TABLE chart_preferences ADD COLUMN drawings TEXT`.

- [ ] **Step 1: Write the failing Rust test** in `chart_preferences.rs` (`#[cfg(test)] mod tests`): open a fresh `SqliteDb` in a unique temp path (mirror an existing DB test if one exists; else `std::env::temp_dir().join(format!("wp1-{}.db", uuid::Uuid::new_v4()))`), `get` → `None`; `save("NASDAQ","AAPL",Some("[1]"),Some("[d]"))` twice with different values → one row, latest values win.
- [ ] **Step 2: Run** `cd src-tauri && cargo test --lib chart_preferences` → FAIL (no module).
- [ ] **Step 3: Implement** migration, DB functions, commands (register under the WP1 handler anchor), and the TS wrapper.
- [ ] **Step 4: Run** `cargo test --lib chart_preferences` → PASS; `cargo test --lib` → all pass.
- [ ] **Step 5: Commit** `feat(charts): persist drawings and indicators per symbol in chart_preferences`

---

### Task 4: `useChartDrawings` hook and `DrawingToolbar`

**Files:**
- Create: `src/hooks/useChartDrawings.ts`
- Create: `src/components/charts/DrawingToolbar.tsx`
- Test: `src/hooks/useChartDrawings.test.tsx`, `src/components/charts/DrawingToolbar.test.tsx` (both `// @vitest-environment jsdom`)

**Interfaces:**
- Produces:
  ```ts
  export type ToolKind = Parameters<DrawingManager['setTool']>[0]
  export interface ChartDrawingsApi { activeTool: ToolKind | null; setTool: (t: ToolKind | null) => void; allHidden: boolean; toggleHidden: () => void; clearAll: () => void; deleteSelection: () => void; hasSelection: boolean }
  export function useChartDrawings(opts: { chart: IChartApi | null; series: ISeriesApi<'Candlestick'> | null; storageKey: string /* `${exchange}:${symbol}` */; initialJson: string | null; onSave: (json: string) => void; onError: (message: string) => void }): ChartDrawingsApi
  ```
  Behavior: create `new DrawingManager(chart, series, { magnet: 'weak' })` when chart and series exist; `importJSON(initialJson ?? '[]')` inside try/catch → `onError('Saved drawings could not be loaded')`; `on('change')` → debounce 500 ms → `onSave(exportJSON())`; keep `lastJsonRef`; when the manager is re-created for the same `storageKey` (chart re-created), import `lastJsonRef` instead of `initialJson`; when `storageKey` changes, cancel the pending save and drop the old manager (`clear()` + unsubscribe; use the library's dispose method if `node_modules/lightweight-charts-drawing/dist/*.d.ts` exposes one).
  ```tsx
  export interface DrawingToolbarProps { activeTool: ToolKind | null; onSelectTool: (t: ToolKind | null) => void; allHidden: boolean; onToggleHidden: () => void; onClearAll: () => void; onDeleteSelection: () => void; hasSelection: boolean }
  export function DrawingToolbar(props: DrawingToolbarProps): JSX.Element
  ```
  Primary tools in order (ids as the library documents them in `docs/TOOLS.md`; verify each id before use): pointer (`null` tool), `trend-line`, `horizontal-line`, `ray`, `rectangle`, `fib-retracement`, `text`. Then a "More tools" dropdown grouped by the library's categories with the remaining tools. Then a separator, then Hide all / Clear all / Delete selection (disabled when `!hasSelection`). Vertical rail, width 40px, 4px gaps, 32×32 ghost icon buttons (lucide icons), active tool `aria-pressed="true"` with accent tint, `title` tooltips.

- [ ] **Step 1: Write the failing tests**
  - Toolbar: renders seven primary tool buttons and a More button; clicking "Trend line" calls `onSelectTool('trend-line')`; the active one has `aria-pressed="true"`; Delete selection is disabled when `hasSelection` is false.
  - Hook (mock `lightweight-charts-drawing` with `vi.mock`, fake timers): "pending save is cancelled on symbol change" (change → advance 100 ms → change storageKey → advance 1 s → `onSave` not called for the old key); "re-created manager re-imports the last JSON" (change producing `'[x]'`, then pass a new chart object with the same key → `importJSON` called with `'[x]'`); "import error is reported, not thrown" (mock `importJSON` throws → `onError` called, hook returns normally).
- [ ] **Step 2: Run** → FAIL (modules missing).
- [ ] **Step 3: Implement** the hook and the toolbar.
- [ ] **Step 4: Run** `npx vitest run src/hooks src/components/charts` → PASS.
- [ ] **Step 5: Commit** `feat(charts): drawing rail and DrawingManager lifecycle hook`

---

### Task 5: Indicators — active list, renderer hook, picker, chips

**Files:**
- Create: `src/lib/chartTools/activeIndicators.ts`, `src/hooks/useChartIndicators.ts`, `src/components/charts/IndicatorPicker.tsx`, `src/components/charts/IndicatorChips.tsx`
- Test: `src/lib/chartTools/activeIndicators.test.ts`, `src/hooks/useChartIndicators.test.tsx`, `src/components/charts/IndicatorPicker.test.tsx`, `src/components/charts/IndicatorChips.test.tsx`

**Interfaces:**
- Consumes: Task 2 catalog.
- Produces:
  ```ts
  export interface ActiveIndicator { key: string /* crypto.randomUUID() */; id: string; inputs: Record<string, number | boolean | string> }
  export function serializeIndicators(list: ActiveIndicator[]): string
  export function parseIndicators(json: string | null): ActiveIndicator[]   // invalid or wrong shape → []
  export function useChartIndicators(opts: { chart: IChartApi | null; mainSeries: ISeriesApi<'Candlestick'> | null; bars: Bar[]; active: ActiveIndicator[]; context: ChartContext }): void
  export function IndicatorPicker(props: { onAdd: (entry: CatalogEntry) => void }): JSX.Element   // 32px "Indicators" button → Popover with search Input and grouped results (first 50 matches)
  export function IndicatorChips(props: { active: ActiveIndicator[]; onChangeInputs: (key: string, inputs: ActiveIndicator['inputs']) => void; onRemove: (key: string) => void }): JSX.Element
  ```
  Hook behavior: one `IndicatorRenderer(chart, { paneIndex, mainSeries })` per `key`; `paneIndex` 0 for overlays, otherwise the next free pane number in order of addition; `render(indicator, indicator.calculate(bars, inputs), bars, { inputs, reuseSeries: true })` on `bars` or `inputs` change, debounced 100 ms; `renderer.clear()` on removal and unmount. `ChartContext` is built as `{ symbol, timezone: 'America/New_York', ... }` with exactly the fields the package's `ChartContext` type requires (read the `.d.ts`).
  Chips: Badge-style, height 24px, radius 6px, 8px gap; each chip shows the catalog name, a gear (Popover with one numeric Input per `int`/`float` input, label from the catalog, applied on blur/Enter) and an × that calls `onRemove`.

- [ ] **Step 1: Write the failing tests**: serialize/parse round-trip and tolerant parse (`'nope'` → `[]`, `'{"a":1}'` → `[]`); hook with a mocked renderer: "second bars update reuses the renderer" (constructor called once, `render` twice) and "ten bar updates in 50 ms render once"; picker: typing `macd` lists an entry whose id matches /macd/i and clicking it calls `onAdd`; chips: × calls `onRemove(key)`.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run src/lib/chartTools src/hooks src/components/charts` → PASS.
- [ ] **Step 5: Commit** `feat(charts): indicator picker, chips and renderer lifecycle`

---

### Task 6: Wire into HistorifyCharts, persistence, help text

**Files:**
- Modify: `src/pages/HistorifyCharts.tsx`

**Interfaces:**
- Consumes: Tasks 2–5; `getChartPreference`/`saveChartPreference`.

- [ ] **Step 1: Layout.** Wrap the chart container in a flex row: `DrawingToolbar` on the left (40px), chart on the right. Add the `IndicatorPicker` button to the existing header controls row after the interval controls. Render `IndicatorChips` in a row under the header only when there are active indicators. Keep the existing `chartRef`; add `candleSeriesRef` set where `chart.addSeries(CandlestickSeries, …)` runs.
- [ ] **Step 2: Load.** When `selectedExchange`/`selectedSymbol` change: `getChartPreference` → `initialJson` (drawings) and `parseIndicators(indicators)`. Failures → `toast.error('Could not load chart drawings')` once.
- [ ] **Step 3: Save.** `onSave` from the drawings hook and any change to the active indicator list call `saveChartPreference(exchange, symbol, { indicators: serializeIndicators(active), drawings })`; failures → `toast.error('Could not save chart drawings')` at most once per symbol.
- [ ] **Step 4: Bars.** Map the page's `OHLCVData[]` to the `Bar[]` the hooks take (`time` in epoch seconds, the same conversion the page already does for the candle series).
- [ ] **Step 5: Help.** Extend the page's `HelpButton` content: how to arm a tool, Escape cancels, Delete removes the selection, drawings and indicators are saved per symbol, Indicators button opens the library.
- [ ] **Step 6: Gates.** `npm run build` → 0; `npm run lint` → 0; `npx vitest run` → all pass; `cd src-tauri && cargo test --lib` → all pass.
- [ ] **Step 7: Commit** `feat(historify): drawing tools and indicator library on the chart page`

**Definition of Done (user, after merge):** open Historify → Charts → a symbol with data (e.g. `/historify/charts/AAPL`). The rail is on the left of the chart. Click Trend line, place two points, the line appears. Switch to another symbol and back: the line is still there. Click Indicators, type `rsi`, pick RSI: a new pane appears below the candles. Close and reopen the app: both persist. Pick Clear all: the line is gone and stays gone after switching symbols.
