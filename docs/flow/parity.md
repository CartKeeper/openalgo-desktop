# Flow parity — goldens, hooks and tests on this app

What of upstream OpenAlgo Desktop's Flow evidence holds here, after WP8
Phase A (branch `wp8a-flow-core`). The behaviour goldens
(`tests/fixtures/web/flow/goldens/`, 787 files, recorded from OpenAlgo
web's own executor) replay through this app's node registry and walk with
`cargo test --lib flow::goldens -- --nocapture`; each family prints one
line. Phase D (`wp8d-flow-parity`) turns this into the final table.

## Goldens, per family

The per-family lines of the last Phase A run are at the end of this file.

| Family | Total | Replayed | Matched | Needs a hook | Web evidence only | Not applicable here |
| --- | --- | --- | --- | --- | --- | --- |
| `orders` | 220 | 219 | 219 | 0 | 0 | 1 (`placeOrder__bad_exchange`) |
| `data` | 184 | 183 | 183 | 0 | 0 | 1 (`margin__position_bad_exchange`) |
| `logic` | 229 | 229 | 229 | 0 | 0 | 0 |
| `triggers` | 74 | 71 | 71 | 0 | 3 | 0 |
| `core` | 61 | 58 | 58 | 3 (`app`) | 0 | 0 |
| `workflows` | 18 | 18 | 18 | 0 | 0 | 0 |

Every replayed golden matched. "Web evidence only" is upstream's own
verdict for a scripted service that raised (parity CL-26: the desktop
client never raises). "Needs a hook" is upstream's verdict for the three
`core` goldens marked `special` (`already_running`, `workflow_missing`,
`no_execution_record`): they need the app's store behind the harness
(upstream's `app` hook, which upstream's harness does not provide either);
the executor's own tests pin those three behaviours here
(`executor/tests/run.rs`: one run per workflow at a time, a missing
workflow records nothing, a run that cannot be recorded does not run).

Every golden is replayed under IST (`set_market_tz_for_tests(Asia::
Kolkata)` in `goldens/replay.rs`): the web recorded them on an IST host,
and the clock field of every golden is an IST wall time.

### Hooks

Upstream's harness lists the hooks a golden may need (`HOOKS` in
`goldens/replay.rs`: `master_contract`, `strategy_book`, `http_stub`,
`feed`, `trading_calendar`, `validator`, `run_context`, `feed_session`,
`feed_exception`); all of them are provided by the carried harness. The
only hook no golden gets is `app` (the three `special` core goldens above),
the same as upstream.

### Not applicable here (by design)

This app accepts the US venues (`NYSE, NASDAQ, AMEX, ARCA, BATS, OTC, US`)
beside the web's exchanges (Task A5). The web recorded these scenarios as
refusals of a US venue; here the same node goes on to call the client, so
the replay reports them `not applicable (the web refuses a US venue this
app accepts)` rather than forcing a match:

- `orders/placeOrder__bad_exchange.json` (`got 'NASDAQ'`)
- `data/margin__position_bad_exchange.json` (`got 'NYSE'`)

Where the web refused a genuinely unknown exchange (`XYZ`, `XX`), the
desktop's message lists its own accepted exchanges (the web's and the US
venues, sorted); the replay compares that list with the US venues set
aside (`goldens/replay.rs::as_the_web_lists`), as the validator goldens do
(`validator/tests/goldens.rs::as_the_web_lists`).

## Carried unit tests that do not run here

| Test | Why | What covers it instead |
| --- | --- | --- |
| `flow/doc_tests.rs` (whole module) | reads upstream's `docs/prompt/flow-import-format.md` and user-guide chapter 21, which this repository does not have | Phase D writes `docs/flow/user-guide.md` |
| `contract/client/app/{tests,refusal_tests}.rs` | upstream's Agent test kit, mock broker and tunnel log capture | `contract/client/app.rs` tests on the headless harness (Task A4) |
| `nodes/internal/tests.rs`, `nodes/order_api_tests.rs`, `nodes/data_tests/app.rs` | `brokers::mock` and `mcp::dispatch` | `nodes/internal.rs` tests (Task A4); the order and data goldens |
| `nodes/alerts/client_tests.rs` | upstream's Telegram and WhatsApp services | Phase C's Telegram tests |
| `triggers/feed/tests.rs` | upstream's Samco streaming corpus; this app has no stream Flow subscribes to | `support/quote_feed.rs` tests (Task A7) |
| `executor/tests/run.rs::a_fresh_start_or_an_account_reset_stops_a_run_in_flight` | upstream's account reset and fresh start | `signing_out_stops_a_run_in_flight` (the same stop path) |
| `timing/tests.rs::the_gate_reads_the_apps_calendar`, `timing/tests/gate.rs::{an_unreadable_calendar_is_shut, an_exchange_without_hours_uses_the_default}`, `triggers/tests/schedule_logs.rs::the_gate_reads_the_nse_calendar_when_no_exchange_is_named` | upstream's per-exchange holiday and timing tables | `timing/tests/eastern.rs` (Task A3: the Eastern gate and its logged fallback) |
| `validator/tests/editor_parity.rs::every_json_example_in_the_import_format_is_a_valid_workflow`, `validator/tests/editor_parity_docs.rs` (whole module) | upstream's prompt document `docs/prompt/flow-import-format.md` | the validator goldens (`validator_goldens.json` includes every example of that document) |
| `triggers/tests.rs::a_holiday_that_does_not_list_the_exchange_leaves_it_open` | upstream's per-exchange holiday table | `timing/tests/eastern.rs` |
| `triggers/tests/lifecycle.rs::{activation_without_an_api_key_is_refused, the_poll_without_a_key_warns_for_each_alert, a_webhook_without_a_key_logs_the_webs_line}` | upstream's "no API key" guard; a run here always has the app's own credential (PORT.md, Task A4) | the key-less path does not exist; the guards around it are pinned by `activation_checks_its_guards_in_the_webs_order` |

### Carried tests adapted to this app's "a run always has a credential"

Upstream's headless harness had no API key, so a triggered run was
recorded as a skip; here the same run executes. Each test keeps its
subject and asserts the executed outcome:

- `triggers/tests.rs::a_webhook_needs_a_runnable_workflow` (was `…_a_key_and_…`): the strict validator's 400 only.
- `triggers/tests.rs::an_order_update_from_the_bus_reaches_its_watch`: signs out first, so the skip it asserts is the signed-out one (upstream's app had no account there).
- `triggers/tests/restart.rs::a_restart_runs_the_stored_fire_within_the_grace_and_records_the_next`: the fire's run is `completed`.
- `triggers/tests/schedule_logs.rs::{a_missed_fire_runs_nothing_and_one_within_the_grace_runs, a_failed_scheduling_read_keeps_the_stored_gate}`: the run's result line is the evidence the fire went on.
- `triggers/tests/service_logs.rs::{activation_checks_its_guards_in_the_webs_order, re_registration_refuses_a_workflow_with_no_trigger (was …_no_trigger_and_no_key), the_order_update_monitor_logs_a_watch_its_fire_and_its_result}`: the validator's refusal where the key guard stood; the run's result is `success`.
- `triggers/tests/price_logs.rs::the_monitor_stops_when_its_last_alert_is_removed`: the poll's sync points are its per-symbol failed-quote lines (no broker) instead of the per-alert "no API key" warnings.

### Ignored until the frontend arrives (Phase B)

`#[ignore]`d with the reason in the attribute; they read
`src/lib/flow/constants.ts`, `src/lib/flow/customLegs.ts` or
`src/components/flow/nodes/index.ts`:

- `flow/mod.rs::the_editor_and_the_run_share_their_lists`
- `indicator_service/tests/catalogue.rs::{the_editor_lists_the_supported_names, the_editor_file_reads_the_same_with_crlf_line_endings, the_editors_parameter_fields_are_the_librarys, the_editors_catalogue_is_the_webs, the_editors_parameter_labels_and_choices_are_the_webs}`
- `nodes/legs/tests.rs::the_editor_templates_match_the_generated_legs`
- `nodes/options/tests.rs::the_dropdown_and_the_resolver_list_the_same_commodities`
- `validator/tests/editor_parity.rs::{every_seeded_template_is_a_basket_the_validator_accepts, the_validator_matches_the_editor_registry, every_node_rendering_branch_handles_is_a_branching_type}`

### Needs a broker (not provable on the headless harness)

- `support/quote_feed.rs`: the live path of `first_frame` (a quote or a
  book through `QuotesService` needs a broker session). The frame
  builders and the no-broker answer are tested; the node's end-to-end
  subscribe is the user's check in Phase B.
- `nodes/timing.rs::is_within_market_hours`: the live-clock branch
  (`MarketStatusService::get_clock`); the regular-session fallback and
  the override rules are tested.

## Per-family lines (Task A8)

`cargo test --lib -- flow::goldens --nocapture --test-threads=1`, branch
`wp8a-flow-core`, after Task A8:

```
goldens core: 61 total, replayed 58, matched 58, not built 0, needs a hook 3, web evidence only 0, not applicable here 0
goldens data: 184 total, replayed 183, matched 183, not built 0, needs a hook 0, web evidence only 0, not applicable here 1
  not applicable (the web refuses a US venue this app accepts): data/margin__position_bad_exchange.json
goldens logic: 229 total, replayed 229, matched 229, not built 0, needs a hook 0, web evidence only 0, not applicable here 0
goldens orders: 220 total, replayed 219, matched 219, not built 0, needs a hook 0, web evidence only 0, not applicable here 1
  not applicable (the web refuses a US venue this app accepts): orders/placeOrder__bad_exchange.json
goldens triggers: 74 total, replayed 71, matched 71, not built 0, needs a hook 0, web evidence only 3, not applicable here 0
goldens workflows: 18 total, replayed 18, matched 18, not built 0, needs a hook 0, web evidence only 0, not applicable here 0
```
