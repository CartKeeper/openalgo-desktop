// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase D (coordinator decision 6b): the Flow page's workflow cards
 * carry the selection system of CLAUDE.md's TABLE INTERACTION DESIGN SPEC.
 * A click selects (it used to open the editor, which the spec bans);
 * double-click, Enter or the card's Open button opens. The card's own
 * buttons never select. Copy puts the export JSON on the clipboard, Move
 * is disabled with its reason, Delete asks first, Duplicate makes an
 * inactive copy.
 */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes, useParams } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Workflow, WorkflowExportData, WorkflowListItem } from '@/api/flow'

const api = vi.hoisted(() => ({
  listWorkflows: vi.fn(),
  getWorkflow: vi.fn(),
  createWorkflow: vi.fn(),
  deleteWorkflow: vi.fn(),
  exportWorkflow: vi.fn(),
  getWebhookInfo: vi.fn(),
}))
vi.mock('@/api/flow', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/flow')>()
  return { ...actual, ...api }
})

vi.mock('@/api/tauri-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api/tauri-client')>()
  return {
    ...actual,
    settingsCommands: {
      ...actual.settingsCommands,
      getAnalyzeMode: async () => ({ analyze_mode: true, mode: 'analyzer' }),
    },
  }
})

const toast = vi.hoisted(() => ({
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
}))
vi.mock('@/utils/toast', () => ({ showToast: toast }))

const { default: FlowIndex } = await import('./FlowIndex')

const item = (id: number, name: string, active = false): WorkflowListItem => ({
  id,
  name,
  description: null,
  is_active: active,
  created_at: '2026-11-12T14:00:00',
  updated_at: '2026-11-12T14:00:00',
  last_execution_status: null,
})

const ORB = item(7, 'Opening range breakout', true)
const GAP = item(8, 'Gap fill')
const DIP = item(9, 'Buy the dip')

const exported = (w: WorkflowListItem): WorkflowExportData => ({
  version: '1.0',
  name: w.name,
  description: null,
  nodes: [{ id: 'node_1', type: 'start', position: { x: 0, y: 0 }, data: {} }],
  edges: [],
  exported_at: '2026-11-12T19:35:02',
})

function Editor() {
  const { id } = useParams()
  return <p>editor for {id}</p>
}

function renderIndex() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } },
  })
  return render(
    <MemoryRouter initialEntries={['/flow']}>
      <QueryClientProvider client={client}>
        <Routes>
          <Route path="/flow" element={<FlowIndex />} />
          <Route path="/flow/editor/:id" element={<Editor />} />
        </Routes>
      </QueryClientProvider>
    </MemoryRouter>
  )
}

const card = (name: string) => screen.findByRole('row', { name: `Workflow ${name}` })

/** The action bar's count, or null while nothing is selected. */
function barCount(): string | null {
  const bar = document.querySelector('[data-action-bar="true"]')
  return bar?.querySelector('span')?.textContent ?? null
}

beforeEach(() => {
  for (const f of [...Object.values(api), ...Object.values(toast)]) f.mockReset()
  api.listWorkflows.mockResolvedValue([ORB, GAP, DIP])
  api.exportWorkflow.mockImplementation(async (id: number) =>
    exported([ORB, GAP, DIP].find((w) => w.id === id) ?? ORB)
  )
  api.deleteWorkflow.mockResolvedValue({ status: 'success', message: 'Workflow deleted' })
})

describe('a click on a card', () => {
  it('selects it, with its checkbox and the attributes, and opens nothing', async () => {
    const user = userEvent.setup()
    renderIndex()
    const orb = await card(ORB.name)
    await user.click(within(orb).getByText(ORB.name))
    expect(orb).toHaveAttribute('aria-selected', 'true')
    expect(orb).toHaveAttribute('data-selected', 'true')
    expect(orb).toHaveAttribute('data-selectable', 'true')
    expect(orb).toHaveAttribute('data-item-id', '7')
    expect(within(orb).getByRole('checkbox', { name: `Select workflow ${ORB.name}` })).toBeChecked()
    expect(screen.queryByText(/editor for/)).not.toBeInTheDocument()
    expect(barCount()).toBe('1 workflow selected')
  })

  it("on the card's own buttons never selects", async () => {
    const user = userEvent.setup()
    renderIndex()
    const orb = await card(ORB.name)
    await user.click(within(orb).getByRole('button', { name: 'Workflow actions' }))
    await user.keyboard('{Escape}')
    expect(orb).toHaveAttribute('aria-selected', 'false')
    await user.click(within(orb).getByRole('button', { name: `Open workflow ${ORB.name}` }))
    expect(await screen.findByText('editor for 7')).toBeInTheDocument()
  })
})

describe('opening a workflow', () => {
  it('by double-click', async () => {
    const user = userEvent.setup()
    renderIndex()
    await user.dblClick(await card(GAP.name))
    expect(await screen.findByText('editor for 8')).toBeInTheDocument()
  })

  it('by Enter on the focused card', async () => {
    const user = userEvent.setup()
    renderIndex()
    await card(ORB.name)
    const grid = screen.getByRole('grid', { name: 'Workflows' })
    grid.focus()
    await user.keyboard('{ArrowDown}{ArrowDown}{Enter}')
    expect(await screen.findByText('editor for 8')).toBeInTheDocument()
  })
})

describe('selecting several', () => {
  it('Shift+click ranges, Cmd+click toggles, select all is indeterminate for some', async () => {
    const user = userEvent.setup()
    renderIndex()
    await user.click(await card(ORB.name))
    await user.keyboard('{Shift>}')
    await user.click(await card(DIP.name))
    await user.keyboard('{/Shift}')
    expect(barCount()).toBe('3 workflows selected')
    await user.keyboard('{Meta>}')
    await user.click(await card(GAP.name))
    await user.keyboard('{/Meta}')
    expect(barCount()).toBe('2 workflows selected')
    const all = screen.getByRole('checkbox', { name: 'Select all workflows' })
    expect(all).toHaveAttribute('data-select-all', 'true')
    expect((all as HTMLInputElement).indeterminate).toBe(true)
    await user.click(all)
    expect(barCount()).toBe('3 workflows selected')
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe('3 workflows selected')
  })

  it('Cmd+A selects all and Escape clears, on the list', async () => {
    const user = userEvent.setup()
    renderIndex()
    await card(ORB.name)
    screen.getByRole('grid', { name: 'Workflows' }).focus()
    await user.keyboard('{Meta>}a{/Meta}')
    expect(barCount()).toBe('3 workflows selected')
    await user.keyboard('{Escape}')
    expect(barCount()).toBeNull()
  })
})

describe('the actions', () => {
  it('Copy puts the export JSON on the clipboard: one workflow as an object, several as an array', async () => {
    const user = userEvent.setup()
    renderIndex()
    await user.click(await card(GAP.name))
    const bar = () => document.querySelector('[data-action-bar="true"]') as HTMLElement
    await user.click(within(bar()).getByRole('button', { name: 'Copy' }))
    await waitFor(async () =>
      expect(await navigator.clipboard.readText()).toBe(JSON.stringify(exported(GAP), null, 2))
    )
    await user.keyboard('{Shift>}')
    await user.click(await card(DIP.name))
    await user.keyboard('{/Shift}')
    await user.click(within(bar()).getByRole('button', { name: 'Copy' }))
    await waitFor(async () =>
      expect(await navigator.clipboard.readText()).toBe(
        JSON.stringify([exported(GAP), exported(DIP)], null, 2)
      )
    )
    expect(toast.success).toHaveBeenCalledWith('2 workflows copied', 'clipboard')
  })

  it('shows Move disabled with the reason, and Delete rightmost', async () => {
    const user = userEvent.setup()
    renderIndex()
    await user.click(await card(GAP.name))
    const bar = document.querySelector('[data-action-bar="true"]') as HTMLElement
    const move = within(bar).getByRole('button', { name: 'Move' })
    expect(move).toBeDisabled()
    expect(move).toHaveAccessibleDescription('Workflows have no folders')
    const buttons = within(bar).getAllByRole('button')
    expect(buttons[buttons.length - 1]).toHaveTextContent('Delete')
  })

  it('Delete asks first, then deletes each chosen workflow, with one toast', async () => {
    const user = userEvent.setup()
    renderIndex()
    await user.click(await card(ORB.name))
    await user.keyboard('{Shift>}')
    await user.click(await card(GAP.name))
    await user.keyboard('{/Shift}')
    const bar = document.querySelector('[data-action-bar="true"]') as HTMLElement
    await user.click(within(bar).getByRole('button', { name: 'Delete' }))
    const dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByRole('heading', { name: 'Delete 2 workflows?' })).toBeInTheDocument()
    expect(within(dialog).getByText(/This can't be undone\./)).toBeInTheDocument()
    expect(api.deleteWorkflow).not.toHaveBeenCalled()
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))
    await waitFor(() => expect(api.deleteWorkflow).toHaveBeenCalledTimes(2))
    expect(api.deleteWorkflow.mock.calls.map((c) => c[0])).toEqual([7, 8])
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('2 workflows deleted', 'flow'))
    expect(toast.success).toHaveBeenCalledTimes(1)
  })

  it('a delete the server refuses is named in the one toast', async () => {
    api.deleteWorkflow
      .mockResolvedValueOnce({ status: 'success', message: 'Workflow deleted' })
      .mockRejectedValueOnce(
        new Error('This workflow is still running. Try again when it finishes.')
      )
    const user = userEvent.setup()
    renderIndex()
    await user.click(await card(ORB.name))
    await user.keyboard('{Shift>}')
    await user.click(await card(GAP.name))
    await user.keyboard('{/Shift}')
    await user.click(screen.getByRole('button', { name: 'Delete' }))
    const dialog = await screen.findByRole('alertdialog')
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        '1 workflow deleted; 1 could not be: Gap fill: This workflow is still running. Try again when it finishes.',
        'flow'
      )
    )
  })

  it('the right-click menu offers Open and Duplicate, which makes an inactive copy', async () => {
    const stored: Workflow = {
      id: 8,
      name: GAP.name,
      description: 'Fill the gap',
      nodes: exported(GAP).nodes,
      edges: [],
      is_active: true,
      schedule_job_id: null,
      webhook_token: 'tok',
      webhook_secret: 'sec',
      webhook_enabled: true,
      webhook_auth_type: 'payload',
      created_at: '2026-11-12T14:00:00',
      updated_at: '2026-11-12T14:00:00',
    }
    api.getWorkflow.mockResolvedValue(stored)
    api.createWorkflow.mockResolvedValue({ ...stored, id: 10, name: 'Gap fill (copy)' })
    const user = userEvent.setup()
    renderIndex()
    fireEvent.contextMenu(await card(GAP.name), { clientX: 100, clientY: 100 })
    const menu = await screen.findByRole('menu', { name: 'Selected workflows' })
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((i) => i.textContent)
    ).toEqual([
      'Copy',
      'Move to…Workflows have no folders',
      'Delete',
      'Select All',
      'Deselect All',
      'Open',
      'Duplicate',
    ])
    await user.click(within(menu).getByRole('menuitem', { name: 'Duplicate' }))
    await waitFor(() =>
      expect(api.createWorkflow).toHaveBeenCalledWith({
        name: 'Gap fill (copy)',
        description: 'Fill the gap',
        nodes: stored.nodes,
        edges: stored.edges,
      })
    )
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith('Workflow duplicated: Gap fill (copy)', 'flow')
    )
    fireEvent.contextMenu(await card(DIP.name), { clientX: 100, clientY: 100 })
    const again = await screen.findByRole('menu', { name: 'Selected workflows' })
    await user.click(within(again).getByRole('menuitem', { name: 'Open' }))
    expect(await screen.findByText('editor for 9')).toBeInTheDocument()
  })
})
