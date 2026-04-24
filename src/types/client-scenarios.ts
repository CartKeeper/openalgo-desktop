export interface ClientScenario {
  id: number | null
  client_id: number
  name: string
  description: string | null
  is_baseline: boolean
  cloned_from_id: number | null
  account_type: string | null
  created_at: string | null
  updated_at: string | null
}

export interface ScenarioPosition {
  id: number | null
  scenario_id: number
  symbol: string
  exchange: string
  quantity: number
  avg_price: number
  side: string
  notes: string | null
  created_at: string | null
  updated_at: string | null
}

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

export interface EnrichedScenarioPosition extends ScenarioPosition {
  current_price: number | null
  market_value: number | null
  cost_basis: number
  unrealized_pnl: number | null
  pnl_pct: number | null
  day_change: number | null
  day_change_pct: number | null
  company_name: string | null
}

export interface ScenarioSummary {
  total_value: number
  total_cost: number
  total_pnl: number
  pnl_pct: number
  position_count: number
}
