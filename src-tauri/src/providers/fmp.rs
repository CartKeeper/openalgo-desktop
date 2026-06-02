//! Financial Modeling Prep (FMP) API client
//!
//! Provides fundamentals, news, analyst data, screener, and economic calendar.
//! Free tier: 250 requests/day. API key required.

use super::ProviderError;
use super::types::*;
use reqwest::Client;

const FMP_BASE_URL: &str = "https://financialmodelingprep.com/stable";

/// FMP API client
pub struct FmpClient {
    client: Client,
    api_key: String,
}

impl FmpClient {
    /// Create a new FMP client
    pub fn new(client: Client, api_key: String) -> Self {
        Self { client, api_key }
    }

    /// Get company profile
    pub async fn get_company_profile(&self, symbol: &str) -> Result<Option<CompanyProfile>, ProviderError> {
        let url = format!("{}/profile?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let resp = self.fetch_fmp_json(&url).await?;

        if let Some(item) = resp.first() {
            Ok(Some(CompanyProfile {
                symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
                company_name: item["companyName"].as_str().unwrap_or("").to_string(),
                exchange: item["exchange"].as_str().or_else(|| item["exchangeShortName"].as_str()).unwrap_or("").to_string(),
                industry: item["industry"].as_str().map(String::from),
                sector: item["sector"].as_str().map(String::from),
                country: item["country"].as_str().map(String::from),
                market_cap: item["marketCap"].as_f64().or_else(|| item["mktCap"].as_f64()),
                description: item["description"].as_str().map(String::from),
                website: item["website"].as_str().map(String::from),
                ceo: item["ceo"].as_str().map(String::from),
                employees: item["fullTimeEmployees"].as_str().and_then(|s| s.parse().ok()),
                ipo_date: item["ipoDate"].as_str().map(String::from),
                currency: item["currency"].as_str().map(String::from),
                price: item["price"].as_f64(),
                beta: item["beta"].as_f64(),
                volume_avg: item["averageVolume"].as_i64().or_else(|| item["volAvg"].as_i64()),
                last_dividend: item["lastDividend"].as_f64().or_else(|| item["lastDiv"].as_f64()),
                image: item["image"].as_str().map(String::from),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get income statements
    // Fundamentals are returned as raw FMP /stable JSON passthrough. The frontend
    // reads FMP's own camelCase field names via pick(); mapping into typed structs
    // silently dropped or renamed fields (FMP's stable API renamed many of them and
    // moved P/E, P/B, margins, dividend yield out of key-metrics into /ratios).
    pub async fn get_income_statement(&self, symbol: &str, period: &str, limit: i32) -> Result<Vec<serde_json::Value>, ProviderError> {
        let url = format!(
            "{}/income-statement?symbol={}&period={}&limit={}&apikey={}",
            FMP_BASE_URL, symbol, period, limit, self.api_key
        );
        self.fetch_fmp_json(&url).await
    }

    /// Get balance sheets
    pub async fn get_balance_sheet(&self, symbol: &str, period: &str, limit: i32) -> Result<Vec<serde_json::Value>, ProviderError> {
        let url = format!(
            "{}/balance-sheet-statement?symbol={}&period={}&limit={}&apikey={}",
            FMP_BASE_URL, symbol, period, limit, self.api_key
        );
        self.fetch_fmp_json(&url).await
    }

    /// Get cash flow statements
    pub async fn get_cash_flow(&self, symbol: &str, period: &str, limit: i32) -> Result<Vec<serde_json::Value>, ProviderError> {
        let url = format!(
            "{}/cash-flow-statement?symbol={}&period={}&limit={}&apikey={}",
            FMP_BASE_URL, symbol, period, limit, self.api_key
        );
        self.fetch_fmp_json(&url).await
    }

    /// Get key financial metrics
    pub async fn get_key_metrics(&self, symbol: &str, period: &str, limit: i32) -> Result<Vec<serde_json::Value>, ProviderError> {
        let url = format!(
            "{}/key-metrics?symbol={}&period={}&limit={}&apikey={}",
            FMP_BASE_URL, symbol, period, limit, self.api_key
        );
        self.fetch_fmp_json(&url).await
    }

    /// Financial ratios (FMP /stable/ratios). The stable API moved P/E, P/B,
    /// margins, dividend yield, debt-to-equity, etc. here, out of key-metrics.
    pub async fn get_ratios(&self, symbol: &str, period: &str, limit: i32) -> Result<Vec<serde_json::Value>, ProviderError> {
        let url = format!(
            "{}/ratios?symbol={}&period={}&limit={}&apikey={}",
            FMP_BASE_URL, symbol, period, limit, self.api_key
        );
        self.fetch_fmp_json(&url).await
    }

    /// Get stock news (uses fmp-articles endpoint on stable API)
    pub async fn get_stock_news(&self, symbols: Option<&str>, limit: i32) -> Result<Vec<NewsArticle>, ProviderError> {
        let url = if let Some(syms) = symbols {
            format!("{}/stock-news?tickers={}&limit={}&apikey={}", FMP_BASE_URL, syms, limit, self.api_key)
        } else {
            format!("{}/fmp-articles?limit={}&apikey={}", FMP_BASE_URL, limit, self.api_key)
        };
        let resp = self.fetch_fmp_json(&url).await?;

        Ok(resp.iter().map(|item| {
            // Handle both stock-news and fmp-articles response formats
            let url_val = item["url"].as_str()
                .or_else(|| item["link"].as_str())
                .unwrap_or("").to_string();
            let date_val = item["publishedDate"].as_str()
                .or_else(|| item["date"].as_str())
                .unwrap_or("").to_string();
            let summary_val = item["text"].as_str()
                .or_else(|| item["content"].as_str())
                .map(String::from);
            let symbol_val = item["symbol"].as_str()
                .or_else(|| item["tickers"].as_str())
                .map(|s| s.replace("STOCK:", ""));

            NewsArticle {
                title: item["title"].as_str().unwrap_or("").to_string(),
                url: url_val,
                published_date: date_val,
                source: item["site"].as_str().map(String::from),
                summary: summary_val,
                symbol: symbol_val,
                image: item["image"].as_str().map(String::from),
            }
        }).collect())
    }

    /// Get analyst estimates
    pub async fn get_analyst_estimates(&self, symbol: &str, limit: i32) -> Result<Vec<AnalystEstimate>, ProviderError> {
        let url = format!(
            "{}/analyst-estimates?symbol={}&period=annual&limit={}&apikey={}",
            FMP_BASE_URL, symbol, limit, self.api_key
        );
        let resp = self.fetch_fmp_json(&url).await?;

        Ok(resp.iter().map(|item| AnalystEstimate {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            date: item["date"].as_str().unwrap_or("").to_string(),
            estimated_revenue_avg: item["estimatedRevenueAvg"].as_f64(),
            estimated_revenue_high: item["estimatedRevenueHigh"].as_f64(),
            estimated_revenue_low: item["estimatedRevenueLow"].as_f64(),
            estimated_eps_avg: item["estimatedEpsAvg"].as_f64(),
            estimated_eps_high: item["estimatedEpsHigh"].as_f64(),
            estimated_eps_low: item["estimatedEpsLow"].as_f64(),
            number_of_analysts: item["numberAnalystEstimatedRevenue"].as_i64().map(|n| n as i32),
        }).collect())
    }

    /// Get price targets
    pub async fn get_price_targets(&self, symbol: &str) -> Result<Vec<PriceTarget>, ProviderError> {
        let url = format!(
            "{}/price-target?symbol={}&apikey={}",
            FMP_BASE_URL, symbol, self.api_key
        );
        let resp = self.fetch_fmp_json(&url).await?;

        Ok(resp.iter().map(|item| PriceTarget {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            published_date: item["publishedDate"].as_str().map(String::from),
            analyst_name: item["analystName"].as_str().map(String::from),
            analyst_company: item["analystCompany"].as_str().map(String::from),
            price_target: item["priceTarget"].as_f64(),
            adj_price_target: item["adjPriceTarget"].as_f64(),
            price_when_posted: item["priceWhenPosted"].as_f64(),
            news_title: item["newsTitle"].as_str().map(String::from),
        }).collect())
    }

    /// Get economic calendar
    pub async fn get_economic_calendar(&self, from_date: &str, to_date: &str) -> Result<Vec<EconomicEvent>, ProviderError> {
        let url = format!(
            "{}/economic-calendar?from={}&to={}&apikey={}",
            FMP_BASE_URL, from_date, to_date, self.api_key
        );
        let resp = self.fetch_fmp_json(&url).await?;

        Ok(resp.iter().map(|item| EconomicEvent {
            date: item["date"].as_str().unwrap_or("").to_string(),
            country: item["country"].as_str().map(String::from),
            event: item["event"].as_str().unwrap_or("").to_string(),
            impact: item["impact"].as_str().map(String::from),
            actual: item["actual"].as_f64(),
            estimate: item["estimate"].as_f64(),
            previous: item["previous"].as_f64(),
            currency: item["currency"].as_str().map(String::from),
        }).collect())
    }

    /// Get latest Senate financial disclosures
    pub async fn get_senate_trades(&self, page: i32, limit: i32) -> Result<Vec<CongressionalTrade>, ProviderError> {
        let url = format!(
            "{}/senate-latest?page={}&limit={}&apikey={}",
            FMP_BASE_URL, page, limit, self.api_key
        );
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| Self::parse_congressional_trade(item)).collect())
    }

    /// Get Senate trading by specific senator name
    pub async fn get_senate_trades_by_name(&self, name: &str) -> Result<Vec<CongressionalTrade>, ProviderError> {
        let url = format!(
            "{}/senate-trading-by-name?name={}&apikey={}",
            FMP_BASE_URL, name, self.api_key
        );
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| Self::parse_congressional_trade(item)).collect())
    }

    /// Get latest House financial disclosures
    pub async fn get_house_trades(&self, page: i32, limit: i32) -> Result<Vec<CongressionalTrade>, ProviderError> {
        let url = format!(
            "{}/house-latest?page={}&limit={}&apikey={}",
            FMP_BASE_URL, page, limit, self.api_key
        );
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| Self::parse_congressional_trade(item)).collect())
    }

    /// Get House trades by specific representative name
    pub async fn get_house_trades_by_name(&self, name: &str) -> Result<Vec<CongressionalTrade>, ProviderError> {
        let url = format!(
            "{}/house-trading-by-name?name={}&apikey={}",
            FMP_BASE_URL, name, self.api_key
        );
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| Self::parse_congressional_trade(item)).collect())
    }

    /// Parse a congressional trade from JSON (handles both Senate and House formats)
    fn parse_congressional_trade(item: &serde_json::Value) -> CongressionalTrade {
        CongressionalTrade {
            date_received: item["dateRecieved"].as_str()
                .or_else(|| item["dateReceived"].as_str())
                .or_else(|| item["disclosureDate"].as_str())
                .map(String::from),
            transaction_date: item["transactionDate"].as_str().map(String::from),
            owner: item["owner"].as_str().map(String::from),
            asset_description: item["assetDescription"].as_str().map(String::from),
            asset_type: item["assetType"].as_str()
                .or_else(|| item["type"].as_str())
                .map(String::from),
            amount: item["amount"].as_str().map(String::from),
            transaction_type: item["type"].as_str()
                .or_else(|| item["transactionType"].as_str())
                .map(String::from),
            comment: item["comment"].as_str().map(String::from),
            link: item["link"].as_str()
                .or_else(|| item["ptrLink"].as_str())
                .map(String::from),
            first_name: item["firstName"].as_str().map(String::from),
            last_name: item["lastName"].as_str().map(String::from),
            office: item["office"].as_str().map(String::from),
            district: item["district"].as_str().map(String::from),
            ticker: item["ticker"].as_str()
                .or_else(|| item["symbol"].as_str())
                .map(String::from),
            cap_gains_over_200: item["capitalGainsOver200USD"].as_bool()
                .or_else(|| item["capGainsOver200"].as_bool()),
        }
    }

    // ==================== NEW FMP API METHODS ====================

    /// Helper to fetch FMP JSON, handling error objects gracefully
    async fn fetch_fmp_json(&self, url: &str) -> Result<Vec<serde_json::Value>, ProviderError> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        let body = response.text().await.map_err(|e| {
            ProviderError::Parse(format!("Failed to read FMP response body: {}", e))
        })?;

        if !status.is_success() {
            return Err(ProviderError::ApiError(format!("FMP returned HTTP {}: {}", status, &body[..body.len().min(200)])));
        }

        let resp: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            ProviderError::Parse(format!("Invalid JSON from FMP: {} (body starts with: {})", e, &body[..body.len().min(100)]))
        })?;

        if let Some(err_msg) = resp.get("Error Message").and_then(|v| v.as_str()) {
            return Err(ProviderError::ApiError(err_msg.to_string()));
        }

        match resp {
            serde_json::Value::Array(arr) => Ok(arr),
            _ => Err(ProviderError::Parse(format!("Unexpected FMP response format: {}", &body[..body.len().min(200)]))),
        }
    }

    // ---------- Earnings Call Transcripts ----------

    pub async fn get_earnings_call_transcript(&self, symbol: &str, year: i32, quarter: Option<i32>) -> Result<Vec<EarningsCallTranscript>, ProviderError> {
        let mut url = format!("{}/earning-call-transcript?symbol={}&year={}&apikey={}", FMP_BASE_URL, symbol, year, self.api_key);
        if let Some(q) = quarter { url.push_str(&format!("&quarter={}", q)); }
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| EarningsCallTranscript {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            quarter: item["quarter"].as_i64().map(|v| v as i32),
            year: item["year"].as_i64().map(|v| v as i32),
            date: item["date"].as_str().map(String::from),
            content: item["content"].as_str().map(String::from),
        }).collect())
    }

    // ---------- Insider Trading ----------

    pub async fn get_insider_trading(&self, symbol: &str, limit: i32) -> Result<Vec<InsiderTrade>, ProviderError> {
        let url = format!("{}/insider-trading?symbol={}&limit={}&apikey={}", FMP_BASE_URL, symbol, limit, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| Self::parse_insider_trade(item)).collect())
    }

    pub async fn get_insider_trading_latest(&self, limit: i32) -> Result<Vec<InsiderTrade>, ProviderError> {
        let url = format!("{}/insider-trading?limit={}&apikey={}", FMP_BASE_URL, limit, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| Self::parse_insider_trade(item)).collect())
    }

    pub async fn get_institutional_holders(&self, symbol: &str) -> Result<Vec<InstitutionalHolder>, ProviderError> {
        let url = format!("{}/institutional-holder?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| InstitutionalHolder {
            holder: item["holder"].as_str().unwrap_or("").to_string(),
            shares: item["shares"].as_f64(),
            date_reported: item["dateReported"].as_str().map(String::from),
            change: item["change"].as_f64(),
            change_percent: item["changePercent"].as_f64(),
        }).collect())
    }

    fn parse_insider_trade(item: &serde_json::Value) -> InsiderTrade {
        InsiderTrade {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            filing_date: item["filingDate"].as_str().map(String::from),
            transaction_date: item["transactionDate"].as_str().map(String::from),
            reporting_name: item["reportingName"].as_str().map(String::from),
            transaction_type: item["transactionType"].as_str().map(String::from),
            securities_owned: item["securitiesOwned"].as_f64(),
            securities_transacted: item["securitiesTransacted"].as_f64(),
            price: item["price"].as_f64(),
            type_of_owner: item["typeOfOwner"].as_str().map(String::from),
            form_type: item["formType"].as_str().map(String::from),
            link: item["link"].as_str().map(String::from),
        }
    }

    // ---------- Market Calendars ----------

    pub async fn get_earnings_calendar(&self, from_date: &str, to_date: &str) -> Result<Vec<EarningsCalendarEvent>, ProviderError> {
        let url = format!("{}/earning-calendar?from={}&to={}&apikey={}", FMP_BASE_URL, from_date, to_date, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| EarningsCalendarEvent {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            date: item["date"].as_str().unwrap_or("").to_string(),
            eps_estimated: item["epsEstimated"].as_f64(),
            eps_actual: item["eps"].as_f64(),
            revenue_estimated: item["revenueEstimated"].as_f64(),
            revenue_actual: item["revenue"].as_f64(),
            fiscal_date_ending: item["fiscalDateEnding"].as_str().map(String::from),
            updated_from_date: item["updatedFromDate"].as_str().map(String::from),
        }).collect())
    }

    pub async fn get_ipo_calendar(&self, from_date: &str, to_date: &str) -> Result<Vec<IpoCalendarEvent>, ProviderError> {
        let url = format!("{}/ipo-calendar?from={}&to={}&apikey={}", FMP_BASE_URL, from_date, to_date, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| IpoCalendarEvent {
            symbol: item["symbol"].as_str().map(String::from),
            company: item["company"].as_str().map(String::from),
            date: item["date"].as_str().unwrap_or("").to_string(),
            exchange: item["exchange"].as_str().map(String::from),
            actions: item["actions"].as_str().map(String::from),
            shares: item["shares"].as_f64(),
            price_range: item["priceRange"].as_str().map(String::from),
            market_cap: item["marketCap"].as_f64(),
        }).collect())
    }

    pub async fn get_dividend_calendar(&self, from_date: &str, to_date: &str) -> Result<Vec<DividendCalendarEvent>, ProviderError> {
        let url = format!("{}/stock-dividend-calendar?from={}&to={}&apikey={}", FMP_BASE_URL, from_date, to_date, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| DividendCalendarEvent {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            date: item["date"].as_str().unwrap_or("").to_string(),
            label: item["label"].as_str().map(String::from),
            adj_dividend: item["adjDividend"].as_f64(),
            dividend: item["dividend"].as_f64(),
            record_date: item["recordDate"].as_str().map(String::from),
            payment_date: item["paymentDate"].as_str().map(String::from),
            declaration_date: item["declarationDate"].as_str().map(String::from),
        }).collect())
    }

    pub async fn get_stock_split_calendar(&self, from_date: &str, to_date: &str) -> Result<Vec<StockSplitCalendarEvent>, ProviderError> {
        let url = format!("{}/stock-split-calendar?from={}&to={}&apikey={}", FMP_BASE_URL, from_date, to_date, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| StockSplitCalendarEvent {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            date: item["date"].as_str().unwrap_or("").to_string(),
            label: item["label"].as_str().map(String::from),
            numerator: item["numerator"].as_f64(),
            denominator: item["denominator"].as_f64(),
        }).collect())
    }

    // ---------- ESG Data ----------

    pub async fn get_esg_scores(&self, symbol: &str) -> Result<Vec<EsgScore>, ProviderError> {
        let url = format!("{}/esg-environmental-social-governance-data?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| EsgScore {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            cik: item["cik"].as_str().map(String::from),
            date: item["date"].as_str().map(String::from),
            environment_score: item["environmentScore"].as_f64(),
            social_score: item["socialScore"].as_f64(),
            governance_score: item["governanceScore"].as_f64(),
            esg_score: item["ESGScore"].as_f64().or_else(|| item["esgScore"].as_f64()),
            company_name: item["companyName"].as_str().map(String::from),
            form_type: item["formType"].as_str().map(String::from),
            url: item["url"].as_str().map(String::from),
        }).collect())
    }

    pub async fn get_esg_ratings(&self, symbol: &str) -> Result<Vec<EsgRating>, ProviderError> {
        let url = format!("{}/esg-environmental-social-governance-data-ratings?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| EsgRating {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            cik: item["cik"].as_str().map(String::from),
            company_name: item["companyName"].as_str().map(String::from),
            environment_rating: item["environmentRating"].as_str().map(String::from),
            social_rating: item["socialRating"].as_str().map(String::from),
            governance_rating: item["governanceRating"].as_str().map(String::from),
            esg_rating: item["ESGRating"].as_str().or_else(|| item["esgRating"].as_str()).map(String::from),
            year: item["year"].as_i64().map(|v| v as i32),
        }).collect())
    }

    // ---------- ETF & Mutual Fund ----------

    pub async fn get_etf_info(&self, symbol: &str) -> Result<Vec<EtfInfo>, ProviderError> {
        let url = format!("{}/etf-info?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| EtfInfo {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            name: item["name"].as_str().map(String::from),
            asset_class: item["assetClass"].as_str().map(String::from),
            avg_volume: item["avgVolume"].as_i64(),
            description: item["description"].as_str().map(String::from),
            etf_company: item["etfCompany"].as_str().map(String::from),
            expense_ratio: item["expenseRatio"].as_f64(),
            inception_date: item["inceptionDate"].as_str().map(String::from),
            holdings_count: item["holdingsCount"].as_i64().map(|v| v as i32),
            nav: item["nav"].as_f64(),
            net_assets: item["netAssets"].as_f64(),
        }).collect())
    }

    pub async fn get_etf_holdings(&self, symbol: &str) -> Result<Vec<EtfHolding>, ProviderError> {
        let url = format!("{}/etf-holder?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| EtfHolding {
            asset: item["asset"].as_str().map(String::from),
            name: item["name"].as_str().map(String::from),
            shares: item["sharesNumber"].as_f64().or_else(|| item["shares"].as_f64()),
            weight: item["weightPercentage"].as_f64().or_else(|| item["weight"].as_f64()),
            market_value: item["marketValue"].as_f64(),
        }).collect())
    }

    pub async fn get_mutual_fund_holders(&self, symbol: &str) -> Result<Vec<InstitutionalHolder>, ProviderError> {
        let url = format!("{}/mutual-fund-holder?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| InstitutionalHolder {
            holder: item["holder"].as_str().unwrap_or("").to_string(),
            shares: item["shares"].as_f64(),
            date_reported: item["dateReported"].as_str().map(String::from),
            change: item["change"].as_f64(),
            change_percent: item["weightPercent"].as_f64(),
        }).collect())
    }

    // ---------- Advanced Market Metrics ----------

    pub async fn get_dcf(&self, symbol: &str) -> Result<Vec<DiscountedCashFlow>, ProviderError> {
        let url = format!("{}/discounted-cash-flow?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| DiscountedCashFlow {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            date: item["date"].as_str().map(String::from),
            dcf: item["dcf"].as_f64(),
            stock_price: item["stockPrice"].as_f64().or_else(|| item["Stock Price"].as_f64()),
        }).collect())
    }

    pub async fn get_historical_dcf(&self, symbol: &str, period: &str, limit: i32) -> Result<Vec<DiscountedCashFlow>, ProviderError> {
        let url = format!("{}/historical-discounted-cash-flow-statement?symbol={}&period={}&limit={}&apikey={}", FMP_BASE_URL, symbol, period, limit, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| DiscountedCashFlow {
            symbol: item["symbol"].as_str().unwrap_or(symbol).to_string(),
            date: item["date"].as_str().map(String::from),
            dcf: item["dcf"].as_f64(),
            stock_price: item["stockPrice"].as_f64().or_else(|| item["Stock Price"].as_f64()),
        }).collect())
    }

    pub async fn get_sector_performance(&self) -> Result<Vec<SectorPerformance>, ProviderError> {
        let url = format!("{}/sector-performance?apikey={}", FMP_BASE_URL, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| SectorPerformance {
            sector: item["sector"].as_str().unwrap_or("").to_string(),
            change_percentage: item["changesPercentage"].as_f64()
                .or_else(|| item["changesPercentage"].as_str().and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())),
        }).collect())
    }

    pub async fn get_market_gainers(&self) -> Result<Vec<MarketMover>, ProviderError> {
        let url = format!("{}/stock-market-gainers?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_market_movers(&url).await
    }

    pub async fn get_market_losers(&self) -> Result<Vec<MarketMover>, ProviderError> {
        let url = format!("{}/stock-market-losers?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_market_movers(&url).await
    }

    pub async fn get_market_most_active(&self) -> Result<Vec<MarketMover>, ProviderError> {
        let url = format!("{}/stock-market-most-active?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_market_movers(&url).await
    }

    async fn fetch_market_movers(&self, url: &str) -> Result<Vec<MarketMover>, ProviderError> {
        let items = self.fetch_fmp_json(url).await?;
        Ok(items.iter().map(|item| MarketMover {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().map(String::from),
            change: item["change"].as_f64(),
            price: item["price"].as_f64(),
            change_percent: item["changesPercentage"].as_f64(),
        }).collect())
    }

    // ---------- Commodity, Forex, Crypto Quotes ----------

    pub async fn get_commodity_quotes(&self) -> Result<Vec<FmpQuote>, ProviderError> {
        let url = format!("{}/batch-request-end-of-day-prices?type=commodity&apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_fmp_quotes(&url).await
    }

    pub async fn get_forex_quotes(&self) -> Result<Vec<FmpQuote>, ProviderError> {
        let url = format!("{}/fx?apikey={}", FMP_BASE_URL, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| FmpQuote {
            symbol: item["ticker"].as_str().unwrap_or("").to_string(),
            name: item["ticker"].as_str().map(String::from),
            price: item["bid"].as_f64(),
            change: item["changes"].as_f64(),
            change_percent: None,
            day_low: item["low"].as_f64(),
            day_high: item["high"].as_f64(),
            year_low: None,
            year_high: None,
            market_cap: None,
            pe: None,
            eps: None,
            volume: None,
            avg_volume: None,
            open: item["open"].as_f64(),
            previous_close: None,
            exchange: None,
            timestamp: None,
            earnings_announcement: None,
            shares_outstanding: None,
        }).collect())
    }

    pub async fn get_crypto_quotes(&self) -> Result<Vec<FmpQuote>, ProviderError> {
        let url = format!("{}/cryptocurrency?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_fmp_quotes(&url).await
    }

    async fn fetch_fmp_quotes(&self, url: &str) -> Result<Vec<FmpQuote>, ProviderError> {
        let items = self.fetch_fmp_json(url).await?;
        Ok(items.iter().map(|item| Self::parse_fmp_quote(item)).collect())
    }

    // ---------- Index Constituents ----------

    pub async fn get_sp500_constituents(&self) -> Result<Vec<IndexConstituent>, ProviderError> {
        let url = format!("{}/sp500-constituent?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_index_constituents(&url).await
    }

    pub async fn get_nasdaq_constituents(&self) -> Result<Vec<IndexConstituent>, ProviderError> {
        let url = format!("{}/nasdaq-constituent?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_index_constituents(&url).await
    }

    pub async fn get_dowjones_constituents(&self) -> Result<Vec<IndexConstituent>, ProviderError> {
        let url = format!("{}/dowjones-constituent?apikey={}", FMP_BASE_URL, self.api_key);
        self.fetch_index_constituents(&url).await
    }

    async fn fetch_index_constituents(&self, url: &str) -> Result<Vec<IndexConstituent>, ProviderError> {
        let items = self.fetch_fmp_json(url).await?;
        Ok(items.iter().map(|item| IndexConstituent {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().map(String::from),
            sector: item["sector"].as_str().map(String::from),
            sub_sector: item["subSector"].as_str().map(String::from),
            head_quarter: item["headQuarter"].as_str().map(String::from),
            date_first_added: item["dateFirstAdded"].as_str().map(String::from),
            cik: item["cik"].as_str().map(String::from),
            founded: item["founded"].as_str().map(String::from),
        }).collect())
    }

    // ---------- Search & Directory ----------

    pub async fn search_fmp_symbols(&self, query: &str, limit: i32) -> Result<Vec<FmpSearchResult>, ProviderError> {
        let url = format!("{}/search?query={}&limit={}&apikey={}", FMP_BASE_URL, query, limit, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| FmpSearchResult {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().map(String::from),
            currency: item["currency"].as_str().map(String::from),
            exchange: item["stockExchange"].as_str().map(String::from),
            exchange_short_name: item["exchangeShortName"].as_str().map(String::from),
        }).collect())
    }

    pub async fn get_stock_list(&self) -> Result<Vec<StockListItem>, ProviderError> {
        let url = format!("{}/stock/list?apikey={}", FMP_BASE_URL, self.api_key);
        let items = self.fetch_fmp_json(&url).await?;
        Ok(items.iter().map(|item| StockListItem {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().map(String::from),
            price: item["price"].as_f64(),
            exchange: item["exchange"].as_str().map(String::from),
            exchange_short_name: item["exchangeShortName"].as_str().map(String::from),
            stock_type: item["type"].as_str().map(String::from),
        }).collect())
    }

    // ---------- Batch/Full Quotes ----------

    pub async fn get_batch_quote(&self, symbols: &str) -> Result<Vec<FmpQuote>, ProviderError> {
        let url = format!("{}/quote?symbol={}&apikey={}", FMP_BASE_URL, symbols, self.api_key);
        self.fetch_fmp_quotes(&url).await
    }

    pub async fn get_fmp_quote(&self, symbol: &str) -> Result<Vec<FmpQuote>, ProviderError> {
        let url = format!("{}/quote?symbol={}&apikey={}", FMP_BASE_URL, symbol, self.api_key);
        self.fetch_fmp_quotes(&url).await
    }

    fn parse_fmp_quote(item: &serde_json::Value) -> FmpQuote {
        FmpQuote {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().map(String::from),
            price: item["price"].as_f64(),
            change: item["change"].as_f64(),
            change_percent: item["changesPercentage"].as_f64(),
            day_low: item["dayLow"].as_f64(),
            day_high: item["dayHigh"].as_f64(),
            year_low: item["yearLow"].as_f64(),
            year_high: item["yearHigh"].as_f64(),
            market_cap: item["marketCap"].as_f64(),
            pe: item["pe"].as_f64(),
            eps: item["eps"].as_f64(),
            volume: item["volume"].as_i64(),
            avg_volume: item["avgVolume"].as_i64(),
            open: item["open"].as_f64(),
            previous_close: item["previousClose"].as_f64(),
            exchange: item["exchange"].as_str().map(String::from),
            timestamp: item["timestamp"].as_i64(),
            earnings_announcement: item["earningsAnnouncement"].as_str().map(String::from),
            shares_outstanding: item["sharesOutstanding"].as_f64(),
        }
    }

    // ==================== EXISTING METHODS ====================

    /// Screen stocks
    pub async fn screen_stocks(&self, filters: &ScreenerFilters) -> Result<Vec<ScreenerResult>, ProviderError> {
        let mut params = vec![format!("apikey={}", self.api_key)];

        if let Some(v) = filters.market_cap_min { params.push(format!("marketCapMoreThan={}", v)); }
        if let Some(v) = filters.market_cap_max { params.push(format!("marketCapLowerThan={}", v)); }
        if let Some(v) = filters.price_min { params.push(format!("priceMoreThan={}", v)); }
        if let Some(v) = filters.price_max { params.push(format!("priceLowerThan={}", v)); }
        if let Some(v) = filters.beta_min { params.push(format!("betaMoreThan={}", v)); }
        if let Some(v) = filters.beta_max { params.push(format!("betaLowerThan={}", v)); }
        if let Some(v) = filters.volume_min { params.push(format!("volumeMoreThan={}", v)); }
        if let Some(ref v) = filters.sector { params.push(format!("sector={}", v)); }
        if let Some(ref v) = filters.industry { params.push(format!("industry={}", v)); }
        if let Some(ref v) = filters.country { params.push(format!("country={}", v)); }
        if let Some(ref v) = filters.exchange { params.push(format!("exchange={}", v)); }
        if let Some(v) = filters.is_etf { params.push(format!("isEtf={}", v)); }
        if let Some(v) = filters.limit { params.push(format!("limit={}", v)); }

        let url = format!("{}/stock-screener?{}", FMP_BASE_URL, params.join("&"));
        let resp = self.fetch_fmp_json(&url).await?;

        Ok(resp.iter().map(|item| ScreenerResult {
            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
            company_name: item["companyName"].as_str().unwrap_or("").to_string(),
            exchange: item["exchangeShortName"].as_str().unwrap_or("").to_string(),
            sector: item["sector"].as_str().map(String::from),
            industry: item["industry"].as_str().map(String::from),
            market_cap: item["marketCap"].as_f64(),
            price: item["price"].as_f64(),
            change_percent: None,
            volume: item["volume"].as_i64(),
            pe_ratio: None,
            beta: item["beta"].as_f64(),
            dividend_yield: None,
            country: item["country"].as_str().map(String::from),
        }).collect())
    }
}
