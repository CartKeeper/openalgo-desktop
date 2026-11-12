// @vitest-environment jsdom
import '@/test/setup'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AllocationTarget, DriftReport, RebalancePlan } from '@/api/analytics'
import { AllocationCard } from '@/components/clients/AllocationCard'
import { useActionQueueStore } from '@/stores/actionQueueStore'

const mocks = vi.hoisted(() => ({
  listTargets: vi.fn(),
  saveTargets: vi.fn(),
  scenarioDrift: vi.fn(),
  rebalancePlan: vi.fn(),
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}))

vi.mock('@/api/analytics', () => ({
  analyticsApi: {
    listTargets: mocks.listTargets,
    saveTargets: mocks.saveTargets,
    scenarioDrift: mocks.scenarioDrift,
    rebalancePlan: mocks.rebalancePlan,
  },
}))
vi.mock('sonner', () => ({ toast: mocks.toast }))

class ObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return []
  }
}

beforeAll(() => {
  Element.prototype.hasPointerCapture = () => false
  Element.prototype.setPointerCapture = () => {}
  Element.prototype.releasePointerCapture = () => {}
  Element.prototype.scrollIntoView = () => {}
  window.ResizeObserver = ObserverStub as unknown as typeof ResizeObserver
  window.IntersectionObserver = ObserverStub as unknown as typeof IntersectionObserver
})

const savedTarget: AllocationTarget = {
  id: 11,
  client_id: 1,
  scope: 'symbol',
  key: 'VOO',
  target_pct: 40,
  band_pct: 2,
}

const drift: DriftReport = {
  total_value: 11000,
  cash: 1000,
  rows: [
    {
      scope: 'symbol',
      key: 'VOO',
      target_pct: 40,
      band_pct: 2,
      actual_pct: 45.4545,
      drift_pct: 5.4545,
      actual_value: 5000,
      status: 'over',
    },
  ],
  untargeted_pct: 54.5455,
  sectors: ['Technology'],
  warnings: [],
}

const plan: RebalancePlan = {
  lines: [
    {
      symbol: 'VOO',
      exchange: 'NYSE',
      side: 'sell',
      quantity: 12,
      price: 50,
      value: 600,
      before_pct: 45.4545,
      after_pct: 40,
      reason: 'Rebalance VOO from 45.5% to 40.0%',
    },
    {
      symbol: 'BND',
      exchange: 'NASDAQ',
      side: 'buy',
      quantity: 6,
      price: 100,
      value: 600,
      before_pct: 10,
      after_pct: 15.5,
      reason: 'Rebalance BND from 10.0% to 15.5%',
    },
  ],
  cash_before: 1000,
  cash_after: 1000,
  unresolved: ['Health: no holding to adjust'],
  note: 'Whole shares; prices as of the plan time; cash never goes below zero.',
}

function renderCard() {
  return render(<AllocationCard clientId={1} scenarioId={5} symbols={['VOO', 'BND']} />)
}

describe('AllocationCard', () => {
  beforeEach(() => {
    useActionQueueStore.getState().close()
    mocks.listTargets.mockReset().mockResolvedValue([])
    mocks.scenarioDrift.mockReset().mockResolvedValue({ ...drift, rows: [], untargeted_pct: 100 })
    mocks.saveTargets.mockReset().mockResolvedValue([savedTarget])
    mocks.rebalancePlan.mockReset().mockResolvedValue(plan)
  })

  it('saves a typed target through the API and shows its drift row', async () => {
    const user = userEvent.setup()
    renderCard()
    await screen.findByRole('button', { name: /add target/i })
    expect(screen.getByRole('button', { name: /^save$/i })).toBeDisabled()

    await user.click(screen.getByRole('button', { name: /add target/i }))
    await user.type(screen.getByLabelText(/symbol or sector/i), 'VOO')
    await user.type(screen.getByLabelText(/target %/i), '40')

    mocks.scenarioDrift.mockResolvedValue(drift)
    await user.click(screen.getByRole('button', { name: /^save$/i }))

    await waitFor(() =>
      expect(mocks.saveTargets).toHaveBeenCalledWith(1, [
        { id: null, client_id: 1, scope: 'symbol', key: 'VOO', target_pct: 40, band_pct: 2 },
      ])
    )
    const table = await screen.findByRole('table', { name: /drift/i })
    expect(within(table).getByText('VOO')).toBeInTheDocument()
    expect(within(table).getByText('45.45%')).toBeInTheDocument()
    expect(within(table).getByText('Over')).toBeInTheDocument()
    expect(within(table).getByText('54.55%')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^save$/i })).toBeDisabled()
  })

  it('shows the server validation message when a save is refused', async () => {
    const user = userEvent.setup()
    mocks.saveTargets.mockRejectedValueOnce(
      new Error('Validation error: symbol targets sum to 110.0%, which is above 100%')
    )
    renderCard()
    await user.click(await screen.findByRole('button', { name: /add target/i }))
    await user.type(screen.getByLabelText(/symbol or sector/i), 'VOO')
    await user.type(screen.getByLabelText(/target %/i), '60')
    await user.click(screen.getByRole('button', { name: /^save$/i }))

    expect(await screen.findByText(/sum to 110\.0%/)).toBeInTheDocument()
  })

  it('builds the plan and opens the review with its two lines as actions', async () => {
    const user = userEvent.setup()
    mocks.listTargets.mockResolvedValue([savedTarget])
    mocks.scenarioDrift.mockResolvedValue(drift)
    renderCard()
    await screen.findByRole('table', { name: /drift/i })

    await user.click(screen.getByRole('button', { name: /build rebalance plan/i }))

    const table = await screen.findByRole('table', { name: /rebalance plan/i })
    expect(mocks.rebalancePlan).toHaveBeenCalledWith(5)
    const rows = within(table).getAllByRole('row')
    expect(rows).toHaveLength(3)
    expect(within(rows[1]).getByText('VOO')).toBeInTheDocument()
    expect(within(rows[1]).getByText('Sell')).toBeInTheDocument()
    expect(within(rows[1]).getByText('12')).toBeInTheDocument()
    expect(within(rows[2]).getByText('Buy')).toBeInTheDocument()
    expect(screen.getByText('Health: no holding to adjust')).toBeInTheDocument()
    expect(screen.getByText(plan.note)).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /review plan/i }))

    const state = useActionQueueStore.getState()
    expect(state.isReviewOpen).toBe(true)
    expect(state.items).toHaveLength(2)
    expect(state.items.map((item) => [item.symbol, item.side, item.quantity])).toEqual([
      ['VOO', 'SELL', 12],
      ['BND', 'BUY', 6],
    ])
  })

  it('refreshes the drift and drops the plan once the review closes', async () => {
    const user = userEvent.setup()
    mocks.listTargets.mockResolvedValue([savedTarget])
    mocks.scenarioDrift.mockResolvedValue(drift)
    renderCard()
    await screen.findByRole('table', { name: /drift/i })
    await user.click(screen.getByRole('button', { name: /build rebalance plan/i }))
    await screen.findByRole('table', { name: /rebalance plan/i })
    await user.click(screen.getByRole('button', { name: /review plan/i }))
    expect(mocks.scenarioDrift).toHaveBeenCalledTimes(1)

    act(() => useActionQueueStore.getState().close())

    await waitFor(() => expect(mocks.scenarioDrift).toHaveBeenCalledTimes(2))
    expect(screen.queryByRole('table', { name: /rebalance plan/i })).not.toBeInTheDocument()
  })

  it('does not offer a plan without saved targets', async () => {
    renderCard()
    await screen.findByRole('button', { name: /add target/i })
    expect(screen.getByRole('button', { name: /build rebalance plan/i })).toBeDisabled()
  })
})
