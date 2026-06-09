//! Provider API key management and data fetching commands

use crate::error::AppError;
use crate::providers::fmp::FmpClient;
use crate::providers::fred::FredClient;
use crate::providers::yahoo::YahooClient;
use crate::providers::types::*;
use crate::state::AppState;
use tauri::State;

/// Save a provider API key (encrypted)
#[tauri::command]
pub async fn save_provider_api_key(
    state: State<'_, AppState>,
    provider: String,
    api_key: String,
) -> Result<(), AppError> {
    state.sqlite.save_provider_key(&provider, &api_key, &state.security)?;
    Ok(())
}

/// Delete a provider API key
#[tauri::command]
pub async fn delete_provider_api_key(
    state: State<'_, AppState>,
    provider: String,
) -> Result<bool, AppError> {
    state.sqlite.delete_provider_key(&provider)
}

/// Get list of configured provider names
#[tauri::command]
pub async fn get_configured_providers(
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    state.sqlite.get_configured_provider_names()
}

/// Check if a specific provider has an API key configured
#[tauri::command]
pub async fn get_provider_key_status(
    state: State<'_, AppState>,
    provider: String,
) -> Result<bool, AppError> {
    let key = state.sqlite.get_provider_key(&provider, &state.security)?;
    Ok(key.is_some())
}

/// Get generic mode status
#[tauri::command]
pub async fn get_generic_mode(
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    state.sqlite.get_generic_mode()
}

/// Set generic mode
#[tauri::command]
pub async fn set_generic_mode(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), AppError> {
    state.sqlite.set_generic_mode(enabled)
}

// ========== Yahoo Finance (no API key needed) ==========

/// Get real-time quotes from Yahoo Finance
#[tauri::command]
pub async fn get_generic_quote(
    state: State<'_, AppState>,
    symbols: Vec<String>,
) -> Result<Vec<GenericQuote>, AppError> {
    let client = YahooClient::new((*state.http_client).clone());
    let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
    Ok(client.get_quotes(&symbol_refs).await?)
}

/// Search symbols via Yahoo Finance
#[tauri::command]
pub async fn search_global_symbols(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
) -> Result<Vec<SymbolSearchResult>, AppError> {
    let client = YahooClient::new((*state.http_client).clone());
    Ok(client.search_symbols(&query, limit.unwrap_or(10)).await?)
}

/// Get historical OHLCV data from Yahoo Finance
#[tauri::command]
pub async fn get_yahoo_historical(
    state: State<'_, AppState>,
    symbol: String,
    interval: String,
    range: String,
) -> Result<Vec<(String, f64, f64, f64, f64, i64)>, AppError> {
    let client = YahooClient::new((*state.http_client).clone());
    Ok(client.get_historical(&symbol, &interval, &range).await?)
}

// ========== FMP (requires API key) ==========

/// Helper to get FMP client with API key
async fn get_fmp_client(state: &State<'_, AppState>) -> Result<FmpClient, AppError> {
    let api_key = state
        .sqlite
        .get_provider_key("fmp", &state.security)?
        .ok_or_else(|| AppError::Provider("FMP API key not configured".to_string()))?;
    Ok(FmpClient::new((*state.http_client).clone(), api_key))
}

/// Get company profile from FMP
#[tauri::command]
pub async fn get_company_profile(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Option<CompanyProfile>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_company_profile(&symbol).await?)
}

/// Get income statements from FMP
#[tauri::command]
pub async fn get_income_statement(
    state: State<'_, AppState>,
    symbol: String,
    period: String,
    limit: Option<i32>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_income_statement(&symbol, &period, limit.unwrap_or(4)).await?)
}

/// Get balance sheets from FMP
#[tauri::command]
pub async fn get_balance_sheet(
    state: State<'_, AppState>,
    symbol: String,
    period: String,
    limit: Option<i32>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_balance_sheet(&symbol, &period, limit.unwrap_or(4)).await?)
}

/// Get cash flow statements from FMP
#[tauri::command]
pub async fn get_cash_flow(
    state: State<'_, AppState>,
    symbol: String,
    period: String,
    limit: Option<i32>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_cash_flow(&symbol, &period, limit.unwrap_or(4)).await?)
}

/// Get key financial metrics from FMP
#[tauri::command]
pub async fn get_key_metrics(
    state: State<'_, AppState>,
    symbol: String,
    period: String,
    limit: Option<i32>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_key_metrics(&symbol, &period, limit.unwrap_or(4)).await?)
}

/// Get financial ratios from FMP (P/E, P/B, margins, dividend yield, etc. —
/// FMP's stable API serves these from /ratios, not /key-metrics)
#[tauri::command]
pub async fn get_ratios(
    state: State<'_, AppState>,
    symbol: String,
    period: String,
    limit: Option<i32>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_ratios(&symbol, &period, limit.unwrap_or(4)).await?)
}

/// Get stock news from FMP
#[tauri::command]
pub async fn get_stock_news(
    state: State<'_, AppState>,
    symbols: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<NewsArticle>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_stock_news(symbols.as_deref(), limit.unwrap_or(20)).await?)
}

/// Get analyst estimates from FMP
#[tauri::command]
pub async fn get_analyst_estimates(
    state: State<'_, AppState>,
    symbol: String,
    limit: Option<i32>,
) -> Result<Vec<AnalystEstimate>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_analyst_estimates(&symbol, limit.unwrap_or(4)).await?)
}

/// Get price targets from FMP
#[tauri::command]
pub async fn get_price_targets(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<PriceTarget>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_price_targets(&symbol).await?)
}

/// Get economic calendar from FMP
#[tauri::command]
pub async fn get_economic_calendar(
    state: State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<EconomicEvent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_economic_calendar(&from_date, &to_date).await?)
}

/// Screen stocks via FMP
#[tauri::command]
pub async fn screen_stocks(
    state: State<'_, AppState>,
    filters: ScreenerFilters,
) -> Result<Vec<ScreenerResult>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.screen_stocks(&filters).await?)
}

// ========== FMP Congressional Trading ==========

/// Get latest Senate financial disclosures
#[tauri::command]
pub async fn get_senate_trades(
    state: State<'_, AppState>,
    page: Option<i32>,
    limit: Option<i32>,
) -> Result<Vec<CongressionalTrade>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_senate_trades(page.unwrap_or(0), limit.unwrap_or(25).min(25)).await?)
}

/// Get Senate trades by senator name
#[tauri::command]
pub async fn get_senate_trades_by_name(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<CongressionalTrade>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_senate_trades_by_name(&name).await?)
}

/// Get latest House financial disclosures
#[tauri::command]
pub async fn get_house_trades(
    state: State<'_, AppState>,
    page: Option<i32>,
    limit: Option<i32>,
) -> Result<Vec<CongressionalTrade>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_house_trades(page.unwrap_or(0), limit.unwrap_or(25).min(25)).await?)
}

/// Get House trades by representative name
#[tauri::command]
pub async fn get_house_trades_by_name(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<CongressionalTrade>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_house_trades_by_name(&name).await?)
}

// ========== FRED (requires API key) ==========

/// Helper to get FRED client with API key
async fn get_fred_client(state: &State<'_, AppState>) -> Result<FredClient, AppError> {
    let api_key = state
        .sqlite
        .get_provider_key("fred", &state.security)?
        .ok_or_else(|| AppError::Provider("FRED API key not configured".to_string()))?;
    Ok(FredClient::new((*state.http_client).clone(), api_key))
}

/// Get FRED series observations
#[tauri::command]
pub async fn get_fred_series(
    state: State<'_, AppState>,
    series_id: String,
    observation_start: Option<String>,
    observation_end: Option<String>,
) -> Result<Vec<FredObservation>, AppError> {
    let client = get_fred_client(&state).await?;
    Ok(client
        .get_series(
            &series_id,
            observation_start.as_deref(),
            observation_end.as_deref(),
        )
        .await?)
}

/// Search FRED series
#[tauri::command]
pub async fn search_fred_series(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
) -> Result<Vec<FredSeries>, AppError> {
    let client = get_fred_client(&state).await?;
    Ok(client.search_series(&query, limit.unwrap_or(20)).await?)
}

/// Get FRED releases
#[tauri::command]
pub async fn get_fred_releases(
    state: State<'_, AppState>,
) -> Result<Vec<(String, String, String)>, AppError> {
    let client = get_fred_client(&state).await?;
    Ok(client.get_releases().await?)
}

// ==================== NEW FMP API COMMANDS ====================

// ---------- Earnings Call Transcripts ----------

#[tauri::command]
pub async fn get_earnings_call_transcript(
    state: State<'_, AppState>,
    symbol: String,
    year: i32,
    quarter: Option<i32>,
) -> Result<Vec<EarningsCallTranscript>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_earnings_call_transcript(&symbol, year, quarter).await?)
}

// ---------- Insider Trading ----------

#[tauri::command]
pub async fn get_insider_trading(
    state: State<'_, AppState>,
    symbol: String,
    limit: Option<i32>,
) -> Result<Vec<InsiderTrade>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_insider_trading(&symbol, limit.unwrap_or(50)).await?)
}

#[tauri::command]
pub async fn get_insider_trading_latest(
    state: State<'_, AppState>,
    limit: Option<i32>,
) -> Result<Vec<InsiderTrade>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_insider_trading_latest(limit.unwrap_or(50)).await?)
}

#[tauri::command]
pub async fn get_institutional_holders(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<InstitutionalHolder>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_institutional_holders(&symbol).await?)
}

// ---------- Market Calendars ----------

#[tauri::command]
pub async fn get_earnings_calendar(
    state: State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<EarningsCalendarEvent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_earnings_calendar(&from_date, &to_date).await?)
}

#[tauri::command]
pub async fn get_ipo_calendar(
    state: State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<IpoCalendarEvent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_ipo_calendar(&from_date, &to_date).await?)
}

#[tauri::command]
pub async fn get_dividend_calendar(
    state: State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<DividendCalendarEvent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_dividend_calendar(&from_date, &to_date).await?)
}

#[tauri::command]
pub async fn get_stock_split_calendar(
    state: State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<StockSplitCalendarEvent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_stock_split_calendar(&from_date, &to_date).await?)
}

// ---------- ESG Data ----------

#[tauri::command]
pub async fn get_esg_scores(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<EsgScore>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_esg_scores(&symbol).await?)
}

#[tauri::command]
pub async fn get_esg_ratings(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<EsgRating>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_esg_ratings(&symbol).await?)
}

// ---------- ETF & Mutual Fund ----------

#[tauri::command]
pub async fn get_etf_info(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<EtfInfo>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_etf_info(&symbol).await?)
}

#[tauri::command]
pub async fn get_etf_holdings(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<EtfHolding>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_etf_holdings(&symbol).await?)
}

#[tauri::command]
pub async fn get_mutual_fund_holders(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<InstitutionalHolder>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_mutual_fund_holders(&symbol).await?)
}

// ---------- Advanced Market Metrics ----------

#[tauri::command]
pub async fn get_dcf(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<DiscountedCashFlow>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_dcf(&symbol).await?)
}

#[tauri::command]
pub async fn get_historical_dcf(
    state: State<'_, AppState>,
    symbol: String,
    period: String,
    limit: Option<i32>,
) -> Result<Vec<DiscountedCashFlow>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_historical_dcf(&symbol, &period, limit.unwrap_or(10)).await?)
}

#[tauri::command]
pub async fn get_sector_performance(
    state: State<'_, AppState>,
) -> Result<Vec<SectorPerformance>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_sector_performance().await?)
}

#[tauri::command]
pub async fn get_market_gainers(
    state: State<'_, AppState>,
) -> Result<Vec<MarketMover>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_market_gainers().await?)
}

#[tauri::command]
pub async fn get_market_losers(
    state: State<'_, AppState>,
) -> Result<Vec<MarketMover>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_market_losers().await?)
}

#[tauri::command]
pub async fn get_market_most_active(
    state: State<'_, AppState>,
) -> Result<Vec<MarketMover>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_market_most_active().await?)
}

// ---------- Commodity, Forex, Crypto ----------

#[tauri::command]
pub async fn get_commodity_quotes(
    state: State<'_, AppState>,
) -> Result<Vec<FmpQuote>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_commodity_quotes().await?)
}

#[tauri::command]
pub async fn get_forex_quotes(
    state: State<'_, AppState>,
) -> Result<Vec<FmpQuote>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_forex_quotes().await?)
}

#[tauri::command]
pub async fn get_crypto_quotes(
    state: State<'_, AppState>,
) -> Result<Vec<FmpQuote>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_crypto_quotes().await?)
}

// ---------- Index Constituents ----------

#[tauri::command]
pub async fn get_sp500_constituents(
    state: State<'_, AppState>,
) -> Result<Vec<IndexConstituent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_sp500_constituents().await?)
}

#[tauri::command]
pub async fn get_nasdaq_constituents(
    state: State<'_, AppState>,
) -> Result<Vec<IndexConstituent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_nasdaq_constituents().await?)
}

#[tauri::command]
pub async fn get_dowjones_constituents(
    state: State<'_, AppState>,
) -> Result<Vec<IndexConstituent>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_dowjones_constituents().await?)
}

// ---------- Search & Directory ----------

#[tauri::command]
pub async fn search_fmp_symbols(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
) -> Result<Vec<FmpSearchResult>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.search_fmp_symbols(&query, limit.unwrap_or(20)).await?)
}

#[tauri::command]
pub async fn get_stock_list(
    state: State<'_, AppState>,
) -> Result<Vec<StockListItem>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_stock_list().await?)
}

// ---------- Batch/Full Quotes ----------

#[tauri::command]
pub async fn get_batch_quote(
    state: State<'_, AppState>,
    symbols: String,
) -> Result<Vec<FmpQuote>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_batch_quote(&symbols).await?)
}

#[tauri::command]
pub async fn get_fmp_quote(
    state: State<'_, AppState>,
    symbol: String,
) -> Result<Vec<FmpQuote>, AppError> {
    let client = get_fmp_client(&state).await?;
    Ok(client.get_fmp_quote(&symbol).await?)
}
