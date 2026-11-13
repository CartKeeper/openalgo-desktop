// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase D (coordinator decision 6c, live-money safety): Run Now places
 * the workflow's orders at once, and a Flow order skips the app's per-order
 * Live/Paper confirmation. So in Live mode Run Now asks first, the way
 * Activate does; in Analyze mode it stays one click. The mode is the app's
 * own, read from the backend when Run Now is chosen; a mode that cannot be
 * read counts as Live.
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Workflow } from '@/api/flow'

const api = vi.hoisted(() => ({
  getWorkflow: vi.fn(),
  executeWorkflow: vi.fn(),
  getIndexSymbolsLotSizes: vi.fn(async () => []),
  getWebhookInfo: vi.fn(),
}))
vi.mock('@/api/flow', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/flow')>()
  return { ...actual, ...api }
})

const mode = vi.hoisted(() => ({ getAnalyzeMode: vi.fn() }))
vi.mock('@/api/tauri-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/tauri-client')>()
  return {
    ...actual,
    settingsCommands: { ...actual.settingsCommands, getAnalyzeMode: mode.getAnalyzeMode },
  }
})

vi.mock('@/utils/toast', () => ({
  showToast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}))

const { default: FlowEditor } = await import('./FlowEditor')

const LIVE = { analyze_mode: false, mode: 'live' }
const ANALYZE = { analyze_mode: true, mode: 'analyzer' }
const SENTENCE =
  'This run will place real orders on the live account without asking for each order.'

const STORED: Workflow = {
  id: 7,
  name: 'Opening range breakout',
  description: null,
  nodes: [
    { id: 'node_1', type: 'start', position: { x: 0, y: 0 }, data: { scheduleType: 'manual' } },
  ],
  edges: [],
  is_active: false,
  schedule_job_id: null,
  webhook_token: null,
  webhook_secret: null,
  webhook_enabled: false,
  webhook_auth_type: 'payload',
  created_at: '2026-10-29T09:00:00',
  updated_at: '2026-10-29T09:00:00',
}

function renderEditor() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } },
  })
  return render(
    <div style={{ width: 1400, height: 900 }}>
      <MemoryRouter initialEntries={['/flow/editor/7']}>
        <QueryClientProvider client={client}>
          <Routes>
            <Route path="/flow/editor/:id" element={<FlowEditor />} />
          </Routes>
        </QueryClientProvider>
      </MemoryRouter>
    </div>
  )
}

/** Choose Run Now in the editor's ... menu. */
async function runNowFromTheEditor() {
  const user = userEvent.setup()
  await screen.findByDisplayValue('Opening range breakout')
  await user.click(screen.getByRole('button', { name: 'Workflow actions' }))
  await user.click(await screen.findByRole('menuitem', { name: 'Run Now' }))
  return user
}

beforeEach(() => {
  for (const f of [...Object.values(api), mode.getAnalyzeMode]) f.mockReset()
  api.getWorkflow.mockResolvedValue(STORED)
  api.getIndexSymbolsLotSizes.mockResolvedValue([])
  api.executeWorkflow.mockResolvedValue({ status: 'success', message: 'ok', logs: [] })
})

describe('Run Now in the editor', () => {
  it('in Live mode asks first, and the question’s Run runs the workflow once', async () => {
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    renderEditor()
    const user = await runNowFromTheEditor()
    const dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByRole('heading', { name: 'Run in Live mode?' })).toBeInTheDocument()
    expect(within(dialog).getByText(SENTENCE)).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toBeInTheDocument()
    expect(api.executeWorkflow).not.toHaveBeenCalled()
    await user.click(within(dialog).getByRole('button', { name: 'Run' }))
    await waitFor(() => expect(api.executeWorkflow).toHaveBeenCalledWith(7))
    expect(api.executeWorkflow).toHaveBeenCalledTimes(1)
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
  })

  it('in Live mode Cancel runs nothing', async () => {
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    renderEditor()
    const user = await runNowFromTheEditor()
    const dialog = await screen.findByRole('alertdialog')
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(api.executeWorkflow).not.toHaveBeenCalled()
  })

  it('asks when the mode cannot be read', async () => {
    mode.getAnalyzeMode.mockRejectedValue(new Error('not available'))
    renderEditor()
    await runNowFromTheEditor()
    const dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByRole('heading', { name: 'Run in Live mode?' })).toBeInTheDocument()
    expect(api.executeWorkflow).not.toHaveBeenCalled()
  })

  it('reads the mode when Run Now is chosen, not before', async () => {
    mode.getAnalyzeMode.mockResolvedValue(ANALYZE)
    renderEditor()
    await screen.findByDisplayValue('Opening range breakout')
    expect(mode.getAnalyzeMode).not.toHaveBeenCalled()
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    await runNowFromTheEditor()
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument()
    expect(api.executeWorkflow).not.toHaveBeenCalled()
  })

  it('in Analyze mode runs at once, without asking', async () => {
    mode.getAnalyzeMode.mockResolvedValue(ANALYZE)
    renderEditor()
    await runNowFromTheEditor()
    await waitFor(() => expect(api.executeWorkflow).toHaveBeenCalledWith(7))
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })
})
