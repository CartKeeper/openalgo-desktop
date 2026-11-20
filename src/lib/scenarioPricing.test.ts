import { describe, expect, it } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'
import { needsLivePrice, priceScenarioItems } from './scenarioPricing'

function item(overrides: Partial<OrderRecommendation> = {}): OrderRecommendation {
  return {
    id: 'a',
    symbol: 'VOO',
    exchange: 'GENERIC',
    side: 'BUY',
    quantity: 9,
    orderType: 'MARKET',
    product: 'CNC',
    price: 0,
    rationale: 'test',
    source: 'copilot',
    ...overrides,
  }
}

describe('priceScenarioItems', () => {
  it('gives a $0 market buy its live quote', () => {
    const { priced, blocked } = priceScenarioItems([item()], { VOO: 696.94 })
    expect(blocked).toEqual([])
    expect(priced).toEqual([item({ price: 696.94 })])
  })

  it('keeps a trade that already has a price', () => {
    const limit = item({ orderType: 'LIMIT', price: 690 })
    expect(priceScenarioItems([limit], { VOO: 696.94 }).priced).toEqual([limit])
  })

  it('blocks a $0 trade with no quote or a $0 quote', () => {
    const missing = item({ id: 'm', symbol: 'BNGI', side: 'SELL' })
    const zero = item({ id: 'z', symbol: 'SMME', side: 'SELL' })
    const { priced, blocked } = priceScenarioItems([missing, zero, item()], {
      SMME: 0,
      VOO: 696.94,
    })
    expect(blocked.map((i) => i.symbol)).toEqual(['BNGI', 'SMME'])
    expect(priced.map((i) => i.symbol)).toEqual(['VOO'])
  })

  it('matches quotes regardless of symbol case', () => {
    expect(priceScenarioItems([item({ symbol: 'voo' })], { VOO: 700 }).priced[0].price).toBe(700)
  })
})

describe('needsLivePrice', () => {
  it('is true only for priced-at-zero trades that are not cancels', () => {
    expect(needsLivePrice(item())).toBe(true)
    expect(needsLivePrice(item({ price: 10 }))).toBe(false)
    expect(needsLivePrice(item({ role: 'cancel' }))).toBe(false)
  })
})
