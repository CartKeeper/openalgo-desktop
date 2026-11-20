export type StrategyRole = 'cancel' | 'fund' | 'buy' | 'protect' | 'standalone'

export interface OrderRecommendation {
  /** Unique client-side ID for list keying and edit tracking */
  id: string
  symbol: string
  exchange: string
  side: 'BUY' | 'SELL'
  quantity: number
  orderType: 'MARKET' | 'LIMIT' | 'SL' | 'SL-M' | 'TRAILING_STOP'
  product: 'CNC' | 'MIS' | 'NRML'
  price: number
  triggerPrice?: number
  trailPrice?: number
  trailPercent?: number
  /** Order time-in-force. Trailing stops are always GTC; everything else defaults to DAY. */
  validity?: 'DAY' | 'GTC'
  /** Strategy group id; legs sharing a group are placed and sequenced together */
  group?: string
  /** This leg's job inside its strategy group */
  role?: StrategyRole
  /** For role 'cancel': the broker order id to cancel */
  cancelOrderId?: string
  rationale: string
  source: 'briefing' | 'copilot' | 'report' | 'research'
}

export interface BasketOrderResult {
  symbol: string
  exchange: string
  orderId: string | null
  success: boolean
  message?: string
}

export interface StrategyLegResult {
  group: string | null
  role: StrategyRole
  symbol: string
  success: boolean
  order_id: string | null
  filled_quantity: number
  message: string
}
