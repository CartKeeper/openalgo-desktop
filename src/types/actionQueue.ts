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
