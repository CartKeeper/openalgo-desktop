//! American Option Pricing Service (binomial tree)
//!
//! Wraps the Cox-Ross-Rubinstein lattice from `optionstratlib`
//! (`pricing::binomial_model::price_binomial`) so the Options page can price
//! American-style contracts — honoring early exercise — beside the closed-form
//! Black-Scholes model in `services/greeks.rs`.
//!
//! Pure math: no I/O, no state. Greeks are derived by central finite
//! differences in the same units `greeks.rs` uses.

/// Number of steps in the binomial lattice (spec: "Binomial steps" = 200).
pub const BINOMIAL_STEPS: usize = 200;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::greeks;
    use optionstratlib::model::Positive;
    use optionstratlib::pricing::binomial_model::{BinomialPricingParams, price_binomial};
    use optionstratlib::{OptionStyle, OptionType, Side};
    use rust_decimal::Decimal;
    use rust_decimal::prelude::ToPrimitive;
    use std::num::NonZeroUsize;

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

        let started = std::time::Instant::now();
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
}
