import { describe, expect, it, vi } from 'vitest'
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { error: vi.fn(), success: vi.fn() } }))

import { computeSummary, enrichPositions } from '@/stores/clientScenarioStore'
import type { ClientScenario, GenericQuote, ScenarioPosition } from '@/types/client-scenarios'
import { baselineFor, metricsFrom } from './useScenarioCardMetrics'

function scenario(over: Partial<ClientScenario>): ClientScenario {
  return {
    id: 1,
    client_id: 11,
    name: 'S',
    description: null,
    is_baseline: false,
    cloned_from_id: null,
    account_type: '401k',
    created_at: null,
    updated_at: null,
    cash: null,
    ...over,
  } as ClientScenario
}

function pos(symbol: string, quantity: number, avg_price: number): ScenarioPosition {
  return {
    id: null,
    scenario_id: 1,
    symbol,
    exchange: 'GENERIC',
    quantity,
    avg_price,
    side: 'long',
    notes: null,
    created_at: null,
    updated_at: null,
  } as ScenarioPosition
}

describe('baselineFor', () => {
  const base401k = scenario({ id: 32, is_baseline: true, account_type: '401k' })
  const baseIra = scenario({ id: 40, is_baseline: true, account_type: 'roth_ira' })
  const clone = scenario({ id: 35, cloned_from_id: 32, account_type: '401k' })
  const cloneOfClone = scenario({ id: 36, cloned_from_id: 35, account_type: '401k' })
  const all = [base401k, baseIra, clone, cloneOfClone]

  it("measures a clone against its account type's baseline", () => {
    expect(baselineFor(clone, all)?.id).toBe(32)
    expect(baselineFor(cloneOfClone, all)?.id).toBe(32)
  })

  it('gives a baseline no baseline of its own', () => {
    expect(baselineFor(base401k, all)).toBeNull()
  })

  it('finds nothing when the account type has no baseline', () => {
    expect(baselineFor(scenario({ id: 50, account_type: 'margin' }), all)).toBeNull()
  })
})

describe('metricsFrom', () => {
  it("matches the Scenario page header's numbers", () => {
    const positions = [pos('VOO', 5, 679.7), pos('AABB', 723, 0.01), pos('NOQUOTE', 10, 2)]
    const quotes = new Map<string, GenericQuote>([
      ['VOO', { symbol: 'VOO', price: 700 } as GenericQuote],
      ['AABB', { symbol: 'AABB', price: 0.02 } as GenericQuote],
    ])
    const m = metricsFrom(scenario({ cash: 1000 }), positions, quotes)
    const header = computeSummary(enrichPositions(positions, quotes))
    expect(m.marketValue).toBe(header.total_value)
    expect(m.pnl).toBe(header.total_pnl)
    expect(m.returnPct).toBe(header.pnl_pct)
    expect(m.cash).toBe(1000)
    expect([m.positionCount, m.pricedCount]).toEqual([3, 2])
  })
})
