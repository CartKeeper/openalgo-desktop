import type { RebalancePlan } from '@/api/analytics'
import type { OrderRecommendation } from '@/types/actionQueue'

/**
 * A rebalance plan as actions for the review modal: market orders, CNC, whole
 * shares, the plan line's reason as the rationale. `price` is 0 so the modal
 * prices each line live before applying it (the plan's price was only for
 * sizing). Lines under one share are dropped.
 */
export function rebalancePlanToActions(plan: RebalancePlan): OrderRecommendation[] {
  return plan.lines
    .map((line) => ({ line, quantity: Math.floor(line.quantity) }))
    .filter(({ quantity }) => quantity >= 1)
    .map(({ line, quantity }): OrderRecommendation => ({
      id: crypto.randomUUID(),
      symbol: line.symbol.toUpperCase(),
      exchange: line.exchange || 'US',
      side: line.side === 'sell' ? 'SELL' : 'BUY',
      quantity,
      orderType: 'MARKET',
      product: 'CNC',
      price: 0,
      validity: 'DAY',
      rationale: line.reason,
      source: 'report',
    }))
}
