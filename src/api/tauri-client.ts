/**
 * Tauri IPC client for OpenAlgo Desktop
 *
 * This module provides type-safe wrappers around Tauri's invoke function
 * to communicate with the Rust backend.
 */

import { invoke } from '@tauri-apps/api/core'

// ============================================================================
// Types
// ============================================================================

export interface TauriError {
  code: string
  message: string
}

export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  success: boolean
  user_id: number
  username: string
}

export interface UserInfo {
  user_id: number
  username: string
  authenticated_at: string
}

export interface BrokerLoginRequest {
  broker_id: string
  credentials: BrokerCredentials
}

export interface BrokerCredentials {
  api_key: string
  api_secret?: string
  client_id?: string
  password?: string
  totp?: string
  request_token?: string
  auth_code?: string
}

export interface BrokerLoginResponse {
  success: boolean
  broker_id: string
  user_id: string
  user_name?: string
}

export interface BrokerStatus {
  connected: boolean
  broker_id?: string
  user_id?: string
  authenticated_at?: string
}

export interface BrokerInfo {
  id: string
  name: string
  logo: string
  requires_totp: boolean
}

export interface OrderRequest {
  symbol: string
  exchange: string
  side: string
  quantity: number
  price: number
  order_type: string
  product: string
  validity: string
  trigger_price?: number
  disclosed_quantity?: number
  amo: boolean
}

export interface ModifyOrderRequest {
  quantity?: number
  price?: number
  order_type?: string
  trigger_price?: number
  validity?: string
}

export interface OrderResponse {
  success: boolean
  order_id: string
  message?: string
}

export interface Order {
  order_id: string
  exchange_order_id?: string
  symbol: string
  exchange: string
  side: string
  quantity: number
  filled_quantity: number
  pending_quantity: number
  price: number
  trigger_price: number
  average_price: number
  order_type: string
  product: 'MIS' | 'NRML' | 'CNC'
  status: string
  validity: string
  order_timestamp: string
  exchange_timestamp?: string
  rejection_reason?: string
  // Additional fields for frontend compatibility
  action: 'BUY' | 'SELL'
  pricetype: 'MARKET' | 'LIMIT' | 'SL' | 'SL-M'
  orderid: string
  order_status: 'complete' | 'rejected' | 'cancelled' | 'open' | 'pending' | 'trigger pending'
  timestamp: string
}

export interface Position {
  symbol: string
  exchange: string
  product: 'MIS' | 'NRML' | 'CNC'
  quantity: number
  overnight_quantity: number
  average_price: number
  ltp: number
  pnl: number
  pnlpercent: number // Added for frontend compatibility
  realized_pnl: number
  unrealized_pnl: number
  buy_quantity: number
  buy_value: number
  sell_quantity: number
  sell_value: number
}

export interface Holding {
  symbol: string
  exchange: string
  isin?: string
  quantity: number
  t1_quantity: number
  average_price: number
  ltp: number
  close_price: number
  pnl: number
  pnl_percentage: number
  pnlpercent: number // Alias for frontend compatibility
  current_value: number
  product: string // Added for frontend compatibility
}

export interface Funds {
  available_cash: number
  used_margin: number
  total_margin: number
  opening_balance: number
  payin: number
  payout: number
  span: number
  exposure: number
  collateral: number
}

export interface QuoteRequest {
  exchange: string
  symbol: string
}

export interface Quote {
  symbol: string
  exchange: string
  ltp: number
  open: number
  high: number
  low: number
  close: number
  volume: number
  bid: number
  ask: number
  bid_qty: number
  ask_qty: number
  oi: number
  change: number
  change_percent: number
  timestamp: string
}

export interface MarketDepth {
  symbol: string
  exchange: string
  bids: DepthLevel[]
  asks: DepthLevel[]
}

export interface DepthLevel {
  price: number
  quantity: number
  orders: number
}

export interface SymbolSearchResult {
  symbol: string
  token: string
  exchange: string
  name: string
  instrument_type: string
  lot_size: number
  // Additional fields for frontend compatibility
  brsymbol: string
  lotsize: number
}

export interface Strategy {
  id: number
  name: string
  webhook_id: string
  exchange: string
  symbol: string
  product: string
  quantity: number
  enabled: boolean
  created_at: string
  updated_at: string
  // Additional fields for frontend compatibility
  platform: 'tradingview' | 'amibroker' | 'python' | 'metatrader' | 'excel' | 'others'
  is_active: boolean
  is_intraday: boolean
  trading_mode: 'LONG' | 'SHORT' | 'BOTH'
  start_time: string | null
  end_time: string | null
  squareoff_time: string | null
}

export interface CreateStrategyRequest {
  name: string
  webhook_id: string
  exchange: string
  symbol: string
  product: string
  quantity: number
  enabled: boolean
  // Additional fields for frontend compatibility
  platform?: string
  strategy_type?: string
  trading_mode?: string
  start_time?: string
  end_time?: string
  squareoff_time?: string
}

export interface UpdateStrategyRequest {
  id: number
  name?: string
  exchange?: string
  symbol?: string
  product?: string
  quantity?: number
  enabled?: boolean
}

export interface Settings {
  id: number
  theme: string
  default_broker?: string
  default_exchange: string
  default_product: string
  order_confirm: boolean
  sound_enabled: boolean
}

export interface AnalyzerModeStatus {
  analyze_mode: boolean
  mode: string // "analyzer" or "live"
}

export interface UpdateSettingsRequest {
  theme?: string
  default_broker?: string
  default_exchange?: string
  default_product?: string
  order_confirm?: boolean
  sound_enabled?: boolean
}

export interface SaveBrokerCredentialsRequest {
  broker_id: string
  api_key: string
  api_secret?: string
  client_id?: string
}

export interface SandboxOrder {
  id: number
  order_id: string
  symbol: string
  exchange: string
  side: string
  quantity: number
  price: number
  order_type: string
  product: string
  status: string
  filled_quantity: number
  average_price: number
  created_at: string
  updated_at: string
}

export interface SandboxPosition {
  id: number
  symbol: string
  exchange: string
  product: string
  quantity: number
  average_price: number
  ltp: number
  pnl: number
  created_at: string
  updated_at: string
}

export interface SandboxOrderRequest {
  symbol: string
  exchange: string
  side: string
  quantity: number
  price: number
  order_type: string
  product: string
}

export interface SandboxHolding {
  id: number
  symbol: string
  exchange: string
  quantity: number
  average_price: number
  ltp: number
  pnl: number
  created_at: string
  updated_at: string
}

export interface SandboxFunds {
  available_cash: number
  used_margin: number
  total_value: number
  updated_at: string
}

export interface SandboxTrade {
  id: number
  order_id: string
  trade_id: string
  symbol: string
  exchange: string
  side: string
  quantity: number
  price: number
  created_at: string
}

export interface SandboxDailyPnl {
  id: number
  date: string
  realized_pnl: number
  unrealized_pnl: number
  total_pnl: number
  portfolio_value: number
  created_at: string
}

export interface SandboxPnlSummary {
  today_realized_pnl: number
  positions_unrealized_pnl: number
  holdings_unrealized_pnl: number
  today_total_mtm: number
  all_time_realized_pnl: number
  portfolio_value: number
}

export interface SandboxPnlData {
  summary: SandboxPnlSummary
  daily_pnl: SandboxDailyPnl[]
  positions: SandboxPosition[]
  holdings: SandboxHolding[]
  trades: SandboxTrade[]
}

export interface SandboxConfig {
  starting_capital: number
  reset_day: string
  reset_time: string
  order_check_interval: number
  mtm_update_interval: number
  nse_mis_leverage: number
  nfo_mis_leverage: number
  cds_mis_leverage: number
  mcx_mis_leverage: number
  nse_cnc_leverage: number
  nfo_nrml_leverage: number
  cds_nrml_leverage: number
  mcx_nrml_leverage: number
  nse_square_off_time: string
  nfo_square_off_time: string
  cds_square_off_time: string
  mcx_square_off_time: string
}

export interface MarketDataQuery {
  symbol: string
  exchange: string
  timeframe: string
  from_date: string
  to_date: string
}

export interface MarketDataRow {
  timestamp: string
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export interface ClosePositionRequest {
  symbol: string
  exchange: string
  product: string
  quantity: number
  position_type: string
}

export interface ClosePositionResponse {
  success: boolean
  order_id?: string
  message: string
}

export interface WebSocketStatus {
  connected: boolean
  broker: string | null
  subscriptions: number
}

export interface WebSocketSubscribeRequest {
  exchange: string
  token: string
  symbol?: string
  mode?: string // 'ltp' | 'quote' | 'snapquote' | 'full' | 'depth'
}

export interface MarketTick {
  symbol: string
  exchange: string
  token: string
  ltp: number
  open: number
  high: number
  low: number
  close: number
  volume: number
  bid: number
  ask: number
  bid_qty: number
  ask_qty: number
  oi: number
  timestamp: number
  change: number
  change_percent: number
}

// ============================================================================
// Tauri Command Wrappers
// ============================================================================

/**
 * Generic invoke wrapper with error handling
 */
async function tauriInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    // Tauri errors come as objects with code and message
    const tauriError = error as TauriError
    throw new Error(tauriError.message || String(error))
  }
}

// ============================================================================
// Auth Commands
// ============================================================================

export const authCommands = {
  login: (request: LoginRequest) => tauriInvoke<LoginResponse>('login', { request }),

  logout: () => tauriInvoke<void>('logout'),

  checkSession: () => tauriInvoke<boolean>('check_session'),

  getCurrentUser: () => tauriInvoke<UserInfo | null>('get_current_user'),
}

// ============================================================================
// Broker Commands
// ============================================================================

export const brokerCommands = {
  brokerLogin: (request: BrokerLoginRequest) =>
    tauriInvoke<BrokerLoginResponse>('broker_login', { request }),

  brokerLogout: () => tauriInvoke<void>('broker_logout'),

  getBrokerStatus: () => tauriInvoke<BrokerStatus>('get_broker_status'),

  setActiveBroker: (brokerId: string) =>
    tauriInvoke<void>('set_active_broker', { broker_id: brokerId }),

  getAvailableBrokers: () => tauriInvoke<BrokerInfo[]>('get_available_brokers'),
}

// ============================================================================
// Order Commands
// ============================================================================

export const orderCommands = {
  placeOrder: (order: OrderRequest) => tauriInvoke<OrderResponse>('place_order', { order }),

  modifyOrder: (orderId: string, order: ModifyOrderRequest) =>
    tauriInvoke<OrderResponse>('modify_order', { order_id: orderId, order }),

  cancelOrder: (orderId: string, variety?: string) =>
    tauriInvoke<OrderResponse>('cancel_order', { order_id: orderId, variety }),

  getOrderBook: () => tauriInvoke<Order[]>('get_order_book'),

  getTradeBook: () => tauriInvoke<Order[]>('get_trade_book'),
}

// ============================================================================
// Position Commands
// ============================================================================

export const positionCommands = {
  getPositions: () => tauriInvoke<Position[]>('get_positions'),

  closePosition: (request: ClosePositionRequest) =>
    tauriInvoke<ClosePositionResponse>('close_position', { request }),

  closeAllPositions: () => tauriInvoke<ClosePositionResponse[]>('close_all_positions'),
}

// ============================================================================
// Holdings Commands
// ============================================================================

export const holdingsCommands = {
  getHoldings: () => tauriInvoke<Holding[]>('get_holdings'),
}

// ============================================================================
// Funds Commands
// ============================================================================

export const fundsCommands = {
  getFunds: () => tauriInvoke<Funds>('get_funds'),
}

// ============================================================================
// Quote Commands
// ============================================================================

export const quoteCommands = {
  getQuote: (symbols: QuoteRequest[]) => tauriInvoke<Quote[]>('get_quote', { symbols }),

  getMarketDepth: (exchange: string, symbol: string) =>
    tauriInvoke<MarketDepth>('get_market_depth', { exchange, symbol }),
}

// ============================================================================
// Symbol Commands
// ============================================================================

export const symbolCommands = {
  searchSymbols: (query: string, exchange?: string, limit?: number) =>
    tauriInvoke<SymbolSearchResult[]>('search_symbols', { query, exchange, limit }),

  getSymbolInfo: (exchange: string, symbol: string) =>
    tauriInvoke<SymbolSearchResult>('get_symbol_info', { exchange, symbol }),

  refreshSymbolMaster: () => tauriInvoke<number>('refresh_symbol_master'),
}

// ============================================================================
// Strategy Commands
// ============================================================================

export const strategyCommands = {
  getStrategies: () => tauriInvoke<Strategy[]>('get_strategies'),

  createStrategy: (request: CreateStrategyRequest) =>
    tauriInvoke<Strategy>('create_strategy', { request }),

  updateStrategy: (request: UpdateStrategyRequest) =>
    tauriInvoke<Strategy>('update_strategy', { request }),

  deleteStrategy: (id: number) => tauriInvoke<void>('delete_strategy', { id }),

  toggleStrategy: (id: number, enabled: boolean) =>
    tauriInvoke<Strategy>('toggle_strategy', { id, enabled }),
}

// ============================================================================
// Settings Commands
// ============================================================================

export const settingsCommands = {
  getSettings: () => tauriInvoke<Settings>('get_settings'),

  updateSettings: (request: UpdateSettingsRequest) =>
    tauriInvoke<Settings>('update_settings', { request }),

  saveBrokerCredentials: (request: SaveBrokerCredentialsRequest) =>
    tauriInvoke<void>('save_broker_credentials', { request }),

  deleteBrokerCredentials: (brokerId: string) =>
    tauriInvoke<void>('delete_broker_credentials', { broker_id: brokerId }),

  getAnalyzeMode: () => tauriInvoke<AnalyzerModeStatus>('get_analyze_mode'),

  setAnalyzeMode: (enabled: boolean) =>
    tauriInvoke<AnalyzerModeStatus>('set_analyze_mode', { enabled }),
}

// ============================================================================
// Sandbox Commands
// ============================================================================

export const sandboxCommands = {
  getSandboxPositions: () => tauriInvoke<SandboxPosition[]>('get_sandbox_positions'),

  getSandboxOrders: () => tauriInvoke<SandboxOrder[]>('get_sandbox_orders'),

  placeSandboxOrder: (order: SandboxOrderRequest) =>
    tauriInvoke<SandboxOrder>('place_sandbox_order', { order }),

  resetSandbox: () => tauriInvoke<void>('reset_sandbox'),

  getSandboxHoldings: () => tauriInvoke<SandboxHolding[]>('get_sandbox_holdings'),

  getSandboxFunds: () => tauriInvoke<SandboxFunds>('get_sandbox_funds'),

  updateSandboxLtp: (exchange: string, symbol: string, ltp: number) =>
    tauriInvoke<void>('update_sandbox_ltp', { request: { exchange, symbol, ltp } }),

  cancelSandboxOrder: (orderId: string) =>
    tauriInvoke<{ success: boolean; order_id: string }>('cancel_sandbox_order', { order_id: orderId }),

  getSandboxConfig: () => tauriInvoke<SandboxConfig>('get_sandbox_config'),

  updateSandboxConfig: (key: string, value: string) =>
    tauriInvoke<void>('update_sandbox_config', { request: { key, value } }),

  getSandboxTrades: () => tauriInvoke<SandboxTrade[]>('get_sandbox_trades'),

  getSandboxDailyPnl: () => tauriInvoke<SandboxDailyPnl[]>('get_sandbox_daily_pnl'),

  getSandboxPnl: () => tauriInvoke<SandboxPnlData>('get_sandbox_pnl'),
}

// ============================================================================
// Historify Commands
// ============================================================================

export const historifyCommands = {
  getMarketData: (query: MarketDataQuery) =>
    tauriInvoke<MarketDataRow[]>('get_market_data', { query }),

  downloadHistoricalData: (request: MarketDataQuery) =>
    tauriInvoke<{ success: boolean; rows_downloaded: number; message: string }>(
      'download_historical_data',
      { request }
    ),
}

// ============================================================================
// WebSocket Commands
// ============================================================================

export const websocketCommands = {
  connect: () => tauriInvoke<boolean>('websocket_connect'),

  disconnect: () => tauriInvoke<boolean>('websocket_disconnect'),

  status: () => tauriInvoke<WebSocketStatus>('websocket_status'),

  subscribe: (symbols: WebSocketSubscribeRequest[]) =>
    tauriInvoke<boolean>('websocket_subscribe', { symbols }),

  unsubscribe: (symbols: [string, string][]) =>
    tauriInvoke<boolean>('websocket_unsubscribe', { symbols }),

  registerSymbol: (token: string, symbol: string, exchange: string) =>
    tauriInvoke<void>('websocket_register_symbol', { token, symbol, exchange }),
}

// ============================================================================
// Provider Commands (Generic / Research Mode)
// ============================================================================

export interface GenericQuote {
  symbol: string
  name: string | null
  exchange: string | null
  currency: string | null
  price: number
  change: number | null
  change_percent: number | null
  open: number | null
  high: number | null
  low: number | null
  previous_close: number | null
  volume: number | null
  market_cap: number | null
  pe_ratio: number | null
  fifty_two_week_high: number | null
  fifty_two_week_low: number | null
  timestamp: string | null
}

export interface GlobalSymbolSearchResult {
  symbol: string
  name: string
  exchange: string | null
  asset_type: string | null
  exchange_display: string | null
}

export interface PortfolioPosition {
  id: number | null
  symbol: string
  exchange: string
  quantity: number
  avg_price: number
  current_price: number | null
  pnl: number | null
  pnl_percent: number | null
  position_type: string
  asset_class: string | null
  notes: string | null
  added_at: string | null
  updated_at: string | null
}

export interface CompanyProfile {
  symbol: string
  company_name: string
  exchange: string
  industry: string | null
  sector: string | null
  country: string | null
  market_cap: number | null
  description: string | null
  website: string | null
  ceo: string | null
  employees: number | null
  ipo_date: string | null
  currency: string | null
  price: number | null
  beta: number | null
  volume_avg: number | null
  last_dividend: number | null
  image: string | null
}

export interface CongressionalTrade {
  date_received: string | null
  transaction_date: string | null
  owner: string | null
  asset_description: string | null
  asset_type: string | null
  amount: string | null
  transaction_type: string | null
  comment: string | null
  link: string | null
  first_name: string | null
  last_name: string | null
  office: string | null
  district: string | null
  ticker: string | null
  cap_gains_over_200: boolean | null
}

export interface ScreenerFilters {
  market_cap_min?: number
  market_cap_max?: number
  price_min?: number
  price_max?: number
  pe_min?: number
  pe_max?: number
  beta_min?: number
  beta_max?: number
  volume_min?: number
  dividend_yield_min?: number
  sector?: string
  industry?: string
  country?: string
  exchange?: string
  is_etf?: boolean
  limit?: number
}

export const providerCommands = {
  // API key management
  saveProviderApiKey: (provider: string, apiKey: string) =>
    tauriInvoke<void>('save_provider_api_key', { provider, api_key: apiKey }),

  deleteProviderApiKey: (provider: string) =>
    tauriInvoke<boolean>('delete_provider_api_key', { provider }),

  getConfiguredProviders: () =>
    tauriInvoke<string[]>('get_configured_providers'),

  getProviderKeyStatus: (provider: string) =>
    tauriInvoke<boolean>('get_provider_key_status', { provider }),

  // Generic mode
  getGenericMode: () => tauriInvoke<boolean>('get_generic_mode'),
  setGenericMode: (enabled: boolean) => tauriInvoke<void>('set_generic_mode', { enabled }),

  // Yahoo Finance (no API key needed)
  getGenericQuote: (symbols: string[]) =>
    tauriInvoke<GenericQuote[]>('get_generic_quote', { symbols }),

  searchGlobalSymbols: (query: string, limit?: number) =>
    tauriInvoke<GlobalSymbolSearchResult[]>('search_global_symbols', { query, limit }),

  getYahooHistorical: (symbol: string, interval: string, range: string) =>
    tauriInvoke<[string, number, number, number, number, number][]>('get_yahoo_historical', { symbol, interval, range }),

  // FMP (requires API key)
  getCompanyProfile: (symbol: string) =>
    tauriInvoke<CompanyProfile | null>('get_company_profile', { symbol }),

  getIncomeStatement: (symbol: string, period: string, limit?: number) =>
    tauriInvoke<unknown[]>('get_income_statement', { symbol, period, limit }),

  getBalanceSheet: (symbol: string, period: string, limit?: number) =>
    tauriInvoke<unknown[]>('get_balance_sheet', { symbol, period, limit }),

  getCashFlow: (symbol: string, period: string, limit?: number) =>
    tauriInvoke<unknown[]>('get_cash_flow', { symbol, period, limit }),

  getKeyMetrics: (symbol: string, period: string, limit?: number) =>
    tauriInvoke<unknown[]>('get_key_metrics', { symbol, period, limit }),

  getStockNews: (symbols?: string, limit?: number) =>
    tauriInvoke<unknown[]>('get_stock_news', { symbols, limit }),

  getAnalystEstimates: (symbol: string, limit?: number) =>
    tauriInvoke<unknown[]>('get_analyst_estimates', { symbol, limit }),

  getPriceTargets: (symbol: string) =>
    tauriInvoke<unknown[]>('get_price_targets', { symbol }),

  getEconomicCalendar: (fromDate: string, toDate: string) =>
    tauriInvoke<unknown[]>('get_economic_calendar', { from_date: fromDate, to_date: toDate }),

  screenStocks: (filters: ScreenerFilters) =>
    tauriInvoke<unknown[]>('screen_stocks', { filters }),

  // Congressional Trading (FMP)
  getSenateTrades: (page?: number, limit?: number) =>
    tauriInvoke<CongressionalTrade[]>('get_senate_trades', { page, limit }),

  getSenateTradesByName: (name: string) =>
    tauriInvoke<CongressionalTrade[]>('get_senate_trades_by_name', { name }),

  getHouseTrades: (page?: number, limit?: number) =>
    tauriInvoke<CongressionalTrade[]>('get_house_trades', { page, limit }),

  getHouseTradesByName: (name: string) =>
    tauriInvoke<CongressionalTrade[]>('get_house_trades_by_name', { name }),

  // FRED (requires API key)
  getFredSeries: (seriesId: string, observationStart?: string, observationEnd?: string) =>
    tauriInvoke<unknown[]>('get_fred_series', { series_id: seriesId, observation_start: observationStart, observation_end: observationEnd }),

  searchFredSeries: (query: string, limit?: number) =>
    tauriInvoke<unknown[]>('search_fred_series', { query, limit }),

  getFredReleases: () =>
    tauriInvoke<[string, string, string][]>('get_fred_releases'),
}

// ============================================================================
// Portfolio Commands (Generic / Research Mode)
// ============================================================================

export const portfolioCommands = {
  addPosition: (params: {
    symbol: string
    exchange: string
    quantity: number
    avg_price: number
    position_type?: string
    asset_class?: string
    notes?: string
  }) => tauriInvoke<PortfolioPosition>('add_portfolio_position', params),

  updatePosition: (params: {
    id: number
    quantity?: number
    avg_price?: number
    notes?: string
  }) => tauriInvoke<PortfolioPosition>('update_portfolio_position', params),

  deletePosition: (id: number) =>
    tauriInvoke<boolean>('delete_portfolio_position', { id }),

  getPositions: () =>
    tauriInvoke<PortfolioPosition[]>('get_portfolio_positions'),

  importCsv: (csvContent: string) =>
    tauriInvoke<number>('import_portfolio_csv', { csv_content: csvContent }),

  exportCsv: () =>
    tauriInvoke<string>('export_portfolio_csv'),
}

// ============================================================================
// Convenience exports
// ============================================================================

export const tauri = {
  auth: authCommands,
  broker: brokerCommands,
  orders: orderCommands,
  positions: positionCommands,
  holdings: holdingsCommands,
  funds: fundsCommands,
  quotes: quoteCommands,
  symbols: symbolCommands,
  strategies: strategyCommands,
  settings: settingsCommands,
  sandbox: sandboxCommands,
  historify: historifyCommands,
  websocket: websocketCommands,
  providers: providerCommands,
  portfolio: portfolioCommands,
}

export default tauri
