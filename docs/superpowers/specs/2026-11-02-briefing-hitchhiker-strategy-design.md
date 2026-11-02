# Briefing Strategy ("Hitchhiker") — Design Spec

**Date:** 2026-11-02
**Status:** Ready for user approval. Data sources verified live 2026-11-02 (see Definitions). No open questions.
**Scope:** Turn the 10-Minute Briefing from a one-day status report into a standing strategy: grade every held position as a "ride", rotate capital out of dormant positions into the best opportunity found, and attach a good-till-cancelled trailing stop to every ride. Covers the Rust briefing command, the copilot tool set, the system prompt, the trade-recommendation format, the affordability reconcile, and the Review Recommendations modal.

---

## Intent

The user's words: the portfolio should move uphill, not sit at the same number. Each stock is a ride. You get on when it is moving, stay on while it climbs, and the moment it stalls you get off and stick your thumb out again. A ride does not have to be spectacular; a stock climbing at 5% beats one wobbling inside a 3% band. The scoreboard is total account value week over week. The failure being fixed is the one the account has now: oscillating between roughly $18,500 and $19,500 with no net progress.

Today's briefing cannot do this. It is a single prompt that reads today's quotes, sector performance, headlines and a screener run, writes seven sections and a list of same-day actions, and forgets everything. It is forbidden by its own instructions from recommending a buy that does not fit settled cash, so it can never say "sell X to fund Y". The frontend affordability check enforces the same rule independently. Every order it proposes defaults to one-day validity at the broker, so every stop dies at the close. A rule added on 2026-06-01 forbids it from citing analyst price targets, so it cannot say what a stock is expected to do.

## Goals

1. Every morning, grade each held position: where you got on, how far it has taken you, whether it is still moving, and whether it is protected.
2. Name dormant positions and propose moving that capital into the best ride available, as one approvable proposal (sells plus buy plus protection) in the Review Recommendations modal.
3. Give every ride, new or existing, a good-till-cancelled trailing stop sized to the stock's own volatility and potential, shown with the number and the reason.
4. Show the scoreboard: account value now against one week ago and one month ago.
5. Let the model see price history and analyst targets so "dormant" and "expected to rise" are grounded in data, not training memory.

## Non-goals

- Automatic order placement. Nothing places without the user approving it in the modal, exactly as today (Gate B unchanged).
- Intraday or multi-briefing-per-day behavior. One briefing per generate click, as today.
- Changing the copilot chat's behavior beyond what it inherits from the shared system prompt and shared affordability code.
- Options, shorting, margin, fractional shares. Whole shares, long only, as today.
- A persistent "thesis journal" or briefing history table. Ride state is derived fresh from the broker each morning.

## Anti-goals (would make this a failure even if it "works")

- A rotation proposal whose buy gets silently shrunk or dropped by the affordability check because the check did not count the sell proceeds in the same proposal.
- A trailing stop placed as a DAY order. The user has explicitly said the stop must not need re-adding every day.
- Calling a position dormant when it was bought a week ago.
- A flat 3% trail on a stock that moves 3% on a normal day. The user said: if it has potential, leave it room and say so.
- Any briefing output that still refuses to state analyst targets or expected upside.

---

## Decisions the user made (verbatim intent, do not reinterpret)

| Topic | Decision |
|---|---|
| Dormant | Net move within ±3% over the last 30 trading days, position still above cost, analyst consensus target within 5% of the current price. |
| Rotation trigger | Dormant capital is redeployed any time dormant positions exist, not only when cash is short. A modest improvement (3% band to a 5–7% climber) still counts. |
| Rotation size | "Dump" means the whole dormant position, not a trim. |
| Exit order | Trailing stop (follows the high), not a fixed stop under cost. |
| Stop lifetime | Good-till-cancelled. Must not need to be added every day. |
| Stop placement | Bundled with the buy in the modal; one approval places both. Nothing places without approval. |
| Existing positions | Positions without a stop get one proposed too, until approved or dismissed. |
| Trail sizing | Sized to the stock's typical swing and its potential. A high-potential stock gets room and the briefing notes why. |
| Price-target rule | The June 2026 rule banning price-target language is removed. The model may say "analysts see it at $X" and name expected upside. |
| Data wiring | Wire whatever is needed for price history and analyst targets. Build entry-date tracking if needed. |
| Before the open | Placing a rotation while the market is closed is refused with "Market opens at 9:30 AM ET." No place-now-finish-later. |

---

## Definitions (exact; these are the builder's constants)

All windows are in **trading days**, computed from daily bars.

### Daily bars
Daily closes come from FMP `/stable/historical-price-eod/light?symbol=&from=&to=` (verified live; fields `date`, `price`, `volume`). If FMP fails for a symbol, fall back to the existing `YahooClient::get_historical(symbol, "1d", range)`. The briefing never mixes sources within one symbol's window. **Designer's call**: FMP first because the user pays for it and it is a supported endpoint; Yahoo's chart API is unofficial.

### Daily swing
Mean of the absolute daily close-to-close percentage change over the last 30 trading days. Expressed as a percentage, rounded to 0.1.

### Analyst consensus target and upside
Sources are tried in this order; the first that yields a number wins, and the briefing names which one it used:
1. `targetConsensus` from Financial Modeling Prep `/stable/price-target-consensus?symbol=` (payload: `targetHigh`, `targetLow`, `targetConsensus`, `targetMedian`).
2. The median of `priceTarget` values from FMP `/stable/price-target-news?symbol=&limit=` published within the last 90 calendar days, if at least 2 exist. (Fields confirmed: `symbol`, `publishedDate`, `analystName`, `analystCompany`, `priceTarget`, `adjPriceTarget`, `priceWhenPosted`, `newsTitle`, `newsURL`, `newsPublisher`, `newsBaseURL`.)
3. Yahoo Finance `quoteSummary` module `financialData`: `targetMeanPrice` with `numberOfAnalystOpinions` as the count.

Analyst count for the last 90 days = `lastQuarterCount` from FMP `/stable/price-target-summary?symbol=` (payload also carries `lastMonthCount`, `lastQuarterAvgPriceTarget`, `lastYearCount`, `allTimeCount`). If unavailable, count the source-2 records.

If all three target sources fail, consensus is **unknown**.

Upside % = (consensus target − current price) / current price × 100, rounded to 0.1.

**Verified 2026-11-02 with the user's FMP key** (from the OpenAlgo Desktop project in Sidekick, `FMP_API_KEY`):

| Endpoint | Result |
|---|---|
| `/stable/profile` (control) | 200, data |
| `/stable/price-target-consensus` | 200, data (PFE consensus 29.17, high 33, low 26) |
| `/stable/price-target-summary` | 200, data (PFE: 4 targets last quarter, 19 last year) |
| `/stable/price-target-news` | 200, data |
| `/stable/grades-consensus` | 200, data (buy/hold/sell counts) |
| `/stable/historical-price-eod/light` | 200, daily bars |
| `/stable/price-target` | **404, empty** |

The last row matters beyond this design: `FmpClient::get_price_targets` in `src-tauri/src/providers/fmp.rs` calls `/stable/price-target`, which is dead, so the Analyst page's price-target panel is empty today. The builder repoints that provider function to `/stable/price-target-news` (same fields) as part of wiring source 2. That fixes the Analyst page as a side effect; no other change to that page.

### Ride age
Trading days since the position was opened. "Opened" = the fill timestamp of the earliest BUY fill in the current unbroken holding period for that symbol (i.e. after the last time net quantity for the symbol was zero). Source: Alpaca account activities of type `FILL`. For manual positions in `portfolio_positions`, use `added_at`. If age cannot be determined, it is **unknown**.

### Ride status (one per held position, computed deterministically in Rust, never by the model)

Evaluated in this order; the first match wins.

| Status | Condition |
|---|---|
| **Too new to judge** | Ride age < 30 trading days, or age unknown. |
| **Falling** | Current price below average cost. |
| **Dormant** | Net move over the last 30 trading days within ±3.0%, **and** current price ≥ average cost, **and** consensus upside ≤ 5.0% (unknown consensus counts as ≤ 5.0% for this test, and the briefing must say "no analyst target available" when that is the case). |
| **Flat, with upside** | Net move within ±3.0%, price ≥ cost, consensus upside > 5.0%. Not dormant: analysts still see room. Hold. |
| **Pulling back** | Net move below −3.0%, price ≥ cost. Not dormant and not a sell by itself; the trailing stop is the exit. The briefing names it so a slide above cost is never labeled "climbing". |
| **Climbing** | Net move above +3.0%. |

Only **Dormant** feeds the Rotation section. The other statuses are labels for the Ride Report. **Designer's call** on the two added rows: the live check below showed NVDA down 7.9% over 30 days while still above cost, which the original four-row table would have called "Climbing".

**Live check of the definitions, 2026-11-02, using FMP data and the average costs from today's briefing** (ride age assumed ≥ 30 days for this check):

| Symbol | Price | Cost | P&L | 30-day net | Daily swing | Consensus | Upside | Targets (90d) | Status | Trail |
|---|---|---|---|---|---|---|---|---|---|---|
| PFE | 30.52 | 25.71 | +18.7% | +10.3% | 1.0% | 29.17 | −4.4% | 4 | Climbing | 3.0% |
| ABT | 106.20 | 90.60 | +17.2% | +3.5% | 1.2% | 121.36 | +14.3% | 9 | Climbing | 3.0% |
| NVDA | 204.64 | 199.91 | +2.4% | −7.9% | 1.5% | 339.17 | +65.7% | 25 | Pulling back | 4.4% |

No position is dormant today by the approved definition. PFE is past its analyst consensus but still climbed 10% in 30 days, so it keeps its ride and its 3% trailing stop is the exit. NVDA's trail is widened to 3× its swing because consensus upside is above 15%.

"Net move over the last 30 trading days" = (close today − close 30 trading days ago) / close 30 trading days ago × 100.

### Protected
A position is protected if the open-orders list contains a working SELL order for that symbol with order type `TRAILING_STOP` and quantity equal to the held quantity. Any other stop (DAY stop-market, partial quantity) does not count as protected; the briefing says what exists and proposes the replacement **as a cancel of the old order followed by the new stop** (see the `cancel` role below).

Open orders for this check come from a dedicated open-orders query (Alpaca `status=open`), not the Order Book's `status=all&limit=100` window, so an old working GTC stop never drops out of view and looks missing. The `Order` struct must carry `trail_percent` and the stop's high-water mark so the Ride Report can show an existing stop's trail.

### Shares pledged to working sell orders
Alpaca holds shares against every working sell order. Once every ride has a GTC trailing stop, a rotation's `fund` sell of that symbol, or a replacement `protect` stop, is rejected with "insufficient qty available" unless the existing sell order is cancelled first. The design therefore treats cancels as first-class actions the user sees and approves (next section). The Ride Report lists, per position, the working sell orders that would have to be cancelled for a `fund` sell or a replacement stop, by order id.

### Trail percent (for a proposed trailing stop)
1. `trail = 2 × daily swing`.
2. If consensus upside ≥ 15.0%, `trail = 3 × daily swing` and the briefing must state that the trail was widened because of the stock's potential.
3. Clamp: minimum 3.0%, maximum 15.0%. Round to 0.1.
4. If daily swing cannot be computed (insufficient history), trail = 3.0% and the briefing says so.

The briefing shows the number and the reason for every proposed stop, in plain words, e.g. "Trail 8.0%: NVDA moves 3.3% on a normal day, and analysts see 40% upside, so it gets room."

### Scoreboard
Account equity now, at the close one week ago (5 trading days), and at the close one month ago (21 trading days), from the broker's portfolio history (`get_portfolio_history`, period `1M`, timeframe `1D`). Also the realized P&L from closed rides in the last 30 days if available from activities; if not available, omit that line rather than estimate.

---

## Architecture

Follows the existing pattern: Rust computes facts deterministically and injects them into the prompt; the model writes the narrative and emits structured actions; the frontend reconciles those actions against the account before display and again at submit.

### 1. Data wiring (Rust)

**New copilot tools** (declared alongside the existing tool schemas in `src-tauri/src/providers/anthropic.rs`, dispatched in `CopilotService::execute_tool_call`, summarized in `summarize_tool_result`):

- `get_price_history` — input `{ symbol, range }` where range is one of `1mo`, `3mo`, `6mo`, `1y`. Daily bars per the **Daily bars** definition (FMP EOD first, Yahoo fallback). Returns the bars plus the derived numbers so the model does not have to compute them: `net_move_30d_pct`, `daily_swing_pct`, `high_30d`, `low_30d`, and `source`.
- `get_analyst_targets` — input `{ symbol }`. New provider functions `FmpClient::get_price_target_consensus` and `FmpClient::get_price_target_summary`, plus the repointed `get_price_targets` (now `/stable/price-target-news`). Returns consensus, median, high, low, `targets_last_90d` (from the summary), the 5 most recent individual targets with analyst, company, date and headline, the derived `upside_pct`, and `source`. Yahoo `financialData` is the fallback per the Definitions.

Both tools are available to the copilot chat too, because the tool list is shared. That is intended.

**Ride Report** (new, computed in `generate_briefing` before the prompt is built, in a new module `src-tauri/src/services/ride_report.rs` so it is unit-testable without the model):

For each held position (live broker positions first, manual `portfolio_positions` fallback, as today), gather:
- symbol, quantity, average cost, current price, unrealized P&L in dollars and percent
- ride age in trading days (from activities; see Definitions)
- net move over 30 trading days, daily swing (from Yahoo history)
- consensus target, upside % (from FMP)
- protected flag and, if a stop exists, its type, quantity, trail or trigger
- status per the Ride status table
- proposed trail percent per the Trail percent rule (computed for every position regardless of status, so the model can quote it)

Also compute the Scoreboard figures.

Every one of these lookups is best-effort per position: a failed lookup yields "unknown" for that field, never a failed briefing. The Ride Report is rendered into the prompt as a table with an explicit legend, and also returned in the `generate_briefing` response as structured JSON (`ride_report` field on `CopilotResponse` or a sibling struct) so the frontend can render it without parsing markdown.

**Entry dates.** Extend `get_activities` to accept `activity_types=FILL` and page back far enough to reconstruct the current holding period for each held symbol (walk fills newest-first per symbol, accumulating signed quantity, until the running total reaches the held quantity; the oldest fill in that run is the entry). Cache the computed entry date per symbol+quantity in the SQLite `settings` table (or a small new table `ride_entries(symbol, quantity, entered_at, computed_at)`) so the activities walk is not repeated every morning. Invalidate when the held quantity for that symbol changes. Never delete rows from existing tables.

### 2. Prompt changes (Rust, `generate_briefing` and `build_system_prompt`)

**System prompt.** Remove this bullet from `build_system_prompt`:

> Replace price-target language ("room to run to $X") with range + base rate … No single price target stated as an expectation.

Replace it with:

> When you cite an analyst target or expected upside, name the source (consensus of N analysts, or the firm) and the as-of date. State targets as what analysts expect, not as a guarantee.

The rest of the 2026-06-01 responsible-advice block stays (as-of timestamps on prices, dollar-downside notes, the intraday-spike warning, the AI self-disclosure). **This is the designer's call**: the user said "if I put that rule in there, ditch it" about the price-target rule specifically. If the user wants more of that block removed, that is a one-line change.

**Briefing prompt.** Replace the current section list with the following, in this order. Keep the existing tool-gathering steps and add: call `get_analyst_targets` and `get_price_history` for every New Stock Corner candidate before ranking them.

1. **Scoreboard** — account value now, one week ago, one month ago, each with the dollar and percent change. One sentence on whether the game is working.
2. **Market Pulse** — unchanged.
3. **Ride Report** — one row per position: symbol, got on (date and price), distance traveled (P&L %), 30-day net move, daily swing, analyst target and upside, status, protection. Rows come straight from the injected Ride Report; the model must not recompute or contradict them. One line of commentary per position.
4. **Rotation** — present whenever at least one position is Dormant. Name each dormant position with its evidence in one line ("PFE: net move +1.2% over 30 days, analysts see +2% upside, up 18% from your cost. We've made what we can."). Name the single best ride found today (from New Stock Corner), with its evidence: analyst consensus target and upside, 30-day move, catalyst. Show the money: proceeds from the sells at current prices, plus available cash, equals the budget; whole-share quantity of the buy; leftover cash. Then the proposed trailing stop for the new ride with its trail percent and reason. If there is no dormant position, this section says so in one line and why (every ride is still climbing / too new to judge / falling and handled in Risk Radar). If a dormant position exists but no candidate beats it on upside, say that too and propose nothing.
5. **Protection** — one line per position that is not protected, with the proposed trail percent and reason. One line per position that is protected ("NVDA: trailing stop 8.0% working, GTC"). This is where existing-position stops are proposed.
6. **Alerts & Watchlist** — unchanged.
7. **Risk Radar** — unchanged in purpose, but any stop it recommends is a GTC trailing stop per the trail rule, never a DAY stop.
8. **Deep Dive** — unchanged.
9. **New Stock Corner** — unchanged in format, plus: every pick shows analyst consensus target, upside %, and 30-day move. Picks are ranked by upside. The top-ranked affordable pick becomes the Rotation target when a rotation exists. Picks that exceed cash **alone** are no longer "watch only" if a rotation funds them.

**Rotation selection rule for the model** (stated in the prompt): the rotation target must have consensus upside at least 10 percentage points higher than the dormant position being sold, must have `targets_last_90d` ≥ 3, and must not be up more than 10% intraday (the existing spike rule applies). If no candidate qualifies, no rotation.

### 3. Trade recommendation format (shared between model, parser, and modal)

Extend the `ACTIONS_JSON` schema and `OrderRecommendation` (`src/types/actionQueue.ts`):

```ts
validity?: 'DAY' | 'GTC'        // default DAY; trailing stops MUST be GTC
group?: string                   // e.g. "rotation-1", "protect-NVDA"
role?: 'cancel' | 'fund' | 'buy' | 'protect' | 'standalone'
cancelOrderId?: string           // required when role is 'cancel'
trailPercent?: number            // already exists; required for TRAILING_STOP
```

Rules the prompt states and the parser (`src/lib/parseActions.ts`) enforces:
- A `TRAILING_STOP` with no `validity` is set to `GTC` by the parser. A `TRAILING_STOP` with `validity: 'DAY'` is corrected to `GTC` by the parser.
- A `TRAILING_STOP` must carry `trailPercent` > 0 and quantity equal to the held (or to-be-bought) quantity; the parser does not fabricate a trail, it drops the action and the modal notice says a stop was dropped for a missing trail.
- A `cancel` action carries `cancelOrderId`, the symbol, and a rationale ("cancel the DAY stop at $192 so the GTC trailing stop can replace it"). The parser drops a `cancel` with no order id. The model may only emit cancels for order ids that appear in the Open Orders block of its prompt; the reconcile drops any cancel whose id is not currently open.
- A rotation is one `group`: `cancel` legs first (one per working sell order on a symbol being sold or re-protected), then the `fund` sells, then the `buy`, then the `protect` trailing stop for the bought symbol. Order within the JSON array is the placement order.
- A protection group for an existing position is `cancel` (if an old stop exists) then `protect`.
- Rotation `buy` orders are `MARKET` (so proceeds deploy immediately and the protective stop can attach the same session). **Designer's call.**
- Cancels are first-class cards in the modal, under the same Gate B acknowledgment as everything else. Nothing is cancelled silently. **Designer's call**, made to keep the user's rule that nothing happens at the broker without approval.

Example the prompt shows the model:

```json
[
  { "symbol": "PFE", "exchange": "NYSE", "side": "SELL", "quantity": 11, "orderType": "MARKET", "product": "CNC", "price": 0, "validity": "DAY", "group": "rotation-1", "role": "fund", "rationale": "Dormant: +1.2% net over 30 days, analysts see +2% upside. We've made what we can." },
  { "symbol": "BAC", "exchange": "NYSE", "side": "BUY", "quantity": 13, "orderType": "MARKET", "product": "CNC", "price": 0, "validity": "DAY", "group": "rotation-1", "role": "buy", "rationale": "Consensus target $58 (+15%), 14 analysts; +6% over 30 days; funded by PFE proceeds + cash." },
  { "symbol": "BAC", "exchange": "NYSE", "side": "SELL", "quantity": 13, "orderType": "TRAILING_STOP", "product": "CNC", "price": 0, "trailPercent": 4.2, "validity": "GTC", "group": "rotation-1", "role": "protect", "rationale": "Trail 4.2%: BAC moves 2.1% on a normal day." },
  { "symbol": "NVDA", "exchange": "NASDAQ", "side": "SELL", "quantity": 4, "orderType": "TRAILING_STOP", "product": "CNC", "price": 0, "trailPercent": 9.9, "validity": "GTC", "group": "protect-NVDA", "role": "protect", "rationale": "Trail 9.9%: NVDA moves 3.3% on a normal day and analysts see 40% upside, so it gets room." }
]
```

### 4. Affordability reconcile (`src/lib/affordability.ts`)

`reconcileRecommendationsWithAccount` changes from "buys are capped to available cash" to "buys are capped to available cash **plus the proceeds of liquidating sells in the same batch**":

- Walk the list in order. A liquidating sell (`side: SELL`, order type `MARKET` or `LIMIT`, not a stop) adds `quantity × price` to the running budget, where price is the limit price for LIMIT or the live quote for MARKET. Stops (`SL`, `SL-M`, `TRAILING_STOP`) add nothing.
- Buys consume the running budget exactly as today (whole shares, cumulative).
- Sells and stops are still capped to held shares as today. A `protect` stop for a symbol being bought in the same batch is capped to the **bought** quantity, not the currently held quantity (which may be zero).
- The notice names what changed, as today, and additionally says when a buy was funded by sells: "BAC ×13 is funded by selling PFE (about $334) plus $351 cash."
- Removing a `fund` sell card in the modal re-runs the reconcile immediately (not only at submit) so the dependent buy shrinks or drops in front of the user with the notice updated. Removing the `buy` card removes its `protect` stop too.
- Removing the `buy` card does **not** remove the `fund` sells. The user said dormant positions get dumped even when the capital has nowhere better to go. The group header changes to "Sell PFE to cash" so the user sees what they are now approving.
- Removing a `fund` sell card removes the `cancel` legs that existed only to free that symbol's shares.

The copilot chat shares this function and inherits the behavior. That is intended.

### 5. Review Recommendations modal (`src/components/trading/ActionReviewModal.tsx`)

- Cards with the same `group` render under one group header: for a rotation, "Rotation: sell PFE → buy BAC"; for protection, "Protect NVDA". Ungrouped cards render as today.
- The `validity` of each order is passed through to `placeBasketOrder` instead of the hardcoded `'DAY'`. Trailing stop cards show "GTC" in their type badge.
- The Gate B cost line shows sell proceeds alongside buy cost when a batch contains liquidating sells.
- Visual spec for the group header and badge follows the project design standards (16px card padding, 12px radius, caption typography). No other layout changes.

### 6. Placement sequencing (backend, `SmartOrderService::place_basket_order` or a new `place_rotation_basket`)

Basket orders are placed sequentially today with no fill waiting. A rotation needs:

1. Place every `cancel`. Wait for each to report cancelled (poll the open-orders query every 2 seconds, up to 30 seconds). A cancel that fails stops the group: nothing further is placed, the results view says which cancel failed and that the position's existing protection is unchanged.
2. Place every `fund` sell. Wait for each to report filled (poll every 2 seconds, up to 60 seconds). If a sell is rejected or not filled in time, stop: place nothing further in that group, report the state in the results view ("PFE sell filled; BAC buy not placed because …"), and leave the remaining legs in the modal for the user to retry.
3. Place the `buy`. Wait for fill the same way. If the buy filled only partially when the wait ends, cancel the unfilled remainder (Alpaca rejects a sell stop on a symbol with an open buy as a wash trade) and continue with the filled quantity.
4. Place the `protect` trailing stop for the filled quantity.
5. Groups are independent: a failure in `rotation-1` does not block `protect-NVDA`.

There is no order-event stream in the backend today, so the waits are orderbook polling inside the command. That is acceptable for v1.

**Market hours.** The fill waits above only work while the market is open. Placement of any group with a `cancel`, `fund`, or `buy` leg is refused while the market is closed (see Market hours below).

**Paper (analyze) mode.** The strategy is a live-account feature. In paper mode: `get_activities` returns nothing (ride age unknown), `get_portfolio_history` returns nothing (no Scoreboard), and the sandbox fills only market orders (a trailing stop sits pending forever). Rather than simulate all of that, paper mode degrades explicitly: the Scoreboard says "not available in paper mode", every ride shows status "Too new to judge (paper mode: no fill history)", Rotation and Protection say "live account only", and the modal still places market legs against the sandbox as today. The Definition of Done is tested on the live account. **Designer's call**; a sandbox simulation of fills, equity history and trailing stops is a separate project if the user wants it.

### 7. Dismissals (small persisted state)

The user said a proposed stop shows up "until you approve or dismiss it". The modal's X removes a card from this batch only, so a new, small persisted set is needed: `dismissed_protection(symbol, quantity, dismissed_at)` in SQLite. When the user removes a `protect` card in the modal and then places the rest (or closes the modal after removing it), the symbol+quantity is recorded. The Ride Report marks that position "protection dismissed by you on <date>" and the prompt is told not to propose it again while the held quantity is unchanged. When the quantity changes, the dismissal expires. Rotation proposals are **not** dismissable; dormant positions are re-evaluated fresh every morning. **Designer's call.**

---

## Constraints

- Whole shares, long only, cash account rules (existing).
- Budget basis stays `non_marginable_buying_power` for the *cash* component. The builder must verify against the live Alpaca account whether that field reflects same-day sale proceeds. If it does not, the rotation buy will be placed on the strength of the proceeds math above and the broker's own `buying_power` check decides; the submit-time re-check must use the same proceeds-inclusive budget so it does not strip the buy before placement.
- Timezone: all dates shown in `America/New_York`, 12-hour AM/PM, per project rules.
- Never delete rows from any existing table. New tables may be added by migration.
- Every page keeps its bare `help-circle-outline` help icon; no new UI elements beyond the group header and the GTC badge described above.

## Risks the user is accepting (state them in the briefing's risk note once, plainly)

- **Good-faith violation.** In a cash account, if the new ride's trailing stop fires before the proceeds that funded it settle (T+1), that is a good-faith violation. The trailing stop is GTC and most fire days later, but a same-day fire is possible on a volatile name. The briefing's risk note names this when a rotation is proposed.
- **Trailing stops fire on noise.** A 2× daily-swing trail will still occasionally sell a ride on an ordinary pullback. The potential widening (3×) reduces this for high-upside names. The user chose trailing over fixed stops knowing this.
- **Analyst targets are opinions.** Consensus can be stale or wrong. The briefing names the analyst count and as-of date every time.
- **Market rotation buys** fill at the live price, not a chosen level. Chosen so the stop can attach the same session.

## Market hours (decided by the user 2026-11-02)

The modal reads the market clock from `market_status_service` when it opens and again when Place is clicked. If the US market is closed, the Place button is disabled and a line in the modal footer says "Market opens at 9:30 AM ET. Place this then." in the project's caption style. Only groups containing a `fund`, `buy`, or `cancel` leg are gated; a batch that is nothing but GTC trailing stops may be placed any time, since Alpaca accepts them queued. The briefing itself can be generated at any hour; only placement waits. There is no persisted pending group and no automatic resume. If the user wants "place now, finish after the open" later, that is a separate project.

## What is left to the builder

- Exact Rust module boundaries, caching strategy for history and targets (any TTL of 15 minutes or longer is fine), and test structure. Unit tests are required for Ride status, Trail percent, the proceeds-inclusive reconcile, and the parser's GTC correction.
- How the fill-wait is implemented (polling the orderbook vs. the existing WebSocket order events). Either is acceptable; the 60-second cap and the per-group stop-on-failure behavior are not negotiable.
- Whether the Ride Report JSON is rendered as its own table above the markdown sections or only as markdown from the model. Either is acceptable in v1 as long as the numbers shown come from the Rust computation, not the model.
- Icons for the new section headers in `SECTION_META`.
- Migration shape for `ride_entries` and `dismissed_protection`.
- Deserializing `trail_percent` and `hwm` from Alpaca orders into the shared `Order` struct (other brokers leave them `None`).
- The Ride Report rides on a sibling field of the briefing response, not on the shared `CopilotResponse` used by chat.

## Definition of done (what the user clicks and sees)

1. On the Briefing page, click **Generate Briefing**.
2. The briefing opens with **Scoreboard** showing account value now vs. one week and one month ago.
3. **Ride Report** lists each position with got-on date, distance traveled, 30-day move, analyst target and upside, status (Climbing / Dormant / Falling / Too new to judge), and protection state.
4. When a position is dormant and a better ride exists, **Rotation** names the sells and the buy with the money math and the stop, and the **Review N Actions** button count includes the whole group.
5. In the modal, the rotation appears under one header. The buy quantity reflects sell proceeds plus cash. Removing the sell shrinks or drops the buy in front of the user with a notice.
6. Placing the group runs cancels, then sells, then the buy, then the trailing stop, and the results view shows each leg's outcome. Before 9:30 AM ET the Place button is disabled and the modal says "Market opens at 9:30 AM ET. Place this then."
7. The next morning, the Ride Report shows the new position as protected with its GTC trailing stop and trail percent, and the Protection section says "working, GTC". (The Order Book page has no validity column today; adding one is not part of this design. The modal's GTC badge and the Protection section are where the user sees it. If the user wants a validity column on the Order Book, that is a separate, small UI change they must approve.)
8. Positions without a stop appear under **Protection** with a proposed trail and reason until approved or dismissed; dismissed ones say so.
