import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'

// --- Mock the broker IPC the reconciliation reads from ----------------------
const getFunds = vi.fn()
const getPositions = vi.fn()
const getHoldings = vi.fn()
const getQuote = vi.fn()
const getOpenOrders = vi.fn()

vi.mock('@/api/tauri-client', () => ({
  fundsCommands: { getFunds: () => getFunds() },
  positionCommands: { getPositions: () => getPositions() },
  holdingsCommands: { getHoldings: () => getHoldings() },
  quoteCommands: { getQuote: (...a: unknown[]) => getQuote(...a) },
  orderCommands: { getOpenOrders: () => getOpenOrders() },
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

beforeEach(() => {
  vi.clearAllMocks()
  getPositions.mockResolvedValue([])
  getHoldings.mockResolvedValue([])
  getQuote.mockResolvedValue([])
  getOpenOrders.mockResolvedValue([])
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

const quotes = (map: Record<string, number>) =>
  getQuote.mockResolvedValue(Object.entries(map).map(([symbol, ltp]) => ({ symbol, ltp })))

const orderBook = (list: Array<{ order_id: string; status: string }>) =>
  getOpenOrders.mockResolvedValue(list)

// A rotation: market-sell PFE to fund a BAC buy, then protect BAC.
const fundSell = (overrides: Partial<OrderRecommendation> = {}) =>
  rec({ symbol: 'PFE', side: 'SELL', orderType: 'MARKET', price: 0, quantity: 11, group: 'r1', role: 'fund', ...overrides })
const groupBuy = (overrides: Partial<OrderRecommendation> = {}) =>
  rec({ symbol: 'BAC', side: 'BUY', orderType: 'MARKET', price: 0, quantity: 13, group: 'r1', role: 'buy', ...overrides })
const protect = (overrides: Partial<OrderRecommendation> = {}) =>
  rec({
    symbol: 'BAC',
    side: 'SELL',
    orderType: 'TRAILING_STOP',
    price: 0,
    quantity: 13,
    trailPercent: 5,
    validity: 'GTC',
    group: 'r1',
    role: 'protect',
    ...overrides,
  })

describe('reconcileRecommendationsWithAccount — sell proceeds, protect and cancel legs', () => {
  it('funds a buy from a market sell earlier in the batch', async () => {
    cash(351)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 30.38, BAC: 50.27 })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy()])
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(13)
    expect(items.find((i) => i.symbol === 'PFE')?.quantity).toBe(11)
    expect(notice).toContain('BAC ×13 is funded by selling PFE (about $334) plus $351 cash')
    expect(notice).not.toMatch(/reduced BAC/)
  })

  it('shrinks the dependent buy when the sell proceeds shrink', async () => {
    cash(351)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 25.0, BAC: 50.27 })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy()])
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(12) // floor((351+275)/50.27)
    expect(notice).toContain('reduced BAC 13→12 to fit your available cash plus sale proceeds ($626.00)')
    expect(notice).not.toContain('to fit your available cash (')
    expect(notice).toContain('BAC ×12 is funded by selling PFE (about $275) plus $351 cash')
  })

  it('does not let a sell that comes after a buy fund that buy', async () => {
    cash(351)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 30.38, BAC: 50.27 })
    const { items } = await reconcileRecommendationsWithAccount([groupBuy(), fundSell()])
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(6) // floor(351/50.27)
  })

  it('sums the proceeds of two fund sells in the same group', async () => {
    cash(0)
    pos([
      { symbol: 'PFE', quantity: 10 },
      { symbol: 'KO', quantity: 10 },
    ])
    quotes({ PFE: 30, KO: 60, BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([
      fundSell({ quantity: 10 }),
      fundSell({ symbol: 'KO', quantity: 10 }),
      groupBuy({ quantity: 18 }),
    ])
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(18) // (300+600)/50
    expect(notice).toContain('BAC ×18 is funded by selling PFE, KO (about $900) plus $0 cash')
  })

  it('uses the limit price (not a quote) for a LIMIT liquidating sell', async () => {
    cash(0)
    pos([{ symbol: 'PFE', quantity: 10 }])
    quotes({ BAC: 50 })
    const { items } = await reconcileRecommendationsWithAccount([
      fundSell({ orderType: 'LIMIT', price: 40, quantity: 10 }),
      groupBuy({ quantity: 8 }),
    ])
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(8) // 400/50
  })

  it('credits proceeds on the post-cap quantity, not the requested one', async () => {
    cash(0)
    pos([{ symbol: 'PFE', quantity: 5 }])
    quotes({ PFE: 30, BAC: 50 })
    const { items } = await reconcileRecommendationsWithAccount([
      fundSell({ quantity: 11 }),
      groupBuy({ quantity: 13 }),
    ])
    expect(items.find((i) => i.symbol === 'PFE')?.quantity).toBe(5)
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(3) // floor(150/50)
  })

  it('gives no proceeds for a sell that cannot be priced and shrinks the buy', async () => {
    cash(100)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy()])
    expect(items.find((i) => i.symbol === 'PFE')?.quantity).toBe(11)
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(2)
    expect(notice).toMatch(/reduced BAC 13→2 to fit your available cash/)
    expect(notice).not.toMatch(/funded by/)
  })

  it('survives getQuote throwing synchronously', async () => {
    cash(351)
    pos([{ symbol: 'PFE', quantity: 11 }])
    getQuote.mockImplementation(() => {
      throw new Error('boom')
    })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy()])
    expect(items.map((i) => i.symbol)).toEqual(['PFE', 'BAC'])
    expect(notice).toMatch(/couldn't confirm cash\/price for BAC/)
  })

  it('caps a protect stop to the bought quantity in its group', async () => {
    cash(351)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 25.0, BAC: 50.27 })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy(), protect()])
    expect(items.find((i) => i.symbol === 'BAC' && i.side === 'BUY')?.quantity).toBe(12)
    const stop = items.find((i) => i.orderType === 'TRAILING_STOP')
    expect(stop?.quantity).toBe(12)
    expect(notice).toContain('reduced BAC 13→12 to match its buy')
    expect(notice).not.toMatch(/shares you actually hold/)
  })

  it('leaves a protect stop alone when it already matches its buy', async () => {
    cash(351)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 30.38, BAC: 50.27 })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy(), protect()])
    expect(items.find((i) => i.orderType === 'TRAILING_STOP')?.quantity).toBe(13)
    expect(notice).not.toMatch(/match its buy/)
  })

  it('falls back to the held rule when the protect stop\'s buy was dropped', async () => {
    cash(10)
    quotes({ BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([groupBuy(), protect()])
    expect(items).toHaveLength(0)
    expect(notice).toMatch(/removed BAC — not enough cash for a single share/)
    expect(notice).toMatch(/removed sell of BAC — you don't hold any shares/)
  })

  it('drops a trailing stop with no trail distance', async () => {
    pos([{ symbol: 'BAC', quantity: 5 }])
    const { items, notice } = await reconcileRecommendationsWithAccount([
      protect({ trailPercent: undefined, group: undefined, role: 'standalone', quantity: 5 }),
    ])
    expect(items).toHaveLength(0)
    expect(notice).toContain('removed trailing stop for BAC — no trail distance given')
  })

  it('keeps a trailing stop that has a trail distance', async () => {
    pos([{ symbol: 'BAC', quantity: 5 }])
    const { items, notice } = await reconcileRecommendationsWithAccount([
      protect({ group: undefined, role: 'standalone', quantity: 5 }),
    ])
    expect(items).toHaveLength(1)
    expect(notice).toBeNull()
  })

  it('drops a cancel whose order is not open, keeps one that is', async () => {
    const cancel = (id: string, symbol: string) =>
      rec({ symbol, side: 'SELL', orderType: 'MARKET', price: 0, quantity: 0, role: 'cancel', group: 'r1', cancelOrderId: id })

    orderBook([{ order_id: 'ord-1', status: 'FILLED' }])
    const filled = await reconcileRecommendationsWithAccount([cancel('ord-1', 'PFE')])
    expect(filled.items).toHaveLength(0)
    expect(filled.notice).toContain('removed cancel for PFE — that order is no longer open')

    orderBook([{ order_id: 'ord-1', status: 'OPEN' }])
    const open = await reconcileRecommendationsWithAccount([cancel('ord-1', 'PFE')])
    expect(open.items).toHaveLength(1)
    expect(open.items[0].quantity).toBe(0)
    expect(open.notice).toBeNull()

    orderBook([{ order_id: 'ord-1', status: 'PARTIALLY_FILLED' }])
    const partial = await reconcileRecommendationsWithAccount([cancel('ord-1', 'PFE')])
    expect(partial.items).toHaveLength(1)

    orderBook([{ order_id: 'other', status: 'OPEN' }])
    const unmatched = await reconcileRecommendationsWithAccount([cancel('ord-1', 'PFE')])
    expect(unmatched.items).toHaveLength(0)
  })

  it('keeps a cancel on an unheld symbol and never counts it as a liquidating sell', async () => {
    cash(100)
    pos([])
    quotes({ BAC: 50 })
    orderBook([{ order_id: 'ord-1', status: 'OPEN' }])
    const { items, notice } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'PFE', side: 'SELL', orderType: 'MARKET', price: 0, quantity: 0, role: 'cancel', group: 'r1', cancelOrderId: 'ord-1' }),
      groupBuy({ quantity: 5 }),
    ])
    expect(items.map((i) => i.symbol)).toEqual(['PFE', 'BAC']) // original order kept
    expect(items[1].quantity).toBe(2) // only the $100 cash funds it
    expect(notice).not.toMatch(/don't hold/)
  })

  it('keeps cancel items with a verify notice when the order book cannot be read', async () => {
    getOpenOrders.mockRejectedValue(new Error('offline'))
    const { items, notice } = await reconcileRecommendationsWithAccount([
      rec({ symbol: 'PFE', side: 'SELL', orderType: 'MARKET', price: 0, quantity: 0, role: 'cancel', group: 'r1', cancelOrderId: 'ord-1' }),
    ])
    expect(items).toHaveLength(1)
    expect(notice).toContain("couldn't confirm open orders; verify the Order Book before cancelling")
  })

  it('does not read the order book when there are no cancel items', async () => {
    pos([{ symbol: 'AAPL', quantity: 10 }])
    await reconcileRecommendationsWithAccount([
      rec({ symbol: 'AAPL', side: 'SELL', orderType: 'LIMIT', price: 100, quantity: 5 }),
    ])
    expect(getOpenOrders).not.toHaveBeenCalled()
  })

  it('does not double-count proceeds when two sells hit the same symbol', async () => {
    cash(0)
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 30, BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([
      fundSell(),
      fundSell(),
      groupBuy({ quantity: 13 }),
    ])
    expect(items.filter((i) => i.symbol === 'PFE')).toHaveLength(1)
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(6) // floor(330/50), 11 x 30 once
    expect(notice).toContain("removed sell of PFE — you don't hold any shares")
    expect(notice).toContain('reduced BAC 13→6 to fit your available cash plus sale proceeds ($330.00)')
  })

  it('caps a stop to the shares left after an earlier sell of the same symbol', async () => {
    pos([{ symbol: 'PFE', quantity: 11 }])
    quotes({ PFE: 30 })
    const { items, notice } = await reconcileRecommendationsWithAccount([
      fundSell({ quantity: 5, group: undefined, role: 'standalone' }),
      protect({ symbol: 'PFE', quantity: 11, group: undefined, role: 'standalone' }),
    ])
    expect(items.find((i) => i.orderType === 'TRAILING_STOP')?.quantity).toBe(6)
    expect(notice).toContain('reduced PFE 11→6 to the shares you actually hold')
  })

  it('does not lose a share to float noise at an exact-cents boundary', async () => {
    cash(0)
    pos([{ symbol: 'PFE', quantity: 3 }])
    const { items } = await reconcileRecommendationsWithAccount([
      fundSell({ orderType: 'LIMIT', price: 10.1, quantity: 3 }),
      groupBuy({ orderType: 'LIMIT', price: 30.3, quantity: 1 }),
    ])
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(1) // 3 x 10.10 = 30.299999999999997
  })

  it('does not cap a protect stop by a buy of a different symbol in its group', async () => {
    cash(1000)
    pos([{ symbol: 'KO', quantity: 20 }])
    quotes({ BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([
      groupBuy({ quantity: 5 }),
      protect({ symbol: 'KO', quantity: 20 }),
    ])
    expect(items.find((i) => i.symbol === 'KO')?.quantity).toBe(20) // held rule, not the BAC buy's 5
    expect(notice).toBeNull()
  })

  it('does not cap a protect stop by a buy in another group', async () => {
    cash(1000)
    pos([{ symbol: 'BAC', quantity: 20 }])
    quotes({ BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([
      groupBuy({ group: 'r1', quantity: 5 }),
      protect({ group: 'r2', quantity: 20 }),
    ])
    expect(items.find((i) => i.orderType === 'TRAILING_STOP')?.quantity).toBe(20) // held rule
    expect(notice ?? '').not.toMatch(/match its buy/)
  })

  it('keeps a sell but credits no proceeds when holdings cannot be read', async () => {
    cash(100)
    // readHeldLongQty swallows rejected reads into [], so only a synchronous throw reaches its null path.
    getPositions.mockImplementation(() => {
      throw new Error('offline')
    })
    quotes({ PFE: 30, BAC: 50 })
    const { items, notice } = await reconcileRecommendationsWithAccount([fundSell(), groupBuy({ quantity: 5 })])
    expect(items.find((i) => i.symbol === 'PFE')?.quantity).toBe(11)
    expect(items.find((i) => i.symbol === 'BAC')?.quantity).toBe(2) // floor(100/50), no proceeds
    expect(notice).toContain("couldn't confirm your holdings")
    expect(notice).not.toMatch(/funded by/)
  })
})
