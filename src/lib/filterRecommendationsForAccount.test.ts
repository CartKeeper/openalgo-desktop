import { describe, expect, it } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'
import { filterRecommendationsForAccount } from './filterRecommendationsForAccount'

function sell(symbol: string, quantity: number): OrderRecommendation {
  return {
    id: `${symbol}-${quantity}`,
    symbol,
    exchange: 'GENERIC',
    side: 'SELL',
    quantity,
    orderType: 'MARKET',
    product: 'CNC',
    price: 0,
    rationale: 'test',
    source: 'report',
  }
}

describe('filterRecommendationsForAccount', () => {
  const ctx401k = { accountType: '401k', longQtyBySymbol: { AABB: 723, VOO: 5 } }

  it('lets one full-position sell through and drops the repeat', () => {
    const { allowed, dropped } = filterRecommendationsForAccount(
      [sell('AABB', 723), sell('AABB', 723)],
      ctx401k
    )
    expect(allowed).toHaveLength(1)
    expect(dropped).toHaveLength(1)
    expect(dropped[0].reason).toContain('already sold')
  })

  it('allows partial sells that together stay within the shares held', () => {
    const { allowed, dropped } = filterRecommendationsForAccount(
      [sell('VOO', 2), sell('voo', 3), sell('VOO', 1)],
      ctx401k
    )
    expect(allowed.map((i) => i.quantity)).toEqual([2, 3])
    expect(dropped.map((d) => d.item.quantity)).toEqual([1])
  })

  it('drops a sell of a symbol the account does not hold', () => {
    const { allowed } = filterRecommendationsForAccount([sell('TSLA', 1)], ctx401k)
    expect(allowed).toHaveLength(0)
  })

  it('leaves accounts that allow shorts alone', () => {
    const items = [sell('AABB', 723), sell('AABB', 723)]
    const { allowed } = filterRecommendationsForAccount(items, {
      accountType: 'margin',
      longQtyBySymbol: { AABB: 723 },
    })
    expect(allowed).toEqual(items)
  })
})
