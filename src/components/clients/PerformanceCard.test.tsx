// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { PerformancePeriod, PerformanceReport, PeriodKey } from '@/api/analytics'
import { PerformanceCard } from '@/components/clients/PerformanceCard'

const mocks = vi.hoisted(() => ({ clientPerformance: vi.fn() }))

vi.mock('@/api/analytics', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/analytics')>()),
  analyticsApi: { clientPerformance: mocks.clientPerformance },
}))

function period(key: PeriodKey, overrides: Partial<PerformancePeriod> = {}): PerformancePeriod {
  return {
    key,
    start: '2026-01-01',
    end: '2026-11-11',
    start_value: 1000,
    end_value: 1150,
    net_flows: 0,
    twr: 0.15,
    twr_annualized: null,
    mwr: 0.1623,
    note: null,
    ...overrides,
  }
}

const report: PerformanceReport = {
  as_of: '2026-11-12',
  prices_as_of: 'Nov 11, 2026 close · 3:45 PM ET',
  periods: [
    period('1M', { start: '2026-10-12', twr: 0.0123, mwr: 0.15 }),
    period('3M', { start: '2026-08-12' }),
    period('6M', { start: '2026-05-12' }),
    period('YTD'),
    period('1Y', { start: '2025-11-12', twr: 0.2, twr_annualized: 0.2 }),
    period('SI', { start: '2025-10-01', twr: 0.21, twr_annualized: 0.1892, net_flows: 250 }),
  ],
  warnings: ['ZZZ: no price history'],
}

describe('PerformanceCard', () => {
  beforeEach(() => {
    mocks.clientPerformance.mockReset()
    mocks.clientPerformance.mockResolvedValue(report)
  })

  it('shows YTD by default with TWR, MWR, values, warnings and the prices-as-of caption', async () => {
    render(<PerformanceCard clientId={7} />)

    const table = await screen.findByRole('table', { name: 'YTD performance' })
    expect(mocks.clientPerformance).toHaveBeenCalledWith(7, null)
    expect(screen.getByText('All accounts')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'YTD' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: '1M' })).toHaveAttribute('aria-pressed', 'false')
    expect(within(table).getByText('+15.00%')).toBeInTheDocument()
    expect(within(table).getByText('+16.23%')).toBeInTheDocument()
    expect(within(table).getByText('$1,000.00')).toBeInTheDocument()
    expect(within(table).getByText('$1,150.00')).toBeInTheDocument()
    expect(screen.getByText('ZZZ: no price history')).toBeInTheDocument()
    expect(screen.getByText('Prices as of Nov 11, 2026 close · 3:45 PM ET')).toBeInTheDocument()
  })

  it('switches the table when another period chip is pressed', async () => {
    const user = userEvent.setup()
    render(<PerformanceCard clientId={7} />)
    await screen.findByRole('table', { name: 'YTD performance' })

    await user.click(screen.getByRole('button', { name: '1M' }))

    const table = screen.getByRole('table', { name: '1M performance' })
    expect(within(table).getByText('+1.23%')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '1M' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'YTD' })).toHaveAttribute('aria-pressed', 'false')
    // Under a year: no annualized figure, and the caption says why.
    expect(within(table).getByText('—')).toBeInTheDocument()
    expect(screen.getByText(/not annualized/)).toBeInTheDocument()
  })

  it('shows the annualized figure for windows of a year or more', async () => {
    const user = userEvent.setup()
    render(<PerformanceCard clientId={7} />)
    await screen.findByRole('table', { name: 'YTD performance' })

    await user.click(screen.getByRole('button', { name: '1Y' }))

    const table = screen.getByRole('table', { name: '1Y performance' })
    expect(within(table).getAllByText('+20.00%')).toHaveLength(2)
    expect(screen.queryByText(/not annualized/)).not.toBeInTheDocument()
  })

  it('scopes the returns to the selected account and says so', async () => {
    render(<PerformanceCard clientId={7} accountType="roth_ira" />)

    await screen.findByRole('table', { name: 'YTD performance' })
    expect(mocks.clientPerformance).toHaveBeenCalledWith(7, 'roth_ira')
    expect(screen.getByText('Roth IRA')).toBeInTheDocument()
  })

  it('shows the empty state when there are no periods', async () => {
    mocks.clientPerformance.mockResolvedValue({ ...report, periods: [], warnings: [] })
    render(<PerformanceCard clientId={7} />)

    expect(await screen.findByText(/No trades yet/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'YTD' })).not.toBeInTheDocument()
  })

  it('shows the error with a Retry button that re-queries', async () => {
    const user = userEvent.setup()
    mocks.clientPerformance.mockRejectedValueOnce(new Error('FMP API key not configured'))
    render(<PerformanceCard clientId={7} />)

    expect(await screen.findByText(/FMP API key not configured/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /retry/i }))

    await screen.findByRole('table', { name: 'YTD performance' })
    expect(mocks.clientPerformance).toHaveBeenCalledTimes(2)
  })
})
