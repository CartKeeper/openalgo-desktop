import { invoke } from '@tauri-apps/api/core'

export type StrategySpec =
  | { kind: 'SmaCrossover'; fast: number; slow: number }
  | { kind: 'EmaCrossover'; fast: number; slow: number }
  | { kind: 'RsiThreshold'; period: number; oversold: number; overbought: number }
  | { kind: 'MacdCross'; fast: number; slow: number; signal: number }
  | { kind: 'BollingerReversion'; period: number; std_dev: number }

export type Sizing =
  | { kind: 'AllIn' }
  | { kind: 'FixedFraction'; value: number }
  | { kind: 'FixedShares'; value: number }

export interface Costs {
  commission_per_trade: number
  slippage_bps: number
  reg_fees_enabled: boolean
}
export interface TaxConfig {
  st_rate: number
  lt_rate: number
}

export interface BacktestConfig {
  symbol: string
  exchange: string
  interval: string
  from_date: string
  to_date: string
  starting_capital: number
  sizing: Sizing
  costs: Costs
  tax: TaxConfig
  fractional: boolean
  risk_free_rate: number
  strategy: StrategySpec
}

export interface Trade {
  entry_time: string
  exit_time: string
  entry_price: number
  exit_price: number
  shares: number
  pnl_after_fees: number
  fees: number
  holding_days: number
  long_term: boolean
}
export interface EquityPoint {
  timestamp: string
  equity: number
  drawdown: number
}
export interface BenchmarkResult {
  total_return: number
  cagr: number
  max_drawdown: number
}
export interface BacktestMetrics {
  total_return: number
  cagr: number
  volatility: number
  sharpe: number
  sortino: number
  calmar: number
  max_drawdown: number
  max_dd_peak: string
  max_dd_trough: string
  num_trades: number
  win_rate: number
  avg_win: number
  avg_loss: number
  profit_factor: number
  max_consecutive_losses: number
  avg_holding_days: number
  time_in_market: number
  total_fees: number
  st_tax: number
  lt_tax: number
  total_tax: number
  net_total_return: number
  net_cagr: number
}
export interface BacktestResult {
  config: BacktestConfig
  equity_curve: EquityPoint[]
  trades: Trade[]
  metrics: BacktestMetrics
  benchmark: BenchmarkResult
  warnings: string[]
}

export interface BacktestRunRecord {
  id: number
  created_at: string
  symbol: string
  exchange: string
  interval: string
  from_date: string
  to_date: string
  strategy_kind: string
  config_json: string
  summary_json: string
}

export const backtestApi = {
  run: (config: BacktestConfig) => invoke<BacktestResult>('run_backtest', { config }),
  save: (config: BacktestConfig, summaryJson: string) =>
    invoke<number>('save_backtest_run', { config, summaryJson }),
  list: () => invoke<BacktestRunRecord[]>('list_backtest_runs'),
  get: (id: number) => invoke<BacktestRunRecord | null>('get_backtest_run', { id }),
}
