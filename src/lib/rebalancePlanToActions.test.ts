import { describe, expect, it } from 'vitest'
import type { RebalancePlan } from '@/api/analytics'
import { rebalancePlanToActions } from './rebalancePlanToActions'

const plan: RebalancePlan = {
  lines: [
    {
      symbol: 'A',
      exchange: 'NYSE',
      side: 'sell',
      quantity: 12,
      price: 50,
      value: 600,
      before_pct: 45.4545,
      after_pct: 40,
      reason: 'Rebalance A from 45.5% to 40.0%',
    },
    {
      symbol: 'b',
      exchange: 'NASDAQ',
      side: 'buy',
      quantity: 2,
      price: 100,
      value: 200,
      before_pct: 10,
      after_pct: 12,
      reason: 'Rebalance B from 10.0% to 12.0%',
    },
  ],
  cash_before: 1000,
  cash_after: 1400,
  unresolved: [],
  note: 'Whole shares; prices as of the plan time; cash never goes below zero.',
}

describe('rebalancePlanToActions', () => {
  it('maps each line to a market CNC recommendation with the reason as rationale', () => {
    const actions = rebalancePlanToActions(plan)

    expect(actions).toHaveLength(2)
    expect(actions[0]).toMatchObject({
      symbol: 'A',
      exchange: 'NYSE',
      side: 'SELL',
      quantity: 12,
      orderType: 'MARKET',
      product: 'CNC',
      price: 0,
      validity: 'DAY',
      rationale: 'Rebalance A from 45.5% to 40.0%',
      source: 'report',
    })
    expect(actions[1]).toMatchObject({
      symbol: 'B',
      exchange: 'NASDAQ',
      side: 'BUY',
      quantity: 2,
      rationale: 'Rebalance B from 10.0% to 12.0%',
    })
  })

  it('gives every action its own id', () => {
    const actions = rebalancePlanToActions(plan)
    expect(typeof actions[0].id).toBe('string')
    expect(actions[0].id.length).toBeGreaterThan(0)
    expect(actions[0].id).not.toBe(actions[1].id)
  })

  it('returns no actions for an empty plan', () => {
    expect(rebalancePlanToActions({ ...plan, lines: [] })).toEqual([])
  })

  it('keeps whole shares only and defaults a missing exchange to US', () => {
    const odd: RebalancePlan = {
      ...plan,
      lines: [
        { ...plan.lines[0], quantity: 3.7, exchange: '' },
        { ...plan.lines[1], quantity: 0.4 },
      ],
    }
    const actions = rebalancePlanToActions(odd)
    expect(actions).toHaveLength(1)
    expect(actions[0].quantity).toBe(3)
    expect(actions[0].exchange).toBe('US')
  })
})
