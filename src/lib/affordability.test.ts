import { afterEach, describe, expect, it, vi } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'

// --- Mock the broker IPC the reconciliation reads from ----------------------
const getFunds = vi.fn()
const getPositions = vi.fn()
const getHoldings = vi.fn()
const getQuote = vi.fn()

vi.mock('@/api/tauri-client', () => ({
  fundsCommands: { getFunds: () => getFunds() },
  positionCommands: { getPositions: () => getPositions() },
  holdingsCommands: { getHoldings: () => getHoldings() },
  quoteCommands: { getQuote: (...a: unknown[]) => getQuote(...a) },
}))

import { reconcileRecommendationsWithAccount } from './affordability'

function rec(overrides: Partial<OrderRecommendation> = {}): OrderRecommendation {
  return {
    id: Math.random().toString(36).slice(2),
    symbol: 'AAPL',
    exchange: 'NASDAQ',
    side: 'BUY',
    quantity: 1,
    orderType: 'LIMIT',
    product: 'CNC',
    price: 100,
    rationale: 'test',
    source: 'copilot',
    ...overrides,
  }
}

const cash = (n: number) => getFunds.mockResolvedValue({ available_cash: n })
const pos = (list: Array<{ symbol: string; quantity: number }>) =>
  getPositions.mockResolvedValue(
    list.map((p) => ({ symbol: p.symbol, exchange: 'NASDAQ', product: 'CNC', quantity: p.quantity, average_price: 0, ltp: 0, pnl: 0, realized_pnl: 0, unrealized_pnl: 0, buy_quantity: 0, buy_value: 0, sell_quantity: 0, sell_value: 0 }))
  )

afterEach(() => {
  vi.clearAllMocks()
  getPositions.mockResolvedValue([])
  getHoldings.mockResolvedValue([])
})

describe('reconcileRecommendationsWithAccount — buys vs cash', () => {
  it('drops a buy that costs more than all available cash (the $360 GOOGL on $67 case)', async () => {
    cash(67.43)
    const { items, notice } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'GOOGL', price: 360 }),
    ])
    expect(items).toHaveLength(0)
    expect(notice).toMatch(/removed GOOGL/i)
  })

  it('reduces a buy quantity to what the shared budget allows', async () => {
    cash(250)
    const { items } = await reconcileRecommendationsWithAccount([rec({ price: 100, quantity: 10 })])
    expect(items).toHaveLength(1)
    expect(items[0].quantity).toBe(2) // floor(250/100)
  })

  it('spends cash as one shared budget across multiple buys in order', async () => {
    cash(300)
    const { items } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'AAPL', price: 100, quantity: 2 }), // takes 200, leaves 100
      rec({ symbol: 'MSFT', price: 100, quantity: 5 }), // only 1 fits
    ])
    expect(items.find((i) => i.symbol === 'AAPL')?.quantity).toBe(2)
    expect(items.find((i) => i.symbol === 'MSFT')?.quantity).toBe(1)
  })
})

describe('reconcileRecommendationsWithAccount — sells vs holdings', () => {
  it('drops a sell for a symbol the account does not hold', async () => {
    pos([])
    const { items, notice } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'TSLA', side: 'SELL', orderType: 'MARKET', price: 0, quantity: 5 }),
    ])
    expect(items).toHaveLength(0)
    expect(notice).toMatch(/don't hold/i)
  })

  it('reduces a sell that exceeds the held quantity', async () => {
    pos([{ symbol: 'AAPL', quantity: 3 }])
    const { items, notice } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'AAPL', side: 'SELL', orderType: 'MARKET', price: 0, quantity: 10 }),
    ])
    expect(items).toHaveLength(1)
    expect(items[0].quantity).toBe(3)
    expect(notice).toMatch(/shares you actually hold/i)
  })

  it('clamps a protective stop (SL-M) to the held quantity', async () => {
    pos([{ symbol: 'AAPL', quantity: 4 }])
    const { items } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'AAPL', side: 'SELL', orderType: 'SL-M', price: 0, triggerPrice: 150, quantity: 99 }),
    ])
    expect(items[0].quantity).toBe(4)
  })

  it('passes a valid sell within the held quantity unchanged', async () => {
    pos([{ symbol: 'AAPL', quantity: 10 }])
    const { items, notice } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'AAPL', side: 'SELL', orderType: 'MARKET', price: 0, quantity: 5 }),
    ])
    expect(items).toHaveLength(1)
    expect(items[0].quantity).toBe(5)
    expect(notice).toBeNull()
  })
})
