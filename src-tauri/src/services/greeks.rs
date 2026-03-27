//! Options Greeks Calculation Service
//!
//! Pure math module implementing Black-Scholes options pricing and Greeks.
//! Uses the statrs crate for Normal distribution CDF calculations.

use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, Normal};
use std::f64::consts::PI;

/// Computed Greeks and pricing for a single option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionGreeksResult {
    pub strike: f64,
    pub expiry: String,
    pub option_type: String, // "call" or "put"
    pub theoretical_price: f64,
    pub implied_volatility: Option<f64>,
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

/// A single point on the implied volatility surface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IVSurfacePoint {
    pub strike: f64,
    pub expiry_days: i32,
    pub iv: Option<f64>,
}

// ============================================================================
// Helper functions
// ============================================================================

/// Calculate d1 in the Black-Scholes formula
///
/// d1 = (ln(S/K) + (r + sigma^2/2) * t) / (sigma * sqrt(t))
pub fn d1(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    ((s / k).ln() + (r + sigma * sigma / 2.0) * t) / (sigma * t.sqrt())
}

/// Calculate d2 in the Black-Scholes formula
///
/// d2 = d1 - sigma * sqrt(t)
pub fn d2(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    d1(s, k, t, r, sigma) - sigma * t.sqrt()
}

// ============================================================================
// Black-Scholes pricing
// ============================================================================

/// Calculate the Black-Scholes option price
///
/// # Arguments
/// * `s` - Spot price of the underlying
/// * `k` - Strike price
/// * `t` - Time to expiry in years
/// * `r` - Risk-free interest rate (annualized, e.g. 0.05 for 5%)
/// * `sigma` - Volatility (annualized, e.g. 0.20 for 20%)
/// * `is_call` - true for call option, false for put option
pub fn black_scholes_price(s: f64, k: f64, t: f64, r: f64, sigma: f64, is_call: bool) -> f64 {
    if t <= 0.0 {
        // At expiry, return intrinsic value
        if is_call {
            return (s - k).max(0.0);
        } else {
            return (k - s).max(0.0);
        }
    }

    let norm = Normal::new(0.0, 1.0).unwrap();
    let d1_val = d1(s, k, t, r, sigma);
    let d2_val = d2(s, k, t, r, sigma);

    if is_call {
        s * norm.cdf(d1_val) - k * (-r * t).exp() * norm.cdf(d2_val)
    } else {
        k * (-r * t).exp() * norm.cdf(-d2_val) - s * norm.cdf(-d1_val)
    }
}

// ============================================================================
// Implied Volatility (Newton-Raphson)
// ============================================================================

/// Calculate implied volatility using Newton-Raphson method
///
/// # Arguments
/// * `market_price` - Observed market price of the option
/// * `s` - Spot price
/// * `k` - Strike price
/// * `t` - Time to expiry in years
/// * `r` - Risk-free rate
/// * `is_call` - true for call, false for put
///
/// # Returns
/// `Some(iv)` if converged, `None` if the method did not converge
pub fn implied_volatility(
    market_price: f64,
    s: f64,
    k: f64,
    t: f64,
    r: f64,
    is_call: bool,
) -> Option<f64> {
    if t <= 0.0 || market_price <= 0.0 {
        return None;
    }

    let max_iterations = 100;
    let tolerance = 1e-8;
    let mut sigma = 0.3; // Initial guess

    for _ in 0..max_iterations {
        let price = black_scholes_price(s, k, t, r, sigma, is_call);
        let vega_val = vega_raw(s, k, t, r, sigma);

        // Avoid division by near-zero vega
        if vega_val.abs() < 1e-12 {
            return None;
        }

        let diff = price - market_price;

        if diff.abs() < tolerance {
            return Some(sigma);
        }

        sigma -= diff / vega_val;

        // Enforce bounds
        if sigma < 0.001 {
            sigma = 0.001;
        }
        if sigma > 5.0 {
            sigma = 5.0;
        }
    }

    None
}

// ============================================================================
// Greeks
// ============================================================================

/// Standard normal probability density function
fn norm_pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / (2.0 * PI).sqrt()
}

/// Calculate Delta
///
/// Call delta = N(d1)
/// Put delta  = N(d1) - 1
pub fn delta(s: f64, k: f64, t: f64, r: f64, sigma: f64, is_call: bool) -> f64 {
    if t <= 0.0 {
        if is_call {
            return if s > k { 1.0 } else { 0.0 };
        } else {
            return if s < k { -1.0 } else { 0.0 };
        }
    }

    let norm = Normal::new(0.0, 1.0).unwrap();
    let d1_val = d1(s, k, t, r, sigma);

    if is_call {
        norm.cdf(d1_val)
    } else {
        norm.cdf(d1_val) - 1.0
    }
}

/// Calculate Gamma (same for calls and puts)
///
/// Gamma = N'(d1) / (S * sigma * sqrt(t))
pub fn gamma(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }

    let d1_val = d1(s, k, t, r, sigma);
    norm_pdf(d1_val) / (s * sigma * t.sqrt())
}

/// Calculate Theta (per day, divided by 365)
///
/// Call theta = -(S * N'(d1) * sigma) / (2 * sqrt(t)) - r * K * e^(-rt) * N(d2)
/// Put theta  = -(S * N'(d1) * sigma) / (2 * sqrt(t)) + r * K * e^(-rt) * N(-d2)
pub fn theta(s: f64, k: f64, t: f64, r: f64, sigma: f64, is_call: bool) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }

    let norm = Normal::new(0.0, 1.0).unwrap();
    let d1_val = d1(s, k, t, r, sigma);
    let d2_val = d2(s, k, t, r, sigma);
    let discount = (-r * t).exp();

    let common = -(s * norm_pdf(d1_val) * sigma) / (2.0 * t.sqrt());

    let annual_theta = if is_call {
        common - r * k * discount * norm.cdf(d2_val)
    } else {
        common + r * k * discount * norm.cdf(-d2_val)
    };

    // Per day
    annual_theta / 365.0
}

/// Raw vega (not divided by 100) - used internally for Newton-Raphson
///
/// Vega = S * N'(d1) * sqrt(t)
fn vega_raw(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }

    let d1_val = d1(s, k, t, r, sigma);
    s * norm_pdf(d1_val) * t.sqrt()
}

/// Calculate Vega (per 1% move in volatility, divided by 100)
///
/// Vega = S * N'(d1) * sqrt(t) / 100
pub fn vega(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    vega_raw(s, k, t, r, sigma) / 100.0
}

/// Calculate Rho
///
/// Call rho = K * t * e^(-rt) * N(d2)
/// Put rho  = -K * t * e^(-rt) * N(-d2)
pub fn rho(s: f64, k: f64, t: f64, r: f64, sigma: f64, is_call: bool) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }

    let norm = Normal::new(0.0, 1.0).unwrap();
    let d2_val = d2(s, k, t, r, sigma);
    let discount = (-r * t).exp();

    if is_call {
        k * t * discount * norm.cdf(d2_val)
    } else {
        -k * t * discount * norm.cdf(-d2_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_put_parity() {
        // Put-Call parity: C - P = S - K*e^(-rT)
        let s = 100.0;
        let k = 100.0;
        let t = 1.0;
        let r = 0.05;
        let sigma = 0.2;

        let call = black_scholes_price(s, k, t, r, sigma, true);
        let put = black_scholes_price(s, k, t, r, sigma, false);
        let parity = s - k * (-r * t).exp();

        assert!((call - put - parity).abs() < 1e-10);
    }

    #[test]
    fn test_atm_call_price() {
        // ATM call with known parameters
        let price = black_scholes_price(100.0, 100.0, 1.0, 0.05, 0.2, true);
        // Expected ~10.45 for these standard parameters
        assert!(price > 10.0 && price < 11.0);
    }

    #[test]
    fn test_implied_volatility_roundtrip() {
        let s = 100.0;
        let k = 100.0;
        let t = 0.5;
        let r = 0.05;
        let sigma = 0.25;

        let price = black_scholes_price(s, k, t, r, sigma, true);
        let iv = implied_volatility(price, s, k, t, r, true);

        assert!(iv.is_some());
        assert!((iv.unwrap() - sigma).abs() < 1e-6);
    }

    #[test]
    fn test_delta_bounds() {
        let s = 100.0;
        let k = 100.0;
        let t = 1.0;
        let r = 0.05;
        let sigma = 0.2;

        let call_delta = delta(s, k, t, r, sigma, true);
        let put_delta = delta(s, k, t, r, sigma, false);

        // Call delta should be between 0 and 1
        assert!(call_delta > 0.0 && call_delta < 1.0);
        // Put delta should be between -1 and 0
        assert!(put_delta > -1.0 && put_delta < 0.0);
        // Call delta - put delta = 1
        assert!((call_delta - put_delta - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_gamma_positive() {
        let g = gamma(100.0, 100.0, 1.0, 0.05, 0.2);
        assert!(g > 0.0);
    }

    #[test]
    fn test_theta_negative_for_long() {
        // Theta should be negative for long options (time decay)
        let call_theta = theta(100.0, 100.0, 1.0, 0.05, 0.2, true);
        let put_theta = theta(100.0, 100.0, 1.0, 0.05, 0.2, false);
        assert!(call_theta < 0.0);
        assert!(put_theta < 0.0);
    }

    #[test]
    fn test_vega_positive() {
        let v = vega(100.0, 100.0, 1.0, 0.05, 0.2);
        assert!(v > 0.0);
    }

    #[test]
    fn test_expired_option() {
        let call = black_scholes_price(105.0, 100.0, 0.0, 0.05, 0.2, true);
        assert!((call - 5.0).abs() < 1e-10);

        let put = black_scholes_price(95.0, 100.0, 0.0, 0.05, 0.2, false);
        assert!((put - 5.0).abs() < 1e-10);
    }
}
