// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase D (coordinator decision 6a): the run history opens from the
 * workflow card's ... menu on the Flow page and from the editor's ...
 * menu; its empty state's Run Now asks first in Live mode (decision 6c).
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Workflow, WorkflowExecution, WorkflowListItem } from '@/api/flow'

const api = vi.hoisted(() => ({
  listWorkflows: vi.fn(),
  getWorkflow: vi.fn(),
  getWorkflowExecutions: vi.fn(),
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

const { default: FlowIndex } = await import('./FlowIndex')
const { default: FlowEditor } = await import('./FlowEditor')

const LIVE = { analyze_mode: false, mode: 'live' }
const ANALYZE = { analyze_mode: true, mode: 'analyzer' }

const LISTED: WorkflowListItem = {
  id: 7,
  name: 'Opening range breakout',
  description: null,
  is_active: true,
  created_at: '2026-11-12T14:00:00',
  updated_at: '2026-11-12T14:00:00',
  last_execution_status: 'completed',
}

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
  created_at: '2026-11-12T14:00:00',
  updated_at: '2026-11-12T14:00:00',
}

const RUN: WorkflowExecution = {
  id: 40,
  workflow_id: 7,
  status: 'completed',
  started_at: '2026-11-12T19:35:02',
  completed_at: '2026-11-12T19:35:04',
  error: null,
  logs: [{ time: '2026-11-12T14:35:02', message: 'Starting workflow: ORB', level: 'info' }],
}

function renderAt(path: string) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } },
  })
  return render(
    <div style={{ width: 1400, height: 900 }}>
      <MemoryRouter initialEntries={[path]}>
        <QueryClientProvider client={client}>
          <Routes>
            <Route path="/flow" element={<FlowIndex />} />
            <Route path="/flow/editor/:id" element={<FlowEditor />} />
          </Routes>
        </QueryClientProvider>
      </MemoryRouter>
    </div>
  )
}

beforeEach(() => {
  for (const f of [...Object.values(api), mode.getAnalyzeMode]) f.mockReset()
  api.listWorkflows.mockResolvedValue([LISTED])
  api.getWorkflow.mockResolvedValue(STORED)
  api.getIndexSymbolsLotSizes.mockResolvedValue([])
  api.getWorkflowExecutions.mockResolvedValue([RUN])
  api.executeWorkflow.mockResolvedValue({ status: 'success', message: 'ok', logs: [] })
})

describe('Run history from the Flow page', () => {
  it("opens from the card's ... menu and lists the workflow's runs in Eastern time", async () => {
    const user = userEvent.setup()
    renderAt('/flow')
    await user.click(await screen.findByRole('button', { name: 'Workflow actions' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Run history' }))
    const history = await screen.findByRole('dialog', { name: 'Run history' })
    expect(within(history).getByText('Nov 12, 2:35:02 PM ET')).toBeInTheDocument()
    expect(within(history).getByText('Opening range breakout')).toBeInTheDocument()
    expect(api.getWorkflowExecutions).toHaveBeenCalledWith(7, 200)
  })

  it('with no runs, Run Now asks first in Live mode, then runs the workflow', async () => {
    api.getWorkflowExecutions.mockResolvedValue([])
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    const user = userEvent.setup()
    renderAt('/flow')
    await user.click(await screen.findByRole('button', { name: 'Workflow actions' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Run history' }))
    const history = await screen.findByRole('dialog', { name: 'Run history' })
    await user.click(await within(history).findByRole('button', { name: 'Run Now' }))
    const question = await screen.findByRole('alertdialog')
    expect(within(question).getByRole('heading', { name: 'Run in Live mode?' })).toBeInTheDocument()
    expect(api.executeWorkflow).not.toHaveBeenCalled()
    await user.click(within(question).getByRole('button', { name: 'Run' }))
    await waitFor(() => expect(api.executeWorkflow).toHaveBeenCalledWith(7))
    // The finished run is read at once, not after the next 10-second refresh.
    await waitFor(() => expect(api.getWorkflowExecutions).toHaveBeenCalledTimes(2))
  })
})

describe('Run history from the editor', () => {
  it("opens from the editor's ... menu", async () => {
    mode.getAnalyzeMode.mockResolvedValue(ANALYZE)
    const user = userEvent.setup()
    renderAt('/flow/editor/7')
    await screen.findByDisplayValue('Opening range breakout')
    await user.click(screen.getByRole('button', { name: 'Workflow actions' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Run history' }))
    const history = await screen.findByRole('dialog', { name: 'Run history' })
    expect(within(history).getByText('Nov 12, 2:35:02 PM ET')).toBeInTheDocument()
  })

  it('with no runs, Run Now in Analyze mode runs at once, and the list reads the run', async () => {
    api.getWorkflowExecutions.mockResolvedValue([])
    mode.getAnalyzeMode.mockResolvedValue(ANALYZE)
    const user = userEvent.setup()
    renderAt('/flow/editor/7')
    await screen.findByDisplayValue('Opening range breakout')
    await user.click(screen.getByRole('button', { name: 'Workflow actions' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Run history' }))
    const history = await screen.findByRole('dialog', { name: 'Run history' })
    api.getWorkflowExecutions.mockResolvedValue([RUN])
    await user.click(await within(history).findByRole('button', { name: 'Run Now' }))
    await waitFor(() => expect(api.executeWorkflow).toHaveBeenCalledWith(7))
    expect(await within(history).findByText('Nov 12, 2:35:02 PM ET')).toBeInTheDocument()
  })
})
