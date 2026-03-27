export interface Client {
  id: number | null
  name: string
  email: string | null
  phone: string | null
  broker: string | null
  account_id: string | null
  notes: string | null
  created_at: string | null
  updated_at: string | null
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
}
