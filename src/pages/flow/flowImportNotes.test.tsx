// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase B: an import's notes reach the trader. The server reads a web
 * workflow's schedule and time-window times as Eastern time and says so in
 * the import reply (`notes`, Phase A Task A3); the Flow page shows that
 * sentence, and the count of legacy fields it upgraded, instead of a bare
 * "imported".
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
  listWorkflows: vi.fn(async () => []),
  importWorkflow: vi.fn(),
}))
vi.mock('@/api/flow', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/flow')>()
  return { ...actual, ...api }
})

const toast = vi.hoisted(() => ({
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
}))
vi.mock('@/utils/toast', () => ({ showToast: toast }))

const { default: FlowIndex } = await import('./FlowIndex')

const NOTE = 'Schedule and time-window times are read as Eastern time in this app'
const WEB_WORKFLOW = JSON.stringify({
  name: 'Opening bell',
  nodes: [{ id: 's', type: 'start', position: { x: 0, y: 0 }, data: { time: '09:15' } }],
  edges: [],
})

function renderIndex() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } },
  })
  return render(
    <MemoryRouter initialEntries={['/flow']}>
      <QueryClientProvider client={client}>
        <Routes>
          <Route path="/flow" element={<FlowIndex />} />
          <Route path="/flow/editor/:id" element={<p>editor page</p>} />
        </Routes>
      </QueryClientProvider>
    </MemoryRouter>
  )
}

async function importJson(json: string) {
  const user = userEvent.setup()
  await user.click(await screen.findByRole('button', { name: /Import/ }))
  const dialog = await screen.findByRole('dialog')
  const box = within(dialog).getByPlaceholderText(
    '{"name": "My Workflow", "nodes": [...], "edges": [...]}'
  )
  await user.click(box)
  await user.paste(json)
  await user.click(within(dialog).getByRole('button', { name: /^Import$/ }))
}

beforeEach(() => {
  for (const fn of Object.values(toast)) fn.mockReset()
  api.importWorkflow.mockReset()
})

describe('an import with notes', () => {
  it('shows the Eastern-time note and the upgraded fields, as a warning', async () => {
    api.importWorkflow.mockResolvedValue({
      id: 9,
      name: 'Opening bell',
      notes: [NOTE],
      migrations: ['one', 'two'],
    })
    renderIndex()
    await importJson(WEB_WORKFLOW)
    expect(await screen.findByText('editor page')).toBeInTheDocument()
    expect(toast.warning).toHaveBeenCalledTimes(1)
    const [message] = toast.warning.mock.calls[0]
    expect(message).toContain('Workflow "Opening bell" imported.')
    expect(message).toContain(NOTE)
    expect(message).toContain('2 legacy field(s) upgraded.')
    expect(toast.success).not.toHaveBeenCalled()
  })

  it('stays a plain success when the reply has no notes', async () => {
    api.importWorkflow.mockResolvedValue({ id: 9, name: 'Opening bell' })
    renderIndex()
    await importJson(WEB_WORKFLOW)
    expect(await screen.findByText('editor page')).toBeInTheDocument()
    expect(toast.success).toHaveBeenCalledWith('Workflow "Opening bell" imported', 'flow')
    expect(toast.warning).not.toHaveBeenCalled()
  })
})
