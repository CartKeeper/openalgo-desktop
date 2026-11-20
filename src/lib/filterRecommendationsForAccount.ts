import { isShortSellingAllowed } from '@/types/clients'
import type { OrderRecommendation } from '@/types/actionQueue'

export interface FilterContext {
  accountType: string | null
  /** Symbol → net long quantity currently held (uppercase keys) */
  longQtyBySymbol: Record<string, number>
}

export interface FilterResult {
  allowed: OrderRecommendation[]
  dropped: Array<{ item: OrderRecommendation; reason: string }>
}

/**
 * Enforce account-type restrictions on AI-generated recommendations.
 *
 * On accounts where short selling is not permitted (401k, IRAs, 529, custodial),
 * any SELL that would take a symbol past its held long quantity (or has no long
 * at all) would create or increase a short position — drop it. Sells count
 * cumulatively: two SELLs of the full position can't both pass.
 *
 * BUYs and partial closes of existing longs are always allowed.
 */
export function filterRecommendationsForAccount(
  items: OrderRecommendation[],
  ctx: FilterContext,
): FilterResult {
  if (isShortSellingAllowed(ctx.accountType)) {
    return { allowed: items, dropped: [] }
  }

  const allowed: OrderRecommendation[] = []
  const dropped: FilterResult['dropped'] = []
  /** Symbol → quantity already sold by the SELLs allowed so far */
  const soldBySymbol: Record<string, number> = {}

  for (const item of items) {
    if (item.side !== 'SELL') {
      allowed.push(item)
      continue
    }
    const key = item.symbol.toUpperCase()
    const held = ctx.longQtyBySymbol[key] ?? 0
    const alreadySold = soldBySymbol[key] ?? 0
    if (held <= 0) {
      dropped.push({
        item,
        reason: `account type does not permit short selling and no long position in ${item.symbol}`,
      })
      continue
    }
    if (alreadySold + item.quantity > held) {
      dropped.push({
        item,
        reason:
          alreadySold > 0
            ? `SELL ${item.quantity} ${item.symbol} plus ${alreadySold} already sold exceeds held long qty ${held} (would create short)`
            : `SELL ${item.quantity} ${item.symbol} exceeds held long qty ${held} (would create short)`,
      })
      continue
    }
    soldBySymbol[key] = alreadySold + item.quantity
    allowed.push(item)
  }

  return { allowed, dropped }
}
