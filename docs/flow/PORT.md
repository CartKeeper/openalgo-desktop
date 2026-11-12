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
| --- | --- |
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
| --- | --- | --- |
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
| --- | --- |
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
| --- | --- |
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

## Task A2: the `AppState` handle and the headless harness

- `AppState` is now `AppState(Arc<AppStateInner>)` with `Deref`; every
  field lives on `AppStateInner`. The 250-odd Tauri commands keep
  `State<'_, AppState>` and compile unchanged (field access derefs).
- Carried Flow code keeps upstream's `Arc<AppState>` / `Weak<AppState>`
  signatures. The `Weak`s taken at start-up (`SymbolStoreMaster`,
  `AppCalendar`, `TriggerSide`) must point at an `Arc` that outlives
  them: `AppState::root()` makes that root once and finds it again while
  it lives (`AppStateInner::me`, a weak back-reference). The app manages
  the root beside the handle (Task A6); a test holds it in `TestCtx::ctx`.
  Per-call `Arc`s (`Arc::new(state.clone())` in a command) are fine: a
  run's task holds its own clone for the run's lifetime.
- `AppState::open(data_dir, OpenOptions)` opens a state on any folder and
  **starts Flow on it** (restore, interrupted-run close, trigger side), as
  upstream's `open` did and as the carried reopen tests expect;
  `AppState::new(&AppHandle)` builds only, and `lib.rs` starts Flow
  (Task A6). `OpenOptions { clock, brokers, app_handle }` replaces
  upstream's `{ keystore, clock, brokers }`.
- `state::testing::build(brokers, now) -> TestCtx { ctx, clock, dir }`
  keeps upstream's signature so the carried tests compile unchanged
  (the plan's zero-argument `build()` was not used; `db()` and
  `object()` exist as the plan lists them). The harness is a fresh
  database: analyzer mode off until a test turns it on, no user or broker
  session, random keys, no window. It must be built inside a tokio
  runtime (`#[tokio::test]`).
- `WebSocketManager` holds `Option<AppHandle>` and gains `headless()`;
  its emits go through one helper that drops the event when there is no
  window. Behaviour with a window is unchanged. (File outside the plan's
  list: `src-tauri/src/websocket/manager.rs`, the one field that blocked a
  headless state.)
- `[dev-dependencies]` gains `tokio = { features = ["full", "test-util"] }`
  for the carried `#[tokio::test(start_paused = true)]` tests (upstream's
  dev-dependency).

### Carried tests adapted in A2

| Upstream test helper | Here |
| --- | --- |
| `AuthService::setup(&app, "trader", ...)` | `flow::support::testkit::sign_in(&app)` (a user session) |
| `AuthService::lock_everything(&app)` | `testkit::sign_out(&app)` (session cleared, runs stopped, triggers kept) |
| `app.security.install(&Unlocked::random())`, `security.is_unlocked()` | removed / `is_authenticated()` (always unlocked here) |
| `OpenOptions { keystore, .. }` | `OpenOptions { app_handle: None, .. }` |
| `app.bus.queue_depths()`, `app.bus.publish(Event::OrderUpdate(u))` | `app.order_updates.receiver_count()`, `app.order_updates.send(u)` |
| `services::schemas::basket_order().load(row)` | the row's keys checked against this app's `BasketOrderItem` fields |
| `include_str!("src/lib/flow/constants.ts")` | read at run time; the five editor tests and `the_editor_and_the_run_share_their_lists` are `#[ignore]`d until the frontend arrives (Phase B) |

Tests retired (not applicable here; recorded in `docs/flow/parity.md`):
`executor/tests/run.rs::a_fresh_start_or_an_account_reset_stops_a_run_in_flight`
(upstream's account reset and fresh start), the calendar-table gate tests
`timing/tests.rs::the_gate_reads_the_apps_calendar`,
`timing/tests/gate.rs::{an_unreadable_calendar_is_shut,
an_exchange_without_hours_uses_the_default}` and
`triggers/tests/schedule_logs.rs::the_gate_reads_the_nse_calendar_when_no_exchange_is_named`
(upstream's holiday and timing tables; Task A3 tests the Eastern gate).

## Task A3: market time is America/New_York

- `flow::contract::time::market_tz()` is the one zone every Flow
  wall-clock time is read and written in (`DEFAULT_MARKET_TZ`,
  `America/New_York`); `market_now(clock)` replaces upstream's `ist_now`,
  `market_label()` gives the short name a sentence shows (`ET`, or `IST`
  under the test override). The plan's `OnceLock` became a `#[cfg(test)]`
  thread-local override (`set_market_tz_for_tests`), so the goldens and the
  tests ported from the web (recorded in IST) and this app's Eastern tests
  share one test process; `AppState::blocking` carries the override to the
  blocking pool.
- Renames (upstream → here): `ist_now` → `market_now`, `iso_ist` →
  `iso_market` (`contract/vars.rs`), `today_ist` → `today_market`
  (`nodes/history.rs`, `support/options.rs`), `ist_minutes` →
  `market_minutes` (`nodes/timing.rs`), `from_ist`/`ist` →
  `from_market`/`market` (`triggers/scheduler.rs`), `in_ist` → `in_market`
  (`indicator_service/records.rs`), `ist_stamp` → `market_stamp`
  (`indicators_lib/compute.rs`). `clock::ist_offset` had two readers, both
  now on `market_tz()`, so no `market_offset` exists; `IST_OFFSET_SECS`,
  `ist_offset` and `ist_slots_between` (no caller here) were not carried.
- Daylight saving in the scheduler (`from_market`): a wall time that
  repeats on the fall-back night fires once, on the later instant
  (upstream's `.latest()` rule); a wall time the spring-forward night
  skips fires the same minute past the hour once the clock has jumped
  (02:30 → 03:30 EDT). Tests: `triggers/scheduler/market_time_tests.rs`.
- The schedule trigger's market-hours gate
  (`nodes::timing::is_within_market_hours`) is async over the app: the
  broker's `MarketStatusService::get_clock` when it can be read (closed
  inside the regular hours is a holiday or an early close), else the
  regular 09:30–16:00 market-time window (`support::market_hours::
  regular_window`) with a warning in the log; the node's own start and end
  replace a side either way. `marketHoursExchange` is read but every US
  venue shares the one session (upstream read per-exchange calendar
  tables; `effective_session_window` and `DEFAULT_MARKET_HOURS_EXCHANGE`
  were removed with them). Tests: `nodes/timing/tests/eastern.rs`
  (spring-forward window, the logged fallback); the live-clock branch has
  no broker in the harness and is recorded in `docs/flow/parity.md`.
- A web workflow's import answers `notes: ["Schedule and time-window times
  are read as Eastern time in this app"]` when any node reads a wall-clock
  time (`transfer::reads_wall_clock_times`: a non-manual schedule, a Time
  Window, Time Condition or Wait Until node). Test:
  `service/tests/market_time.rs`.
- Sentences that named IST now name the market zone
  (`market_label()`): the past one-time schedule refusal and the
  missed-fire line of the restore.
- Carried IST tests set the zone in their own setup (the plan's rule):
  `nodes/testkit::{ist, ctx_at}`, `nodes/order_testing::Run::at`,
  `nodes/data_tests/support::ist`, `triggers/scheduler/tests::ist_at`,
  `triggers/tests::app`, `triggers/tests/schedule_logs::ist_at`,
  `service/tests::app`, `executor/tests/{run,w6_run}::app`, the
  `vars.rs` built-ins test, the indicator fixture loaders
  (`indicator_service/tests/support::json`, `indicators_lib/tests/
  support::json`: the web's package recorded them on an IST host) and the
  goldens replay (`goldens/replay::replay`).

## Task A4: nodes call this app's `/api/v1` handlers in process

- `webhook::server::router(state) -> axum::Router` is the one router
  (extracted from `WebhookServer::start`); `AppState::api_router()` builds
  it once per state for Flow. Each router has its own `RateLimiterState`,
  so Flow has its own share of the `/api/v1` limit and never spends an
  SDK client's (upstream gives Flow its own caller address for the same
  reason). The limiter, analyzer routing and Gate B are untouched.
- `flow::support::dispatch::run` hands the request to the router with
  `tower::ServiceExt::oneshot` inside a task the app owns, and reads the
  answer within `CALL_TIMEOUT` (120 s, the SDK's); a dropped node future
  never cuts an order off mid-flight. `Raw`/`Transport` are upstream's
  `mcp::dispatch` types.
- The credential is the per-process internal token, in the header
  `x-openalgo-internal-token`. The router's innermost layer
  (`internal_trust`) checks it in constant time and writes it into the
  JSON body as `apikey`, where every `/api/v1` request schema reads the
  key (a header-only scheme would fail JSON extraction before any check);
  a wrong token is refused 403; a request without the header passes
  untouched. `WebhookState::validate_api_key` accepts the token, and
  `service_key` hands the services `None` for it — the trust a Tauri
  command has (the services re-check any `Some(key)` against the
  `api_keys` table, which the token is not in). A trader never needs an
  API key for a workflow.
- `WebhookState` holds a `WeakAppState` (the router lives inside the
  state, so a strong handle would be a cycle) and an `Option<AppHandle>`
  for its frontend events (none for a headless state).
- `with_broker_cause`: this app's handlers answer a broker-less call with
  "Broker not connected" inside the service's error, or the key sentence;
  either becomes upstream's no-broker-session sentence at the node when no
  broker is logged in (Review Focus 5).
- `nodes::internal` (upstream's internal service entry): a scoped close is
  `/api/v1/openposition` then the opposite `/api/v1/placeorder` through
  the same router (the position service's own sentences for a flat
  symbol); absolute-strike legs answer 400 "Legs with an absolute strike
  are not available in this app: use an offset (ATM, ITM1, OTM1, ...)
  instead." (this app's options service sizes legs by offset only).
- Tests: `contract/client/app.rs` (funds through the handlers, a sandbox
  order with its id, the token gate, the no-broker cause) and
  `nodes/internal.rs` (the strike refusal, a flat and a held scoped close).

## Task A5: US exchanges and the `CNC` default

- `flow::support::FLOW_EXCHANGES` is the web's fourteen exchanges in the
  web's order followed by `US_EXCHANGES` (`NYSE, NASDAQ, AMEX, ARCA, BATS,
  OTC, US`); `flow::support::mapping::Exchange` has the same variants
  (`is_us()`, and `is_cash()` includes them). Every node that checks an
  exchange (`placeOrder`, `basketOrder`, the option nodes, the margin
  basket) and the validator's `Vocabulary::Exchange` read that one list.
- `node_contracts::default_product_for_exchange`: `NRML` on the web's
  derivative segments (unchanged), `CNC` on the US venues (the spec's
  default), `MIS` elsewhere (unchanged).
- A refusal's accepted list is this app's (sorted, so it begins `AMEX,
  ARCA, BATS, BCD, ...`). The web recorded its own fourteen, so the
  validator goldens (`validator/tests/goldens.rs::as_the_web_lists`) and
  the behaviour goldens (`goldens/replay.rs::as_the_web_lists`) compare the
  desktop's list with the US venues set aside; the carried orders test
  expects this app's list.
- Goldens the web recorded as refusals of a US venue (`got 'NASDAQ'`,
  `got 'NYSE'`) are this app's accepted path by design: the replay reports
  them `not applicable here` with the reason, per family (see
  `docs/flow/parity.md`).

## Task A6: store, migration, start and stop

- Migration `100_flow_core` (the program's reserved name) runs
  `flow::store::migrate` through `run_migration_fn`, a Rust-bodied
  variant of this app's `run_migration` under the WP8 anchors in
  `db/sqlite/migrations.rs`: run once, recorded in the `migrations` table
  like the SQL ones. The tables are upstream's `flow_workflows` and
  `flow_workflow_executions`, column for column (the webhook secret is
  encrypted with this app's `SecurityManager`, no AAD).
- `lib.rs` (the construction site): after `app.manage(app_state)` the root
  `Arc` is managed too (`app.manage(root)`, so Flow's start-time weak
  references stay alive), and `flow::start(&root)` runs inside Tauri's
  async runtime (the trigger side spawns its drivers): it closes the runs
  the last session left `running` (recorded `failed` with upstream's
  interrupted sentence, never resumed), installs the node services, the
  scheduler and monitors, and restores the active workflows.
- On `RunEvent::ExitRequested` the app calls `AppState::shutdown_with(10
  s)`: the trigger side stops, then every run in flight gets ten seconds
  to write its record before it is ended (upstream's `STOP_GRACE` of two
  seconds remains the tests' `shutdown()`).
- Tests: `flow/store/app_tests.rs` (the app's own runner creates the
  tables and records the migration; a run left running is closed at the
  next open and its workflow is free — Review Focus 3).

### `SqliteDb::conn()` is a pool checkout (a hang found and fixed)

Upstream's `ctx.sqlite.conn()` checks a connection out of an r2d2 pool,
so carried code holds one while it checks out another, or across an
await, without a thought (`refuse_updates` while a test holds its `conn`;
the executor's `blocking` reads while the test that started the run holds
one). Task A1 first implemented `conn()` as the guard of this app's single
`Mutex<Connection>`, and two carried tests deadlocked on it forever
(`nodes::http::tests::analyzer_mode_still_sends_the_request`, which
holds `conn` across `executor::execute(..).await`, and
`triggers::tests::lifecycle::activation_answers_500_when_it_cannot_persist_or_register`,
which calls `conn()` twice on one thread); the whole `flow::` run sat at
0% CPU with 808 of 810 tests finished. `SqliteDb` now owns an r2d2 pool of
four connections beside the app's own connection (`r2d2_sqlite` was
already a dependency): `conn()` returns a `PooledConn`, waits at most the
pool's 30 s for a free one and then answers an error, never hangs. Both
sources share the file in WAL mode with `busy_timeout` 5 s (the app's
connection gains that pragma: it waits for a Flow writer instead of
failing busy); the pooled connections have foreign keys on (Flow's run
history cascades with its workflow). The schedule gate's broker call
(`MarketStatusService::get_clock`) is time-boxed to 10 s
(`MARKET_CLOCK_TIMEOUT`) before the logged fallback: Flow's own awaits on
an external boundary all carry a timeout (`dispatch::run` 120 s, the feed
the node's own, the gate 10 s).

## Task A7: the support implementations

- `flow::support::public_url::public_base_url(app)`: the webhook
  settings' `ngrok_url` (trimmed of a trailing slash) when one is set, else
  `http://{webhook_host}:{webhook_port}` from the same settings (the
  defaults are `127.0.0.1:5000`); the local default when the settings
  cannot be read, logged. Upstream read its built-in tunnel's address.
- `flow::support::quote_feed::AppFeed` (`MarketFeed` for the subscribe and
  price nodes): this app has no stream Flow can subscribe to, so a frame
  is a one-shot quote through `QuotesService::get_quote` (modes `LTP` and
  `Quote`) or `get_market_depth` plus the quote's price (mode `Depth`),
  shaped as the web's websocket frame the nodes read
  (`{symbol, exchange, mode: 1|2|3, data: {...}}`; `data.prev_close` is the
  quote's `close`, a book carries `bids`/`asks` as `{price, quantity}` and
  the two totals). The node's own timeout bounds the call; `accept`
  decides whether the one frame counts; `unsubscribe` has nothing to give
  back; with no broker logged in every call answers "No broker is logged
  in, so there is no live market data feed." at once, and "OpenAlgo is
  shutting down." once the app is gone. The live path needs a broker and
  is not provable on the headless harness (`docs/flow/parity.md`).
- `values`, `numbers`, `order_vocab` and `testkit` were filled in Task A1
  (copies of upstream's helpers); their tests are the copied ones.

## Task A8: node families green, goldens replayed

- `cargo test --lib flow::` runs to completion (the deadlock of Task A6's
  note is gone) with every node family green and all six golden families
  matching; the per-family lines are in `docs/flow/parity.md`.
- Carried tests that pinned upstream's "no API key" branch were adapted
  to this app, where a run always has the app's own credential (Task A4):
  the fire now runs and the tests assert the executed outcome; the three
  whose subject was the key guard itself were retired. The holiday-table
  gate test was retired with the calendar tables (Task A3). The tests that
  read the Flow frontend sources are `#[ignore]`d until Phase B carries
  them; the two that read upstream's prompt document were removed
  (`validator/tests/editor_parity_docs.rs` and one test of
  `editor_parity.rs`). Every case is listed in `docs/flow/parity.md`.
- Review Focus 5 (a broker call with no broker logged in) is pinned at
  the client (`contract/client/app.rs::a_broker_call_without_a_broker_names_the_cause`,
  Task A4): the node reads upstream's no-broker-session sentence and the
  run fails at that node; the trigger side's own tests show the workflow
  stays registered after a failed run.

### `indicators_lib` in this app

- `discovery.rs`, `scan.rs` and `outcome.rs` (the Agent's tools) are
  `#[cfg(test)]` modules: only their tests run here.
- The Agent's dispatcher (`compute::compute`, `coerce`, `json`, parts of
  `registry`) is reached only by the library's goldens; the module carries
  `#![cfg_attr(not(test), allow(dead_code))]` with the reason, rather than
  deleting a tested half of a carried library. Flow's Indicator node uses
  `ta`, `registry::get_spec` and `compute::bind`.
- `Json::from_value` (referenced by nothing, tests included) was removed.
- Unused re-exports in `indicators_lib/mod.rs` were removed.

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

## Gates on the merged branch (2026-11-12)

After `git merge feat/historify-desktop-backend` (integration head
`57638b65`, WP1, WP6, WP5, WP7, WP2 and WP4 merged; the merge was clean,
no duplicate command names, migration names or crate keys), with
`npm ci` from the integration lock:

| Gate | Result | Whose |
| --- | --- | --- |
| `cd src-tauri && cargo test --lib` | `test result: ok. 1279 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 48.44s` (no compiler warnings) | passes |
| `npx vitest run` | `Test Files  4 failed \| 29 passed (33)`, `Tests  7 failed \| 223 passed \| 1 skipped (231)` | the integration tree's: `src/config/navigation.test.ts` (3), `src/components/ui/page-loader.test.tsx` (4) assert stale expectations; `MobileBottomNav.test.tsx` and `ui/button.test.tsx` import `jest-axe`, which no `package.json` declares. All four files are byte-identical to the base branch (23480b90) and to the integration branch; WP8 Phase A changes no frontend file. |
| `npm run build` | exit 2: the seven `tsc` errors above | the base branch's (unchanged on integration) |
| `npm run lint` | exit 1: `× Some errors were emitted while running checks.` (Biome errors in `components/dashboard/WatchlistCard.tsx`, `reports/ReportBuilderDialog.tsx`, `backtest/TradeTable.tsx`, `HelpButton.tsx`, `api/strategy.ts`, `pages/telegram/TelegramUsers.tsx` and five more; 153 warnings) | the integration tree's: none of the files is touched by WP8 Phase A |

The 12 ignored Rust tests are the Flow tests that read the frontend
sources (`docs/flow/parity.md`); they un-ignore when WP8 Phase B carries
those files.
