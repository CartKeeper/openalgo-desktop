//! Options Greeks Tauri IPC commands
//!
//! Exposes Black-Scholes Greeks computation to the frontend via Tauri's invoke system.

use crate::error::AppError;
use crate::services::greeks::{self, IVSurfacePoint, OptionGreeksResult};

/// Compute all Greeks for a single option
///
/// If `market_price` is provided, implied volatility will also be computed.
#[tauri::command]
pub async fn compute_greeks(
    spot_price: f64,
    strike: f64,
    expiry_days: i32,
    risk_free_rate: f64,
    volatility: f64,
    is_call: bool,
    market_price: Option<f64>,
) -> Result<OptionGreeksResult, AppError> {
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

    let theoretical_price =
        greeks::black_scholes_price(spot_price, strike, t, risk_free_rate, volatility, is_call);

    let iv = market_price
        .and_then(|mp| greeks::implied_volatility(mp, spot_price, strike, t, risk_free_rate, is_call));

    let option_type = if is_call {
        "call".to_string()
    } else {
        "put".to_string()
    };

    Ok(OptionGreeksResult {
        strike,
        expiry: format!("{}d", expiry_days),
        option_type,
        theoretical_price,
        implied_volatility: iv,
        delta: greeks::delta(spot_price, strike, t, risk_free_rate, volatility, is_call),
        gamma: greeks::gamma(spot_price, strike, t, risk_free_rate, volatility),
        theta: greeks::theta(spot_price, strike, t, risk_free_rate, volatility, is_call),
        vega: greeks::vega(spot_price, strike, t, risk_free_rate, volatility),
        rho: greeks::rho(spot_price, strike, t, risk_free_rate, volatility, is_call),
    })
}

/// Compute Greeks for multiple strikes at once (both call and put for each strike)
#[tauri::command]
pub async fn compute_greeks_batch(
    spot_price: f64,
    strikes: Vec<f64>,
    expiry_days: i32,
    risk_free_rate: f64,
    volatility: f64,
) -> Result<Vec<OptionGreeksResult>, AppError> {
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
    let mut results = Vec::with_capacity(strikes.len() * 2);

    for &k in &strikes {
        if k <= 0.0 {
            continue;
        }

        // Call
        results.push(OptionGreeksResult {
            strike: k,
            expiry: expiry_label.clone(),
            option_type: "call".to_string(),
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
        });

        // Put
        results.push(OptionGreeksResult {
            strike: k,
            expiry: expiry_label.clone(),
            option_type: "put".to_string(),
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
