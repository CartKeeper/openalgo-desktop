//! Options Greeks Tauri IPC commands
//!
//! Exposes option pricing and Greeks to the frontend via Tauri's invoke
//! system: Black-Scholes (`services::greeks`) by default, or the binomial
//! American model (`services::american_pricing`) when `model = "binomial"`.

use crate::error::AppError;
use crate::services::american_pricing::{self, AmericanInputs, AmericanResult};
use crate::services::greeks::{self, IVSurfacePoint, OptionGreeksResult};

/// Pricing model selected by the Options page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PricingModel {
    BlackScholes,
    Binomial,
}

impl PricingModel {
    /// `None` and `"black_scholes"` keep the original Black-Scholes path.
    fn parse(model: Option<&str>) -> Result<Self, AppError> {
        match model {
            None | Some("black_scholes") => Ok(Self::BlackScholes),
            Some("binomial") => Ok(Self::Binomial),
            Some(other) => Err(AppError::Validation(format!(
                "Unknown pricing model: {other}"
            ))),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::BlackScholes => "black_scholes",
            Self::Binomial => "binomial",
        }
    }
}

fn option_type_name(is_call: bool) -> String {
    if is_call {
        "call".to_string()
    } else {
        "put".to_string()
    }
}

/// The app has no dividend-yield input; the lattice is run with q = 0.
fn american_inputs(
    spot_price: f64,
    strike: f64,
    expiry_days: i32,
    risk_free_rate: f64,
    volatility: f64,
    is_call: bool,
) -> AmericanInputs {
    AmericanInputs {
        spot: spot_price,
        strike,
        rate: risk_free_rate,
        dividend_yield: 0.0,
        volatility,
        days_to_expiry: expiry_days as f64,
        is_call,
    }
}

/// Shape a lattice result like the Black-Scholes results the page already renders.
fn binomial_result(
    strike: f64,
    expiry: String,
    is_call: bool,
    implied_volatility: Option<f64>,
    res: &AmericanResult,
) -> OptionGreeksResult {
    OptionGreeksResult {
        strike,
        expiry,
        option_type: option_type_name(is_call),
        theoretical_price: res.price,
        implied_volatility,
        delta: res.delta,
        gamma: res.gamma,
        theta: res.theta,
        vega: res.vega,
        rho: res.rho,
        model: PricingModel::Binomial.name().to_string(),
        early_exercise_premium: Some(res.early_exercise_premium),
    }
}

/// The lattice is CPU-bound (tens of milliseconds per contract), so it runs
/// off the async runtime's worker threads.
async fn run_blocking<T, F>(f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(format!("binomial pricing task failed: {e}")))?
}

/// Compute all Greeks for a single option
///
/// If `market_price` is provided, implied volatility will also be computed.
/// `model` selects `"black_scholes"` (default) or `"binomial"` (American).
#[tauri::command]
pub async fn compute_greeks(
    spot_price: f64,
    strike: f64,
    expiry_days: i32,
    risk_free_rate: f64,
    volatility: f64,
    is_call: bool,
    market_price: Option<f64>,
    model: Option<String>,
) -> Result<OptionGreeksResult, AppError> {
    let model = PricingModel::parse(model.as_deref())?;
    if expiry_days < 0 {
        return Err(AppError::Validation(
            "expiry_days must be non-negative".to_string(),
        ));
    }
    if spot_price <= 0.0 {
        return Err(AppError::Validation(
            "spot_price must be positive".to_string(),
        ));
    }
    if strike <= 0.0 {
        return Err(AppError::Validation(
            "strike must be positive".to_string(),
        ));
    }
    if volatility <= 0.0 {
        return Err(AppError::Validation(
            "volatility must be positive".to_string(),
        ));
    }

    let t = expiry_days as f64 / 365.0;
    let expiry = format!("{}d", expiry_days);

    if model == PricingModel::Binomial {
        let inp = american_inputs(spot_price, strike, expiry_days, risk_free_rate, volatility, is_call);
        let (res, iv) = run_blocking(move || {
            let res = american_pricing::price_american(&inp)?;
            let iv = market_price
                .and_then(|mp| american_pricing::implied_volatility_american(mp, &inp));
            Ok((res, iv))
        })
        .await?;
        return Ok(binomial_result(strike, expiry, is_call, iv, &res));
    }

    let theoretical_price =
        greeks::black_scholes_price(spot_price, strike, t, risk_free_rate, volatility, is_call);

    let iv = market_price
        .and_then(|mp| greeks::implied_volatility(mp, spot_price, strike, t, risk_free_rate, is_call));

    Ok(OptionGreeksResult {
        strike,
        expiry,
        option_type: option_type_name(is_call),
        theoretical_price,
        implied_volatility: iv,
        delta: greeks::delta(spot_price, strike, t, risk_free_rate, volatility, is_call),
        gamma: greeks::gamma(spot_price, strike, t, risk_free_rate, volatility),
        theta: greeks::theta(spot_price, strike, t, risk_free_rate, volatility, is_call),
        vega: greeks::vega(spot_price, strike, t, risk_free_rate, volatility),
        rho: greeks::rho(spot_price, strike, t, risk_free_rate, volatility, is_call),
        model: PricingModel::BlackScholes.name().to_string(),
        early_exercise_premium: None,
    })
}

/// Compute Greeks for multiple strikes at once (both call and put for each strike)
///
/// `model` selects `"black_scholes"` (default) or `"binomial"` (American).
#[tauri::command]
pub async fn compute_greeks_batch(
    spot_price: f64,
    strikes: Vec<f64>,
    expiry_days: i32,
    risk_free_rate: f64,
    volatility: f64,
    model: Option<String>,
) -> Result<Vec<OptionGreeksResult>, AppError> {
    let model = PricingModel::parse(model.as_deref())?;
    if expiry_days < 0 {
        return Err(AppError::Validation(
            "expiry_days must be non-negative".to_string(),
        ));
    }
    if spot_price <= 0.0 {
        return Err(AppError::Validation(
            "spot_price must be positive".to_string(),
        ));
    }
    if volatility <= 0.0 {
        return Err(AppError::Validation(
            "volatility must be positive".to_string(),
        ));
    }

    let t = expiry_days as f64 / 365.0;
    let expiry_label = format!("{}d", expiry_days);

    if model == PricingModel::Binomial {
        // Call then put for every positive strike, in input order (as the Black-Scholes path).
        let contracts: Vec<(f64, bool)> = strikes
            .iter()
            .copied()
            .filter(|k| *k > 0.0)
            .flat_map(|k| [(k, true), (k, false)])
            .collect();
        let inputs: Vec<AmericanInputs> = contracts
            .iter()
            .map(|&(k, is_call)| {
                american_inputs(spot_price, k, expiry_days, risk_free_rate, volatility, is_call)
            })
            .collect();
        let priced = run_blocking(move || Ok(american_pricing::price_american_batch(&inputs))).await?;

        let mut results = Vec::with_capacity(priced.len());
        for ((k, is_call), res) in contracts.into_iter().zip(priced) {
            results.push(binomial_result(k, expiry_label.clone(), is_call, None, &res?));
        }
        return Ok(results);
    }

    let mut results = Vec::with_capacity(strikes.len() * 2);

    for &k in &strikes {
        if k <= 0.0 {
            continue;
        }

        // Call
        results.push(OptionGreeksResult {
            strike: k,
            expiry: expiry_label.clone(),
            option_type: option_type_name(true),
            theoretical_price: greeks::black_scholes_price(
                spot_price,
                k,
                t,
                risk_free_rate,
                volatility,
                true,
            ),
            implied_volatility: None,
            delta: greeks::delta(spot_price, k, t, risk_free_rate, volatility, true),
            gamma: greeks::gamma(spot_price, k, t, risk_free_rate, volatility),
            theta: greeks::theta(spot_price, k, t, risk_free_rate, volatility, true),
            vega: greeks::vega(spot_price, k, t, risk_free_rate, volatility),
            rho: greeks::rho(spot_price, k, t, risk_free_rate, volatility, true),
            model: PricingModel::BlackScholes.name().to_string(),
            early_exercise_premium: None,
        });

        // Put
        results.push(OptionGreeksResult {
            strike: k,
            expiry: expiry_label.clone(),
            option_type: option_type_name(false),
            theoretical_price: greeks::black_scholes_price(
                spot_price,
                k,
                t,
                risk_free_rate,
                volatility,
                false,
            ),
            implied_volatility: None,
            delta: greeks::delta(spot_price, k, t, risk_free_rate, volatility, false),
            gamma: greeks::gamma(spot_price, k, t, risk_free_rate, volatility),
            theta: greeks::theta(spot_price, k, t, risk_free_rate, volatility, false),
            vega: greeks::vega(spot_price, k, t, risk_free_rate, volatility),
            rho: greeks::rho(spot_price, k, t, risk_free_rate, volatility, false),
            model: PricingModel::BlackScholes.name().to_string(),
            early_exercise_premium: None,
        });
    }

    Ok(results)
}

/// Compute implied volatility for a grid of strike x expiry combinations
///
/// `market_prices` is a 2D grid: `market_prices[i][j]` is the price for
/// `strikes[j]` at `expiry_days_list[i]`.
#[tauri::command]
pub async fn compute_iv_surface(
    spot_price: f64,
    strikes: Vec<f64>,
    expiry_days_list: Vec<i32>,
    risk_free_rate: f64,
    market_prices: Vec<Vec<f64>>,
    is_call: bool,
) -> Result<Vec<IVSurfacePoint>, AppError> {
    if spot_price <= 0.0 {
        return Err(AppError::Validation(
            "spot_price must be positive".to_string(),
        ));
    }
    if market_prices.len() != expiry_days_list.len() {
        return Err(AppError::Validation(
            "market_prices rows must match expiry_days_list length".to_string(),
        ));
    }

    let mut surface = Vec::with_capacity(expiry_days_list.len() * strikes.len());

    for (i, &expiry_days) in expiry_days_list.iter().enumerate() {
        let t = expiry_days as f64 / 365.0;
        let row = &market_prices[i];

        if row.len() != strikes.len() {
            return Err(AppError::Validation(format!(
                "market_prices row {} length ({}) must match strikes length ({})",
                i,
                row.len(),
                strikes.len()
            )));
        }

        for (j, &k) in strikes.iter().enumerate() {
            let mp = row[j];
            let iv = if mp > 0.0 && k > 0.0 && t > 0.0 {
                greeks::implied_volatility(mp, spot_price, k, t, risk_free_rate, is_call)
            } else {
                None
            };

            surface.push(IVSurfacePoint {
                strike: k,
                expiry_days,
                iv,
            });
        }
    }

    Ok(surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINOMIAL: Option<&str> = Some("binomial");

    fn model(m: Option<&str>) -> Option<String> {
        m.map(str::to_string)
    }

    #[tokio::test]
    async fn default_model_is_black_scholes_with_unchanged_numbers() {
        let res = compute_greeks(100.0, 100.0, 365, 0.05, 0.2, true, None, None)
            .await
            .expect("priced");
        assert_eq!(res.model, "black_scholes");
        assert_eq!(res.early_exercise_premium, None);
        assert_eq!(res.option_type, "call");
        assert_eq!(res.expiry, "365d");

        // Pinned Black-Scholes values for S 100, K 100, 1y, 5%, 20% (call).
        assert!((res.theoretical_price - 10.4506).abs() < 1e-3, "{}", res.theoretical_price);
        assert!((res.delta - 0.6368).abs() < 1e-4, "{}", res.delta);
        assert_eq!(
            res.theoretical_price,
            greeks::black_scholes_price(100.0, 100.0, 1.0, 0.05, 0.2, true)
        );
        assert_eq!(res.gamma, greeks::gamma(100.0, 100.0, 1.0, 0.05, 0.2));
        assert_eq!(res.theta, greeks::theta(100.0, 100.0, 1.0, 0.05, 0.2, true));
        assert_eq!(res.vega, greeks::vega(100.0, 100.0, 1.0, 0.05, 0.2));
        assert_eq!(res.rho, greeks::rho(100.0, 100.0, 1.0, 0.05, 0.2, true));

        // Naming the model explicitly is the same path.
        let explicit = compute_greeks(100.0, 100.0, 365, 0.05, 0.2, true, None, model(Some("black_scholes")))
            .await
            .expect("priced");
        assert_eq!(explicit.theoretical_price, res.theoretical_price);
        assert_eq!(explicit.delta, res.delta);
        assert_eq!(explicit.model, "black_scholes");
    }

    #[tokio::test]
    async fn black_scholes_implied_volatility_is_unchanged() {
        let market = greeks::black_scholes_price(100.0, 100.0, 0.5, 0.05, 0.25, true);
        let res = compute_greeks(100.0, 100.0, 182, 0.05, 0.2, true, Some(market), None)
            .await
            .expect("priced");
        let expected = greeks::implied_volatility(market, 100.0, 100.0, 182.0 / 365.0, 0.05, true);
        assert_eq!(res.implied_volatility, expected);
    }

    #[tokio::test]
    async fn binomial_put_reports_an_early_exercise_premium() {
        let res = compute_greeks(100.0, 100.0, 365, 0.05, 0.2, false, None, model(BINOMIAL))
            .await
            .expect("priced");
        assert_eq!(res.model, "binomial");
        assert_eq!(res.option_type, "put");
        let premium = res.early_exercise_premium.expect("binomial reports the premium");
        assert!(premium > 0.1, "premium {premium}");
        assert!(
            res.theoretical_price > greeks::black_scholes_price(100.0, 100.0, 1.0, 0.05, 0.2, false),
            "American put must be worth more than the Black-Scholes European put"
        );
        assert_eq!(res.implied_volatility, None);
        assert!(res.delta < 0.0 && res.gamma > 0.0 && res.theta < 0.0 && res.vega > 0.0);
    }

    #[tokio::test]
    async fn binomial_call_without_dividends_has_zero_premium() {
        let res = compute_greeks(100.0, 100.0, 365, 0.05, 0.2, true, None, model(BINOMIAL))
            .await
            .expect("priced");
        assert_eq!(res.model, "binomial");
        assert_eq!(res.early_exercise_premium, Some(0.0));
    }

    #[tokio::test]
    async fn binomial_implied_volatility_comes_from_the_lattice() {
        let priced = compute_greeks(100.0, 100.0, 182, 0.05, 0.25, false, None, model(BINOMIAL))
            .await
            .expect("priced");
        let res = compute_greeks(
            100.0,
            100.0,
            182,
            0.05,
            0.20,
            false,
            Some(priced.theoretical_price),
            model(BINOMIAL),
        )
        .await
        .expect("priced");
        let iv = res.implied_volatility.expect("solved on the lattice");
        assert!((iv - 0.25).abs() < 1e-3, "implied vol {iv}");

        // A quote below intrinsic has no implied volatility: the page shows "--".
        let deep = compute_greeks(60.0, 100.0, 365, 0.05, 0.2, false, Some(30.0), model(BINOMIAL))
            .await
            .expect("priced");
        assert_eq!(deep.implied_volatility, None);
        assert!(deep.theoretical_price >= 40.0 - 1e-9);
    }

    #[tokio::test]
    async fn unknown_model_is_a_validation_error() {
        match compute_greeks(100.0, 100.0, 30, 0.05, 0.2, true, None, model(Some("trinomial"))).await {
            Err(AppError::Validation(msg)) => {
                assert_eq!(msg, "Unknown pricing model: trinomial")
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
        match compute_greeks_batch(100.0, vec![100.0], 30, 0.05, 0.2, model(Some("trinomial"))).await {
            Err(AppError::Validation(msg)) => {
                assert_eq!(msg, "Unknown pricing model: trinomial")
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn existing_input_validation_still_applies_to_binomial() {
        assert!(matches!(
            compute_greeks(100.0, 100.0, -1, 0.05, 0.2, true, None, model(BINOMIAL)).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            compute_greeks(100.0, 100.0, 30, 0.05, 0.0, true, None, model(BINOMIAL)).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            compute_greeks_batch(0.0, vec![100.0], 30, 0.05, 0.2, model(BINOMIAL)).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn batch_default_is_black_scholes_and_binomial_adds_premiums() {
        let bs = compute_greeks_batch(100.0, vec![60.0, 100.0], 365, 0.05, 0.2, None)
            .await
            .expect("priced");
        assert_eq!(bs.len(), 4);
        assert!(bs.iter().all(|r| r.model == "black_scholes" && r.early_exercise_premium.is_none()));
        // Call then put for each strike, as before.
        assert_eq!(
            bs.iter().map(|r| (r.strike, r.option_type.as_str())).collect::<Vec<_>>(),
            vec![(60.0, "call"), (60.0, "put"), (100.0, "call"), (100.0, "put")]
        );
        assert_eq!(
            bs[0].theoretical_price,
            greeks::black_scholes_price(100.0, 60.0, 1.0, 0.05, 0.2, true)
        );
        assert_eq!(bs[3].delta, greeks::delta(100.0, 100.0, 1.0, 0.05, 0.2, false));

        let bin = compute_greeks_batch(100.0, vec![60.0, 100.0], 365, 0.05, 0.2, model(BINOMIAL))
            .await
            .expect("priced");
        assert_eq!(bin.len(), 4);
        assert!(bin.iter().all(|r| r.model == "binomial" && r.expiry == "365d"));
        assert_eq!(
            bin.iter().map(|r| (r.strike, r.option_type.as_str())).collect::<Vec<_>>(),
            vec![(60.0, "call"), (60.0, "put"), (100.0, "call"), (100.0, "put")]
        );
        assert_eq!(bin[2].early_exercise_premium, Some(0.0)); // ATM call
        assert!(bin[3].early_exercise_premium.expect("put premium") > 0.1); // ATM put
        assert!(bin[3].theoretical_price > bs[3].theoretical_price);

        // Non-positive strikes are skipped, as in the Black-Scholes path.
        let skipped = compute_greeks_batch(100.0, vec![0.0, -5.0, 100.0], 30, 0.05, 0.2, model(BINOMIAL))
            .await
            .expect("priced");
        assert_eq!(skipped.len(), 2);
        assert!(compute_greeks_batch(100.0, vec![], 30, 0.05, 0.2, model(BINOMIAL))
            .await
            .expect("priced")
            .is_empty());
    }
}
