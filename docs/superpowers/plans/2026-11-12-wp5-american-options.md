# WP5 — American Option Pricing (OptionStratLib) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Options page can price and compute greeks with a binomial model that honors early exercise, beside the existing Black-Scholes.

**Architecture:** Add `optionstratlib` (MIT, Rust) with only its `pricing` feature. A new `american_pricing.rs` wraps `pricing::binomial_model::price_binomial` and derives greeks by central finite differences in the same units `services/greeks.rs` uses. The greeks commands take an optional `model`; the page gets a two-way switch. Black-Scholes results are byte-for-byte unchanged.

**Tech Stack:** `optionstratlib = { version = "0.24.3", default-features = false, features = ["pricing"] }`, `rust_decimal = "1"`; Rust MSRV raised to 1.89 (toolchain is 1.93.1).

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md` (WP5 requirements; "Binomial steps" row).

## Global Constraints

- Branch `wp5-american-options`; files below plus the WP5 anchors (`Cargo.toml` deps, handler list only if a new command is added — none is planned).
- `rust-version = "1.89"` in `src-tauri/Cargo.toml` (document in the commit body: optionstratlib 0.24.3 requires it).
- Units for greeks match `greeks.rs` exactly (read `delta/gamma/theta/vega/rho` there first: theta per day or per year, vega per 1% or per 1.0). The test in Task 2 pins parity on a European option.
- Steps = 200. Decimal ↔ f64 via `Decimal::from_f64_retain` / `to_f64`.

## Review Focus

1. Expiry today or in the past (`days_to_expiry ≤ 0`): return intrinsic value, zero greeks, no panic. (Task 2 test.)
2. Volatility 0 or negative: `Err(AppError::Validation)`, never NaN in the UI. (Task 2 test.)
3. Deep OTM option with a market price below intrinsic bounds: implied-vol solve returns `None`, the chain shows "–". (Task 2 test.)
4. Dividend yield above the rate on a call: binomial still converges (no early exercise for calls without dividends; with dividends it may) — assert price ≥ intrinsic. (Task 2 test.)
5. A batch of 200 contracts: binomial with 200 steps must stay under ~1 s total; measure in a test with `std::time::Instant` and assert < 5 s to catch accidental O(steps²) per greek. (Task 2 test.)

---

### Task 1: Dependency, MSRV, compile smoke

**Files:**
- Modify: `src-tauri/Cargo.toml` (`rust-version`, WP5 deps anchor)
- Create: `src-tauri/src/services/american_pricing.rs` (skeleton with the smoke test)
- Modify: `src-tauri/src/services/mod.rs` (WP5 anchor)

- [ ] **Step 1: Write the failing smoke test**: price a European call (S 100, K 100, T 1y, r 5%, σ 20%, q 0) with `price_binomial` at 200 steps and compare to `greeks::black_scholes_price(100.0, 100.0, 1.0, 0.05, 0.2, true)` (10.4506) within 0.5%.
- [ ] **Step 2: Run** `cd src-tauri && cargo test --lib american_pricing` → FAIL (crate missing).
- [ ] **Step 3: Add the dependencies and MSRV**, implement the smoke using the crate's `BinomialPricingParams` (read its fields in `docs.rs/optionstratlib-pricing/0.24.*` or the vendored source under `~/.cargo/registry`).
- [ ] **Step 4: Run** → PASS; `cargo build` warnings unchanged.
- [ ] **Step 5: Commit** `build(options): add optionstratlib pricing and raise MSRV to 1.89`

---

### Task 2: American pricing API

**Files:**
- Modify: `src-tauri/src/services/american_pricing.rs`

**Interfaces:**
- Produces:
  ```rust
  pub const BINOMIAL_STEPS: usize = 200;
  pub struct AmericanInputs { pub spot: f64, pub strike: f64, pub rate: f64, pub dividend_yield: f64, pub volatility: f64, pub days_to_expiry: f64, pub is_call: bool }
  pub struct AmericanResult { pub price: f64, pub delta: f64, pub gamma: f64, pub theta: f64, pub vega: f64, pub rho: f64, pub european_price: f64, pub early_exercise_premium: f64 }
  pub fn price_american(inp: &AmericanInputs) -> Result<AmericanResult, AppError>
  pub fn implied_volatility_american(market_price: f64, inp: &AmericanInputs) -> Option<f64>   // bisection on [0.01, 5.0], tol 1e-5, ≤ 100 iterations
  ```
  Greeks: delta `(P(S·1.01) − P(S·0.99)) / (0.02·S)`; gamma second difference with the same bumps; theta `P(T) − P(T − 1 day)` in the same sign/units as `greeks::theta`; vega bump ±1 vol point; rho bump ±1% rate — each scaled to `greeks.rs` conventions. `european_price` is the same tree with `OptionStyle::European`.

- [ ] **Step 1: Write the failing tests**: American put ≥ European put (S 100, K 100, 1y, 5%, 20%); deep ITM American put (S 60) has `early_exercise_premium > 0` and `price ≥ 40`; European-parity: delta of the European call within 0.01 of `greeks::delta(..)`, vega within 2% of `greeks::vega(..)`; expiry today → price = intrinsic, greeks 0; `volatility 0` → `Err`; IV round-trip (price at σ 0.25 → `implied_volatility_american` ≈ 0.25 ± 1e-3); "below intrinsic → None"; batch timing (200 contracts < 5 s).
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5: Commit** `feat(options): binomial American pricing with finite-difference greeks`

---

### Task 3: Model parameter on the greeks commands

**Files:**
- Modify: `src-tauri/src/commands/greeks.rs` (`compute_greeks`, `compute_greeks_batch`), `src-tauri/src/services/greeks.rs` (`OptionGreeksResult`)
- Modify: the TS wrapper that calls `compute_greeks`/`compute_greeks_batch` (find it with `grep -rn "compute_greeks" src/api src/pages/options`)

**Interfaces:**
- Commands gain `model: Option<String>`: `None | "black_scholes"` → existing path; `"binomial"` → `price_american` per contract. `OptionGreeksResult` gains `pub model: String` and `pub early_exercise_premium: Option<f64>` (None for Black-Scholes). Unknown model → `AppError::Validation("Unknown pricing model: …")`.

- [ ] **Step 1: Write the failing tests**: `compute_greeks` with `model: None` returns `model == "black_scholes"` and identical numbers to before (pin one existing expected value); with `"binomial"` on a put, `early_exercise_premium.is_some()`; unknown → `Err`.
- [ ] **Step 2–4:** fail → implement → pass; `cargo test --lib` all pass.
- [ ] **Step 5: Commit** `feat(options): choose Black-Scholes or binomial for greeks`

---

### Task 4: Options page switch and help

**Files:**
- Modify: `src/pages/options/OptionsPage.tsx`
- Test: `src/pages/options/ModelSwitch.test.tsx` (jsdom) if the switch is extracted to `src/components/options/ModelSwitch.tsx` (do extract it)

**Interfaces:**
- `ModelSwitch({ value: 'black_scholes' | 'binomial', onChange })`: two 32px toggle buttons "Black-Scholes" and "Binomial (American)", `aria-pressed`; default `black_scholes`; placed next to the existing greeks controls. When `binomial`, the greeks table shows an extra column "Early exercise" for puts (and calls with dividends), formatted like the price column. Help text explains the two models and that American pricing matters for puts and dividend-paying names.

- [ ] **Step 1: Write the failing test**: renders both buttons; clicking "Binomial (American)" calls `onChange('binomial')`.
- [ ] **Step 2–4:** fail → implement → pass; `npm run build`, `npm run lint` → 0.
- [ ] **Step 5: Gates**: `cd src-tauri && cargo test --lib`; `npx vitest run`; `npm run build`; `npm run lint`.
- [ ] **Step 6: Commit** `feat(options): pricing model switch on the Options page`

**Definition of Done (user, after merge):** Options → load a chain (e.g. AAPL) → greeks visible as before. Click **Binomial (American)**: greeks update, puts show an "Early exercise" value greater than zero deep in the money, calls on a non-dividend name show 0. Switch back: the original numbers return unchanged.
