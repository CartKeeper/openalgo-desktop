// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase B: every Flow page carries the app's shared HelpButton (the
 * floating "Help" trigger every page of this app has), with items that name
 * that page's controls in this app's words (US venues, Eastern time, the
 * Sandbox page). The carried pages are upstream's; the help is this app's.
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Workflow, WorkflowListItem } from '@/api/flow'

const api = vi.hoisted(() => ({
  listWorkflows: vi.fn(),
  getWorkflow: vi.fn(),
  getIndexSymbolsLotSizes: vi.fn(async () => []),
  getWebhookInfo: vi.fn(),
}))
vi.mock('@/api/flow', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/flow')>()
  return { ...actual, ...api }
})
vi.mock('@/utils/toast', () => ({
  showToast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}))

const { default: FlowIndex } = await import('./FlowIndex')
const { default: FlowEditor } = await import('./FlowEditor')
const { default: FlowKeyboardShortcuts } = await import('./FlowKeyboardShortcuts')

const LISTED: WorkflowListItem = {
  id: 7,
  name: 'Opening range breakout',
  description: 'AAPL at the open',
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
    { id: 'node_1', type: 'start', position: { x: 0, y: 0 }, data: { scheduleType: 'manual' } },
    {
      id: 'node_2',
      type: 'getQuote',
      position: { x: 0, y: 160 },
      data: { symbol: 'AAPL', exchange: 'NASDAQ' },
    },
  ],
  edges: [{ id: 'e1', source: 'node_1', target: 'node_2' }],
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
            <Route path="/flow/shortcuts" element={<FlowKeyboardShortcuts />} />
          </Routes>
        </QueryClientProvider>
      </MemoryRouter>
    </div>
  )
}

/**
 * Open the shared help panel and return it. The pages have buttons named
 * Import, Save and Activate too, so the items are looked up inside the panel.
 */
async function openHelp(pageTitle: string): Promise<HTMLElement> {
  await userEvent.setup().click(await screen.findByRole('button', { name: 'Help' }))
  const panel = screen.getByRole('button', { name: 'Close help' }).closest('.w-80')
  expect(panel).not.toBeNull()
  expect(within(panel as HTMLElement).getByText(pageTitle)).toBeInTheDocument()
  return panel as HTMLElement
}

beforeEach(() => {
  api.listWorkflows.mockReset()
  api.listWorkflows.mockResolvedValue([LISTED])
  api.getWorkflow.mockReset()
  api.getWorkflow.mockResolvedValue(STORED)
})

describe('the Flow page', () => {
  it('has the Help button, and its panel names the list controls', async () => {
    renderAt('/flow')
    await screen.findByRole('heading', { level: 1, name: 'Flow Editor' })
    const panel = await openHelp('Flow')
    for (const title of [
      'New Workflow',
      'Import',
      'Activate and Deactivate',
      'Webhook',
      'Schedules',
      'Price alerts',
      'Webhook alerts',
      'Order Update triggers',
      'Telegram alerts',
      'Test in analyzer mode first',
    ]) {
      expect(within(panel).getByText(title)).toBeInTheDocument()
    }
    // A US broker's sidebar has no Sandbox page; in analyzer mode the
    // Orderbook shows the sandbox's orders (get_order_book routes there).
    expect(within(panel).getByText(/the Orderbook shows the sandbox's orders/)).toBeInTheDocument()
    expect(within(panel).queryByText(/Sandbox page/)).not.toBeInTheDocument()
    // WP8 Phase C connected the triggers and Telegram: each says how it
    // behaves here, in Eastern 12-hour time.
    expect(within(panel).queryByText('Not connected yet')).not.toBeInTheDocument()
    expect(within(panel).getByText(/10:01:02 AM ET, then 10:02:02 AM ET/)).toBeInTheDocument()
    expect(within(panel).getByText(/skipped and leaves nothing in the run history/)).toBeInTheDocument()
    expect(within(panel).getByText(/every 5 seconds while the workflow is active/)).toBeInTheDocument()
    // The listener is IPv4 127.0.0.1 only: `localhost` can resolve to ::1.
    const webhook = within(panel).getByText(/http:\/\/127\.0\.0\.1:5000\/flow\/webhook\/<token>/)
    expect(webhook.textContent).toMatch(/Enable API Server is on \(Admin → Server Settings/)
    expect(webhook.textContent).toMatch(/lock the webhook for 15 minutes/)
    expect(panel.textContent).not.toMatch(/localhost/)
    expect(within(panel).getByText(/reads the order book every 5 seconds/)).toBeInTheDocument()
    expect(
      within(panel).getByText(/to the chat set up on the Telegram page \(profile menu → Telegram Bot\)/)
    ).toBeInTheDocument()
    // Activation in Live mode asks first (desktop/FlowLiveActivation).
    expect(within(panel).getByText(/In Live mode it asks first/)).toBeInTheDocument()
    // Flow orders reach OrderService with no acknowledged mode, so the app's
    // per-order Live/Paper check (Gate B) does not apply: the help says so.
    expect(within(panel).getByText(/no confirmation per order/)).toBeInTheDocument()
    expect(within(panel).queryByText(/order checks apply/)).not.toBeInTheDocument()
  })
})

describe('the Flow editor', () => {
  it('has the Help button, and its panel names the editor controls in Eastern time', async () => {
    renderAt('/flow/editor/7')
    await screen.findByDisplayValue('Opening range breakout')
    const panel = await openHelp('Flow Editor')
    for (const title of [
      'Build the workflow',
      'Save',
      'Run Now and the Execution Log',
      'Activate',
      'Variables',
      'Times are Eastern',
      'Export and Replace from JSON',
      'Keyboard shortcuts',
    ]) {
      expect(within(panel).getByText(title)).toBeInTheDocument()
    }
    expect(within(panel).getByText(/America\/New_York/)).toBeInTheDocument()
    expect(within(panel).getByText(/AAPL on NASDAQ/)).toBeInTheDocument()
    expect(within(panel).getByText(/In Live mode it asks first/)).toBeInTheDocument()
    // The new nodes' US defaults, as the editor gives them (WP8 follow-up).
    expect(
      within(panel).getByText(/A new node starts on the US exchange, product CNC/)
    ).toBeInTheDocument()
    expect(within(panel).getByText(/A new Schedule runs at 9:35 AM ET/)).toBeInTheDocument()
    expect(within(panel).getByText(/the Orderbook shows them/)).toBeInTheDocument()
    expect(within(panel).queryByText(/Sandbox page/)).not.toBeInTheDocument()
    // The variables example is one a run can produce on this branch.
    expect(within(panel).queryByText(/webhook\./)).not.toBeInTheDocument()
  })
})

describe('the keyboard shortcuts page', () => {
  it('has the Help button, and its panel says how to get back', async () => {
    renderAt('/flow/shortcuts')
    await screen.findByRole('heading', { level: 1, name: 'Keyboard Shortcuts' })
    const panel = await openHelp('Keyboard Shortcuts')
    expect(within(panel).getByText('Back to the editor')).toBeInTheDocument()
    expect(within(panel).getByText('Open this page from the editor')).toBeInTheDocument()
  })
})
