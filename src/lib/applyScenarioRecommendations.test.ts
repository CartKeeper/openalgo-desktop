import { describe, expect, it, vi } from 'vitest'
import { applyScenarioRecommendations } from './applyScenarioRecommendations'
import type { OrderRecommendation } from '@/types/actionQueue'

function item(overrides: Partial<OrderRecommendation> = {}): OrderRecommendation {
  return {
    id: 'a',
    symbol: 'AAPL',
    exchange: 'NASDAQ',
    side: 'BUY',
    quantity: 10,
    orderType: 'MARKET',
    product: 'CNC',
    price: 180,
    rationale: 'test',
    source: 'copilot',
    ...overrides,
  }
}

describe('applyScenarioRecommendations', () => {
  it('on non-baseline: applies each trade to current scenario, does not clone, returns targetId = current', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    const cloneScenario = vi.fn()
    const result = await applyScenarioRecommendations({
      items: [item({ symbol: 'AAPL' }), item({ id: 'b', symbol: 'MSFT' })],
      clientId: 1,
      currentScenarioId: 42,
      isBaseline: false,
      cloneName: undefined,
      invoke,
      cloneScenario,
    })

    expect(cloneScenario).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke).toHaveBeenNthCalledWith(1, 'apply_scenario_trade', {
      scenarioId: 42,
      symbol: 'AAPL',
      exchange: 'NASDAQ',
      side: 'long',
      quantity: 10,
      price: 180,
    })
    expect(result).toEqual({ targetScenarioId: 42, cloned: false, applied: 2, failures: [] })
  })

  it('on baseline: clones once with the provided name, then applies trades to the new clone', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    const cloneScenario = vi.fn().mockResolvedValue({ id: 99 })

    const result = await applyScenarioRecommendations({
      items: [item()],
      clientId: 1,
      currentScenarioId: 7,
      isBaseline: true,
      cloneName: 'Aggressive Growth',
      invoke,
      cloneScenario,
    })

    expect(cloneScenario).toHaveBeenCalledTimes(1)
    expect(cloneScenario).toHaveBeenCalledWith(7, 1, 'Aggressive Growth', undefined)
    expect(invoke).toHaveBeenCalledWith('apply_scenario_trade', expect.objectContaining({ scenarioId: 99 }))
    expect(result).toEqual({ targetScenarioId: 99, cloned: true, applied: 1, failures: [] })
  })

  it('on baseline with empty cloneName: throws before any invoke/clone call', async () => {
    const invoke = vi.fn()
    const cloneScenario = vi.fn()
    await expect(
      applyScenarioRecommendations({
        items: [item()],
        clientId: 1,
        currentScenarioId: 7,
        isBaseline: true,
        cloneName: '  ',
        invoke,
        cloneScenario,
      })
    ).rejects.toThrow(/clone name/i)
    expect(invoke).not.toHaveBeenCalled()
    expect(cloneScenario).not.toHaveBeenCalled()
  })

  it('maps BUY -> long and SELL -> short for the backend side field', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    const cloneScenario = vi.fn()
    await applyScenarioRecommendations({
      items: [item({ side: 'SELL' })],
      clientId: 1,
      currentScenarioId: 42,
      isBaseline: false,
      cloneName: undefined,
      invoke,
      cloneScenario,
    })
    expect(invoke).toHaveBeenCalledWith('apply_scenario_trade', expect.objectContaining({ side: 'short' }))
  })

  it('collects per-trade failures without aborting the loop and reports them', async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error('insufficient qty'))
      .mockResolvedValueOnce(undefined)
    const cloneScenario = vi.fn()

    const result = await applyScenarioRecommendations({
      items: [
        item({ id: 'a', symbol: 'AAPL' }),
        item({ id: 'b', symbol: 'MSFT' }),
        item({ id: 'c', symbol: 'GOOG' }),
      ],
      clientId: 1,
      currentScenarioId: 42,
      isBaseline: false,
      cloneName: undefined,
      invoke,
      cloneScenario,
    })

    expect(result.applied).toBe(2)
    expect(result.failures).toEqual([{ symbol: 'MSFT', message: 'insufficient qty' }])
  })

  it('on baseline clone failure: does not call invoke for any trade, rethrows', async () => {
    const invoke = vi.fn()
    const cloneScenario = vi.fn().mockRejectedValue(new Error('db locked'))
    await expect(
      applyScenarioRecommendations({
        items: [item()],
        clientId: 1,
        currentScenarioId: 7,
        isBaseline: true,
        cloneName: 'X',
        invoke,
        cloneScenario,
      })
    ).rejects.toThrow(/db locked/)
    expect(invoke).not.toHaveBeenCalled()
  })
})
