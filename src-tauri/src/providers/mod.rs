//! Data providers module
//!
//! External data provider clients for market data, fundamentals, news, and economic data.
//! These providers are used in Generic (Research Only) mode and supplement broker data.
//!
//! # Providers
//!
//! - `FmpClient` - Financial Modeling Prep (fundamentals, news, screener, analyst)
//! - `YahooClient` - Yahoo Finance (prices, quotes, options chains, global symbols)
//! - `FredClient` - Federal Reserve Economic Data (economic indicators, interest rates)

pub mod types;
pub mod fmp;
pub mod yahoo;
pub mod fred;
pub mod anthropic;

use crate::error::AppError;

/// Provider-specific error that converts to AppError::Provider
#[derive(Debug)]
pub enum ProviderError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// Failed to parse response
    Parse(String),
    /// API key not configured
    NoApiKey(String),
    /// Rate limit exceeded
    RateLimited(String),
    /// Provider returned an error
    ApiError(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Http(e) => write!(f, "HTTP error: {}", e),
            ProviderError::Parse(msg) => write!(f, "Parse error: {}", msg),
            ProviderError::NoApiKey(provider) => write!(f, "API key not configured for {}", provider),
            ProviderError::RateLimited(provider) => write!(f, "Rate limit exceeded for {}", provider),
            ProviderError::ApiError(msg) => write!(f, "API error: {}", msg),
        }
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(err: reqwest::Error) -> Self {
        ProviderError::Http(err)
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(err: serde_json::Error) -> Self {
        ProviderError::Parse(err.to_string())
    }
}

impl From<ProviderError> for AppError {
    fn from(err: ProviderError) -> Self {
        AppError::Provider(err.to_string())
    }
}
