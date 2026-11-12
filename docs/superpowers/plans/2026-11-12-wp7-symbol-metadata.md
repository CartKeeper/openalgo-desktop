# WP7 — Symbol Metadata (FinanceDatabase → DuckDB) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The app holds sector, industry, country and identifiers for every US-listed equity and ETF, downloaded from FinanceDatabase (MIT) into the existing DuckDB `symbol_metadata` table, searchable from the Search page.

**Architecture:** A service downloads the pinned CSVs for the US exchanges from raw.githubusercontent.com, parses them with the `csv` crate, maps exchange codes to the app's names, and loads them in one DuckDB transaction. The table gains columns by migration. Three `symbol_metadata_*` commands expose refresh, status and search. The Search page gets a download control with a status line and a "Company data" results table.

**Tech Stack:** Rust (`reqwest`, `csv = "1"`, duckdb 1.0), React.

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md` (WP7 requirements; "FinanceDatabase pin" row; open item 3).

## Global Constraints

- Branch `wp7-symbol-metadata`; files below plus WP7 anchors.
- Source pinned: `FD_COMMIT = "1ac8809da480"` (2026-11-08). URLs: `https://raw.githubusercontent.com/JerBouma/FinanceDatabase/{FD_COMMIT}/database/equities/{NYQ,NMS,NGM,NCM,ASE,PCX,BTS,OQB}.csv` and `.../database/etfs/{PCX,BTS,NGM,NMS,NYQ,NCM}.csv`; OTC `equities/PNK.csv` (14.5 MB) only when `include_otc`.
- Exchange map: NYQ→`NYSE`, NMS/NGM/NCM→`NASDAQ`, ASE→`AMEX`, PCX→`ARCA`, BTS→`BATS`, OQB/PNK→`OTC`.
- Load is all-or-nothing per refresh (one transaction); a failed download leaves the previous data in place.
- Attribution: `docs/third-party/FinanceDatabase.md` with the MIT text from the repo at the pinned commit and the URL; the Search page status line says "Data: FinanceDatabase (MIT)".
- `summary` stored truncated to 2,000 characters; delisted rows loaded with `delisted = true` and excluded from search by default.
- Times shown in `America/New_York`, 12-hour.

## Review Focus

1. Interrupted download (network drop on file 5 of 14): nothing is written; status still shows the previous refresh. (Task 2 test "a failed file aborts the load" with a fake fetcher.)
2. Quoted commas and embedded newlines in `summary` (common in these CSVs): parsed as one field. (Task 1 fixture.)
3. The same symbol on two exchanges (e.g. dual listings): primary key `(symbol, exchange)` keeps both; search shows the exchange column. (Task 3 test.)
4. UTF-8 BOM at the start of a file: the first header is still `symbol`. (Task 1 test.)
5. Empty `name` for a row: filled with the symbol so the `NOT NULL` column is satisfied. (Task 1 test.)

---

### Task 1: CSV parsing and mapping (pure)

**Files:**
- Modify: `src-tauri/Cargo.toml` (WP7 deps anchor: `csv = "1"`)
- Create: `src-tauri/src/services/symbol_metadata_service.rs`
- Modify: `src-tauri/src/services/mod.rs` (WP7 anchor)

**Interfaces:**
- Produces:
  ```rust
  pub const FD_COMMIT: &str = "1ac8809da480";
  pub const FD_DATA_DATE: &str = "2026-11-08";
  pub const EQUITY_FILES: [&str; 8] = ["NYQ","NMS","NGM","NCM","ASE","PCX","BTS","OQB"];
  pub const ETF_FILES: [&str; 6] = ["PCX","BTS","NGM","NMS","NYQ","NCM"];
  pub const OTC_FILE: &str = "PNK";
  pub fn map_exchange(code: &str) -> Option<&'static str>
  pub struct SymbolMetadataRow { pub symbol: String, pub exchange: String, pub name: String, pub sector: Option<String>, pub industry: Option<String>, pub industry_group: Option<String>, pub country: Option<String>, pub market_cap: Option<f64>, pub size_bucket: Option<String>, pub isin: Option<String>, pub cusip: Option<String>, pub figi: Option<String>, pub website: Option<String>, pub summary: Option<String>, pub asset_type: String /* "equity"|"etf" */, pub delisted: bool }
  pub fn parse_equities(csv_text: &str) -> Result<Vec<SymbolMetadataRow>>   // header: symbol,name,summary,currency,sector,industry_group,industry,exchange,mic,market,country,state,city,zipcode,website,market_cap,isin,cusip,figi,composite_figi,shareclass_figi,delisted
  pub fn parse_etfs(csv_text: &str) -> Result<Vec<SymbolMetadataRow>>       // header: symbol,name,currency,summary,category_group,category,family,exchange,mic,isin,delisted → sector=category_group, industry=category, industry_group=family
  ```
  In this dataset the equities `market_cap` column is a size bucket ("Large Cap"), not a number: it goes to `size_bucket`; `market_cap` stays `None`. Rows whose `exchange` code is not in the map are skipped. `delisted` parses `True`/`False` case-insensitively.

- [ ] **Step 1: Write the failing tests** with inline fixtures: a 3-row equities CSV (one row with a quoted comma-and-newline summary, one with empty name, one on an unmapped exchange) → 2 rows, summary intact, name == symbol; a BOM-prefixed header parses; an ETF CSV row maps category fields; `map_exchange("NMS") == Some("NASDAQ")`, `map_exchange("LSE") == None`.
- [ ] **Step 2–4:** `cargo test --lib symbol_metadata_service` fail → implement (`csv::ReaderBuilder::new().flexible(true)`; strip a leading BOM) → pass.
- [ ] **Step 5: Commit** `feat(symbols): parse FinanceDatabase equity and ETF files`

---

### Task 2: Download and transactional load

**Files:**
- Modify: `src-tauri/src/services/symbol_metadata_service.rs`
- Modify: `src-tauri/src/db/duckdb/migrations.rs` (next migration in the file's own naming: add columns to `symbol_metadata`: `country VARCHAR, industry_group VARCHAR, size_bucket VARCHAR, isin VARCHAR, cusip VARCHAR, figi VARCHAR, website VARCHAR, summary VARCHAR, asset_type VARCHAR, delisted BOOLEAN DEFAULT FALSE, source VARCHAR, source_as_of VARCHAR`)
- Create: `src-tauri/src/db/duckdb/symbol_metadata.rs`; register the module in `src-tauri/src/db/duckdb/mod.rs`

**Interfaces:**
- Produces (`impl DuckDb`): `replace_symbol_metadata(&self, rows: &[SymbolMetadataRow], source: &str, source_as_of: &str) -> Result<usize>` (BEGIN; DELETE FROM symbol_metadata WHERE source = ?; batched INSERT OR REPLACE; COMMIT; on any error ROLLBACK), `search_symbol_metadata(&self, query: &str, limit: usize, include_delisted: bool) -> Result<Vec<SymbolMetadataRow>>` (symbol prefix matches first, then name contains, case-insensitive), `symbol_metadata_count(&self) -> Result<i64>`.
- Service: `#[async_trait] pub trait Fetcher: Send + Sync { async fn get_text(&self, url: &str) -> Result<String>; }`, `pub struct HttpFetcher<'a>(&'a reqwest::Client)` (timeout 120 s per file, 4 concurrent), `pub async fn refresh(db: &DuckDb, settings: &SqliteDb, fetcher: &dyn Fetcher, include_otc: bool) -> Result<MetadataStatus>` (download all → parse all → one `replace_symbol_metadata` → write `settings` keys `symbol_metadata_last_refresh` (UTC ISO) and `symbol_metadata_include_otc`), `pub fn status(db, settings) -> Result<MetadataStatus>`, `pub struct MetadataStatus { pub rows: i64, pub source_commit: String, pub source_as_of: String, pub last_refresh: Option<String>, pub include_otc: bool }`.

- [ ] **Step 1: Write the failing tests**: with a `FakeFetcher` serving the Task 1 fixtures for every URL, `refresh` loads `rows == 2 × files`, `status().rows` matches; "a failed file aborts the load" (fetcher errors on the fifth URL → `Err`, table count unchanged from a previous successful refresh); DuckDB tests use `DuckDb::new(&unique temp path)`.
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(symbols): download pinned FinanceDatabase files into DuckDB transactionally`

---

### Task 3: Commands

**Files:**
- Create: `src-tauri/src/commands/symbol_metadata.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (WP7 anchors)

**Interfaces:**
- `symbol_metadata_refresh(include_otc: bool) -> Result<MetadataStatus, AppError>`, `symbol_metadata_status() -> Result<MetadataStatus, AppError>`, `symbol_metadata_search(query: String, limit: Option<u32>, include_delisted: Option<bool>) -> Result<Vec<SymbolMetadataRow>, AppError>` (limit default 25, max 200).

- [ ] **Step 1: Test**: search on a DB holding `A/NYSE` and `A/ARCA` returns both with their exchanges; limit respected.
- [ ] **Step 2–4:** fail → implement → pass; `cargo test --lib` all pass.
- [ ] **Step 5: Commit** `feat(symbols): symbol_metadata_* commands`

---

### Task 4: Search page and attribution

**Files:**
- Create: `src/api/symbolMetadata.ts`, `src/components/search/CompanyDataCard.tsx`, `docs/third-party/FinanceDatabase.md`
- Modify: `src/pages/Search.tsx`
- Test: `src/components/search/CompanyDataCard.test.tsx` (jsdom)

**Interfaces:**
- `CompanyDataCard({ query })`: status line (`"{rows} symbols · FinanceDatabase {commit} (data as of {source_as_of}) · downloaded {last_refresh in ET}"` or `"Not downloaded yet"`), checkbox "Include OTC (pink sheets)", button **Download symbol metadata** (secondary, 40px; shows a spinner and disables while running; toast on success/failure), and when `query.length >= 1` a table Symbol / Name / Exchange / Sector / Industry / Country from `symbol_metadata_search`. Footer caption "Data: FinanceDatabase (MIT)". Loading skeleton, empty state ("No company data matches"), error with Retry.

- [ ] **Step 1: Write the failing test**: with mocked API, the status renders rows and the ET time; clicking Download calls `refresh(false)`; checking the box then Download calls `refresh(true)`; a query renders the result rows.
- [ ] **Step 2–4:** fail → implement → pass; `npm run build`, `npm run lint` → 0. Help text on the Search page describes the download and what the data is for.
- [ ] **Step 5: Gates**: `cd src-tauri && cargo test --lib`; `npx vitest run`; `npm run build`; `npm run lint`.
- [ ] **Step 6: Commit** `feat(search): company data from FinanceDatabase with download and status`

**Definition of Done (user, after merge):** Search page → **Download symbol metadata** → after a short wait the status line shows a count in the tens of thousands, the pinned commit and "data as of Nov 8, 2026", and the download time in ET. Type `agilent`: the Company data table lists `A · Agilent Technologies, Inc. · NYSE · Health Care · Diagnostics & Research · United States`. Quit and reopen: the data is still there without downloading again.
