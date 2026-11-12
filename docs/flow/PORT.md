# Flow port — what was carried, what was adapted

WP8 carries upstream OpenAlgo Desktop's Flow module (the visual workflow
builder) into this app. This file is the record a later upstream sync diffs
against: the exact commit carried, every seam where this app differs, and
every edit made to a carried file. Phase A (branch `wp8a-flow-core`) is the
Rust engine; the frontend (Phase B), the triggers (Phase C) and the parity
pass (Phase D) add their own sections.

## Carried commit

- Remote `upstream` = `marketcalls/openalgo-desktop`, ref `upstream/master`.
- Commit: `52580437b0991774576272d007c442cd6eb820fd` (their v1.0.7).
- Carried with `git checkout upstream/master -- <paths>` on 2026-11-12:
  - `src-tauri/src/flow/**` (the module),
  - `src-tauri/src/clock.rs`,
  - `src-tauri/src/security/secret.rs`,
  - `src-tauri/src/agent/indicators/**` → `src-tauri/src/indicators_lib/**` (renamed with `git mv`; the empty `agent/` directory was removed),
  - `tests/fixtures/web/flow/goldens/**` (787 goldens + README, repository root, the path `flow/goldens/replay.rs` reads),
  - `src-tauri/tests/fixtures/flow/` (`logic_vectors.json`, `validator_goldens.json`),
  - `src-tauri/tests/fixtures/flow_indicators/` (the indicator service goldens),
  - `src-tauri/tests/fixtures/agent_indicators/` (the indicator library's own goldens; the plan did not list it, the library's tests `include_str!` it).

Upstream's `edition` is 2021 like this app's; no edition-2024 syntax is in
the carried tree.

## Not carried (and why)

| Upstream path | Why |
|---|---|
| `src-tauri/src/flow/doc_tests.rs` | Tests upstream's `docs/prompt/flow-import-format.md` and user-guide chapter 21 against the code. Neither document exists here (Phase D writes `docs/flow/user-guide.md`). |
| `flow/contract/client/app/{tests,refusal_tests}.rs` | Built on upstream's Agent test kit, mock broker and tunnel log capture. Task A4 writes this app's client tests on the headless harness. |
| `flow/nodes/internal/tests.rs`, `flow/nodes/order_api_tests.rs`, `flow/nodes/data_tests/app.rs` | Mock-broker (`brokers::mock`) and `mcp::dispatch` tests. |
| `flow/nodes/alerts/client_tests.rs` | Upstream's Telegram/WhatsApp services; Telegram arrives in Phase C with its own tests. |
| `flow/triggers/feed/tests.rs` | Upstream's Samco streaming corpus; this app has no broker stream Flow can subscribe to (see `quote_feed` below). |
| `src-tauri/src/events.rs` of upstream | Upstream has none at that path; its event bus lives in `events/`. This app's `events.rs` is new (below). |

## Seam table (upstream symbol → here)

Every `crate::` path the carried code imported that this app does not have,
and what it was rewritten to. The rewrite was a mechanical `sed` over the
carried tree, followed by the hand edits listed in the next section.

| Upstream | Here | Notes |
|---|---|---|
| `crate::agent::indicators::*` | `crate::indicators_lib::*` | Directory rename. `discovery.rs` and `scan.rs` (the Agent's tools) are carried so the library's tests stay whole. |
| `crate::agent::contract::tool::ToolOutcome`, `agent::tools::market::normalise::refuse` | `indicators_lib::outcome::{ToolOutcome, refuse}` | A two-variant stand-in with `invalid_argument`, `text`, `is_ok`. |
| `crate::agent::tools::market::candles::{CandleFrame, ist_timestamp, ...}` | `indicators_lib::candles` | `Columns`, `candle_columns`, `candle_frame`, `CandleFrame` copied; `ist_timestamp`/`iso_ist` became `market_timestamp`/`iso_market` over the market zone. The Agent's chart/summary helpers were not carried. |
| `crate::agent::tools::values::*` | `flow::support::values` | Copied; the `py_repr` of upstream's `agent/tools/service.rs` inlined as `service_py_repr`. |
| `crate::agent::tools::numbers::*` | `flow::support::numbers` | Copied; naive timestamps are read in the market zone, not IST. |
| `crate::agent::tools::order_vocab::*` | `flow::support::order_vocab` | Copied; `split_exchange_prefix` answers `Err(String)` instead of an Agent `ToolOutcome`. |
| `crate::agent::tools::testkit::{row, master_rows}` | `flow::support::testkit` | Plus `capture_logs` (was `services::tunnel::fake::capture_logs`) and `sign_in` (what `AuthService::setup` left behind). |
| `crate::agent::tools::market::VALID_EXCHANGES`, `crate::services::schemas::VALID_EXCHANGES` | `flow::support::FLOW_EXCHANGES` (imported `as VALID_EXCHANGES`) | Upstream's 14 exchanges in web order; Task A5 appends the US venues. |
| `crate::services::core::{Reply, INVALID_API_KEY, WEBHOOK_LOCKED}` | `flow::support::reply` (`Reply` re-exported as `flow::support::Reply`) | `Reply { status, body }` without upstream's placement outcome. Phase B's `FlowReply` is built from it. |
| `crate::mcp::dispatch::{Raw, Transport, UNKNOWN_OUTCOME, SDK_TIMEOUT, post_request, run_as}` | `flow::support::dispatch` | The request carries the internal token in `x-openalgo-internal-token`; `run` goes through the app's own axum router with `tower::ServiceExt::oneshot` (Task A4). |
| `crate::brokers::common::symbols::{SymToken, SymbolGeneration}`, `app.symbols` | `flow::support::symbols::{SymToken, SymbolGeneration, SymbolStore}`; `AppState::symbols` | Built from this app's `SymbolInfo` (no expiry/strike: US equities). `AppState::load_symbol_cache` fills it. |
| `crate::brokers::common::underlying::*` | `flow::support::underlying` | Copied. |
| `crate::brokers::common::mapping::{Exchange, Product, PriceType, Action, ...}` | `flow::support::mapping` | Copied (the `oa_enum!` macro and the enums). |
| `crate::strategy::session::session_day` | `flow::support::session::session_day` | Market zone instead of IST. |
| `crate::strategy::dispatch::NO_BROKER_SESSION` | `flow::support::NO_BROKER_SESSION` | Same sentence. |
| `crate::strategy::book::StrategyBook` | `nodes::books::AppStrategyBook` is a unit struct | `legs()` answers `Err("Strategy P&L is not available in this app")`. |
| `crate::services::options_service::{NO_SPOT, quote_exchange, option_exchange, today_ist, option_chain_structure}`, `services::symbol_service::expiry` | `flow::support::options` | The pure helpers copied; `expiry` and `option_chain_structure` answer 501 "not available yet" until Phase D wires `OptionsService::get_option_chain`. |
| `crate::services::market_calendar_service::holidays_api` | `flow::support::calendar::holidays_api` | Over this app's `market_holidays` table; `holiday_type` is always `trading`, `open_exchanges` empty, `timezone` the market zone. |
| `crate::db::sqlite::market_calendar::{SessionWindow, effective_session_window}` | `flow::support::market_hours` | The regular 09:30–16:00 market-time window; Task A3 reads the live clock. |
| `crate::db::sqlite::write_transaction` | `flow::support::db::write_transaction` | Copied (`Transaction::new_unchecked`, immediate). |
| `crate::db::sqlite::key_vault::Witness`, `crate::security::crypto::Aad` | removed | This app's `SecurityManager::{encrypt, decrypt}` take no associated data and the key never changes without the data folder; `store::witness()` and `secret_aad()` were dropped. |
| `crate::security::{Unlocked, keystore}`, `security.is_unlocked()` | `AppState::is_authenticated()` | Always unlocked; "signed in" is a user session. |
| `crate::services::apikey_service::ApiKeyService::current` | `flow::service::api_key()` answers the internal token | A workflow needs no API key in this app (consequence recorded below). |
| `crate::services::auth_service` | `app.user_session` | |
| `crate::services::tunnel` | `flow::support::public_url::public_base_url` | The webhook settings' `ngrok_url` when set, else `http://{host}:{port}` of the webhook server (Task A7). |
| `crate::events::{Event, Lane, Subscriber, Topic, OrderUpdate}` | `crate::events::{OrderUpdate, OrderUpdates}` (new file) | A `tokio::sync::broadcast` channel on `AppState::order_updates`; `drive::subscribe_order_updates` reads it in a task the trigger side owns. Phase C's order-book poller sends into it. |
| `crate::feed::*`, `brokers::common::streaming`, `websocket::WebSocketManager` (in `triggers/feed.rs`) | `flow::support::quote_feed::AppFeed`, re-exported at `flow::triggers::feed` | One-shot quotes through `QuotesService` (Task A7). |
| `crate::messaging::{telegram, whatsapp}` | `AppFlowClient::telegram` answers "Telegram is not set up: add the bot token and chat id on the Telegram page" (Phase C replaces it); `whatsapp` answers "WhatsApp alerts are not available in this app" | |
| `crate::server::{api_v1, envelope}`, `services::{options_order_service, order_service, schemas, Route}` (in `nodes/internal.rs`) | rewritten | A scoped close goes through `/api/v1/openposition` + `/api/v1/placeorder`; absolute-strike legs are refused with one sentence (Task A4). |
| `app.server_config()` (session boundary, host server), `app.listening_port()` | `AppState::session_boundary()` (the auto-logout time), `public_base_url` | |
| `app.blocking`, `app.spawn`, `app.now`, `app.clock`, `app.flow` | added to `AppState` | |
| `app.sqlite.conn()` | added `SqliteDb::conn()` | Returns the mutex guard; the caller drops it before any await, as upstream's pooled connection. |
| `SecurityManager::for_tests()` | added (`#[cfg(test)]`) | |
| `chrono_tz::Asia::Kolkata`, `ist_*` | `flow::contract::time::market_tz()` (America/New_York) | Task A3. A thread-local test override (`set_market_tz_for_tests`) instead of the plan's `OnceLock`, so the IST goldens and the Eastern tests share one test process. |
| `#[cfg(any(test, feature = "test-support"))]` | `#[cfg(test)]` | This app has no `test-support` feature (the fake client is test-only). |

## Hand edits to carried files (Phase A)

| File | Edit |
|---|---|
| `flow/mod.rs` | `pub mod support;` added; `mod doc_tests;` removed. |
| `flow/nodes/mod.rs`, `flow/nodes/alerts.rs`, `flow/nodes/data_tests/mod.rs` | The `mod` lines of the test files not carried removed. |
| `flow/contract/client/app.rs` | Rewritten for this app (see the seam table); `shaped`, `transport_failure`, `NO_ANSWER`, `NOT_SENT`, `CALL_TIMEOUT` kept as upstream wrote them. `with_broker_cause` matches this app's refusal texts. |
| `flow/nodes/internal.rs` | Rewritten; `route()` and `Internal` kept. |
| `flow/nodes/books.rs` | `AppStrategyBook` is a unit struct answering the not-available sentence. |
| `flow/nodes/data.rs` | `install_env` installs it without a strategy book. |
| `flow/triggers/feed.rs` | Replaced by a re-export of `flow::support::quote_feed`. |
| `flow/triggers/drive.rs` | `subscribe_order_updates` reads the broadcast channel instead of the bus. |
| `flow/triggers/runs.rs` | `paused()` reads `is_authenticated()`. |
| `flow/store/mod.rs` | `witness()` removed. |
| `flow/store/workflows.rs` | No AAD on encrypt/decrypt; the decrypted secret wrapped in `Secret`. |
| `flow/service/mod.rs` | `api_key()` answers the internal token. |
| `flow/service/webhooks.rs` | `base()` calls `public_base_url`. |
| `flow/executor/mod.rs` | `context()` reads `AppState::session_boundary()`. |
| `flow/contract/time.rs` | `market_tz()`, `set_market_tz_for_tests()` added (Task A3 completes the renames). |
| `flow/nodes/timing.rs` | `market_hours::effective_session_window` (module alias after the path rewrite). |
| `indicators_lib/mod.rs` | `pub mod candles; pub mod outcome;` |
| `indicators_lib/compute.rs` | `market_timestamp` for `ist_timestamp`. |

## Files of this app touched outside the carried tree

| File | Change |
|---|---|
| `src-tauri/src/lib.rs` | WP8 module anchor: `pub mod clock; pub mod events; pub mod flow; pub mod indicators_lib;` |
| `src-tauri/Cargo.toml` | WP8 deps anchor: `tower` (util), `tokio-util` (rt), `regex`, `zeroize`, `subtle`, `oa_core` (git, upstream's pin). |
| `src-tauri/src/security/mod.rs` | `pub mod secret; pub use secret::Secret;` and `SecurityManager::for_tests()`. |
| `src-tauri/src/state.rs` | Fields `clock`, `flow`, `symbols`, `internal_token`, `order_updates`; methods `now`, `blocking`, `spawn`, `session_boundary`; `load_symbol_cache` fills `symbols`. |
| `src-tauri/src/db/sqlite/mod.rs` | `SqliteDb::conn()` (outside the plan's file list; the smallest way to give the carried store a connection). |
| `src-tauri/src/events.rs` | New. |

## Consequences a trader will notice

- **Workflows do not need an API key.** Upstream runs a workflow with the
  trader's current API key and skips a run when there is none. Here every
  in-process `/api/v1` call carries the app's per-process internal token,
  so a run never asks for a key; the skip for "no API key" cannot happen.
- **Market time is America/New_York.** Every schedule time, time window,
  session date and run timestamp is Eastern (Task A3); a workflow exported
  from OpenAlgo web (IST) is reinterpreted, and the import reply says so.
- **Not available in this app**, answered as a node error, never a crash:
  Strategy P&L, WhatsApp alerts, absolute-strike legs; Telegram until
  Phase C; option expiry/strike lookups until Phase D.

## Environment notes for whoever rebuilds this

- `src-tauri/Cargo.lock` and `dist/` are git-ignored in this repository.
  A fresh worktree resolves the lock from scratch and may pick
  `parking_lot 0.13` (needs rustc 1.95); seeding it from the main
  checkout's lock avoids that. `tauri::generate_context!` needs `dist/`
  to exist, so `npm run build` (or a copy of a built `dist/`) must precede
  any cargo command.
- `npm run build` (`tsc -b`) fails on the base branch in files this work
  package does not touch (`src/api/trading.ts`, `PlaceOrderDialog.tsx`,
  `Historify.tsx`: `PlaceOrderRequest` lacks the bracket-order fields,
  `HistorifyDownloadJob` is not a `DownloadJob`). Recorded for the
  coordinator; not fixed here (out of scope).
