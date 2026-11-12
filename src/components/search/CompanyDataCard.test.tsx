// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('sonner', () => ({
  toast: { success: mocks.toastSuccess, error: mocks.toastError },
}))

import { CompanyDataCard } from './CompanyDataCard'

const downloadedStatus = {
  rows: 16342,
  source_commit: '1ac8809da480',
  source_as_of: '2026-11-08',
  // 20:30 UTC on 2026-11-12 is 3:30 PM Eastern (EST, UTC-5).
  last_refresh: '2026-11-12T20:30:00Z',
  include_otc: false,
}

const emptyStatus = {
  rows: 0,
  source_commit: '1ac8809da480',
  source_as_of: '2026-11-08',
  last_refresh: null,
  include_otc: false,
}

const agilent = {
  symbol: 'A',
  exchange: 'NYSE',
  name: 'Agilent Technologies, Inc.',
  sector: 'Health Care',
  industry: 'Diagnostics & Research',
  industry_group: 'Pharmaceuticals, Biotechnology & Life Sciences',
  country: 'United States',
  market_cap: null,
  size_bucket: 'Large Cap',
  isin: 'US00846U1016',
  cusip: '00846U101',
  figi: 'BBG000C2V541',
  website: 'https://www.agilent.com',
  summary: 'Agilent provides solutions.',
  asset_type: 'equity',
  delisted: false,
}

type Handlers = Partial<Record<string, (args?: Record<string, unknown>) => unknown>>

function backend(handlers: Handlers) {
  mocks.invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    const handler = handlers[cmd]
    if (!handler) throw new Error(`unexpected command ${cmd}`)
    return handler(args)
  })
}

const refreshCalls = () =>
  mocks.invoke.mock.calls.filter(([cmd]) => cmd === 'symbol_metadata_refresh')
const searchCalls = () =>
  mocks.invoke.mock.calls.filter(([cmd]) => cmd === 'symbol_metadata_search')

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.toastSuccess.mockReset()
  mocks.toastError.mockReset()
})

describe('CompanyDataCard status and download', () => {
  it('renders the row count, pinned commit, data date and the download time in ET', async () => {
    backend({ symbol_metadata_status: () => downloadedStatus })
    render(<CompanyDataCard query="" />)

    const status = await screen.findByTestId('company-data-status')
    expect(status).toHaveTextContent(/16,342 symbols/)
    expect(status).toHaveTextContent(/FinanceDatabase 1ac8809da480/)
    expect(status).toHaveTextContent(/data as of Nov 8, 2026/)
    expect(status).toHaveTextContent(/downloaded Nov 12, 2026,? 3:30 PM EST/)
    expect(screen.getByText('Data: FinanceDatabase (MIT)')).toBeInTheDocument()
  })

  it('says "Not downloaded yet" before the first download', async () => {
    backend({ symbol_metadata_status: () => emptyStatus })
    render(<CompanyDataCard query="" />)
    expect(await screen.findByText('Not downloaded yet')).toBeInTheDocument()
  })

  it('Download calls symbol_metadata_refresh without OTC and shows the new status', async () => {
    let resolveRefresh: (status: typeof downloadedStatus) => void = () => {}
    backend({
      symbol_metadata_status: () => emptyStatus,
      symbol_metadata_refresh: () =>
        new Promise((resolve) => {
          resolveRefresh = resolve
        }),
    })
    render(<CompanyDataCard query="" />)
    await screen.findByText('Not downloaded yet')

    const button = screen.getByRole('button', { name: /Download symbol metadata/ })
    await userEvent.click(button)

    expect(refreshCalls()).toEqual([['symbol_metadata_refresh', { includeOtc: false }]])
    expect(button).toBeDisabled()
    expect(within(button).getByTestId('download-spinner')).toBeInTheDocument()

    resolveRefresh(downloadedStatus)
    await waitFor(() => expect(button).toBeEnabled())
    expect(screen.getByTestId('company-data-status')).toHaveTextContent(/16,342 symbols/)
    expect(mocks.toastSuccess).toHaveBeenCalledTimes(1)
    expect(mocks.toastError).not.toHaveBeenCalled()
  })

  it('checking "Include OTC (pink sheets)" then Download calls refresh with OTC', async () => {
    backend({
      symbol_metadata_status: () => emptyStatus,
      symbol_metadata_refresh: () => ({ ...downloadedStatus, include_otc: true, rows: 30000 }),
    })
    render(<CompanyDataCard query="" />)
    await screen.findByText('Not downloaded yet')

    await userEvent.click(screen.getByRole('checkbox', { name: /Include OTC \(pink sheets\)/ }))
    await userEvent.click(screen.getByRole('button', { name: /Download symbol metadata/ }))

    expect(refreshCalls()).toEqual([['symbol_metadata_refresh', { includeOtc: true }]])
    expect(await screen.findByTestId('company-data-status')).toHaveTextContent(/30,000 symbols/)
  })

  it('a failed download shows an error toast and keeps the previous status', async () => {
    backend({
      symbol_metadata_status: () => downloadedStatus,
      symbol_metadata_refresh: () => {
        throw new Error('network down')
      },
    })
    render(<CompanyDataCard query="" />)
    await screen.findByTestId('company-data-status')

    const button = screen.getByRole('button', { name: /Download symbol metadata/ })
    await userEvent.click(button)

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1))
    expect(mocks.toastSuccess).not.toHaveBeenCalled()
    expect(button).toBeEnabled()
    expect(screen.getByTestId('company-data-status')).toHaveTextContent(/16,342 symbols/)
  })
})

describe('CompanyDataCard search', () => {
  it('renders no table without a query', async () => {
    backend({ symbol_metadata_status: () => downloadedStatus })
    render(<CompanyDataCard query="" />)
    await screen.findByTestId('company-data-status')
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(searchCalls()).toHaveLength(0)
  })

  it('a query renders the matching rows with symbol, name, exchange, sector, industry and country', async () => {
    backend({
      symbol_metadata_status: () => downloadedStatus,
      symbol_metadata_search: () => [agilent],
    })
    render(<CompanyDataCard query="agilent" />)

    const row = await screen.findByTestId('company-data-row-A-NYSE')
    const cells = within(row)
      .getAllByRole('cell')
      .map((c) => c.textContent?.trim())
    expect(cells).toEqual([
      'A',
      'Agilent Technologies, Inc.',
      'NYSE',
      'Health Care',
      'Diagnostics & Research',
      'United States',
    ])
    expect(searchCalls()[0][1]).toMatchObject({ query: 'agilent' })

    const headers = screen.getAllByRole('columnheader').map((h) => h.textContent?.trim())
    expect(headers).toEqual(['Symbol', 'Name', 'Exchange', 'Sector', 'Industry', 'Country'])
  })

  it('shows the empty state when nothing matches', async () => {
    backend({
      symbol_metadata_status: () => downloadedStatus,
      symbol_metadata_search: () => [],
    })
    render(<CompanyDataCard query="zzzz" />)
    expect(await screen.findByText('No company data matches')).toBeInTheDocument()
  })

  it('shows an error with Retry when the search fails, and Retry runs it again', async () => {
    let failures = 1
    backend({
      symbol_metadata_status: () => downloadedStatus,
      symbol_metadata_search: () => {
        if (failures > 0) {
          failures -= 1
          throw new Error('duckdb busy')
        }
        return [agilent]
      },
    })
    render(<CompanyDataCard query="agilent" />)

    const retry = await screen.findByRole('button', { name: /Retry/ })
    expect(screen.getByText(/Something went wrong/)).toBeInTheDocument()

    await userEvent.click(retry)
    expect(await screen.findByTestId('company-data-row-A-NYSE')).toBeInTheDocument()
    expect(searchCalls()).toHaveLength(2)
  })
})
