// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase C (coordinator decision, live-money safety): a Flow order skips
 * the app's per-order Live/Paper confirmation, so activating a workflow while
 * the app is in Live mode asks once first, from the Flow list and from the
 * editor. In Analyze mode activation stays one click. The mode is the app's
 * own (`get_analyze_mode`, what routes the orders), read when Activate is
 * chosen; a mode that cannot be read is treated as Live.
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Workflow, WorkflowListItem } from '@/api/flow'

const api = vi.hoisted(() => ({
  listWorkflows: vi.fn(),
  getWorkflow: vi.fn(),
  activateWorkflow: vi.fn(),
  updateWorkflow: vi.fn(),
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
const WARNING = 'This workflow will place real orders on the live account without asking each time.'

const LISTED: WorkflowListItem = {
  id: 7,
  name: 'Opening range breakout',
  description: null,
  is_active: false,
  created_at: '2026-10-29T09:00:00',
  updated_at: '2026-10-29T09:00:00',
  last_execution_status: null,
}

const STORED: Workflow = {
  id: 7,
  name: 'Opening range breakout',
  description: null,
  nodes: [
    {
      id: 'node_1',
      type: 'start',
      position: { x: 0, y: 0 },
      data: { scheduleType: 'daily', time: '09:35' },
    },
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

/** Choose Activate on the list's card. */
async function activateFromTheList() {
  const user = userEvent.setup()
  await user.click(await screen.findByRole('button', { name: 'Workflow actions' }))
  await user.click(await screen.findByRole('menuitem', { name: 'Activate' }))
  return user
}

/** Choose Activate in the editor's header. */
async function activateFromTheEditor() {
  const user = userEvent.setup()
  await screen.findByDisplayValue('Opening range breakout')
  await user.click(screen.getByRole('button', { name: 'Activate' }))
  return user
}

beforeEach(() => {
  for (const f of [...Object.values(api), mode.getAnalyzeMode]) f.mockReset()
  api.listWorkflows.mockResolvedValue([LISTED])
  api.getWorkflow.mockResolvedValue(STORED)
  api.getIndexSymbolsLotSizes.mockResolvedValue([])
  api.activateWorkflow.mockResolvedValue({ status: 'success', message: 'ok' })
})

describe('activating from the Flow list', () => {
  it('in Live mode asks first, and the dialog’s Activate activates', async () => {
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    renderAt('/flow')
    const user = await activateFromTheList()
    const dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByText(WARNING)).toBeInTheDocument()
    expect(api.activateWorkflow).not.toHaveBeenCalled()
    await user.click(within(dialog).getByRole('button', { name: 'Activate' }))
    await waitFor(() => expect(api.activateWorkflow).toHaveBeenCalledWith(7))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
  })

  it('in Live mode Cancel activates nothing', async () => {
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    renderAt('/flow')
    const user = await activateFromTheList()
    const dialog = await screen.findByRole('alertdialog')
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(api.activateWorkflow).not.toHaveBeenCalled()
  })

  it('asks when the mode cannot be read', async () => {
    mode.getAnalyzeMode.mockRejectedValue(new Error('not available'))
    renderAt('/flow')
    await activateFromTheList()
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument()
    expect(api.activateWorkflow).not.toHaveBeenCalled()
  })

  it('in Analyze mode activates at once, without asking', async () => {
    mode.getAnalyzeMode.mockResolvedValue(ANALYZE)
    renderAt('/flow')
    await activateFromTheList()
    await waitFor(() => expect(api.activateWorkflow).toHaveBeenCalledWith(7))
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })
})

describe('activating from the editor', () => {
  it('in Live mode asks first, and the dialog’s Activate activates', async () => {
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    renderAt('/flow/editor/7')
    const user = await activateFromTheEditor()
    const dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByText(WARNING)).toBeInTheDocument()
    expect(api.activateWorkflow).not.toHaveBeenCalled()
    await user.click(within(dialog).getByRole('button', { name: 'Activate' }))
    await waitFor(() => expect(api.activateWorkflow).toHaveBeenCalledWith(7))
  })

  it('in Analyze mode activates at once, without asking', async () => {
    mode.getAnalyzeMode.mockResolvedValue(ANALYZE)
    renderAt('/flow/editor/7')
    await activateFromTheEditor()
    await waitFor(() => expect(api.activateWorkflow).toHaveBeenCalledWith(7))
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })
})

describe('the Live-mode question', () => {
  it('keeps to the type scale and stays still under reduced motion (CLAUDE.md)', async () => {
    mode.getAnalyzeMode.mockResolvedValue(LIVE)
    renderAt('/flow')
    await activateFromTheList()
    const dialog = await screen.findByRole('alertdialog')
    const title = within(dialog).getByRole('heading', { name: 'Activate in Live mode?' })
    // Heading: 16px, weight 600; 18px (text-lg) is not on the scale.
    expect(title.className).toMatch(/(^|\s)text-base(\s|$)/)
    expect(title.className).not.toMatch(/(^|\s)text-lg(\s|$)/)
    const overlay = document.querySelector('[data-slot="alert-dialog-overlay"]')
    expect(overlay?.className).toMatch(/motion-reduce:animate-none/)
    expect(dialog.className).toMatch(/motion-reduce:animate-none/)
  })
})
