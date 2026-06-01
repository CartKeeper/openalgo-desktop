# Fractional Order Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the app place fractional-share and notional (dollar-amount) orders on Alpaca, and display fractional holdings/positions correctly — so a small (~$2k) account can actually diversify across high-priced stocks.

**Architecture:** Quantities are currently `i32` (whole shares) in the shared `Order`/`Position`/`Holding`/`OrderRequest` structs, used by all 7 brokers, and the order UI is integer-only. We (A) widen the shared quantity fields to `f64` so fractional shares flow and display, (B) add an additive `notional: Option<f64>` to `OrderRequest` and a notional/fractional path in the Alpaca adapter, (C) persist Alpaca's `fractionable` flag (folded-in feature #5) so the UI knows which symbols qualify, and (D) add a Shares/Dollars toggle + fractional input to the order dialog. All changes are additive for the other 6 brokers (whole-number `f64` renders identically; `notional` defaults to `None`).

**Tech Stack:** Rust (Tauri backend, `serde`, `async_trait`, `rusqlite`), React + TypeScript frontend, Alpaca Trading API v2.

**Money-path caution:** This changes real-order placement on a live account. Every order-construction change gets a unit test asserting the exact request body. Final verification places a tiny real/paper order and confirms the fill.

---

## File Structure

- `src-tauri/src/brokers/types.rs` — widen quantity fields to `f64`; add `OrderRequest.notional`; add `SymbolData.fractionable`.
- `src-tauri/src/brokers/{alpaca,angel,zerodha,fyers,schwab,tradier,ibkr}/mod.rs` — fix all `SymbolData`/`Order`/`Position`/`Holding` construction sites the compiler flags; Alpaca gains notional handling + `fractionable` propagation.
- `src-tauri/src/db/sqlite/migrations.rs` — new `055_symtoken_fractionable` migration.
- `src-tauri/src/db/sqlite/symbol.rs` — persist + read `fractionable`.
- `src/api/tauri-client.ts`, `src/api/trading.ts` — `PlaceOrderRequest` gains `notional?`; quantity stays `number`.
- `src/components/trading/PlaceOrderDialog.tsx` — Shares/Dollars toggle, fractional input.
- `src/pages/{Holdings,Positions,OrderBook}.tsx` — format fractional quantities.

---

## Task 1: Widen shared quantity fields to f64 (fractional display + order qty)

**Files:**
- Modify: `src-tauri/src/brokers/types.rs`
- Modify (compiler-guided): all `src-tauri/src/brokers/*/mod.rs`

- [ ] **Step 1: Change the field types in `types.rs`**

In `src-tauri/src/brokers/types.rs`, change these fields from `i32` to `f64`:
- `OrderRequest.quantity`
- `Order.quantity`, `Order.filled_quantity`, `Order.pending_quantity`
- `Position.quantity`, `Position.overnight_quantity`, `Position.buy_quantity`, `Position.sell_quantity`
- `Holding.quantity`, `Holding.t1_quantity`

Leave `OrderRequest.disclosed_quantity: Option<i32>`, `ModifyOrderRequest.quantity: Option<i32>`, and `DepthLevel.quantity: i32` unchanged (whole-number concepts, not fractional).

- [ ] **Step 2: Compile to surface every affected site**

Run: `cd src-tauri && cargo check --lib --message-format=short`
Expected: a list of type-mismatch errors (e.g. `qty as i32`, integer literals, `row.get` typing) across the broker modules. This list IS the work-list for Step 3.

- [ ] **Step 3: Fix each flagged site**

For every error:
- Replace `x as i32` casts on quantities with `x as f64` (or drop the cast if already `f64`).
- Replace `let qty_i32 = qty as i32;` patterns with the `f64` value directly.
- Integer literals assigned to these fields (e.g. `pending_quantity: 0`) become `0.0`.
- Comparisons like `if qty > 0` keep working with `f64` (compare against `0.0` if the compiler requires).
Do NOT change order-display logic beyond the type — preserve behavior.

- [ ] **Step 4: Compile clean**

Run: `cd src-tauri && cargo check --lib --message-format=short`
Expected: `Finished` with no errors.

- [ ] **Step 5: Format fractional quantities in the frontend tables**

The TS types are already `number`, so no type change. In `src/pages/Holdings.tsx`, `src/pages/Positions.tsx`, and `src/pages/OrderBook.tsx`, where `quantity` is rendered, format it so whole numbers show plainly and fractional values show up to 4 decimals. Add this helper near the existing `formatCurrency` in each (or a shared util):

```ts
function formatQty(q: number): string {
  return Number.isInteger(q) ? q.toString() : q.toFixed(4)
}
```
Replace direct `{holding.quantity}` / `{position.quantity}` / `{order.quantity}` renders with `{formatQty(...)}`.

- [ ] **Step 6: Typecheck frontend**

Run: `npx tsc --noEmit -p tsconfig.json`
Expected: no `error TS` lines.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(orders): widen quantity fields to f64 for fractional shares"
```

---

## Task 2: Notional (dollar) order support on Alpaca

**Files:**
- Modify: `src-tauri/src/brokers/types.rs` (add `OrderRequest.notional`)
- Modify: `src-tauri/src/brokers/alpaca/mod.rs` (`place_order` body + test)

- [ ] **Step 1: Write the failing unit test**

Add to the bottom of `src-tauri/src/brokers/alpaca/mod.rs`:

```rust
#[cfg(test)]
mod order_body_tests {
    use super::*;

    fn req(notional: Option<f64>, quantity: f64) -> OrderRequest {
        OrderRequest {
            symbol: "AAPL".into(), exchange: "NASDAQ".into(), side: "BUY".into(),
            quantity, price: 0.0, order_type: "MARKET".into(), product: "CNC".into(),
            validity: "DAY".into(), trigger_price: None, disclosed_quantity: None,
            amo: false, trail_price: None, trail_percent: None,
            broker_symbol: None, symbol_token: None, notional,
        }
    }

    #[test]
    fn notional_order_sends_notional_not_qty() {
        let body = AlpacaBroker::build_order_body(&req(Some(50.0), 0.0));
        assert_eq!(body.get("notional").and_then(|v| v.as_str()), Some("50"));
        assert!(body.get("qty").is_none());
    }

    #[test]
    fn fractional_qty_order_sends_qty() {
        let body = AlpacaBroker::build_order_body(&req(None, 0.5));
        assert_eq!(body.get("qty").and_then(|v| v.as_str()), Some("0.5"));
        assert!(body.get("notional").is_none());
    }
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cd src-tauri && cargo test --lib order_body_tests 2>&1 | tail -20`
Expected: FAIL — `OrderRequest` has no field `notional`, and `build_order_body` does not exist.

- [ ] **Step 3: Add the `notional` field to `OrderRequest`**

In `src-tauri/src/brokers/types.rs`, inside `OrderRequest`, add (additive, defaulted):

```rust
    /// Dollar-amount (notional) order. When set, the broker places a
    /// market-day notional order instead of a share-quantity order.
    #[serde(default)]
    pub notional: Option<f64>,
```

- [ ] **Step 4: Extract `build_order_body` and add notional/fractional logic**

In `src-tauri/src/brokers/alpaca/mod.rs`, refactor `place_order` so the JSON body is built by a testable associated fn. Replace the inline `serde_json::json!({...})` for the base body with a call to:

```rust
impl AlpacaBroker {
    /// Build the Alpaca /v2/orders request body. Notional (dollar) orders take
    /// precedence over share quantity; both are sent as strings.
    fn build_order_body(order: &OrderRequest) -> serde_json::Map<String, serde_json::Value> {
        let mut body = serde_json::Map::new();
        body.insert(
            "symbol".into(),
            serde_json::json!(order.broker_symbol.as_deref().unwrap_or(&order.symbol)),
        );
        match order.notional {
            Some(n) if n > 0.0 => {
                body.insert("notional".into(), serde_json::json!(n.to_string()));
            }
            _ => {
                body.insert("qty".into(), serde_json::json!(order.quantity.to_string()));
            }
        }
        body.insert("side".into(), serde_json::json!(order.side.to_lowercase()));
        body.insert("type".into(), serde_json::json!(map_order_type(&order.order_type)));
        body.insert("time_in_force".into(), serde_json::json!(map_validity(&order.validity)));
        body
    }
}
```

Then in `place_order`, replace the base `let mut body = serde_json::json!({...});` with:

```rust
        let mut body = serde_json::Value::Object(Self::build_order_body(&order));
```

Keep the existing `limit_price` / `stop_price` / trailing / `extended_hours` blocks below it unchanged (they index `body["..."]`, which still works on a `Value::Object`).

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cd src-tauri && cargo test --lib order_body_tests 2>&1 | tail -20`
Expected: PASS (2 passed).

- [ ] **Step 6: Fix other `OrderRequest` construction sites**

Run `cd src-tauri && cargo check --lib --message-format=short`. For any Rust site that constructs `OrderRequest { .. }` (e.g. smart/basket order services), add `notional: None,`. Compile clean.

- [ ] **Step 7: Thread `notional` through the frontend order types**

In `src/api/tauri-client.ts` (and `src/api/trading.ts` re-exports), add `notional?: number` to the `PlaceOrderRequest`/order-args interface used by `placeOrder`. Ensure the invoke passes `notional` through.

Run: `npx tsc --noEmit -p tsconfig.json` — expect no errors.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(orders): notional (dollar) order support on Alpaca"
```

---

## Task 3: Persist Alpaca `fractionable` flag (folded-in feature #5)

**Files:**
- Modify: `src-tauri/src/brokers/types.rs` (`SymbolData.fractionable`)
- Modify: `src-tauri/src/db/sqlite/migrations.rs` (new migration)
- Modify: `src-tauri/src/db/sqlite/symbol.rs` (insert + selects)
- Modify: `src-tauri/src/brokers/*/mod.rs` (construction sites)

- [ ] **Step 1: Add the field to `SymbolData`**

In `src-tauri/src/brokers/types.rs`, add to `SymbolData`:
```rust
    /// Whether the symbol supports fractional/notional orders (Alpaca). Other
    /// brokers default to false.
    pub fractionable: bool,
```

- [ ] **Step 2: Add the migration**

In `src-tauri/src/db/sqlite/migrations.rs`, define the SQL constant near the other `symtoken` SQL:
```rust
const ADD_SYMTOKEN_FRACTIONABLE: &str =
    "ALTER TABLE symtoken ADD COLUMN fractionable INTEGER NOT NULL DEFAULT 0;";
```
And register it after the last migration (`054_holdings_market_value`):
```rust
    run_migration(conn, "055_symtoken_fractionable", ADD_SYMTOKEN_FRACTIONABLE)?;
```

- [ ] **Step 3: Persist + read it in `symbol.rs`**

In `src-tauri/src/db/sqlite/symbol.rs`:
- The `INSERT OR REPLACE INTO symtoken (...)` column list: append `, fractionable`; the `VALUES` list: append a bind for `symbol.fractionable as i64` (or `if symbol.fractionable {1} else {0}`).
- Each of the four `SELECT symbol, token, exchange, name, lot_size, tick_size, instrument_type, brsymbol, brexchange FROM symtoken` queries: append `, fractionable` to the column list, and in the row→`SymbolData` mapping add `fractionable: row.get::<_, i64>(9)? != 0,` (use the correct next index per query).

- [ ] **Step 4: Set `fractionable` at every `SymbolData` construction site**

Run `cd src-tauri && cargo check --lib --message-format=short`. For each flagged `SymbolData { .. }`:
- In `alpaca/mod.rs` `download_master_contract`: set `fractionable: asset.fractionable,` (the `AlpacaAsset` DTO already parses it).
- In `angel`, `zerodha`, `fyers`, `schwab`, `tradier`, `ibkr` and the `types.rs` default: set `fractionable: false,`.

- [ ] **Step 5: Compile clean + expose in frontend symbol type**

Run: `cd src-tauri && cargo check --lib`. Expected: `Finished`.
If a frontend symbol-search type mirrors `SymbolData`, add `fractionable: boolean`. Run `npx tsc --noEmit`.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(symbols): persist Alpaca fractionable flag"
```

---

## Task 4: Order dialog — Shares/Dollars toggle + fractional input

**Files:**
- Modify: `src/components/trading/PlaceOrderDialog.tsx`

- [ ] **Step 1: Add an order-mode toggle to form state**

In the `form` state, add `orderMode: 'shares' | 'dollars'` (default `'shares'`) and `notional: number` (default 0). Default `quantity` stays `1`.

- [ ] **Step 2: Render the toggle + conditional input**

Above the Quantity input, add a two-button toggle (Shares | Dollars) styled like the existing buttons (fixed height, `radius-md`). When `orderMode === 'shares'`, show the quantity input but change `step={1}` → `step="any"` and `parseInt(e.target.value)` → `parseFloat(e.target.value)`. When `orderMode === 'dollars'`, replace it with a Dollars input bound to `form.notional` (`step="any"`, min 1) and a label "Amount ($)".

Dollars mode is only valid for **Market / Day** orders (Alpaca limitation): when `orderMode === 'dollars'`, force `orderType` to `MARKET` and disable the order-type select with a hint "Dollar orders are market-day only."

- [ ] **Step 3: Validation**

Replace the `form.quantity <= 0` guard so that: in shares mode require `quantity > 0`; in dollars mode require `notional >= 1`.

- [ ] **Step 4: Submit the right field**

In the submit handler, build the order args: in shares mode pass `quantity: form.quantity` (and omit `notional`); in dollars mode pass `notional: form.notional` and `quantity: 0`.

- [ ] **Step 5: Estimated cost**

Update the estimated-cost line: dollars mode shows the entered amount directly; shares mode keeps `price * quantity` (now fractional-aware).

- [ ] **Step 6: Typecheck**

Run: `npx tsc --noEmit -p tsconfig.json` — expect no errors.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(ui): Shares/Dollars toggle and fractional quantity in order dialog"
```

---

## Task 5: End-to-end verification

- [ ] **Step 1: Build the app**

Confirm `tauri dev` rebuilt and relaunched (binary mtime newer than edits; `save`/order commands present). No compile errors in the dev log.

- [ ] **Step 2: Place a tiny dollar order (paper first)**

With the broker in **paper** mode (or live with settled funds), open the order dialog, choose **Dollars**, enter `$5` of `AAPL`, submit. Expected: order accepted (market-day notional), no rejection.

- [ ] **Step 3: Confirm fractional display**

After fill, open Holdings/Positions. Expected: a fractional AAPL position (e.g. `0.0xxx` shares) displays with decimals and a non-zero market value — not `0`.

- [ ] **Step 4: Confirm whole-share path unaffected**

Place a normal **Shares** order for a whole quantity on any broker. Expected: identical behavior to before (no `.0` artifacts, correct fill).

- [ ] **Step 5: Final commit / branch wrap-up**

```bash
git add -A && git commit -m "test: verify fractional + notional order path end-to-end"
```
Then use `superpowers:finishing-a-development-branch` to decide merge/PR.

---

## Task 6: Bracket / OCO orders (folded-in opportunity — shares the order files)

**Files:**
- Modify: `src-tauri/src/brokers/types.rs` (`OrderRequest` bracket fields)
- Modify: `src-tauri/src/brokers/alpaca/mod.rs` (`build_order_body` + test)
- Modify: `src/components/trading/PlaceOrderDialog.tsx` (TP/SL inputs)

- [ ] **Step 1: Write the failing unit test**

Add to the `order_body_tests` module in `src-tauri/src/brokers/alpaca/mod.rs`:

```rust
    #[test]
    fn bracket_order_includes_tp_and_sl() {
        let mut r = req(None, 1.0);
        r.order_class = Some("bracket".into());
        r.take_profit_price = Some(110.0);
        r.stop_loss_price = Some(90.0);
        let body = AlpacaBroker::build_order_body(&r);
        assert_eq!(body.get("order_class").and_then(|v| v.as_str()), Some("bracket"));
        assert_eq!(
            body.get("take_profit").and_then(|v| v.get("limit_price")).and_then(|v| v.as_str()),
            Some("110")
        );
        assert_eq!(
            body.get("stop_loss").and_then(|v| v.get("stop_price")).and_then(|v| v.as_str()),
            Some("90")
        );
    }
```

(The `req` helper from Task 2 must also set the new fields to `None` by default — update it.)

- [ ] **Step 2: Run the test, confirm it fails**

Run: `cd src-tauri && cargo test --lib order_body_tests 2>&1 | tail -20`
Expected: FAIL — `OrderRequest` has no `order_class` / `take_profit_price` / `stop_loss_price`.

- [ ] **Step 3: Add bracket fields to `OrderRequest`**

In `src-tauri/src/brokers/types.rs`, add to `OrderRequest`:
```rust
    /// Order class: "simple" (default), "bracket", "oco", "oto".
    #[serde(default)]
    pub order_class: Option<String>,
    /// Take-profit limit price (bracket/oto).
    #[serde(default)]
    pub take_profit_price: Option<f64>,
    /// Stop-loss stop price (bracket/oco/oto).
    #[serde(default)]
    pub stop_loss_price: Option<f64>,
    /// Optional stop-loss limit price (stop-limit exit).
    #[serde(default)]
    pub stop_loss_limit_price: Option<f64>,
```

- [ ] **Step 4: Emit bracket legs in `build_order_body`**

In `AlpacaBroker::build_order_body`, after the side/type/time_in_force inserts, append:
```rust
        if let Some(class) = order.order_class.as_deref() {
            if class != "simple" {
                body.insert("order_class".into(), serde_json::json!(class));
            }
        }
        if let Some(tp) = order.take_profit_price {
            body.insert(
                "take_profit".into(),
                serde_json::json!({ "limit_price": tp.to_string() }),
            );
        }
        if order.stop_loss_price.is_some() || order.stop_loss_limit_price.is_some() {
            let mut sl = serde_json::Map::new();
            if let Some(sp) = order.stop_loss_price {
                sl.insert("stop_price".into(), serde_json::json!(sp.to_string()));
            }
            if let Some(lp) = order.stop_loss_limit_price {
                sl.insert("limit_price".into(), serde_json::json!(lp.to_string()));
            }
            body.insert("stop_loss".into(), serde_json::Value::Object(sl));
        }
```

- [ ] **Step 5: Run the test, confirm pass**

Run: `cd src-tauri && cargo test --lib order_body_tests 2>&1 | tail -20`
Expected: PASS (all order-body tests).

- [ ] **Step 6: Fix remaining `OrderRequest` construction sites**

Run `cd src-tauri && cargo check --lib`. Add `order_class: None, take_profit_price: None, stop_loss_price: None, stop_loss_limit_price: None,` to any other `OrderRequest { .. }` literals the compiler flags. Compile clean.

- [ ] **Step 7: UI — optional bracket section**

In `src/components/trading/PlaceOrderDialog.tsx`, add an optional "Bracket (take-profit / stop-loss)" toggle. When enabled, show two price inputs (Take-profit, Stop-loss), set `order_class: 'bracket'` and pass `take_profit_price` / `stop_loss_price`. Bracket requires share quantity — when enabled, force **Shares** mode (disable Dollars/notional) and a limit or market entry. Thread the fields through `PlaceOrderRequest` in `tauri-client.ts`.

Run: `npx tsc --noEmit -p tsconfig.json` — expect no errors.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(orders): bracket/OCO orders with take-profit + stop-loss"
```

---

## Notes / Risks

- **Blast radius (Task 1):** the `i32→f64` change touches ~142 sites, but every one is a compile error until fixed — nothing fails silently. Do not "fix" by re-casting back to `i32`; that defeats the feature.
- **Alpaca notional constraints:** notional orders must be **market, time-in-force day**, on **fractionable** symbols. The UI enforces market-day; symbol fractionability is surfaced by Task 3 (gray out Dollars mode for non-fractionable symbols as a follow-up if desired).
- **Other brokers:** Indian brokers reject fractional/notional; their `place_order` continues to send whole `qty` strings (`5.0_f64.to_string()` == `"5"`). `notional` stays `None` for them.
- **Modify orders:** `ModifyOrderRequest.quantity` stays `Option<i32>`; modifying a fractional order's quantity is out of scope (cancel + re-place instead).
