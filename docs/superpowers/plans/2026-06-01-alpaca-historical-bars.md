# Alpaca Historical Bars Download — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development or executing-plans.

**Goal:** Implement real historical-bars download from Alpaca into DuckDB so the backtester has data to run on; trigger it from the Backtest page on a "no data" result.

**Architecture:** New `Broker::get_history` (default unsupported) + Alpaca impl hitting `data.alpaca.markets/v2/stocks/{symbol}/bars` (auth via existing `get_headers`, paginated). `HistoryService::download_history` (currently a stub) calls it and stores to DuckDB via `store_market_data`. The `download_historical_data` Tauri command (also a stub) is wired to the service. The Backtest page surfaces a "Download history" button when a run returns no data.

**Tech Stack:** Rust (reqwest, serde), React/TS.

---

## Reference (real signatures, verified)
```rust
// brokers/alpaca/mod.rs
const DATA_BASE_URL: &str = "https://data.alpaca.markets";
fn parse_auth_token(auth_token: &str) -> Result<(String, String)>;   // (api_key, api_secret)
impl AlpacaBroker { fn get_headers(&self, api_key: &str, api_secret: &str) -> reqwest::header::HeaderMap; self.client: reqwest::Client }

// services/history_service.rs
pub struct CandleData { timestamp: String, open: f64, high: f64, low: f64, close: f64, volume: i64 }
HistoryService::download_history(state, symbol, exchange, interval, from_date, to_date, api_key) -> Result<usize>  // STUB returns Ok(0)
HistoryService::store_market_data(state, symbol, exchange, interval, candles: Vec<CandleData>) -> Result<usize>

// commands/historify.rs  (download_historical_data is a STUB)
struct DownloadRequest { symbol, exchange, timeframe, from_date, to_date: String }
struct DownloadResponse { success: bool, rows_downloaded: usize, message: String }

// AppState: state.get_broker_session() -> Option<{ auth_token, broker_id }>; state.brokers.get(&broker_id)
```

---

## Task 1: `HistoricalBar` type + `Broker::get_history` default

**Files:** `src-tauri/src/brokers/types.rs`, `src-tauri/src/brokers/mod.rs`

- [ ] Add to `brokers/types.rs`:
```rust
/// One OHLCV bar returned by a broker's historical-data endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalBar {
    pub timestamp: String, // RFC3339 / ISO
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}
```
- [ ] Add a default trait method in `brokers/mod.rs` (alongside `get_portfolio_history`):
```rust
/// Get historical OHLCV bars. `interval` is the app interval string
/// (e.g. "D", "1h", "5m"); the broker maps it to its own timeframe.
/// Default: not supported.
async fn get_history(
    &self,
    _auth_token: &str,
    _symbol: &str,
    _exchange: &str,
    _interval: &str,
    _from_date: &str,
    _to_date: &str,
) -> Result<Vec<crate::brokers::types::HistoricalBar>> {
    Err(crate::error::AppError::Broker(
        "Historical data is not supported by this broker".to_string(),
    ))
}
```
- [ ] `cargo check` clean. Commit: `feat(history): add HistoricalBar type + Broker::get_history default`.

---

## Task 2: Alpaca `get_history` implementation (TDD on mapping + parse)

**Files:** `src-tauri/src/brokers/alpaca/mod.rs`

- [ ] **Write failing unit tests** (in a `#[cfg(test)] mod history_tests`):
```rust
#[test]
fn maps_app_interval_to_alpaca_timeframe() {
    assert_eq!(interval_to_timeframe("D").unwrap(), "1Day");
    assert_eq!(interval_to_timeframe("W").unwrap(), "1Week");
    assert_eq!(interval_to_timeframe("1h").unwrap(), "1Hour");
    assert_eq!(interval_to_timeframe("5m").unwrap(), "5Min");
    assert_eq!(interval_to_timeframe("1m").unwrap(), "1Min");
    assert!(interval_to_timeframe("nope").is_err());
}

#[test]
fn parses_alpaca_bars_payload() {
    let body = r#"{"bars":[
        {"t":"2024-01-02T05:00:00Z","o":187.1,"h":188.4,"l":183.9,"c":185.6,"v":82488200},
        {"t":"2024-01-03T05:00:00Z","o":184.2,"h":185.9,"l":183.4,"c":184.3,"v":58414500}
    ],"symbol":"AAPL","next_page_token":null}"#;
    let page: AlpacaBarsPage = serde_json::from_str(body).unwrap();
    let bars = page.into_bars();
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[0].timestamp, "2024-01-02T05:00:00Z");
    assert!((bars[0].close - 185.6).abs() < 1e-9);
    assert_eq!(bars[1].volume, 58414500);
}
```
- [ ] Run: `cd src-tauri && cargo test --lib history_tests` → FAIL (undefined).
- [ ] **Implement.** Add DTOs + mapping + the trait impl:
```rust
use crate::brokers::types::HistoricalBar;

#[derive(serde::Deserialize)]
struct AlpacaBar { t: String, o: f64, h: f64, l: f64, c: f64, v: f64 }
#[derive(serde::Deserialize)]
struct AlpacaBarsPage {
    #[serde(default)] bars: Vec<AlpacaBar>,
    #[serde(default)] next_page_token: Option<String>,
}
impl AlpacaBarsPage {
    fn into_bars(self) -> Vec<HistoricalBar> {
        self.bars.into_iter().map(|b| HistoricalBar {
            timestamp: b.t, open: b.o, high: b.h, low: b.l, close: b.c, volume: b.v as i64,
        }).collect()
    }
}

/// Map the app interval string to an Alpaca `timeframe` value.
fn interval_to_timeframe(interval: &str) -> Result<String> {
    let tf = match interval {
        "D" | "1d" | "1D" | "1Day" => "1Day",
        "W" | "1W" | "1Week" => "1Week",
        "1h" | "1H" | "60m" => "1Hour",
        "1m" => "1Min", "5m" => "5Min", "15m" => "15Min", "30m" => "30Min",
        other => return Err(AppError::Broker(format!("Unsupported interval for Alpaca history: {other}"))),
    };
    Ok(tf.to_string())
}
```
Add the trait method inside `impl Broker for AlpacaBroker` (note: `bars` data host is always `DATA_BASE_URL`, not `get_base_url`):
```rust
async fn get_history(
    &self,
    auth_token: &str,
    symbol: &str,
    _exchange: &str,
    interval: &str,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<HistoricalBar>> {
    let (api_key, api_secret) = parse_auth_token(auth_token)?;
    let timeframe = interval_to_timeframe(interval)?;
    let mut all: Vec<HistoricalBar> = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut url = format!(
            "{DATA_BASE_URL}/v2/stocks/{symbol}/bars?timeframe={timeframe}&start={from_date}&end={to_date}&feed=iex&adjustment=split&limit=10000"
        );
        if let Some(tok) = &page_token {
            url.push_str(&format!("&page_token={tok}"));
        }
        let resp = self.client.get(&url)
            .headers(self.get_headers(&api_key, &api_secret))
            .send().await?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Broker(format!("Alpaca bars request failed: {body}")));
        }
        let page: AlpacaBarsPage = resp.json().await?;
        let next = page.next_page_token.clone();
        all.extend(page.into_bars());
        match next {
            Some(tok) if !tok.is_empty() => page_token = Some(tok),
            _ => break,
        }
    }
    Ok(all)
}
```
> If `from_date`/`to_date` arrive as plain dates (YYYY-MM-DD), Alpaca accepts them; if it rejects, append `T00:00:00Z`. Verify against a live call during the smoke test.
- [ ] Run tests → PASS. `cargo check` clean. Commit: `feat(alpaca): historical bars download via /v2/stocks/{symbol}/bars`.

---

## Task 3: Wire `HistoryService::download_history` → DuckDB

**Files:** `src-tauri/src/services/history_service.rs`

- [ ] Replace the stub body of `download_history` (keep signature) with:
```rust
// Resolve the active broker session.
let session = state.get_broker_session()
    .ok_or_else(|| crate::error::AppError::Auth("Broker not connected".to_string()))?;
let broker = state.brokers.get(&session.broker_id)
    .ok_or_else(|| crate::error::AppError::Broker(format!("Broker '{}' not found", session.broker_id)))?;

let bars = broker.get_history(&session.auth_token, symbol, exchange, interval, from_date, to_date).await?;
if bars.is_empty() {
    return Ok(0);
}
let candles: Vec<CandleData> = bars.into_iter().map(|b| CandleData {
    timestamp: b.timestamp, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
}).collect();
Self::store_market_data(state, symbol, exchange, interval, candles)
```
> Remove the now-unused `_state`/`_api_key` leading underscores as needed (params become used). Confirm `state.get_broker_session()` and `state.brokers` field names against `state.rs` and match exactly.
- [ ] `cargo check` clean. Commit: `feat(history): download_history fetches from broker and stores to DuckDB`.

---

## Task 4: Wire the `download_historical_data` command

**Files:** `src-tauri/src/commands/historify.rs`

- [ ] Replace the placeholder body of `download_historical_data` with a real call:
```rust
let rows = crate::services::HistoryService::download_history(
    &state, &request.symbol, &request.exchange, &request.timeframe,
    &request.from_date, &request.to_date, None,
).await?;
Ok(DownloadResponse {
    success: true,
    rows_downloaded: rows,
    message: if rows == 0 { "No bars returned for that symbol/range".into() } else { format!("Downloaded {rows} bars") },
})
```
> Remove the `_state` underscore (now used). Confirm `HistoryService` is reachable via `crate::services::HistoryService`.
- [ ] `cargo check` + `cargo test --lib` clean. Commit: `feat(history): wire download_historical_data command to the service`.

---

## Task 5: Backtest page — "Download history" on no-data

**Files:** `src/api/backtest.ts` (or reuse `historifyCommands`), `src/pages/Backtest.tsx`

- [ ] The Tauri binding already exists: `historifyCommands.downloadHistoricalData(request)` in `src/api/tauri-client.ts` (returns `{ success, rows_downloaded, message }`). Use it. Its request shape is `{ symbol, exchange, timeframe, from_date, to_date }` — map the page's config fields (use the page's `barInterval` for `timeframe`, and the from/to dates).
- [ ] In `Backtest.tsx`: when a Run fails with a "no historical data" / empty-data error (the backend returns `AppError::Validation` with "No historical data…"), show an inline panel with a **"Download {symbol} {interval} history"** button instead of (or alongside) the error. On click:
  - set a `downloading` state (button shows spinner + disabled),
  - call `historifyCommands.downloadHistoricalData({ symbol, exchange, timeframe: barInterval, from_date, to_date })`,
  - on success toast `Downloaded N bars` and automatically re-run the backtest,
  - on `rows_downloaded === 0` toast a clear "no bars returned (check symbol/range/market data entitlement)" message,
  - on error show it inline.
- [ ] Respect existing design standards (h-10 button, spinner `motion-reduce:animate-none`).
- [ ] `npx tsc --noEmit -p tsconfig.json` clean. Commit: `feat(backtest): download history on no-data, then auto-rerun`.

---

## Task 6: Verify

- [ ] `cd src-tauri && cargo test --lib` (all pass), `cargo check` (0 warnings), `npx tsc --noEmit` (clean).
- [ ] Manual smoke (the original goal): on Backtest, run SMA 20/50 for AAPL/NASDAQ daily over 2020→today → "no data" → click Download → bars load → auto-rerun → equity curve + metrics + trades populate.

---

## Self-review notes
- Layering: broker returns `HistoricalBar` (brokers/types), service maps to `CandleData` (services), DuckDB stores rows — no cross-layer leakage.
- `feed=iex` matches the account tier; free IEX data has limited history depth — if early years come back empty, that's an entitlement limit, not a bug (surface via the rows==0 message).
- `adjustment=split` keeps prices continuous across splits (correct for price backtests).
- Verify-before-use: `state.get_broker_session()` / `state.brokers` exact names; Alpaca date format acceptance (plain date vs RFC3339).
