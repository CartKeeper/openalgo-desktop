/**
 * Portfolio analytics (WP2): returns, tax lots, allocation targets, drift and
 * the rebalance plan.
 *
 * Thin invoke wrappers over the `analytics_*` Tauri commands. The types mirror
 * `src-tauri/src/analytics/*` and `services/portfolio_analytics_service.rs`;
 * dates are `YYYY-MM-DD` strings, money is in dollars, percentages in percent.
 */
import { invoke } from '@tauri-apps/api/core'
import { ACCOUNT_TYPES } from '@/types/clients'

// ---- Lots ------------------------------------------------------------------

export type LotMethod = 'fifo' | 'lifo' | 'hifo' | 'average'
export type TradeSide = 'buy' | 'sell'
/** Long when held strictly more than 365 days. */
export type Term = 'short' | 'long'

export interface OpenLot {
  symbol: string
  opened: string
  quantity: number
  cost_per_share: number
  cost_basis: number
  /** The lot predates the trade history and was priced at the holding's average cost. */
  estimated: boolean
  market_value: number | null
  unrealized_pnl: number | null
  term: Term
}

export interface Disposal {
  symbol: string
  opened: string
  closed: string
  quantity: number
  proceeds: number
  cost_basis: number
  realized_pnl: number
  holding_days: number
  term: Term
  estimated: boolean
}

export interface LotTotals {
  cost_basis: number
  market_value: number
  unrealized_short: number
  unrealized_long: number
  realized_short_ytd: number
  realized_long_ytd: number
}

export interface LotReport {
  method: LotMethod
  as_of: string
  open_lots: OpenLot[]
  disposals: Disposal[]
  totals: LotTotals
  warnings: string[]
}

// ---- Performance -----------------------------------------------------------

export type PeriodKey = '1M' | '3M' | '6M' | 'YTD' | '1Y' | 'SI'

export interface PerformancePeriod {
  key: PeriodKey
  start: string
  end: string
  start_value: number
  end_value: number
  /** Money into securities inside the window (buys positive, sells negative). */
  net_flows: number
  /** Time-weighted return as a fraction (0.15 = 15%). */
  twr: number | null
  /** Only for windows of a year or more. */
  twr_annualized: number | null
  /** XIRR of the window's flows, annualized by construction. */
  mwr: number | null
  note: string | null
}

export interface PerformanceReport {
  as_of: string
  /** e.g. `Nov 11, 2026 close · 3:45 PM ET` */
  prices_as_of: string
  periods: PerformancePeriod[]
  warnings: string[]
}

// ---- Allocation ------------------------------------------------------------

export type TargetScope = 'symbol' | 'sector'

export interface AllocationTarget {
  id: number | null
  client_id: number
  scope: TargetScope
  /** A symbol (`VOO`) or a sector name (`Technology`). */
  key: string
  target_pct: number
  band_pct: number
}

export type DriftStatus = 'in_band' | 'over' | 'under'

export interface DriftRow {
  scope: TargetScope
  key: string
  target_pct: number
  band_pct: number
  actual_pct: number
  /** `actual_pct − target_pct` */
  drift_pct: number
  actual_value: number
  status: DriftStatus
}

export interface DriftReport {
  /** Σ priced holding value + cash */
  total_value: number
  cash: number
  rows: DriftRow[]
  /** Share of the total that no target covers (cash included). */
  untargeted_pct: number
  /** Distinct sectors of the holdings, sorted. */
  sectors: string[]
  warnings: string[]
}

export interface PlanLine {
  symbol: string
  exchange: string
  side: TradeSide
  quantity: number
  price: number
  value: number
  before_pct: number
  after_pct: number
  reason: string
}

export interface RebalancePlan {
  lines: PlanLine[]
  cash_before: number
  cash_after: number
  /** Targets the plan could not act on, one sentence each. */
  unresolved: string[]
  note: string
}

// ---- Calls -----------------------------------------------------------------

function message(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw new Error(message(error))
  }
}

/**
 * The label of an account scope: `null` is every account; otherwise the
 * account type's label (`401(k)`), with the manual-trades bucket spelled out.
 */
export function accountScopeLabel(accountType: string | null): string {
  if (accountType == null || accountType === 'all') return 'All accounts'
  if (accountType === 'manual') return 'Manual trades'
  return ACCOUNT_TYPES.find((t) => t.value === accountType)?.label ?? accountType
}

export const analyticsApi = {
  /**
   * Tax lots of a client's trades with the given matching method. With an
   * `accountType` only that account's trades are matched (and no opening lots
   * are estimated, since holdings are stored per client).
   */
  clientLots: (clientId: number, method: LotMethod, accountType: string | null = null) =>
    call<LotReport>('analytics_client_lots', { clientId, method, accountType }),
  /** Time- and money-weighted returns over 1M, 3M, 6M, YTD, 1Y and since inception. */
  clientPerformance: (clientId: number, accountType: string | null = null) =>
    call<PerformanceReport>('analytics_client_performance', { clientId, accountType }),
  /** The client's saved allocation targets. */
  listTargets: (clientId: number) =>
    call<AllocationTarget[]>('analytics_list_targets', { clientId }),
  /** Replace the client's targets; rejects (with the sum) above 100% per scope. */
  saveTargets: (clientId: number, targets: AllocationTarget[]) =>
    call<AllocationTarget[]>('analytics_save_targets', { clientId, targets }),
  /** Each target's drift for a scenario. */
  scenarioDrift: (scenarioId: number) =>
    call<DriftReport>('analytics_scenario_drift', { scenarioId }),
  /** Whole-share trades that bring a scenario back to its targets. */
  rebalancePlan: (scenarioId: number) =>
    call<RebalancePlan>('analytics_rebalance_plan', { scenarioId }),
}
