import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  toast: { error: vi.fn(), warning: vi.fn() },
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('sonner', () => ({ toast: mocks.toast }))

import { allowedScenarioTrades, loadScenarioReviewContext, longQtyBySymbol } from './scenarioReview'

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
    source: 'report',
    ...overrides,
  }
}

const scenario = {
  id: 5,
  client_id: 3,
  name: 'Current Portfolio — 401(k)',
  description: null,
  is_baseline: true,
  cloned_from_id: null,
  account_type: null,
  created_at: null,
  updated_at: null,
}

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.toast.error.mockReset()
  mocks.toast.warning.mockReset()
})

describe('longQtyBySymbol', () => {
  it('sums long quantities by upper-cased symbol and ignores shorts', () => {
    expect(
      longQtyBySymbol([
        { symbol: 'aapl', side: 'long', quantity: 2 },
        { symbol: 'AAPL', side: 'long', quantity: 3 },
        { symbol: 'TSLA', side: 'short', quantity: 4 },
      ])
    ).toEqual({ AAPL: 5 })
  })
})

describe('loadScenarioReviewContext', () => {
  it("falls back to the client's account type when the scenario has none", async () => {
    mocks.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_client_scenario') return scenario
      if (cmd === 'get_scenario_positions') return [{ symbol: 'TQQQ', side: 'long', quantity: 3 }]
      if (cmd === 'get_client') return { account_type: '401k' }
      throw new Error(cmd)
    })
    const ctx = await loadScenarioReviewContext(5)
    expect(ctx.scenario.is_baseline).toBe(true)
    expect(ctx.accountType).toBe('401k')
    expect(ctx.longQtyBySymbol).toEqual({ TQQQ: 3 })
    expect(mocks.invoke).toHaveBeenCalledWith('get_client', { id: 3 })
  })

  it("uses the scenario's own account type without asking for the client", async () => {
    mocks.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_client_scenario') return { ...scenario, account_type: 'roth_ira' }
      if (cmd === 'get_scenario_positions') return []
      throw new Error(cmd)
    })
    expect((await loadScenarioReviewContext(5)).accountType).toBe('roth_ira')
  })
})

describe('allowedScenarioTrades', () => {
  it('drops a short in a retirement account and keeps the rest', () => {
    const allowed = allowedScenarioTrades(
      [item(), item({ id: 'b', symbol: 'TSLA', side: 'SELL', quantity: 5 })],
      { accountType: '401k', longQtyBySymbol: {} }
    )
    expect(allowed.map((i) => i.symbol)).toEqual(['VOO'])
    expect(mocks.toast.warning).toHaveBeenCalledOnce()
  })

  it('says so when there are no trades', () => {
    expect(allowedScenarioTrades([], { accountType: null, longQtyBySymbol: {} })).toEqual([])
    expect(mocks.toast.error).toHaveBeenCalledWith('No recommended trades found in this message')
  })
})
