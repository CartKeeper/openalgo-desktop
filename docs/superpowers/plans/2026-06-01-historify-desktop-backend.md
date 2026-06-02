# Historify Desktop Backend — Implementation Plan

> Core US data loop. Port the Historify page off its dead Flask backend onto Tauri commands.

**Goal:** Make Historify functional in the desktop app: symbol search → add/list/remove watchlist → download history (US/Alpaca) → Data Catalog + DB stats + download-jobs list. Exchange is **auto-resolved from Alpaca assets** so it's correct and consistent with the rest of the app (the local cache keys on `(symbol, exchange, interval)`).

**Out of scope:** F&O option chains, bulk upload/export, job pause/resume/cancel/retry, csrf-token, Socket.IO progress (jobs run synchronously and the UI polls `historify_get_jobs`).

**Architecture:** New Tauri commands in `commands/historify.rs` backed by new DuckDb methods (tables already exist: `watchlist`, `data_catalog`, `download_jobs`, `job_items`). Reuse `search_symbols` (exists) and `HistoryService::download_history` (exists, Alpaca). Rewire `Historify.tsx` from `fetch()` to `invoke()`. Add an Alpaca exchange resolver.

---

## Exchange handling (the key decision)
- Alpaca routes US equities by **ticker**; its bars endpoint ignores exchange (`get_history(_exchange)`). The local cache, however, keys on `(symbol, exchange, interval)`, so the exchange string must be **consistent everywhere**.
- **Resolve the exchange from Alpaca** when adding a symbol: new `AlpacaBroker` path `GET /v2/assets/{symbol}` → read `.exchange`, map via existing `map_alpaca_exchange`. Default to `"NASDAQ"` on failure. Store the resolved exchange in the watchlist; downloads + catalog use that same value.
- Default the Historify exchange UI to US venues (`["NASDAQ","NYSE","ARCA","BATS","AMEX","OTC"]`), not NSE.

---

## Backend (Task 1) — `commands/historify.rs` + DuckDb methods + migration

New DuckDB migration `006_download_jobs_v2` (idempotent `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`): add `job_type VARCHAR DEFAULT 'custom'`, `interval VARCHAR DEFAULT 'D'`, `start_date VARCHAR`, `end_date VARCHAR`, `failed_items INTEGER DEFAULT 0`, `started_at TIMESTAMP`, `error_message VARCHAR` to `download_jobs`.

New DuckDb methods (`db/duckdb/mod.rs`) — all keyed on the `historify.duckdb` connection:
- `historify_get_watchlist() -> Vec<HistorifyWatchlistItem>` — `SELECT id, symbol, exchange, name, CAST(created_at AS VARCHAR) FROM watchlist ORDER BY order_index, created_at DESC`.
- `historify_add_watchlist(symbol, exchange, name) -> i64` — insert with id `(SELECT COALESCE(MAX(id),0)+1 ...)`, `order_index` next; ignore conflicts.
- `historify_remove_watchlist(symbol, exchange) -> bool` — `DELETE ... WHERE symbol=? AND exchange=?`.
- `historify_bulk_add_watchlist(&[(String,String)]) -> usize`.
- `historify_get_catalog() -> Vec<CatalogItemResponse>` — from `data_catalog`, cast timestamps to VARCHAR, map `timeframe`→`interval`, `row_count`→`record_count`, `from_date`/`to_date`→`first_date`/`last_date` and to epoch seconds for `first_timestamp`/`last_timestamp`.
- `historify_get_stats() -> HistorifyStats` — `SUM(row_count)` (total_records), `COUNT(DISTINCT symbol||':'||exchange)` (total_symbols) from `data_catalog`, `COUNT(*)` from `watchlist`; `database_size_mb` from `std::fs::metadata(duckdb_path).len()/1MB`.
- `historify_get_jobs(limit) -> Vec<HistorifyJobResponse>` — from `download_jobs` (post-migration columns), map `total_items`/`completed_items`→`total_symbols`/`completed_symbols`, `failed_items`→`failed_symbols`, `id` cast to String.
- `create_download_job(...) -> i64` + `job_items` rows; `update_job_status`, `update_job_item_status`.
- `upsert_data_catalog(symbol, exchange, timeframe, from_date, to_date, row_count)` — `ON CONFLICT (symbol,exchange,timeframe) DO UPDATE` widening the date range + updating row_count.

> The `DuckDb` connection is `Mutex<Connection>`; follow the existing `query_market_data`/`insert_market_data` lock pattern. Use `CAST(col AS VARCHAR)` whenever reading a `TIMESTAMP`/`DATE` column into a Rust `String` (same class of bug just fixed in `query_market_data`).

New Tauri commands (all `#[tauri::command] pub async fn ... (state: State<'_, AppState>, ...) -> Result<T>`), registered in `commands/mod.rs` is already done (module exists) and added to `lib.rs` `generate_handler!`:
`historify_get_watchlist`, `historify_add_watchlist`, `historify_remove_watchlist`, `historify_bulk_add_watchlist`, `historify_get_catalog`, `historify_get_stats`, `historify_get_storage_intervals`, `historify_get_exchanges`, `historify_get_jobs`, `historify_create_job`.

- `historify_add_watchlist(symbol, exchange)`: **resolve exchange from Alpaca** (`AlpacaBroker::resolve_exchange` via the active broker session; fall back to the passed exchange, then `"NASDAQ"`), store resolved value.
- `historify_get_storage_intervals` / `historify_get_exchanges`: static — storage `["1m","D","W"]`, computed `["5m","15m","30m","1h"]`; exchanges `["NASDAQ","NYSE","ARCA","BATS","AMEX","OTC"]`.
- `historify_create_job`: insert job + items, then for each symbol call `HistoryService::download_history` (synchronous loop), update job/item status + `upsert_data_catalog` with the rows stored, set job `completed`/`failed`. Return `{ success, job_id, total_symbols, message }`.

Alpaca exchange resolver — `brokers/alpaca/mod.rs`: `async fn resolve_exchange(&self, auth_token, symbol) -> Result<String>` → `GET {get_base_url}/v2/assets/{symbol}` (trading host), read `.exchange`, `map_alpaca_exchange`. Add as a `Broker` trait default (`Ok("NASDAQ".into())`) + Alpaca override, OR a standalone helper used by the command. Keep it best-effort (never block add on failure).

Tests: unit-test `upsert_data_catalog` date-range widening logic if extractable; otherwise rely on `cargo check` + the manual smoke. (Most of this is DB plumbing; keep tests light.)

---

## Frontend (Task 2) — `Historify.tsx` + `tauri-client.ts`

Add `historifyCommands.*` to `tauri-client.ts` (see contract): getWatchlist/addWatchlist/removeWatchlist/bulkAddWatchlist/getCatalog/getStats/getStorageIntervals/getExchanges/getJobs/createJob, plus `HistorifyWatchlistItem`/`Stats`/`CreateJobRequest`/`CatalogItem`/`DownloadJob` interfaces matching the Rust shapes.

Rewire each `fetch('/historify/api/...')` / `fetch('/search/api/...')` call in `Historify.tsx` to the matching command (drop the CSRF fetch; Tauri throws on error so use try/catch). Use `symbolCommands.searchSymbols(query, exchange, 10)` for `performSearch`. Default the exchange state/dropdown to US venues. The `loadIntervals` call (whose result `_intervals` is unused) can be removed from the mount effect.

Verify: `npx tsc --noEmit` clean; the page loads with **no red console errors**; add a symbol works (resolves its exchange); download populates the catalog + stats; jobs list shows the run.

---

## Verify (Task 3)
- `cargo test --lib` + `cargo check` clean.
- `npx tsc --noEmit` clean.
- Manual: open Historify → no error spam; search AAPL → Add (exchange auto-fills NASDAQ) → it appears in Symbols; set Daily + a recent range → Download All → job appears, Data Catalog shows AAPL/NASDAQ/D with a row count, header stats update (MB / records / symbols).
