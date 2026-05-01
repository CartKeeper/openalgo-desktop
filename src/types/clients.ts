export interface Client {
  id: number | null
  name: string
  email: string | null
  phone: string | null
  broker: string | null
  account_id: string | null
  account_type: string | null
  notes: string | null
  created_at: string | null
  updated_at: string | null
}

/** Account types and their short-selling permissions */
export const ACCOUNT_TYPES = [
  { value: 'individual', label: 'Individual Brokerage', shortSellingAllowed: true },
  { value: 'joint', label: 'Joint Brokerage', shortSellingAllowed: true },
  { value: 'margin', label: 'Margin Account', shortSellingAllowed: true },
  { value: '401k', label: '401(k)', shortSellingAllowed: false },
  { value: 'roth_401k', label: 'Roth 401(k)', shortSellingAllowed: false },
  { value: 'traditional_ira', label: 'Traditional IRA', shortSellingAllowed: false },
  { value: 'roth_ira', label: 'Roth IRA', shortSellingAllowed: false },
  { value: 'sep_ira', label: 'SEP IRA', shortSellingAllowed: false },
  { value: 'simple_ira', label: 'SIMPLE IRA', shortSellingAllowed: false },
  { value: '529', label: '529 Plan', shortSellingAllowed: false },
  { value: 'trust', label: 'Trust', shortSellingAllowed: true },
  { value: 'custodial', label: 'Custodial (UGMA/UTMA)', shortSellingAllowed: false },
  { value: 'other', label: 'Other', shortSellingAllowed: true },
] as const

export type AccountTypeValue = (typeof ACCOUNT_TYPES)[number]['value']

/** Check if short selling is allowed for a given account type */
export function isShortSellingAllowed(accountType: string | null): boolean {
  if (!accountType) return true // No type set = no restriction
  const found = ACCOUNT_TYPES.find((t) => t.value === accountType)
  return found?.shortSellingAllowed ?? true
}

export interface ClientTrade {
  id: number | null
  client_id: number
  import_batch_id: number | null
  symbol: string
  exchange: string
  trade_date: string
  trade_type: string
  quantity: number
  price: number
  fees: number
  order_id: string | null
  notes: string | null
  created_at: string | null
}

export interface ImportBatch {
  id: number | null
  client_id: number
  filename: string
  row_count: number
  account_type: string | null
  imported_at: string | null
}

export interface ClientPosition {
  symbol: string
  exchange: string
  net_quantity: number
  avg_price: number
  total_fees: number
  trade_count: number
  realized_pnl: number
  account_type?: string
}

export interface ClientAccount {
  account_type: string
  trade_count: number
  batch_count: number
}

// ---------------------------------------------------------------------------
// Schwab dual-document import (Transactions + optional Order Status, 401k rules)
// ---------------------------------------------------------------------------

export interface ClientHolding {
  id: number | null
  client_id: number
  symbol: string
  description: string | null
  quantity: number
  avg_cost: number
  total_cost: number
  realized_pnl: number
  last_activity_date: string | null
  updated_at: string | null
}

export interface ClientOpenOrder {
  id: number | null
  client_id: number
  order_number: string | null
  symbol: string
  description: string | null
  action: string
  quantity: number
  order_type: string | null
  limit_price: number | null
  stop_price: number | null
  time_in_force: string | null
  status: string
  placed_at: string | null
  last_activity_at: string | null
  updated_at: string | null
}

export interface ComplianceViolation {
  id: number | null
  client_id: number
  rule_set: string
  violation_type: string
  severity: string
  symbol: string | null
  quantity: number | null
  message: string
  detected_at: string | null
  resolved: boolean
  resolved_reason?: string | null
  resolved_at?: string | null
}

export interface ReconciliationMismatch {
  order_number: string | null
  symbol: string
  action: string
  order_quantity: number
  order_fill_price: number | null
  transaction_quantity: number | null
  transaction_price: number | null
  mismatch_kind: string
  note: string
}

export interface ImportReportSummary {
  transactions_processed: number
  order_status_processed: number
  total_holdings: number
  total_cost_basis: number
  open_buy_orders: number
  open_sell_orders: number
  violation_count: number
  is_compliant: boolean
}

export interface StrandedPosition {
  symbol: string
  description: string
  quantity: number
  cost_basis?: number | null
  asset_type: string
  reason: string
}

export interface ImportReport {
  client_id: number
  summary: ImportReportSummary
  holdings: ClientHolding[]
  open_orders: ClientOpenOrder[]
  violations: ComplianceViolation[]
  reconciliation_mismatches: ReconciliationMismatch[]
  /** Symbols whose net-from-transactions came out negative on a no-shorts
   *  account but where no Positions baseline was supplied. Surfaced for
   *  transparency, never as compliance violations. */
  incomplete_history_symbols?: string[]
  /** Untradable rows from the Positions snapshot (revoked, restricted, escrow). */
  stranded_positions?: StrandedPosition[]
}

// Re-export Goldman brief types so callers don't need to dig into /components.
export type { GoldmanBrief } from '@/components/reports/goldman/types'
