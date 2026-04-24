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
