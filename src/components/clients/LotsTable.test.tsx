// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import type { LotReport } from '@/api/analytics'
import { LotsTable } from '@/components/clients/LotsTable'

const mocks = vi.hoisted(() => ({ clientLots: vi.fn() }))

vi.mock('@/api/analytics', () => ({
  analyticsApi: { clientLots: mocks.clientLots },
}))

class ObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return []
  }
}

beforeAll(() => {
  // Radix Select needs pointer capture and scrollIntoView, which jsdom lacks, and
  // its popper constructs observers; the shared setup's arrow mocks are not constructible.
  Element.prototype.hasPointerCapture = () => false
  Element.prototype.setPointerCapture = () => {}
  Element.prototype.releasePointerCapture = () => {}
  Element.prototype.scrollIntoView = () => {}
  window.ResizeObserver = ObserverStub as unknown as typeof ResizeObserver
  window.IntersectionObserver = ObserverStub as unknown as typeof IntersectionObserver
})

/** The Task 1 FIFO fixture with a synthesized opening lot in front. */
const fifo: LotReport = {
  method: 'fifo',
  as_of: '2026-11-12',
  open_lots: [
    {
      symbol: 'XYZ',
      opened: '2026-01-04',
      quantity: 5,
      cost_per_share: 90,
      cost_basis: 450,
      estimated: true,
      market_value: 625,
      unrealized_pnl: 175,
      term: 'short',
    },
    {
      symbol: 'XYZ',
      opened: '2026-02-01',
      quantity: 10,
      cost_per_share: 120.1,
      cost_basis: 1201,
      estimated: false,
      market_value: 1250,
      unrealized_pnl: 49,
      term: 'short',
    },
  ],
  disposals: [
    {
      symbol: 'XYZ',
      opened: '2026-01-05',
      closed: '2026-03-01',
      quantity: 5,
      proceeds: 649,
      cost_basis: 500.5,
      realized_pnl: 148.5,
      holding_days: 55,
      term: 'short',
      estimated: false,
    },
  ],
  totals: {
    cost_basis: 1651,
    market_value: 1875,
    unrealized_short: 224,
    unrealized_long: 0,
    realized_short_ytd: 148.5,
    realized_long_ytd: 0,
  },
  warnings: ['XYZ: trades sell 1 more shares than the history explains; those sells were ignored'],
}

describe('LotsTable', () => {
  beforeEach(() => {
    mocks.clientLots.mockReset()
    mocks.clientLots.mockResolvedValue(fifo)
  })

  it('loads FIFO lots and shows the realized P/L, the est. badge and the warning', async () => {
    render(<LotsTable clientId={7} />)

    const realized = await screen.findByRole('table', { name: /realized/i })
    expect(within(realized).getByText(/148\.50/)).toBeInTheDocument()
    expect(mocks.clientLots).toHaveBeenCalledWith(7, 'fifo')

    const openLots = screen.getByRole('table', { name: /open lots/i })
    expect(within(openLots).getAllByText('est.')).toHaveLength(1)
    expect(within(openLots).getAllByRole('row')).toHaveLength(3) // header + two lots

    expect(screen.getByText(/those sells were ignored/)).toBeInTheDocument()
  })

  it('re-queries with lifo when the method changes', async () => {
    const user = userEvent.setup()
    render(<LotsTable clientId={7} />)
    await screen.findByRole('table', { name: /open lots/i })

    await user.click(screen.getByRole('combobox', { name: /lot method/i }))
    await user.click(await screen.findByRole('option', { name: 'LIFO' }))

    await waitFor(() => expect(mocks.clientLots).toHaveBeenLastCalledWith(7, 'lifo'))
  })

  it('shows an empty state when there are no lots', async () => {
    mocks.clientLots.mockResolvedValue({ ...fifo, open_lots: [], disposals: [], warnings: [] })
    render(<LotsTable clientId={7} />)

    expect(await screen.findByText(/No trades yet/)).toBeInTheDocument()
    expect(screen.queryByRole('table', { name: /open lots/i })).not.toBeInTheDocument()
  })

  it('shows the error with a Retry button that re-queries', async () => {
    const user = userEvent.setup()
    mocks.clientLots.mockRejectedValueOnce(new Error('FMP is down'))
    render(<LotsTable clientId={7} />)

    expect(await screen.findByText(/FMP is down/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /retry/i }))

    await screen.findByRole('table', { name: /open lots/i })
    expect(mocks.clientLots).toHaveBeenCalledTimes(2)
  })
})
