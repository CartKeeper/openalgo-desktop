import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'

vi.mock('@/api/tauri-client', () => ({
  rideCommands: { recordProtectionDismissal: vi.fn() },
  orderCommands: { placeStrategyBasket: vi.fn() },
}))

vi.mock('@/lib/affordability', () => ({
  reconcileRecommendationsWithAccount: vi.fn(),
}))

import { rideCommands } from '@/api/tauri-client'
import { reconcileRecommendationsWithAccount } from '@/lib/affordability'
import { groupHeader, useActionQueueStore } from './actionQueueStore'

const dismissal = vi.mocked(rideCommands.recordProtectionDismissal)
const reconcile = vi.mocked(reconcileRecommendationsWithAccount)

function rec(over: Partial<OrderRecommendation> & { id: string }): OrderRecommendation {
  return {
    symbol: 'AAA',
    exchange: 'NASDAQ',
    side: 'BUY',
    quantity: 10,
    orderType: 'MARKET',
    product: 'CNC',
    price: 0,
    rationale: '',
    source: 'briefing',
    ...over,
  }
}

const rotation = (): OrderRecommendation[] => [
  rec({ id: 'c1', symbol: 'PFE', side: 'SELL', group: 'g1', role: 'cancel', cancelOrderId: 'o1' }),
  rec({ id: 'f1', symbol: 'PFE', side: 'SELL', group: 'g1', role: 'fund' }),
  rec({ id: 'b1', symbol: 'BAC', side: 'BUY', group: 'g1', role: 'buy' }),
  rec({ id: 'p1', symbol: 'BAC', side: 'SELL', orderType: 'TRAILING_STOP', group: 'g1', role: 'protect' }),
]

const ids = () => useActionQueueStore.getState().items.map((i) => i.id)

beforeEach(() => {
  vi.clearAllMocks()
  dismissal.mockResolvedValue(undefined)
  reconcile.mockImplementation(async (items) => ({
    items,
    availableCash: 0,
    notice: 'reconciled',
  }))
  useActionQueueStore.setState({ items: [], isReviewOpen: true, fundsNotice: null })
})

describe('actionQueueStore removeItem cascades', () => {
  it('removing the buy removes its protect but keeps the fund sells', async () => {
    useActionQueueStore.setState({ items: rotation() })
    useActionQueueStore.getState().removeItem('b1')
    expect(ids()).toEqual(['c1', 'f1'])
    await vi.waitFor(() => expect(useActionQueueStore.getState().fundsNotice).toBe('reconciled'))
  })

  it('removing a fund removes its cancels', async () => {
    useActionQueueStore.setState({ items: rotation() })
    useActionQueueStore.getState().removeItem('f1')
    expect(ids()).toEqual(['b1', 'p1'])
    await vi.waitFor(() => expect(useActionQueueStore.getState().fundsNotice).toBe('reconciled'))
  })

  it("removing a held position's protect records a dismissal with its current quantity", () => {
    useActionQueueStore.setState({
      items: [
        ...rotation(),
        rec({ id: 'p9', symbol: 'NVDA', side: 'SELL', orderType: 'TRAILING_STOP', group: 'g9', role: 'protect' }),
      ],
    })
    useActionQueueStore.getState().updateItem('p9', { quantity: 7 })
    useActionQueueStore.getState().removeItem('p9')
    expect(ids()).toEqual(['c1', 'f1', 'b1', 'p1'])
    expect(dismissal).toHaveBeenCalledTimes(1)
    expect(dismissal).toHaveBeenCalledWith('NVDA', 7)
  })

  it("removing a rotation's protect records no dismissal", () => {
    useActionQueueStore.setState({ items: rotation() })
    useActionQueueStore.getState().removeItem('p1')
    expect(ids()).toEqual(['c1', 'f1', 'b1'])
    expect(dismissal).not.toHaveBeenCalled()
  })

  it('swallows a rejected dismissal', async () => {
    dismissal.mockRejectedValue(new Error('boom'))
    useActionQueueStore.setState({
      items: [rec({ id: 'p9', symbol: 'NVDA', side: 'SELL', orderType: 'TRAILING_STOP', group: 'g9', role: 'protect' })],
    })
    expect(() => useActionQueueStore.getState().removeItem('p9')).not.toThrow()
    await vi.waitFor(() => expect(dismissal).toHaveBeenCalled())
  })

  it('removing a buy does not remove a protect of the same symbol in a different group', () => {
    useActionQueueStore.setState({
      items: [
        ...rotation(),
        rec({ id: 'p2', symbol: 'bac', side: 'SELL', group: 'g2', role: 'protect' }),
        rec({ id: 'p3', symbol: 'MSFT', side: 'SELL', group: 'g1', role: 'protect' }),
      ],
    })
    useActionQueueStore.getState().removeItem('b1')
    expect(ids()).toEqual(['c1', 'f1', 'p2', 'p3'])
  })

  it('matches symbols case-insensitively and removes every matching protect', () => {
    useActionQueueStore.setState({
      items: [
        rec({ id: 'b1', symbol: 'BAC', group: 'g1', role: 'buy' }),
        rec({ id: 'p1', symbol: 'bac', side: 'SELL', group: 'g1', role: 'protect' }),
        rec({ id: 'p2', symbol: 'BAC', side: 'SELL', group: 'g1', role: 'protect' }),
      ],
    })
    useActionQueueStore.getState().removeItem('b1')
    expect(ids()).toEqual([])
  })

  it('removing a protect directly also removes its same-group same-symbol cancel and records one dismissal', () => {
    useActionQueueStore.setState({
      items: [
        rec({ id: 'c1', symbol: 'NVDA', side: 'SELL', group: 'g1', role: 'cancel', cancelOrderId: 'o1' }),
        rec({ id: 'p1', symbol: 'nvda', side: 'SELL', group: 'g1', role: 'protect' }),
        rec({ id: 'c2', symbol: 'NVDA', side: 'SELL', group: 'g1', role: 'cancel', cancelOrderId: 'o2' }),
      ],
    })
    useActionQueueStore.getState().removeItem('p1')
    expect(ids()).toEqual([])
    expect(dismissal).toHaveBeenCalledTimes(1)
    expect(dismissal).toHaveBeenCalledWith('nvda', 10)
  })

  it('removing a protect does not remove a cancel in another group or for another symbol', async () => {
    useActionQueueStore.setState({
      items: [
        rec({ id: 'c1', symbol: 'NVDA', side: 'SELL', group: 'g1', role: 'cancel' }),
        rec({ id: 'p1', symbol: 'NVDA', side: 'SELL', group: 'g1', role: 'protect' }),
        rec({ id: 'c2', symbol: 'NVDA', side: 'SELL', group: 'g2', role: 'cancel' }),
        rec({ id: 'c3', symbol: 'AAPL', side: 'SELL', group: 'g1', role: 'cancel' }),
        rec({ id: 'c4', symbol: 'NVDA', side: 'SELL', role: 'cancel' }),
      ],
    })
    useActionQueueStore.getState().removeItem('p1')
    expect(ids()).toEqual(['c2', 'c3', 'c4'])
    await vi.waitFor(() => expect(useActionQueueStore.getState().fundsNotice).toBe('reconciled'))
  })

  it('cascades do not chain: removing a buy drops its protect but keeps a same-symbol cancel', async () => {
    useActionQueueStore.setState({
      items: [
        rec({ id: 'c1', symbol: 'BAC', side: 'SELL', group: 'g1', role: 'cancel' }),
        rec({ id: 'b1', symbol: 'BAC', group: 'g1', role: 'buy' }),
        rec({ id: 'p1', symbol: 'BAC', side: 'SELL', group: 'g1', role: 'protect' }),
      ],
    })
    useActionQueueStore.getState().removeItem('b1')
    expect(ids()).toEqual(['c1'])
    expect(dismissal).not.toHaveBeenCalled()
    await vi.waitFor(() => expect(useActionQueueStore.getState().fundsNotice).toBe('reconciled'))
  })

  it('a protect removed by cascade records no dismissal', () => {
    useActionQueueStore.setState({ items: rotation() })
    useActionQueueStore.getState().removeItem('b1')
    expect(dismissal).not.toHaveBeenCalled()
  })

  it('does not cascade when group is missing on either side', () => {
    useActionQueueStore.setState({
      items: [
        rec({ id: 'b1', symbol: 'BAC', role: 'buy' }),
        rec({ id: 'p1', symbol: 'BAC', side: 'SELL', role: 'protect' }),
        rec({ id: 'b2', symbol: 'NVDA', group: 'g1', role: 'buy' }),
        rec({ id: 'p2', symbol: 'NVDA', side: 'SELL', role: 'protect' }),
      ],
    })
    useActionQueueStore.getState().removeItem('b1')
    useActionQueueStore.getState().removeItem('b2')
    expect(ids()).toEqual(['p1', 'p2'])
  })

  it('removing a cancel or a standalone cascades nothing', () => {
    useActionQueueStore.setState({
      items: [...rotation(), rec({ id: 's1', symbol: 'MSFT', role: 'standalone' })],
    })
    useActionQueueStore.getState().removeItem('c1')
    useActionQueueStore.getState().removeItem('s1')
    expect(ids()).toEqual(['f1', 'b1', 'p1'])
    expect(dismissal).not.toHaveBeenCalled()
  })
})

describe('actionQueueStore post-removal reconcile', () => {
  it('reconciles the remaining items and applies items + notice', async () => {
    useActionQueueStore.setState({ items: rotation() })
    const adjusted = [rec({ id: 'c1', quantity: 3, group: 'g1', role: 'cancel' })]
    reconcile.mockResolvedValueOnce({ items: adjusted, availableCash: 0, notice: 'adjusted' })
    useActionQueueStore.getState().removeItem('f1')
    expect(reconcile).toHaveBeenCalledTimes(1)
    expect(reconcile.mock.calls[0][0].map((i) => i.id)).toEqual(['b1', 'p1'])
    await vi.waitFor(() => expect(useActionQueueStore.getState().fundsNotice).toBe('adjusted'))
    expect(useActionQueueStore.getState().items).toEqual(adjusted)
  })

  it('does not reconcile when no remaining item has a group', () => {
    useActionQueueStore.setState({
      items: [rec({ id: 'a' }), rec({ id: 'b', group: 'g1', role: 'buy' }), rec({ id: 'c' })],
    })
    useActionQueueStore.getState().removeItem('b')
    expect(ids()).toEqual(['a', 'c'])
    expect(reconcile).not.toHaveBeenCalled()
  })

  it('a stale reconcile from an earlier removal never overwrites newer state', async () => {
    useActionQueueStore.setState({ items: rotation() })
    let releaseFirst: (v: { items: OrderRecommendation[]; availableCash: number; notice: string }) => void = () => {}
    reconcile.mockImplementationOnce(
      () => new Promise((resolve) => { releaseFirst = resolve }),
    )
    useActionQueueStore.getState().removeItem('c1')
    useActionQueueStore.getState().removeItem('f1')
    await vi.waitFor(() => expect(useActionQueueStore.getState().fundsNotice).toBe('reconciled'))
    releaseFirst({ items: rotation(), availableCash: 0, notice: 'stale' })
    await Promise.resolve()
    await Promise.resolve()
    expect(useActionQueueStore.getState().fundsNotice).toBe('reconciled')
    expect(ids()).toEqual(['b1', 'p1'])
  })

  it('keeps the cascaded list when the reconcile rejects', async () => {
    reconcile.mockRejectedValueOnce(new Error('offline'))
    useActionQueueStore.setState({ items: rotation() })
    useActionQueueStore.getState().removeItem('b1')
    await vi.waitFor(() => expect(reconcile).toHaveBeenCalled())
    await Promise.resolve()
    expect(ids()).toEqual(['c1', 'f1'])
    expect(useActionQueueStore.getState().fundsNotice).toBeNull()
  })
})

describe('groupHeader', () => {
  it('describes a rotation (fund + buy)', () => {
    expect(groupHeader(rotation(), 'g1')).toBe('Rotation: sell PFE → buy BAC')
  })

  it('joins multiple fund symbols', () => {
    const items = [
      rec({ id: 'f1', symbol: 'PFE', side: 'SELL', group: 'g', role: 'fund' }),
      rec({ id: 'f2', symbol: 'KO', side: 'SELL', group: 'g', role: 'fund' }),
      rec({ id: 'b', symbol: 'BAC', group: 'g', role: 'buy' }),
    ]
    expect(groupHeader(items, 'g')).toBe('Rotation: sell PFE, KO → buy BAC')
  })

  it('describes fund only', () => {
    const items = [
      rec({ id: 'c', symbol: 'PFE', group: 'g', role: 'cancel' }),
      rec({ id: 'f', symbol: 'PFE', side: 'SELL', group: 'g', role: 'fund' }),
    ]
    expect(groupHeader(items, 'g')).toBe('Sell PFE to cash')
  })

  it('describes protect only', () => {
    const items = [rec({ id: 'p', symbol: 'NVDA', side: 'SELL', group: 'g', role: 'protect' })]
    expect(groupHeader(items, 'g')).toBe('Protect NVDA')
  })

  it('describes cancel + protect as Protect', () => {
    const items = [
      rec({ id: 'c', symbol: 'NVDA', group: 'g', role: 'cancel' }),
      rec({ id: 'p', symbol: 'NVDA', side: 'SELL', group: 'g', role: 'protect' }),
    ]
    expect(groupHeader(items, 'g')).toBe('Protect NVDA')
  })

  it('describes buy without fund', () => {
    const items = [
      rec({ id: 'b', symbol: 'BAC', group: 'g', role: 'buy' }),
      rec({ id: 'p', symbol: 'BAC', side: 'SELL', group: 'g', role: 'protect' }),
    ]
    expect(groupHeader(items, 'g')).toBe('Buy BAC')
  })

  it('describes cancel only', () => {
    const items = [rec({ id: 'c', symbol: 'NVDA', group: 'g', role: 'cancel' })]
    expect(groupHeader(items, 'g')).toBe('Cancel order on NVDA')
  })

  it('returns empty string for an unknown group', () => {
    expect(groupHeader(rotation(), 'nope')).toBe('')
  })

  it('ignores items from other groups', () => {
    const items = [...rotation(), rec({ id: 'x', symbol: 'ZZZ', group: 'g9', role: 'buy' })]
    expect(groupHeader(items, 'g1')).toBe('Rotation: sell PFE → buy BAC')
  })
})
