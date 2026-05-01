//! Shared data types for all providers

use serde::{Deserialize, Serialize};

/// Company profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProfile {
    pub symbol: String,
    pub company_name: String,
    pub exchange: String,
    pub industry: Option<String>,
    pub sector: Option<String>,
    pub country: Option<String>,
    pub market_cap: Option<f64>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub ceo: Option<String>,
    pub employees: Option<i64>,
    pub ipo_date: Option<String>,
    pub currency: Option<String>,
    pub price: Option<f64>,
    pub beta: Option<f64>,
    pub volume_avg: Option<i64>,
    pub last_dividend: Option<f64>,
    pub image: Option<String>,
}

/// Income statement data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatement {
    pub date: String,
    pub period: String,
    pub revenue: Option<f64>,
    pub cost_of_revenue: Option<f64>,
    pub gross_profit: Option<f64>,
    pub operating_expenses: Option<f64>,
    pub operating_income: Option<f64>,
    pub net_income: Option<f64>,
    pub eps: Option<f64>,
    pub eps_diluted: Option<f64>,
    pub ebitda: Option<f64>,
    pub weighted_avg_shares: Option<f64>,
    pub weighted_avg_shares_diluted: Option<f64>,
}

/// Balance sheet data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheet {
    pub date: String,
    pub period: String,
    pub total_assets: Option<f64>,
    pub total_current_assets: Option<f64>,
    pub cash_and_equivalents: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_current_liabilities: Option<f64>,
    pub long_term_debt: Option<f64>,
    pub total_equity: Option<f64>,
    pub retained_earnings: Option<f64>,
    pub total_debt: Option<f64>,
    pub net_debt: Option<f64>,
}

/// Cash flow statement data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowStatement {
    pub date: String,
    pub period: String,
    pub operating_cash_flow: Option<f64>,
    pub investing_cash_flow: Option<f64>,
    pub financing_cash_flow: Option<f64>,
    pub net_change_in_cash: Option<f64>,
    pub free_cash_flow: Option<f64>,
    pub capital_expenditure: Option<f64>,
    pub dividends_paid: Option<f64>,
    pub stock_repurchased: Option<f64>,
}

/// Key financial metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetrics {
    pub date: String,
    pub period: String,
    pub pe_ratio: Option<f64>,
    pub pb_ratio: Option<f64>,
    pub ps_ratio: Option<f64>,
    pub ev_to_ebitda: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub current_ratio: Option<f64>,
    pub roe: Option<f64>,
    pub roa: Option<f64>,
    pub gross_margin: Option<f64>,
    pub operating_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub payout_ratio: Option<f64>,
    pub revenue_per_share: Option<f64>,
    pub book_value_per_share: Option<f64>,
    pub free_cash_flow_per_share: Option<f64>,
    pub market_cap: Option<f64>,
    pub enterprise_value: Option<f64>,
}

/// News article
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsArticle {
    pub title: String,
    pub url: String,
    pub published_date: String,
    pub source: Option<String>,
    pub summary: Option<String>,
    pub symbol: Option<String>,
    pub image: Option<String>,
}

/// Analyst estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalystEstimate {
    pub symbol: String,
    pub date: String,
    pub estimated_revenue_avg: Option<f64>,
    pub estimated_revenue_high: Option<f64>,
    pub estimated_revenue_low: Option<f64>,
    pub estimated_eps_avg: Option<f64>,
    pub estimated_eps_high: Option<f64>,
    pub estimated_eps_low: Option<f64>,
    pub number_of_analysts: Option<i32>,
}

/// Analyst price target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTarget {
    pub symbol: String,
    pub published_date: Option<String>,
    pub analyst_name: Option<String>,
    pub analyst_company: Option<String>,
    pub price_target: Option<f64>,
    pub adj_price_target: Option<f64>,
    pub price_when_posted: Option<f64>,
    pub news_title: Option<String>,
}

/// Economic calendar event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicEvent {
    pub date: String,
    pub country: Option<String>,
    pub event: String,
    pub impact: Option<String>,
    pub actual: Option<f64>,
    pub estimate: Option<f64>,
    pub previous: Option<f64>,
    pub currency: Option<String>,
}

/// Stock screener filters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenerFilters {
    pub market_cap_min: Option<f64>,
    pub market_cap_max: Option<f64>,
    pub price_min: Option<f64>,
    pub price_max: Option<f64>,
    pub pe_min: Option<f64>,
    pub pe_max: Option<f64>,
    pub beta_min: Option<f64>,
    pub beta_max: Option<f64>,
    pub volume_min: Option<i64>,
    pub dividend_yield_min: Option<f64>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub country: Option<String>,
    pub exchange: Option<String>,
    pub is_etf: Option<bool>,
    pub limit: Option<i32>,
}

/// Stock screener result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenerResult {
    pub symbol: String,
    pub company_name: String,
    pub exchange: String,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub market_cap: Option<f64>,
    pub price: Option<f64>,
    pub change_percent: Option<f64>,
    pub volume: Option<i64>,
    pub pe_ratio: Option<f64>,
    pub beta: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub country: Option<String>,
}

/// FRED economic data observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FredObservation {
    pub date: String,
    pub value: Option<f64>,
}

/// FRED series metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FredSeries {
    pub id: String,
    pub title: String,
    pub frequency: Option<String>,
    pub units: Option<String>,
    pub seasonal_adjustment: Option<String>,
    pub last_updated: Option<String>,
    pub observation_start: Option<String>,
    pub observation_end: Option<String>,
    pub popularity: Option<i32>,
}

/// Yahoo Finance quote data (for generic mode)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericQuote {
    pub symbol: String,
    pub name: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    pub price: f64,
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub previous_close: Option<f64>,
    pub volume: Option<i64>,
    pub market_cap: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
    pub timestamp: Option<String>,
}

/// Yahoo Finance symbol search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub symbol: String,
    pub name: String,
    pub exchange: Option<String>,
    pub asset_type: Option<String>,
    pub exchange_display: Option<String>,
}

/// Congressional trade disclosure (Senate/House)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CongressionalTrade {
    pub date_received: Option<String>,
    pub transaction_date: Option<String>,
    pub owner: Option<String>,
    pub asset_description: Option<String>,
    pub asset_type: Option<String>,
    pub amount: Option<String>,
    pub transaction_type: Option<String>,  // "Purchase", "Sale", "Exchange", etc.
    pub comment: Option<String>,
    pub link: Option<String>,
    // Senator/Representative info
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub office: Option<String>,
    pub district: Option<String>,
    // Stock-specific fields (may not always be present)
    pub ticker: Option<String>,
    pub cap_gains_over_200: Option<bool>,
}

// ==================== NEW FMP API TYPES ====================

/// Earnings call transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsCallTranscript {
    pub symbol: String,
    pub quarter: Option<i32>,
    pub year: Option<i32>,
    pub date: Option<String>,
    pub content: Option<String>,
}

/// Insider trade (Form 4 filing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsiderTrade {
    pub symbol: String,
    pub filing_date: Option<String>,
    pub transaction_date: Option<String>,
    pub reporting_name: Option<String>,
    pub transaction_type: Option<String>,
    pub securities_owned: Option<f64>,
    pub securities_transacted: Option<f64>,
    pub price: Option<f64>,
    pub type_of_owner: Option<String>,
    pub form_type: Option<String>,
    pub link: Option<String>,
}

/// Institutional holder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstitutionalHolder {
    pub holder: String,
    pub shares: Option<f64>,
    pub date_reported: Option<String>,
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
}

/// Earnings calendar event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsCalendarEvent {
    pub symbol: String,
    pub date: String,
    pub eps_estimated: Option<f64>,
    pub eps_actual: Option<f64>,
    pub revenue_estimated: Option<f64>,
    pub revenue_actual: Option<f64>,
    pub fiscal_date_ending: Option<String>,
    pub updated_from_date: Option<String>,
}

/// IPO calendar event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpoCalendarEvent {
    pub symbol: Option<String>,
    pub company: Option<String>,
    pub date: String,
    pub exchange: Option<String>,
    pub actions: Option<String>,
    pub shares: Option<f64>,
    pub price_range: Option<String>,
    pub market_cap: Option<f64>,
}

/// Dividend calendar event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendCalendarEvent {
    pub symbol: String,
    pub date: String,
    pub label: Option<String>,
    pub adj_dividend: Option<f64>,
    pub dividend: Option<f64>,
    pub record_date: Option<String>,
    pub payment_date: Option<String>,
    pub declaration_date: Option<String>,
}

/// Stock split calendar event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSplitCalendarEvent {
    pub symbol: String,
    pub date: String,
    pub label: Option<String>,
    pub numerator: Option<f64>,
    pub denominator: Option<f64>,
}

/// ESG score data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsgScore {
    pub symbol: String,
    pub cik: Option<String>,
    pub date: Option<String>,
    pub environment_score: Option<f64>,
    pub social_score: Option<f64>,
    pub governance_score: Option<f64>,
    pub esg_score: Option<f64>,
    pub company_name: Option<String>,
    pub form_type: Option<String>,
    pub url: Option<String>,
}

/// ESG rating
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsgRating {
    pub symbol: String,
    pub cik: Option<String>,
    pub company_name: Option<String>,
    pub environment_rating: Option<String>,
    pub social_rating: Option<String>,
    pub governance_rating: Option<String>,
    pub esg_rating: Option<String>,
    pub year: Option<i32>,
}

/// ETF information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtfInfo {
    pub symbol: String,
    pub name: Option<String>,
    pub asset_class: Option<String>,
    pub avg_volume: Option<i64>,
    pub description: Option<String>,
    pub etf_company: Option<String>,
    pub expense_ratio: Option<f64>,
    pub inception_date: Option<String>,
    pub holdings_count: Option<i32>,
    pub nav: Option<f64>,
    pub net_assets: Option<f64>,
}

/// ETF holding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtfHolding {
    pub asset: Option<String>,
    pub name: Option<String>,
    pub shares: Option<f64>,
    pub weight: Option<f64>,
    pub market_value: Option<f64>,
}

/// Discounted cash flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountedCashFlow {
    pub symbol: String,
    pub date: Option<String>,
    pub dcf: Option<f64>,
    pub stock_price: Option<f64>,
}

/// Sector performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorPerformance {
    pub sector: String,
    pub change_percentage: Option<f64>,
}

/// Market mover (gainers, losers, most active)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMover {
    pub symbol: String,
    pub name: Option<String>,
    pub change: Option<f64>,
    pub price: Option<f64>,
    pub change_percent: Option<f64>,
}

/// Index constituent (S&P 500, Nasdaq, Dow Jones)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConstituent {
    pub symbol: String,
    pub name: Option<String>,
    pub sector: Option<String>,
    pub sub_sector: Option<String>,
    pub head_quarter: Option<String>,
    pub date_first_added: Option<String>,
    pub cik: Option<String>,
    pub founded: Option<String>,
}

/// FMP batch/full quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmpQuote {
    pub symbol: String,
    pub name: Option<String>,
    pub price: Option<f64>,
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
    pub day_low: Option<f64>,
    pub day_high: Option<f64>,
    pub year_low: Option<f64>,
    pub year_high: Option<f64>,
    pub market_cap: Option<f64>,
    pub pe: Option<f64>,
    pub eps: Option<f64>,
    pub volume: Option<i64>,
    pub avg_volume: Option<i64>,
    pub open: Option<f64>,
    pub previous_close: Option<f64>,
    pub exchange: Option<String>,
    pub timestamp: Option<i64>,
    pub earnings_announcement: Option<String>,
    pub shares_outstanding: Option<f64>,
}

/// FMP search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmpSearchResult {
    pub symbol: String,
    pub name: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    pub exchange_short_name: Option<String>,
}

/// Stock list item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockListItem {
    pub symbol: String,
    pub name: Option<String>,
    pub price: Option<f64>,
    pub exchange: Option<String>,
    pub exchange_short_name: Option<String>,
    pub stock_type: Option<String>,
}

/// Manual portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPosition {
    pub id: Option<i64>,
    pub symbol: String,
    pub exchange: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub current_price: Option<f64>,
    pub pnl: Option<f64>,
    pub pnl_percent: Option<f64>,
    pub position_type: String, // "long" or "short"
    pub asset_class: Option<String>,
    pub notes: Option<String>,
    pub added_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Client profile for client management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: Option<i64>,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub broker: Option<String>,
    pub account_id: Option<String>,
    pub account_type: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A single trade record for a client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTrade {
    pub id: Option<i64>,
    pub client_id: i64,
    pub import_batch_id: Option<i64>,
    pub symbol: String,
    pub exchange: String,
    pub trade_date: String,
    pub trade_type: String, // "buy" or "sell"
    pub quantity: f64,
    pub price: f64,
    pub fees: f64,
    pub order_id: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
}

/// A batch of imported trades from a CSV file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBatch {
    pub id: Option<i64>,
    pub client_id: i64,
    pub filename: String,
    pub row_count: i64,
    pub account_type: Option<String>,
    pub imported_at: Option<String>,
}

/// Computed position derived from trade history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPosition {
    pub symbol: String,
    pub exchange: String,
    pub net_quantity: f64,
    pub avg_price: f64,
    pub total_fees: f64,
    pub trade_count: i64,
    pub realized_pnl: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
}

/// Reconstructed long-only holding for a client (one row per symbol)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHolding {
    pub id: Option<i64>,
    pub client_id: i64,
    pub symbol: String,
    pub description: Option<String>,
    pub quantity: f64,
    pub avg_cost: f64,
    pub total_cost: f64,
    pub realized_pnl: f64,
    pub last_activity_date: Option<String>,
    pub updated_at: Option<String>,
    /// Live price from the Positions snapshot at import time. None when the
    /// holding was reconstructed from transactions only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_price: Option<f64>,
    /// Market value from the Positions snapshot at import time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_value: Option<f64>,
    /// Unrealized gain % from cost basis at the time of the snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_percent: Option<f64>,
}

/// Open / pending order parsed from broker order-status export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientOpenOrder {
    pub id: Option<i64>,
    pub client_id: i64,
    pub order_number: Option<String>,
    pub symbol: String,
    pub description: Option<String>,
    pub action: String,
    pub quantity: f64,
    pub order_type: Option<String>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub time_in_force: Option<String>,
    pub status: String,
    pub placed_at: Option<String>,
    pub last_activity_at: Option<String>,
    pub updated_at: Option<String>,
}

/// 401k or other rule-set violation flagged during import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub id: Option<i64>,
    pub client_id: i64,
    pub rule_set: String,
    pub violation_type: String,
    pub severity: String,
    pub symbol: Option<String>,
    pub quantity: Option<f64>,
    pub message: String,
    pub detected_at: Option<String>,
    pub resolved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

/// Mismatch between Order Status (Filled) and Transactions ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationMismatch {
    pub order_number: Option<String>,
    pub symbol: String,
    pub action: String,
    pub order_quantity: f64,
    pub order_fill_price: Option<f64>,
    pub transaction_quantity: Option<f64>,
    pub transaction_price: Option<f64>,
    pub mismatch_kind: String,
    pub note: String,
}

/// Summary header for an import report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReportSummary {
    pub transactions_processed: i64,
    pub order_status_processed: i64,
    pub total_holdings: i64,
    pub total_cost_basis: f64,
    pub open_buy_orders: i64,
    pub open_sell_orders: i64,
    pub violation_count: i64,
    pub is_compliant: bool,
}

/// Full report returned by `import_schwab_documents`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub client_id: i64,
    pub summary: ImportReportSummary,
    pub holdings: Vec<ClientHolding>,
    pub open_orders: Vec<ClientOpenOrder>,
    pub violations: Vec<ComplianceViolation>,
    pub reconciliation_mismatches: Vec<ReconciliationMismatch>,
    /// Symbols whose net-from-transactions came out negative on a no-shorts
    /// account but where no Positions baseline was supplied. These are NOT
    /// real shorts; they're pre-existing positions sold within the imported
    /// window. Surfaced for transparency, never as compliance violations.
    #[serde(default)]
    pub incomplete_history_symbols: Vec<String>,
    /// Untradable rows from the Positions snapshot (revoked, restricted,
    /// escrow, CUSIP-only). Schwab still carries these on the books but they
    /// can't be acted on; they get their own bucket on the report.
    #[serde(default)]
    pub stranded_positions: Vec<StrandedPosition>,
}

/// Untradable position from a Schwab Positions snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrandedPosition {
    pub symbol: String,
    pub description: String,
    pub quantity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<f64>,
    pub asset_type: String,
    pub reason: String,
}

// ===========================================================================
// Goldman Sax & Violins brief — house report schema (mirrors goldman/types.ts)
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldmanBrief {
    pub client_name: String,
    pub document_label: String,
    pub tempo: String,
    pub generated_date: String,
    pub subtitle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidential_label: Option<String>,
    pub movements: Vec<GoldmanMovement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum GoldmanMovement {
    Diagnosis(DiagnosisMovement),
    Restructuring(RestructuringMovement),
    Tactical(TacticalMovement),
    Coda(CodaMovement),
    Narrative(NarrativeMovement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisMovement {
    pub numeral: String,
    pub tempo: String,
    pub title_main: String,
    pub title_accent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    pub issues: Vec<DiagnosisIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headline: Option<HeadlineNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisIssue {
    pub title: String,
    pub body: String,
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlineNote {
    pub lead: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestructuringMovement {
    pub numeral: String,
    pub tempo: String,
    pub title_main: String,
    pub title_accent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    pub allocation_cards: Vec<AllocationCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_core_table: Option<ProposedCoreTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector_sleeve: Option<BulletSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immediate_eliminations: Option<BulletSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationCard {
    pub percentage: String,
    pub label: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedCoreTable {
    pub heading: String,
    pub rows: Vec<PortfolioRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletSection {
    pub heading: String,
    pub bullets: Vec<BulletItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRow {
    pub ticker: String,
    pub name: String,
    pub pl: String,
    pub pl_positive: bool,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalMovement {
    pub numeral: String,
    pub tempo: String,
    pub title_main: String,
    pub title_accent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    pub sections: Vec<TacticalSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalSection {
    pub number: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullets: Option<Vec<BulletItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<TacticalTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalTable {
    pub columns: Vec<String>,
    pub rows: Vec<PortfolioRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodaMovement {
    pub numeral: String,
    pub tempo: String,
    pub title_main: String,
    pub title_accent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    pub groups: Vec<ChecklistGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closing_note: Option<HeadlineNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistGroup {
    pub heading: String,
    pub items: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub lead: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeMovement {
    pub numeral: String,
    pub tempo: String,
    pub title_main: String,
    pub title_accent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    pub paragraphs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullets: Option<Vec<BulletItem>>,
}

/// A what-if scenario for a client portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientScenario {
    pub id: Option<i64>,
    pub client_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_baseline: bool,
    pub cloned_from_id: Option<i64>,
    pub account_type: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A position within a client scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioPosition {
    pub id: Option<i64>,
    pub scenario_id: i64,
    pub symbol: String,
    pub exchange: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub side: String,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
