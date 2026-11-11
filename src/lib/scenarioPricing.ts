import type { OrderRecommendation } from '@/types/actionQueue'

/** A scenario trade with no price of its own (market orders arrive at $0). */
export function needsLivePrice(item: OrderRecommendation): boolean {
  return item.role !== 'cancel' && !(item.price > 0)
}

/**
 * Give every $0 scenario trade its live quote. A trade with no usable quote is
 * blocked: a sandbox trade recorded at $0 would corrupt the position's cost.
 * `quotes` is keyed by upper-cased symbol.
 */
export function priceScenarioItems(
  items: OrderRecommendation[],
  quotes: Record<string, number>
): { priced: OrderRecommendation[]; blocked: OrderRecommendation[] } {
  const priced: OrderRecommendation[] = []
  const blocked: OrderRecommendation[] = []
  for (const item of items) {
    if (!needsLivePrice(item)) {
      priced.push(item)
      continue
    }
    const quote = quotes[item.symbol.toUpperCase()]
    if (quote !== undefined && quote > 0) {
      priced.push({ ...item, price: quote })
    } else {
      blocked.push(item)
    }
  }
  return { priced, blocked }
}
