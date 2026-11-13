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

## Phase B — the Flow frontend (`wp8b-flow-ui`)

### Carried (Task B1)

- From the same commit (`52580437b0991774576272d007c442cd6eb820fd`), with
  `git checkout 52580437 -- <paths>` on 2026-11-12: `src/api/flow.ts`,
  `src/api/flowDesktop.ts`, `src/pages/flow/**`, `src/components/flow/**`,
  `src/components/desktop/{FlowConfirmDialogs, FlowEditorAccountControls,
  FlowLeaveGuard, FlowPanelNotes, FlowWebhookReachability}.tsx` (+ tests),
  `src/lib/flow/**`, `src/lib/desktopFlow.ts` (+ test),
  `src/stores/flowWorkflowStore.ts` (+ 3 tests), `src/types/flow.ts`,
  `src/types/flow.surface.json` (+ test): 168 files, 77 of them tests.
  `src/api/flow.test.ts` is carried in Task B2 with the invoke bridge it
  tests.
- Packages: `@xyflow/react ^12.10.0` (upstream's range; the lock resolves
  12.12.1, the plan's pin). `@uiw/react-codemirror`, `@uiw/codemirror-themes`
  and `@codemirror/lang-json` were already dependencies at upstream's ranges,
  and the carried tree imports none of them. Beyond the plan's four:
  `react-router ^7.12.0` declared (37 carried files import `react-router`,
  upstream's habit; `react-router-dom@7.12.0` already brought it into the
  lock, so no package was added), and the dev dependencies `jest-axe
  ^10.0.0` + `@types/jest-axe ^3.5.9` (7 carried tests use
  `src/test/a11y-utils.ts`, which this repository ships but whose package
  was never declared). The two base test files that import `jest-axe`
  (`components/ui/button.test.tsx`, `components/layout/MobileBottomNav.test.tsx`)
  now get past the import and still fail, on `document is not defined`:
  they have no jsdom pragma. They are the base branch's, not fixed here.

### Not carried

| Upstream path | Why |
| --- | --- |
| `src/lib/flow/indicatorOffsetWording.test.ts` | Two of its four cases read `docs/prompt/flow-import-format.md` and `docs/userguide/21-flow-visual-builder/README.md`, which this repository does not have (Phase D writes `docs/flow/user-guide.md`); the panel wording it also pins is pinned by `ConfigPanel.desktop.test.tsx`. |
| `src/utils/toast.ts`, `src/components/layout/AccountControls.tsx`, `src/api/tunnel.ts`, `src/hooks/useProfileMenuItems.ts`, `src/stores/updateStore.ts`, `src/components/auth/LogoutConfirmDialog.tsx`, `src/test/axiosAnswer.ts`, `vitest.config.ts`, `src/test/setup.ts` | App modules the carried tree imports or assumes; the adapters below stand in, or this app's own files do. |

### Adapters (new files of this app)

| File | Stands in for |
| --- | --- |
| `src/utils/toast.ts` (+ test) | Upstream's `showToast`: the same call shape `(message, category?, options?)`, every toast shown through sonner. This app has no per-category toast settings, so the category is accepted and unused. |
| `src/components/layout/AccountControls.tsx` (+ test) | Upstream's header account controls, imported by the carried `FlowEditorAccountControls`: the Navbar's own pieces (broker badge, mode badge, the Live/Analyze switch with its sandbox warning, the theme switch, the profile menu with this app's `profileMenuItems`, Docs, Logout), preceded by `AccountSwitcher`; `onNavigate` lets the editor's leave guard ask first. Copied from `Navbar.tsx`, which this WP may not edit, so a change to the Navbar's account controls is made here too. No Check for Updates (no in-app updater here) and no logout confirmation dialog (the Navbar has none). It logs out once, through the store's `logout()`; the Navbar sends the request twice (`authApi.logout()` and then the store's `logout()`, which sends it again) — reported, not changed. |
| `src/components/flow/testSetup.ts` | The part of upstream's `src/test/setup.ts` this repository's lacks: `ResizeObserver`/`IntersectionObserver` as real classes, Radix pointer-capture and `scrollIntoView` stubs. Imported by every carried `*.test.tsx`; nothing in the app imports it. |

### Hand edits to carried files (Task B1)

| File | Edit |
| --- | --- |
| every carried `*.test.tsx` (56 files) | Three lines prepended: `// @vitest-environment jsdom`, `import '@/test/setup'`, `import '@/components/flow/testSetup'`. This repository has no vitest config (upstream's `vitest.config.ts` sets jsdom and the setup file for every test); each jsdom test here declares its own. |
| `src/api/flowDesktop.ts` | `tunnelApi` and `onTunnelAddress` removed: `getWebhookInfoForTrader` is `getWebhookInfo` (the server already answers on `public_base_url`, Task A7). |
| `src/lib/desktopFlow.test.ts` | The case reading `src-tauri/src/services/schemas.rs` (`SUPPORTED_INTERVALS`) removed with its `node:fs` imports: this app has no such constant (the History node passes the interval through to `/api/v1/history`). |
| `src/pages/flow/FlowKeyboardShortcuts.pinned.test.tsx`, `src/components/flow/panels/ConfigPanel.options.test.tsx` | `:scope > div` selectors replaced by child traversal: this repository's jsdom 24 mishandles `:scope` (upstream runs jsdom 27). |
| `src/pages/flow/FlowIndex.test.tsx` | The test "shows the tunnel address while the built-in tunnel is online" removed with the `@/api/tunnel` mock (no tunnel API here). |
| `FlowIndex.w6.test.tsx`, `FlowIndex.pinned.test.tsx`, `ConfigPanel.{desktop, cancelOrders, triggers, titles, everyNode}.test.tsx` | The dead `@/api/tunnel` mocks removed. |
| `src/pages/flow/FlowEditor.test.tsx` | "offers Check for Updates in the profile menu" became "shows the Dashboard link and the profile menu with Logout" (the Profile entry asserted instead); the leave-guard test goes through the Profile entry (`/profile`) instead of upstream's mocked Positions entry; the `@/hooks/useProfileMenuItems` mock removed. |
| `src/pages/flow/FlowEditor.w6.test.tsx` | The logout test asserts a direct logout (no "Confirm Logout" dialog here); the `useProfileMenuItems` mock removed. |
| `src/index.css` (this app's file) | The `@theme inline` mappings `--color-buy`, `--color-sell`, `--color-node-*` added, and upstream's "FLOW EDITOR STYLES" block (through `.bg-surface-2`) appended; the `--buy`/`--sell` HSL triplets already existed in this app's base layer. |

### vitest here, against upstream's

- Under vitest, a `react-router-dom` re-export and a direct `react-router`
  import load as two module instances, so a hook from one cannot see a
  `Router` from the other (the first run failed every FlowEditor test on
  `useNavigate() may be used only in the context of a <Router>` from the
  adapter). Components the carried tree renders import from `react-router`,
  as the carried tests' `MemoryRouter` does. In the Vite bundle both names
  resolve to one module.
- CSS imports are stubbed (vitest's default `css: false`; upstream sets
  `css: true`); the one test that reads `src/index.css` reads the file.
- jsdom 24 here, 27 upstream: the two `:scope` edits above. `HTMLCanvasElement
  .getContext` "not implemented" lines on stderr are jsdom's, from passing
  tests.

### Task B2: `/flow/api` as Tauri commands

- `src-tauri/src/commands/flow.rs` (new): upstream's 22 routes
  (`server/routes/flow/{mod,triggers,lookups}.rs`, not carried) as `flow_*`
  commands, registered under the WP8 handler anchor in `lib.rs`
  (`pub mod flow;` under the commands anchor). Each command reads its
  arguments as the route read the request and calls the one service
  function the route called; the service's `Reply` is answered as
  `FlowReply { status, body }`. A JSON body from the editor goes through
  `Body::parse` again (serialised, then read), so `null`, `{}`, `[]`, `""`
  and `0` are the web's "no data" here too. Commands take the root `Arc`
  (`AppState::root()`), the state the service functions and Flow's
  long-lived tasks share. The plain functions the commands wrap are what
  the tests call (`commands/flow.rs` tests on the headless harness).
- Against the plan's signatures: `flow_option_strikes(params:
  HashMap<String, String>)` is the web's query map (`StrikeQuery::
  from_params` reads `underlying`, `optionType`, `expiry`, `expiryType`,
  `exchange`; the plan's `(underlying, exchange, expiry)` dropped two the
  carried leg builder sends); `flow_list_executions(id, limit: Option<u32>)`
  hands the limit on as the query string the web read;
  `flow_execute_workflow(id, body)` accepts and ignores the body, as the
  route did; `flow_symbol_lotsizes(body)` always has JSON (the route's
  "not JSON" case cannot arrive from `invoke`).
- `src/api/flow.ts`: the web's types, function names, signatures and query
  keys kept; every body is now `flowReply(invoke('flow_…', args))`.
  `flowReply<T>` resolves the body below 400 and throws `FlowApiError`
  above, with `message` read as upstream's web client read a refusal
  (`message`, then `error`, then `detail`, then a plain-text body) and
  `response.data` = the body, so the pages' `error.message` toasts and
  `failedRunLog(error)` (`lib/desktopFlow.ts`) work unchanged. A command
  that rejects (an `AppError`) is a 500 with its message. No wrapper for
  `flow_monitor_status`: no carried page calls the route.
- `src/api/flow.test.ts`: upstream's test rewritten over a mocked `invoke`
  (command and arguments per call, what each function hands back, the
  refusal rules). Its 401 (redirect to `/login`) and 403 (CSRF sentence)
  cases are not carried: no HTTP session or CSRF in the bridge.
- `src/components/desktop/FlowWebhookReachability.tsx`: already warned on a
  `127.*`, `localhost`, `0.0.0.0` or `[::1]` host; its remedy now names this
  app's setting (Admin → Server Settings → "Ngrok/Public URL") instead of
  upstream's built-in tunnel and Host Server URL. Its test and
  `FlowIndex.test.tsx` assert the new words.
- The 11 Rust tests Phase A `#[ignore]`d for want of the frontend sources
  (`flow/mod.rs` 1, `validator/tests/editor_parity.rs` 3,
  `nodes/options/tests.rs` 1, `nodes/legs/tests.rs` 1,
  `indicator_service/tests/catalogue.rs` 5) run again on the carried
  `src/lib/flow/constants.ts`, `src/lib/flow/customLegs.ts` and
  `src/components/flow/nodes/`, and pass; the `NEEDS_EDITOR` scaffolding
  went with the attributes. Phase A's gate counted 12 ignored: the twelfth
  is `nodes/http/client/tests.rs`'s proxy test, which
  `system_proxy_settings_are_ignored` runs in a child process with proxy
  variables set, and stays ignored by design. `docs/flow/parity.md`'s list
  "Ignored until the frontend arrives (Phase B)" is therefore resolved; that
  file is outside Phase B's list and was not edited (Phase D owns its final
  table).

### Task B3: routes, the sidebar entry and help

- `src/App.tsx`: `/flow` (the list) and `/flow/shortcuts` under `Layout`,
  `/flow/editor/:id` under `FullWidthLayout` (no navbar: the canvas has the
  viewport, and the editor's own header carries the account controls), as
  upstream mounts them; lazy pages like every other route.
- `src/config/navigation.ts`: `{ href: '/flow', label: 'Flow', icon:
  Workflow }` in the Strategy group for every region, and `isActiveRoute`
  keeps `/flow` lit on `/flow/editor/:id` and `/flow/shortcuts` (exact
  `/flow` or a `/flow/` prefix), as the Strategies entry does on its nested
  pages. `src/config/navigation.test.ts` gains a `describe` for both; its
  three stale assertions are the base branch's and were left alone.
- Help: `src/pages/flow/flowHelp.ts` (new) holds the three pages' items for
  the app's shared `HelpButton` (`src/components/HelpButton.tsx`, the
  floating trigger every page has), adapted from upstream's user guide
  chapter 21 in this app's words (AAPL on NASDAQ, whole shares, CNC on US
  venues, Eastern time and the 9:30 AM to 4:00 PM ET session, the Sandbox
  page, the Ngrok/Public URL setting). `src/pages/flow/flowHelp.test.tsx`
  renders each page and reads the panel.
- The help says only what this branch does. Two facts shaped it: the
  Sandbox page is in the sidebar for Indian brokers only, so the help points
  a US trader to the Orderbook, which shows the sandbox's orders in analyzer
  mode (`get_order_book` routes there); and the public webhook route, the
  order-update poller and Telegram are Phase C's, so a "Not connected yet"
  item says a webhook address does not receive alerts yet, an Order Update
  trigger does not fire, and a Telegram Alert node stops with "Telegram is
  not set up". Phase C replaces that item when it connects them.

### Bundle note: React Flow's own chunk (applied in Follow-up 3)

First reported (`vite.config.ts` was outside WP8's files), then applied
with the coordinator's authorization (Follow-up 3 below). Before it, the
`manualChunks` rule `id.includes('/react/')` also matched
`node_modules/@xyflow/react/`, so React Flow landed in `vendor-react`, the
chunk every page loads: `vendor-react` grew from 220.54 kB to 358.34 kB
(minified) with a 15.87 kB stylesheet, and because React Flow imports
`d3-zoom`/`d3-drag`/`d3-selection` (the `vendor-charts` rule), the entry
also preloaded `vendor-charts` (247.33 kB) on every page. Measured with
a copy of the config outside the repository, these two rules put React
Flow in a `vendor-flow` chunk (136.49 kB + 15.87 kB CSS) that only the
`/flow/editor` route loads, return `vendor-react` to 220.55 kB, and take
`vendor-charts` off the startup preloads, with no circular-chunk warning.
The first rule matters: without it Rollup's CommonJS helpers land in
`vendor-flow`, React's chunk imports them back, and Rollup warns
"Circular chunk: vendor-react -> vendor-flow -> vendor-react".

```ts
        manualChunks: (id) => {
          // Rollup's CommonJS interop helpers (a virtual module, outside node_modules):
          // with React's chunk, which every page loads, so a lazy vendor chunk never holds them.
          if (id.includes('commonjsHelpers')) {
            return 'vendor-react'
          }
          if (id.includes('node_modules')) {
            // Flow editor (React Flow) - only /flow/editor. Before the React rule:
            // '@xyflow/react/' contains '/react/'.
            if (id.includes('/@xyflow/')) {
              return 'vendor-flow'
            }
            // Core React - most stable, cached long-term
```

### Hand edits to carried files (Task B3)

| File | Edit |
| --- | --- |
| `src/pages/flow/FlowIndex.tsx`, `FlowEditor.tsx`, `FlowKeyboardShortcuts.tsx` | `<HelpButton items={…} pageTitle="…" />` as the first child of the page's root (the editor's inside `FlowEditorContent`, before the Replace dialog), with the two imports. |
| `src/pages/flow/FlowEditorMenu.pinned.test.tsx` | The header test's `getByText('Flow Editor')` scoped to the header bar: the help panel's title, in the DOM while closed, is "Flow Editor" too. |

### Review fix pass (after the whole-branch review)

| File | Edit |
| --- | --- |
| `src/lib/flow/constants.ts` | `EXCHANGES` gains this app's seven US venues after upstream's fourteen, in the run's order (`flow::support::US_EXCHANGES`: NYSE, NASDAQ, AMEX, ARCA, BATS, OTC, US), and `defaultProductForExchange` answers `CNC` on them (`US_VENUES`), as `node_contracts::default_product_for_exchange` does. Without them a US trader could not pick NASDAQ in any exchange field (the `{ }` text mode flips back to the select once the text has no `{{`), and a node imported on NASDAQ showed a blank exchange and MIS while the run sent CNC. New test `src/lib/flow/usExchanges.test.ts` reads the Rust list. |
| `src/lib/flow/constants.pinned.test.ts`, `components/flow/panels/OrderFields.test.tsx`, `components/flow/panels/ConfigPanel.triggers.test.tsx` | Their pinned exchange lists (upstream's fourteen) gain the seven US venues; the OrderFields test's name says so. |
| `src/api/flow.ts` (Task B2's) | `importWorkflow` also hands back the reply's `notes` and `migrations` when it has them (`ImportedWorkflow`). |
| `src/pages/flow/FlowIndex.tsx` | The import's success toast: with notes (the Eastern-time sentence), a warning that carries them; with upgraded legacy fields only, their count, as Replace from JSON counts them; otherwise upstream's sentence. Test `src/pages/flow/flowImportNotes.test.tsx`. Review Focus 1's note now reaches the trader, not only the reply. |
| `src/components/desktop/FlowLeaveGuard.tsx` | `askBeforeLeaving(action)` exported: runs the action at once, or asks with the editor's "Leave without saving?" prompt when the canvas has unsaved changes. `src/components/layout/AccountControls.tsx` sends Logout through it: upstream confirmed every logout with a dialog of its own (the reason the guard lets `/login` through), this app's header logs out at once, so Logout from the editor dropped unsaved edits without asking. Test `src/pages/flow/flowLogout.test.tsx`. |
| `src/pages/flow/FlowEditor.tsx` | Export Workflow saves through this app's `saveTextFile` (`src/lib/exportFile.ts` → the `save_export_file` command: the file goes to Downloads and a toast gives its path) instead of upstream's `<a download>`, which does nothing in Tauri's macOS webview, as `exportFile.ts` records. Found after the review, while checking the Definition of Done. `FlowEditor.test.tsx` and `FlowEditorMenu.pinned.test.tsx` assert the saved file's name and two-space JSON and that no anchor is clicked; their blob and anchor capture, and the now-unused `readBlob` helper, were removed. |
| `src/pages/flow/flowHelp.ts` | "What Flow is" says the true thing about Live mode: Flow orders reach `OrderService::place_order` with no acknowledged mode (`expected_mode: None`, as every `/api/v1` caller), so the app's per-order Live/Paper check does not apply; the help says a Live-mode workflow places real orders with no confirmation per order, instead of "the order checks apply". |

### Known deviations after Phase B (recorded, not changed)

- **Times on the Flow pages are upstream's.** The workflow list's dates
  (`src/lib/flow/listDate.ts`) are written in Asia/Kolkata, so a workflow
  saved after 1:30 PM ET (2:30 PM in summer) shows the next day's date,
  and the Execution Log's times are 24-hour (`ExecutionLogPanel.tsx`).
  CLAUDE.md asks for America/New_York, 12-hour with AM/PM: plan Task D4.
- **New nodes carried upstream's NSE defaults** (exchange NSE, a 09:15
  daily schedule gated 09:15–15:15 on NSE, a 09:15–15:30 Time Window,
  which in this app let a default schedule run before the 9:30 AM ET open)
  until Follow-up 2 below gave them this app's US defaults.
- **The carried Flow UI keeps upstream's styling, not this app's design
  tokens.** The Flow editor CSS block has literal colors (handle and node
  glow `#e879f9`/`#22d3ee`, edges `#888888`, the hovered edge `#ffffff`),
  the carried components use Tailwind palette colors directly
  (`bg-purple-500`, `text-amber-500`, `bg-green-500`, `text-red-500` and
  the like, hundreds of uses), and the editor's controls are mostly 32 px
  (`h-8`) with a few 28 px and 24 px ones (`h-7`, `h-6`) that are outside
  CLAUDE.md's button scale. Restyling upstream's 168 files was not part of
  the carry; it would be its own piece of work.
- **The editor's help trigger** (the shared `HelpButton`, fixed at the
  bottom right) covers the MiniMap's corner, or the bottom of the side
  panel when one is open. Where the editor's help icon goes is the user's
  call; the shared component is outside WP8's files.
### Follow-up 1: the `/api/v1` key (coordinator-authorized, after the review)

Two holes the review found, fixed in `webhook/handlers.rs`,
`webhook/server.rs` and `flow/support/dispatch.rs` (Phase A's and the app's
files, opened to this phase for it):

- **No key in a log line or an event.** The six order routes logged their
  request with `{:?}` (`info!("Place order request: {:?}", req)`) and eight
  routes sent it to the frontend in `api_*` events, `apikey` included: the
  user's own OpenAlgo API key on an outside call, the internal token on a
  Flow call. They now log and emit `redacted(&req)`, the request's JSON with
  every `apikey`, at any depth, as `"[redacted]"`.
- **The internal token works only in process.** It was accepted from any
  request's body (`validate_api_key`) or header (`internal_trust`), so the
  network listener, and a public URL in front of it, took it too.
  `dispatch::run` now marks every request it hands the app's router
  (`InProcessCall`, a request extension, which only code in this process
  can add); `internal_trust` refuses the token header, as a wrong key, on a
  request without the mark, and runs a marked request with the right token
  inside the task-local `IN_PROCESS_CALL`; `WebhookState::is_internal`
  honors the token only inside that scope, so over the network the token is
  checked like any other key and refused with exactly a wrong key's answer.
- Tests (`webhook/server.rs` `key_tests`, `webhook/handlers.rs`
  `redaction_tests`): the six routes' log lines with the key `k-123` carry
  no `k-123`; an order's `api_order` event with a user key carries
  `"[redacted]"`; the listener refuses the internal token in a header and in
  a body (no sandbox order); the in-process order path takes the token and
  neither its log nor its event carries it; nested keys are masked. The
  test-only `handlers::emitted` records what the handlers emit on the
  test's thread (a headless state has no window).

### Follow-up 2: US defaults for new nodes (coordinator-decided)

The defaults a new node gets in the editor, and the ones the run reads for
a field a saved node lacks, are this app's US market now (the coordinator's
decision under the user's delegation):

- **Start**: `time: '09:35'`, `marketHoursOnly: true`, `marketHoursStart:
  '09:30'`, `marketHoursEnd: '16:00'`, `marketHoursExchange: 'US'` (was a
  09:15 run gated 09:15–15:15 on NSE, which in this app let a default
  schedule run before the 9:30 AM ET open). **Time Window**: 09:30 to 16:00.
- **Equity and order nodes** whose default exchange was `NSE` start on `US`:
  Price Alert, Place Order, Smart Order, Split Order, Close Positions,
  Position Check, Price Condition, Get Quote, Get Depth, Multi Quotes,
  History, Indicator, Prior Period OHLC, Bar Offset, Open Position, Symbol,
  and the four streaming nodes. They store no product: it follows the
  exchange, so `US` reads CNC (`defaultProductForExchange`), on the face and
  in the panel as in the run.
- **Kept as upstream** until Phase D's US lookups (Task D2): the option and
  index nodes (`optionsOrder`/`optionsMultiOrder` NSE_INDEX with NRML,
  `optionSymbol`, `optionChain`, `syntheticFuture` NSE_INDEX, `expiry` NFO)
  and Margin (NSE: it needs an Indian broker). The Indian exchanges stay
  selectable beside the US venues.
- **The editor's fallbacks for a missing field** (`src/lib/flow/constants.ts`
  defaults, `ConfigPanel.tsx`'s Time, Start, End, Calendar, Start Time, End
  Time and every equity or order node's Exchange, `OrderFields.tsx`'s
  `ExchangeField` fallback, the Product control and the node faces'
  exchange, time and product) read the same values.
- **The run's fallbacks** read them too: `flow::node_contracts::{
  default_exchange, default_schedule_time, default_time_window}` replace the
  literal `"NSE"` of the 17 equity and order node fallbacks (`nodes/data.rs`,
  `history.rs`, `streaming.rs`, `conditions.rs`, `triggers.rs`,
  `symbols.rs`, `indicator.rs`, `orders.rs`), the scheduler's `"09:15"`
  (`triggers/scheduler.rs`) and the Time Window's `"09:15"`/`"15:30"`
  (`nodes/timing.rs`). Margin's fallback stays `"NSE"`. Under test, a run
  replaying OpenAlgo web (market zone set to IST, as the goldens and the
  tests ported from the web set it) reads the web's NSE, 09:15 and
  09:15–15:30, so that evidence holds unchanged: all six golden families
  still match.
- **Help** (`flowHelp.ts`): a new node starts on the US exchange, product
  CNC, with the Indian exchanges still in the list; a new Schedule runs at
  9:35 AM ET held to the session, and a new Time Window covers it.
- **Tests added**: `src/lib/flow/usDefaults.test.ts` (a new Start node's
  defaults, Time Window, every equity or order node on US with CNC, the
  option/index/margin nodes unchanged, Indian exchanges selectable);
  `src/components/flow/usDefaults.test.tsx` (the panel and the faces for a
  node saved without its exchange, times or product); Rust
  `node_contracts::us_default_tests`, `nodes::data_tests::us_defaults`
  (Get Quote with no exchange asks for US),
  `scheduler::market_time_tests::a_daily_schedule_without_a_time_runs_at_0935_eastern`,
  `timing::tests::eastern::a_time_window_without_times_is_the_us_regular_session`.
- **Carried tests updated** (they pinned upstream's defaults): frontend
  `constants.pinned.test.ts` (new-node defaults), `marketHoursDefaults.test.ts`,
  `orderNodeDefaults.test.ts`, `nodeFaces.pinned.test.tsx`,
  `nodeFaces.data.pinned.test.tsx`, `ConfigPanel.{data, desktop, indicator,
  logic, orders, streaming, triggers}.test.tsx` and `OrderFields.test.tsx`
  (the unset value read as US, 09:35, 09:30–16:00 or CNC; "stores the
  window edited" now types 09:45, since 09:30 is the default and React
  fires no change for it; the desktop test reads `default_exchange()` in
  `history.rs` instead of the literal); Rust `nodes/streaming/tests.rs` and
  `nodes/triggers/tests.rs` (ported from the web, their harnesses now set
  the IST web replay, as the other ported tests do).

### Follow-up 3: React Flow's own chunk

`vite.config.ts` gains the two rules in the bundle note above
(`commonjsHelpers` → `vendor-react` before the `node_modules` check,
`/@xyflow/` → `vendor-flow` before the React rule), with the coordinator's
authorization.

## Phase C — triggers and Telegram alerts (`wp8c-flow-triggers`)

Phase A carried the trigger side (scheduler, price monitor, order-update
monitor, start-up restore) and installs it at app start (`flow::start` ->
`TriggerSide::install`). Phase C proves it in Eastern time, fixes what the
proof found, connects what this app lacked (the public webhook route, an
order-book poller in place of upstream's event bus, Telegram), and adds the
Live-mode confirmation on activation.

### Task C1: schedules in Eastern time

- `triggers/tests/eastern.rs` (new): the plan's C1 list on the headless
  harness, market zone `America/New_York`, a manual clock, each fire stepped
  as the driver steps it (`Scheduler::tick`, then `run_scheduled`), so no
  test waits on wall time. Every 1 minute activated at 10:00:30 AM ET runs at
  10:01:02 and 10:02:02 AM ET (upstream's alignment, the minute plus 2 s) and
  not between; a daily 9:35 AM ET schedule runs once a day, at 13:35Z while
  the zone is EDT and 14:35Z after 2026-11-01 (Review Focus 2); a one-time
  10:05 AM ET schedule runs once and stands its workflow down; a fire while
  signed out is recorded with the reason and the next fire after sign-in
  runs; reopening the app (`AppState::open` on the same folder) registers an
  active schedule again and records its next fire.
- **Fix, hand edit to a carried file** (`triggers/scheduler.rs`,
  `Schedule::next_at_or_after`): an interval's next fire was computed on
  whole seconds, so for a time a few milliseconds after a fire (the tick asks
  from `now + 1 ms`) it answered that same fire. The driver, which re-ticks
  at most every 50 ms while something is due, then fired an interval
  schedule again and again for the first second after each fire: a short
  run ran again, and a Place Order node sent again. The answer is now never
  before the time asked. Test `a_fire_moves_an_interval_job_a_whole_interval_on`
  (failed first: the next fire stayed `15:03:02Z`).
- Against the plan: the "tokio 1 s tick task owned by `FlowModule::spawn`"
  is upstream's driver instead (sleeps until the next fire, at most 60 s,
  woken when a job is added, owned by `TriggerSide`, stopped by
  `TriggerHooks::stop`); and a signed-out fire is recorded as upstream
  records it (status `failed`, error "Skipped: OpenAlgo was signed out when
  this schedule fired, …"), not with a `skipped` status the run history
  does not know.
- Times: run records are stored in UTC (`flow_workflow_executions.started_at`
  and `completed_at`, `flow_workflows.next_run_at`); the lines inside a run's
  log carry market time (upstream's `iso_market`). Showing them in Eastern
  12-hour time is Phase D's Task D4.

### The flaky `lifecycle::a_monitor_run_logs_its_result`

- Cause, found from the failing run's own log: the run had finished (its
  one-shot watch was retired) and only two lines were missing, both
  `drive.rs` call sites the test thread itself reached. tracing-core caches,
  for the whole process, whether any subscriber wants a call site and the
  highest level any wants; while at most one subscriber is registered it
  asks only the current thread's, so a line first logged on a thread with
  no subscriber (another test's task) was cached as off everywhere until
  the next registration. A longer wait could never have seen it.
- Fix (`triggers/tests/support.rs`, `triggers/tests/lifecycle.rs`):
  `logged()` installs once per test process a global subscriber that wants
  every line and prints none (a bare `tracing_subscriber::registry()`), so
  no call site is cached off; and the test waits on the side's run tasks
  (`settle`, a join, no time bound) instead of polling the log for 5 s.
  Tests that capture with `flow::support::testkit::capture_logs` (outside
  Phase C's files) get the same protection once any `logged()` has run in
  the process.

### Task C2: price alerts on the quote poll

- The price monitor was carried: every 5 s while an alert is armed, each
  alert asks for its quote through this app's own `/api/v1/quotes` in
  process (`QuotesService::get_quote` on the broker session; Alpaca reads
  the symbol and ignores the venue, so `US` works) and applies its
  condition; while signed out the poll pauses (logged once); with no broker
  logged in each quote fails at debug level and nothing fires.
- **Hand edit** (`triggers/drive.rs`): `check_alerts(client: &dyn
  FlowClient)` split out of `check_prices` (and `check_alert` takes any
  `FlowClient`), so a test scripts the quotes. The poll's behaviour is
  unchanged.
- Tests (`triggers/tests/price_poll.rs`, Eastern time): "crosses above 190"
  on AAPL fires once when the quote goes 189 -> 191, not again at 192, and
  the one-time alert stands its workflow down; an every-time alert fires on
  each new cross and never while the price stays above; one run at a time
  per workflow (a second cross while the first run is queued is dropped);
  an alert past its one-hour window stands down without asking for a quote.
- Against the plan: the poll keeps upstream's in-process `/api/v1/quotes`
  call instead of calling `QuotesService::get_quote` directly; the handler
  calls it, with the key check, the rate limit and the reply shape the
  monitor reads.
- Not changed: `price_monitor::Alert::from_node` keeps upstream's `NSE` for
  a node with no exchange. It cannot be reached here: the strict validator
  refuses a Price Alert without an exchange before activation, and a new
  node stores `US`.
