//! American Option Pricing Service (binomial tree)
//!
//! Wraps the Cox-Ross-Rubinstein lattice from `optionstratlib`
//! (`pricing::binomial_model`) so the Options page can price American-style
//! contracts — honoring early exercise — beside the closed-form Black-Scholes
//! model in `services/greeks.rs`.
//!
//! Pure math: no I/O, no state. Greeks are central finite differences in the
//! units `greeks.rs` uses:
//! - delta, gamma and theta are read off the base lattice (delta from the two
//!   step-1 nodes, gamma from the three step-2 nodes, theta from the step-2
//!   middle node against the root), which keeps all three valuations on one
//!   price grid so the lattice's discretization error cancels;
//! - vega (±1 vol point) and rho (±1 percentage point) re-price the lattice.
//!
//! The lattice arithmetic is `rust_decimal`; one 200-step American tree costs
//! roughly 18 ms optimized, so a chain of contracts goes through
//! [`price_american_batch`], which spreads contracts across the CPU cores.

use crate::error::AppError;
use optionstratlib::model::Positive;
use optionstratlib::pricing::binomial_model::{
    BinomialPricingParams, generate_binomial_tree, price_binomial,
};
use optionstratlib::{OptionStyle, OptionType, Side};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// Number of steps in the binomial lattice (spec: "Binomial steps" = 200).
pub const BINOMIAL_STEPS: usize = 200;

/// Day count shared with the Options page and `greeks.rs` (theta is per calendar day).
const DAYS_PER_YEAR: f64 = 365.0;

/// Volatility bump for vega: one vol point, or half the volatility when that is smaller.
const VOL_BUMP: f64 = 0.01;
/// Rate bump for rho: one percentage point.
const RATE_BUMP: f64 = 0.01;

// Implied-volatility bisection (plan: [0.01, 5.0], tolerance 1e-5, at most 100 iterations).
const IV_LOW: f64 = 0.01;
const IV_HIGH: f64 = 5.0;
const IV_TOLERANCE: f64 = 1e-5;
const IV_MAX_ITERATIONS: usize = 100;

/// Inputs for one American-style contract.
///
/// `dividend_yield` must be `0.0`: the lattice has no dividend term, and a
/// non-zero yield is refused instead of being priced approximately.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmericanInputs {
    pub spot: f64,
    pub strike: f64,
    /// Annualized risk-free rate (0.05 = 5%).
    pub rate: f64,
    /// Annualized continuous dividend yield; only `0.0` is supported.
    pub dividend_yield: f64,
    /// Annualized volatility (0.20 = 20%).
    pub volatility: f64,
    /// Calendar days to expiry; `<= 0` means expired (intrinsic value, zero greeks).
    pub days_to_expiry: f64,
    pub is_call: bool,
}

/// Binomial price and greeks, in the units of `greeks.rs`: delta per $1 of
/// spot, gamma per $1², theta per calendar day, vega per one vol point, rho
/// per 1.0 of rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmericanResult {
    pub price: f64,
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
    /// The same lattice with European exercise.
    pub european_price: f64,
    /// `price − european_price` (the value of the right to exercise early).
    pub early_exercise_premium: f64,
}

// ============================================================================
// Validation and conversion helpers
// ============================================================================

fn validation(msg: impl Into<String>) -> AppError {
    AppError::Validation(msg.into())
}

fn validate(inp: &AmericanInputs) -> Result<(), AppError> {
    if !(inp.spot.is_finite() && inp.spot > 0.0) {
        return Err(validation("spot must be a positive number"));
    }
    if !(inp.strike.is_finite() && inp.strike > 0.0) {
        return Err(validation("strike must be a positive number"));
    }
    if !(inp.volatility.is_finite() && inp.volatility > 0.0) {
        return Err(validation("volatility must be a positive number"));
    }
    if !inp.rate.is_finite() {
        return Err(validation("rate must be a finite number"));
    }
    if !inp.days_to_expiry.is_finite() {
        return Err(validation("days_to_expiry must be a finite number"));
    }
    if !(inp.dividend_yield.is_finite() && inp.dividend_yield == 0.0) {
        return Err(validation(
            "dividend yield is not supported by the binomial model (the lattice has no dividend term); use 0",
        ));
    }
    Ok(())
}

fn positive(value: f64, what: &str) -> Result<Positive, AppError> {
    Positive::new(value).map_err(|e| validation(format!("{what} must be a positive number: {e}")))
}

fn decimal(value: f64, what: &str) -> Result<Decimal, AppError> {
    Decimal::from_f64_retain(value)
        .ok_or_else(|| validation(format!("{what} is not representable as a decimal: {value}")))
}

fn to_f64(value: Decimal, what: &str) -> Result<f64, AppError> {
    value
        .to_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| validation(format!("{what} is not representable as a number")))
}

fn intrinsic(spot: f64, strike: f64, is_call: bool) -> f64 {
    if is_call {
        (spot - strike).max(0.0)
    } else {
        (strike - spot).max(0.0)
    }
}

// ============================================================================
// Lattice
// ============================================================================

/// Exercise style to run on the lattice.
///
/// A call on a non-dividend asset with a non-negative rate is never exercised
/// early: at every node the continuation value is at least `S − K·e^{−rΔt} ≥
/// S − K`, so the American lattice equals the European one node for node and
/// the cheaper European pass is exact. Puts, and calls under negative rates,
/// take the American pass.
fn exercise_style(is_call: bool, rate: f64) -> OptionType {
    if is_call && rate >= 0.0 {
        OptionType::European
    } else {
        OptionType::American
    }
}

fn option_style(is_call: bool) -> OptionStyle {
    if is_call {
        OptionStyle::Call
    } else {
        OptionStyle::Put
    }
}

fn build_params<'a>(
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    years: f64,
    exercise: &'a OptionType,
    style: &'a OptionStyle,
) -> Result<BinomialPricingParams<'a>, AppError> {
    Ok(BinomialPricingParams {
        asset: positive(spot, "spot")?,
        volatility: positive(volatility, "volatility")?,
        int_rate: decimal(rate, "rate")?,
        strike: positive(strike, "strike")?,
        expiry: positive(years, "time to expiry")?,
        no_steps: NonZeroUsize::new(BINOMIAL_STEPS).expect("BINOMIAL_STEPS is non-zero"),
        option_type: exercise,
        option_style: style,
        side: &Side::Long,
    })
}

fn engine_error(e: impl std::fmt::Display) -> AppError {
    validation(format!("binomial model could not price these inputs: {e}"))
}

/// One lattice valuation with an explicit exercise style. `years <= 0` is intrinsic value.
fn lattice_price_as(
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    years: f64,
    is_call: bool,
    exercise: &OptionType,
) -> Result<f64, AppError> {
    if years <= 0.0 {
        return Ok(intrinsic(spot, strike, is_call));
    }
    let style = option_style(is_call);
    let params = build_params(spot, strike, rate, volatility, years, exercise, &style)?;
    let price = price_binomial(params).map_err(engine_error)?;
    to_f64(price, "binomial price")
}

/// One lattice valuation of the contract under early exercise (see [`exercise_style`]).
fn lattice_price(
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    years: f64,
    is_call: bool,
) -> Result<f64, AppError> {
    let exercise = exercise_style(is_call, rate);
    lattice_price_as(spot, strike, rate, volatility, years, is_call, &exercise)
}

struct LatticeGreeks {
    price: f64,
    delta: f64,
    gamma: f64,
    /// Per calendar day.
    theta: f64,
}

/// Price, delta, gamma and theta from one lattice (`years > 0`).
///
/// Node `n` of a step counts down moves, so step 1 is `[up, down]` and step 2
/// is `[up-up, up-down, down-down]`.
fn lattice_greeks(
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    years: f64,
    is_call: bool,
) -> Result<LatticeGreeks, AppError> {
    let exercise = exercise_style(is_call, rate);
    let style = option_style(is_call);
    let params = build_params(spot, strike, rate, volatility, years, &exercise, &style)?;
    let (assets, values) = generate_binomial_tree(&params).map_err(engine_error)?;

    let node = |tree: &[Vec<Decimal>], step: usize, down_moves: usize| -> Result<f64, AppError> {
        let value = tree
            .get(step)
            .and_then(|row| row.get(down_moves))
            .copied()
            .ok_or_else(|| validation("binomial lattice is missing a node"))?;
        to_f64(value, "binomial lattice node")
    };

    let price = node(&values, 0, 0)?;
    let (v_u, v_d) = (node(&values, 1, 0)?, node(&values, 1, 1)?);
    let (s_u, s_d) = (node(&assets, 1, 0)?, node(&assets, 1, 1)?);
    let (v_uu, v_ud, v_dd) = (node(&values, 2, 0)?, node(&values, 2, 1)?, node(&values, 2, 2)?);
    let (s_uu, s_ud, s_dd) = (node(&assets, 2, 0)?, node(&assets, 2, 1)?, node(&assets, 2, 2)?);
    let dt = years / BINOMIAL_STEPS as f64;

    let delta = (v_u - v_d) / (s_u - s_d);
    let gamma = ((v_uu - v_ud) / (s_uu - s_ud) - (v_ud - v_dd) / (s_ud - s_dd)) / (0.5 * (s_uu - s_dd));
    // Spot is back at its start after one up and one down move, 2Δt later.
    let theta = (v_ud - price) / (2.0 * dt) / DAYS_PER_YEAR;

    Ok(LatticeGreeks {
        price,
        delta,
        gamma,
        theta,
    })
}

// ============================================================================
// Public API
// ============================================================================

/// Price an American-style contract on the binomial lattice with its greeks
/// (see the module docs for how each greek is formed).
pub fn price_american(inp: &AmericanInputs) -> Result<AmericanResult, AppError> {
    validate(inp)?;

    let years = inp.days_to_expiry / DAYS_PER_YEAR;
    if years <= 0.0 {
        let value = intrinsic(inp.spot, inp.strike, inp.is_call);
        return Ok(AmericanResult {
            price: value,
            delta: 0.0,
            gamma: 0.0,
            theta: 0.0,
            vega: 0.0,
            rho: 0.0,
            european_price: value,
            early_exercise_premium: 0.0,
        });
    }

    let (s, k, r, sigma, is_call) = (inp.spot, inp.strike, inp.rate, inp.volatility, inp.is_call);
    let base = lattice_greeks(s, k, r, sigma, years, is_call)?;
    let european = match exercise_style(is_call, r) {
        OptionType::European => base.price,
        _ => lattice_price_as(s, k, r, sigma, years, is_call, &OptionType::European)?,
    };

    let dv = VOL_BUMP.min(sigma / 2.0);
    let v_up = lattice_price(s, k, r, sigma + dv, years, is_call)?;
    let v_dn = lattice_price(s, k, r, sigma - dv, years, is_call)?;
    let r_up = lattice_price(s, k, r + RATE_BUMP, sigma, years, is_call)?;
    let r_dn = lattice_price(s, k, r - RATE_BUMP, sigma, years, is_call)?;

    Ok(AmericanResult {
        price: base.price,
        delta: base.delta,
        gamma: base.gamma,
        theta: base.theta,
        // Per one vol point.
        vega: (v_up - v_dn) / (2.0 * dv) * VOL_BUMP,
        // Per 1.0 of rate, like `greeks::rho`.
        rho: (r_up - r_dn) / (2.0 * RATE_BUMP),
        european_price: european,
        // Early exercise can only add value; the clamp only guards rounding.
        early_exercise_premium: (base.price - european).max(0.0),
    })
}

/// [`price_american`] for many contracts, spread across the available CPU
/// cores (each worker takes the next unpriced contract). Results keep the
/// input order; one contract failing does not fail the others.
pub fn price_american_batch(inputs: &[AmericanInputs]) -> Vec<Result<AmericanResult, AppError>> {
    let workers = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(inputs.len());
    if workers <= 1 {
        return inputs.iter().map(price_american).collect();
    }

    let next = AtomicUsize::new(0);
    let collected: Vec<Vec<(usize, Result<AmericanResult, AppError>)>> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(inp) = inputs.get(i) else { break };
                        out.push((i, price_american(inp)));
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    });

    let mut results: Vec<Option<Result<AmericanResult, AppError>>> =
        (0..inputs.len()).map(|_| None).collect();
    for (i, res) in collected.into_iter().flatten() {
        results[i] = Some(res);
    }
    results
        .into_iter()
        .map(|slot| {
            slot.unwrap_or_else(|| {
                Err(AppError::Internal(
                    "binomial pricing worker panicked".to_string(),
                ))
            })
        })
        .collect()
}

/// Solve the volatility whose lattice price matches `market_price` by
/// bisection on [0.01, 5.0]. `None` when the inputs are invalid, the contract
/// is expired, or no volatility in range explains the price (for example a
/// quote at or below intrinsic value).
pub fn implied_volatility_american(market_price: f64, inp: &AmericanInputs) -> Option<f64> {
    if !(market_price.is_finite() && market_price > 0.0) {
        return None;
    }
    // Volatility is the unknown here; validate everything else.
    let probe = AmericanInputs {
        volatility: IV_LOW,
        ..*inp
    };
    validate(&probe).ok()?;
    let years = inp.days_to_expiry / DAYS_PER_YEAR;
    if years <= 0.0 {
        return None;
    }

    let price_at =
        |vol: f64| lattice_price(inp.spot, inp.strike, inp.rate, vol, years, inp.is_call).ok();

    let mut lo = IV_LOW;
    let mut hi = IV_HIGH;
    let p_lo = price_at(lo)?;
    let p_hi = price_at(hi)?;
    // Price rises with volatility: a quote at or below the floor price (intrinsic
    // for deep ITM contracts) or above the cap carries no volatility information.
    if market_price <= p_lo + IV_TOLERANCE || market_price > p_hi {
        return None;
    }

    for _ in 0..IV_MAX_ITERATIONS {
        let mid = 0.5 * (lo + hi);
        let diff = price_at(mid)? - market_price;
        if diff.abs() <= IV_TOLERANCE || (hi - lo) <= IV_TOLERANCE {
            return Some(mid);
        }
        if diff > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::greeks;
    use std::time::Instant;

    /// Compile smoke: the crate's lattice prices a European call within 0.5%
    /// of our Black-Scholes reference (S 100, K 100, T 1y, r 5%, σ 20%, q 0).
    #[test]
    fn smoke_binomial_european_call_matches_black_scholes() {
        let bs = greeks::black_scholes_price(100.0, 100.0, 1.0, 0.05, 0.2, true);
        assert!(
            (bs - 10.4506).abs() < 1e-3,
            "Black-Scholes reference moved: {bs}"
        );

        let params = BinomialPricingParams {
            asset: Positive::new(100.0).expect("spot"),
            volatility: Positive::new(0.2).expect("vol"),
            int_rate: Decimal::from_f64_retain(0.05).expect("rate"),
            strike: Positive::new(100.0).expect("strike"),
            expiry: Positive::new(1.0).expect("years"),
            no_steps: NonZeroUsize::new(BINOMIAL_STEPS).expect("steps"),
            option_type: &OptionType::European,
            option_style: &OptionStyle::Call,
            side: &Side::Long,
        };

        let started = Instant::now();
        let price = price_binomial(params)
            .expect("binomial price")
            .to_f64()
            .expect("decimal -> f64");
        let elapsed = started.elapsed();

        let rel = (price - bs).abs() / bs;
        assert!(
            rel < 0.005,
            "binomial {price} vs Black-Scholes {bs}: relative error {rel} (took {elapsed:?})"
        );
        eprintln!("smoke: binomial {price:.4} vs BS {bs:.4}, one 200-step tree took {elapsed:?}");
    }

    // ------------------------------------------------------------------
    // price_american / price_american_batch / implied_volatility_american
    // ------------------------------------------------------------------

    /// S, K, days, call? with r 5%, q 0, σ 20%.
    fn inputs(spot: f64, strike: f64, days_to_expiry: f64, is_call: bool) -> AmericanInputs {
        AmericanInputs {
            spot,
            strike,
            rate: 0.05,
            dividend_yield: 0.0,
            volatility: 0.20,
            days_to_expiry,
            is_call,
        }
    }

    fn assert_all_finite(res: &AmericanResult) {
        for (name, v) in [
            ("price", res.price),
            ("delta", res.delta),
            ("gamma", res.gamma),
            ("theta", res.theta),
            ("vega", res.vega),
            ("rho", res.rho),
            ("european_price", res.european_price),
            ("early_exercise_premium", res.early_exercise_premium),
        ] {
            assert!(v.is_finite(), "{name} is not finite: {v}");
        }
    }

    #[test]
    fn american_put_is_worth_at_least_the_european_put() {
        let res = price_american(&inputs(100.0, 100.0, 365.0, false)).expect("priced");
        assert_all_finite(&res);
        assert!(
            res.price >= res.european_price,
            "American put {} < European put {}",
            res.price,
            res.european_price
        );
        // An ATM one-year put at 5% rates carries a visible early-exercise premium.
        assert!(
            res.early_exercise_premium > 0.1,
            "premium too small: {}",
            res.early_exercise_premium
        );
        assert!(
            (res.early_exercise_premium - (res.price - res.european_price)).abs() < 1e-9,
            "premium must be price − European price"
        );
        // Same lattice: the European leg agrees with Black-Scholes within 0.5%.
        let bs_put = greeks::black_scholes_price(100.0, 100.0, 1.0, 0.05, 0.2, false);
        assert!(
            (res.european_price - bs_put).abs() / bs_put < 0.005,
            "European leg {} vs Black-Scholes {}",
            res.european_price,
            bs_put
        );
        assert!(res.theta < 0.0, "long put theta should be negative, got {}", res.theta);
        assert!(res.rho < 0.0, "put rho should be negative, got {}", res.rho);
        assert!(res.vega > 0.0 && res.gamma > 0.0);
    }

    #[test]
    fn deep_itm_american_put_exercises_early() {
        let res = price_american(&inputs(60.0, 100.0, 365.0, false)).expect("priced");
        assert_all_finite(&res);
        assert!(res.price >= 40.0 - 1e-9, "price {} below intrinsic 40", res.price);
        assert!(
            res.early_exercise_premium > 0.0,
            "deep ITM put must carry an early-exercise premium, got {}",
            res.early_exercise_premium
        );
        assert!(res.delta < -0.9, "deep ITM put delta {} should be near -1", res.delta);
    }

    /// A call on a non-dividend name is never exercised early, so the American
    /// lattice equals the European one and its greeks must agree with
    /// Black-Scholes in the units `greeks.rs` uses.
    #[test]
    fn call_without_dividends_matches_black_scholes_greeks() {
        let (s, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.2);

        // The shortcut in `exercise_style` rests on this equality; check it on the real lattices.
        let american = lattice_price_as(s, k, r, sigma, t, true, &OptionType::American).unwrap();
        let european = lattice_price_as(s, k, r, sigma, t, true, &OptionType::European).unwrap();
        assert_eq!(american, european, "American call lattice must equal the European one");

        let res = price_american(&inputs(s, k, 365.0, true)).expect("priced");
        assert_all_finite(&res);
        eprintln!(
            "parity: lattice price {:.4} delta {:.4} gamma {:.5} theta {:.5} vega {:.4} rho {:.3} | BS price {:.4} delta {:.4} gamma {:.5} theta {:.5} vega {:.4} rho {:.3}",
            res.price, res.delta, res.gamma, res.theta, res.vega, res.rho,
            greeks::black_scholes_price(s, k, t, r, sigma, true),
            greeks::delta(s, k, t, r, sigma, true),
            greeks::gamma(s, k, t, r, sigma),
            greeks::theta(s, k, t, r, sigma, true),
            greeks::vega(s, k, t, r, sigma),
            greeks::rho(s, k, t, r, sigma, true),
        );

        assert_eq!(res.price, european);
        assert_eq!(res.early_exercise_premium, 0.0);

        let bs_delta = greeks::delta(s, k, t, r, sigma, true);
        assert!(
            (res.delta - bs_delta).abs() < 0.01,
            "delta {} vs Black-Scholes {}",
            res.delta,
            bs_delta
        );

        let bs_vega = greeks::vega(s, k, t, r, sigma); // per 1 vol point
        assert!(
            (res.vega - bs_vega).abs() / bs_vega < 0.02,
            "vega {} vs Black-Scholes {}",
            res.vega,
            bs_vega
        );

        let bs_gamma = greeks::gamma(s, k, t, r, sigma);
        assert!(
            (res.gamma - bs_gamma).abs() / bs_gamma < 0.10,
            "gamma {} vs Black-Scholes {}",
            res.gamma,
            bs_gamma
        );

        let bs_theta = greeks::theta(s, k, t, r, sigma, true); // per day, negative
        assert!(res.theta < 0.0, "theta should be negative for a long call, got {}", res.theta);
        assert!(
            (res.theta - bs_theta).abs() / bs_theta.abs() < 0.10,
            "theta {} vs Black-Scholes {}",
            res.theta,
            bs_theta
        );

        let bs_rho = greeks::rho(s, k, t, r, sigma, true); // per 1.0 of rate
        assert!(
            (res.rho - bs_rho).abs() / bs_rho < 0.02,
            "rho {} vs Black-Scholes {}",
            res.rho,
            bs_rho
        );
    }

    /// Under negative rates a deep ITM call is worth exercising now, so the
    /// European shortcut must not apply and the premium must show.
    #[test]
    fn deep_itm_call_under_negative_rates_exercises_early() {
        let mut inp = inputs(150.0, 100.0, 365.0, true);
        inp.rate = -0.02;
        let res = price_american(&inp).expect("priced");
        assert_all_finite(&res);
        assert!(res.price >= 50.0 - 1e-9, "price {} below intrinsic 50", res.price);
        assert!(
            res.early_exercise_premium > 0.5,
            "expected a clear premium, got {}",
            res.early_exercise_premium
        );
    }

    #[test]
    fn expired_option_returns_intrinsic_and_zero_greeks() {
        let call = price_american(&inputs(105.0, 100.0, 0.0, true)).expect("priced");
        assert_all_finite(&call);
        assert!((call.price - 5.0).abs() < 1e-9, "call price {}", call.price);
        assert!((call.european_price - 5.0).abs() < 1e-9);
        assert_eq!(call.early_exercise_premium, 0.0);
        assert_eq!(
            (call.delta, call.gamma, call.theta, call.vega, call.rho),
            (0.0, 0.0, 0.0, 0.0, 0.0)
        );

        let put = price_american(&inputs(95.0, 100.0, -3.0, false)).expect("priced");
        assert!((put.price - 5.0).abs() < 1e-9, "put price {}", put.price);
        assert_eq!(put.delta, 0.0);

        let otm = price_american(&inputs(95.0, 100.0, 0.0, true)).expect("priced");
        assert_eq!(otm.price, 0.0);
    }

    #[test]
    fn zero_or_negative_volatility_is_a_validation_error() {
        for vol in [0.0, -0.2] {
            let mut inp = inputs(100.0, 100.0, 30.0, false);
            inp.volatility = vol;
            match price_american(&inp) {
                Err(AppError::Validation(msg)) => {
                    assert!(msg.to_lowercase().contains("volatility"), "message: {msg}")
                }
                other => panic!("volatility {vol} should fail validation, got {other:?}"),
            }
        }
    }

    #[test]
    fn nonpositive_spot_or_strike_is_a_validation_error() {
        let mut inp = inputs(0.0, 100.0, 30.0, true);
        assert!(matches!(price_american(&inp), Err(AppError::Validation(_))));
        inp = inputs(100.0, -5.0, 30.0, true);
        assert!(matches!(price_american(&inp), Err(AppError::Validation(_))));
        inp = inputs(f64::NAN, 100.0, 30.0, true);
        assert!(matches!(price_american(&inp), Err(AppError::Validation(_))));
    }

    /// The lattice has no dividend term (and optionstratlib's own
    /// `Options::calculate_price_binomial` ignores `dividend_yield`), so a
    /// non-zero yield is refused loudly instead of priced wrongly.
    #[test]
    fn dividend_yield_is_rejected_by_this_engine() {
        let mut inp = inputs(100.0, 100.0, 365.0, true);
        inp.dividend_yield = 0.08; // above the 5% rate
        match price_american(&inp) {
            Err(AppError::Validation(msg)) => {
                assert!(msg.to_lowercase().contains("dividend"), "message: {msg}")
            }
            other => panic!("dividend yield should fail validation, got {other:?}"),
        }
    }

    #[test]
    fn implied_volatility_round_trips_through_the_lattice() {
        let mut inp = inputs(100.0, 100.0, 182.5, false);
        inp.volatility = 0.25;
        let market = price_american(&inp).expect("priced").price;

        // The solver must not peek at the true volatility.
        inp.volatility = 0.20;
        let iv = implied_volatility_american(market, &inp).expect("converged");
        assert!((iv - 0.25).abs() < 1e-3, "implied vol {iv} vs 0.25");
    }

    #[test]
    fn implied_volatility_is_none_when_no_volatility_can_explain_the_price() {
        let deep_put = inputs(60.0, 100.0, 365.0, false); // intrinsic 40
        assert_eq!(implied_volatility_american(30.0, &deep_put), None);
        assert_eq!(implied_volatility_american(0.0, &deep_put), None);
        assert_eq!(implied_volatility_american(-1.0, &deep_put), None);

        let expired = inputs(100.0, 100.0, 0.0, true);
        assert_eq!(implied_volatility_american(1.0, &expired), None);

        // Far above anything σ ≤ 5.0 can produce.
        let call = inputs(100.0, 100.0, 30.0, true);
        assert_eq!(implied_volatility_american(150.0, &call), None);
    }

    #[test]
    fn batch_matches_single_contract_pricing_and_keeps_order() {
        let contracts = [
            inputs(100.0, 90.0, 45.0, true),
            inputs(100.0, 100.0, 45.0, false),
            inputs(100.0, 110.0, 45.0, true),
            inputs(100.0, 110.0, 45.0, false),
        ];
        let batch = price_american_batch(&contracts);
        assert_eq!(batch.len(), contracts.len());
        for (inp, res) in contracts.iter().zip(&batch) {
            let single = price_american(inp).expect("priced");
            assert_eq!(res.as_ref().expect("priced in batch"), &single);
        }
        assert!(price_american_batch(&[]).is_empty());
    }

    #[test]
    fn batch_reports_a_bad_contract_without_failing_the_others() {
        let mut bad = inputs(100.0, 100.0, 45.0, true);
        bad.volatility = 0.0;
        let batch = price_american_batch(&[inputs(100.0, 95.0, 45.0, true), bad]);
        assert!(batch[0].is_ok());
        assert!(matches!(batch[1], Err(AppError::Validation(_))));
    }

    /// Tripwire for an accidental O(steps²) evaluation per greek (that would be
    /// a ≥100× slowdown). Measured 4.65 s on an idle 6-core i5-8500; the bound
    /// is loose because this is wall-clock time on a machine that also builds.
    #[test]
    fn a_batch_of_200_contracts_prices_within_the_time_budget() {
        let t0 = Instant::now();
        price_american(&inputs(100.0, 100.0, 45.0, false)).expect("priced");
        let one_put = t0.elapsed();
        let t0 = Instant::now();
        price_american(&inputs(100.0, 100.0, 45.0, true)).expect("priced");
        let one_call = t0.elapsed();

        let mut contracts = Vec::with_capacity(200);
        for strike in 50..150 {
            for is_call in [true, false] {
                contracts.push(inputs(100.0, strike as f64, 45.0, is_call));
            }
        }
        let started = Instant::now();
        let batch = price_american_batch(&contracts);
        let elapsed = started.elapsed();
        eprintln!(
            "batch: {} contracts × 200 steps in {elapsed:?} (one put {one_put:?}, one call {one_call:?}, {} workers)",
            batch.len(),
            thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
        );

        assert_eq!(batch.len(), 200);
        for res in &batch {
            let res = res.as_ref().expect("priced");
            assert_all_finite(res);
            assert!(res.price >= 0.0);
        }
        assert!(
            elapsed.as_secs_f64() < 30.0,
            "200 contracts took {elapsed:?} (budget 30 s)"
        );
    }
}
