// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase B: Logout from the Flow editor's header does not throw away
 * unsaved changes without asking. Upstream confirmed every logout with its
 * own dialog, and the editor's leave guard lets the sign-out through for that
 * reason; this app's header logs out at once, so with unsaved changes the
 * editor's own "Leave without saving?" prompt asks first.
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Workflow } from '@/api/flow'
import { useAuthStore } from '@/stores/authStore'
import { useFlowWorkflowStore } from '@/stores/flowWorkflowStore'

const api = vi.hoisted(() => ({
  getWorkflow: vi.fn(),
  getIndexSymbolsLotSizes: vi.fn(async () => []),
  getWebhookInfo: vi.fn(),
}))
vi.mock('@/api/flow', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/flow')>()
  return { ...actual, ...api }
})

const logoutRequest = vi.hoisted(() => vi.fn(async () => undefined))
vi.mock('@/api/auth', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/auth')>()
  return { ...actual, authApi: { ...actual.authApi, logout: logoutRequest } }
})

vi.mock('@/utils/toast', () => ({
  showToast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}))

const { default: FlowEditor } = await import('./FlowEditor')

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
            <Route path="/login" element={<p>login page</p>} />
          </Routes>
        </QueryClientProvider>
      </MemoryRouter>
    </div>
  )
}

async function logOut(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Open user menu' }))
  await user.click(await screen.findByRole('menuitem', { name: 'Logout' }))
}

beforeEach(() => {
  logoutRequest.mockClear()
  api.getWorkflow.mockResolvedValue(STORED)
  useFlowWorkflowStore.getState().resetWorkflow()
  useAuthStore.setState({
    user: {
      username: 'trader',
      broker: 'alpaca',
      brokerId: 'alpaca',
      isLoggedIn: true,
      loginTime: null,
    },
    isAuthenticated: true,
  })
})

afterEach(() => {
  useAuthStore.setState({ user: null, isAuthenticated: false })
})

describe('Logout from the editor', () => {
  it('with unsaved changes, asks first: Stay keeps the session and the edits', async () => {
    renderEditor()
    const user = userEvent.setup()
    await user.type(await screen.findByDisplayValue('Opening range breakout'), ' v2')
    await logOut(user)
    expect(await screen.findByText('Leave without saving?')).toBeInTheDocument()
    expect(logoutRequest).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Stay' }))
    expect(useAuthStore.getState().isAuthenticated).toBe(true)
    expect(screen.getByDisplayValue('Opening range breakout v2')).toBeInTheDocument()
  })

  it('with unsaved changes, Leave without saving logs out and opens the login page', async () => {
    renderEditor()
    const user = userEvent.setup()
    await user.type(await screen.findByDisplayValue('Opening range breakout'), ' v2')
    await logOut(user)
    await user.click(await screen.findByRole('button', { name: 'Leave without saving' }))
    expect(await screen.findByText('login page')).toBeInTheDocument()
    expect(logoutRequest).toHaveBeenCalledTimes(1)
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
  })

  it('with nothing unsaved, logs out at once', async () => {
    renderEditor()
    const user = userEvent.setup()
    await screen.findByDisplayValue('Opening range breakout')
    await logOut(user)
    expect(await screen.findByText('login page')).toBeInTheDocument()
    expect(screen.queryByText('Leave without saving?')).not.toBeInTheDocument()
    expect(logoutRequest).toHaveBeenCalledTimes(1)
  })
})
