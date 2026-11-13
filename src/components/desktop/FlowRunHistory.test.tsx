// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
/**
 * WP8 Phase D (coordinator decision 6a): a workflow's run history, with
 * times in Eastern 12-hour time, each run's log, the loading, empty and
 * error states, the refresh while open, and the selection system of
 * CLAUDE.md's TABLE INTERACTION DESIGN SPEC with Copy, Move (disabled) and
 * Delete (always asked first).
 */

import { focusManager, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { WorkflowExecution } from '@/api/flow'

const api = vi.hoisted(() => ({
  getWorkflowExecutions: vi.fn(),
  deleteWorkflowExecutions: vi.fn(),
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

const { FlowRunHistory } = await import('./FlowRunHistory')

/** Newest first, as the server answers: failed, completed, still running. */
const RUNS: WorkflowExecution[] = [
  {
    id: 33,
    workflow_id: 7,
    status: 'failed',
    started_at: '2026-11-12T19:35:02',
    completed_at: '2026-11-12T19:35:05',
    error: 'placeOrder: Order rejected: insufficient funds',
    logs: [
      { time: '2026-11-12T14:35:02', message: 'Starting workflow: ORB', level: 'info' },
      {
        time: '2026-11-12T14:35:05',
        message: 'Order rejected: insufficient funds',
        level: 'error',
      },
    ],
  },
  {
    id: 32,
    workflow_id: 7,
    status: 'completed',
    started_at: '2026-11-12T19:34:02',
    completed_at: '2026-11-12T19:35:07',
    error: null,
    logs: [
      { time: '2026-11-12T14:34:02', message: 'Starting workflow: ORB', level: 'info' },
      { time: '2026-11-12T14:35:07', message: 'Workflow executed successfully', level: 'info' },
    ],
  },
  {
    id: 31,
    workflow_id: 7,
    status: 'running',
    started_at: '2026-11-12T19:33:02',
    completed_at: null,
    error: null,
    logs: [],
  },
]

const onOpenChange = vi.fn()
const onRunNow = vi.fn()

function renderHistory() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 }, mutations: { retry: false } },
  })
  return render(
    <QueryClientProvider client={client}>
      <FlowRunHistory
        workflowId={7}
        workflowName="Opening range breakout"
        open
        onOpenChange={onOpenChange}
        onRunNow={onRunNow}
      />
    </QueryClientProvider>
  )
}

/** The row of the run that started at `started` (as the row shows it). */
async function row(started: string) {
  const cell = await screen.findByText(started)
  const r = cell.closest('[data-selectable="true"]')
  if (!r) throw new Error(`no row for ${started}`)
  return r as HTMLElement
}

/** The action bar's count, or null while nothing is selected. */
function barCount(): string | null {
  const bar = document.querySelector('[data-action-bar="true"]')
  return bar?.querySelector('span')?.textContent ?? null
}

const FAILED = 'Nov 12, 2:35:02 PM ET'
const COMPLETED = 'Nov 12, 2:34:02 PM ET'
const RUNNING = 'Nov 12, 2:33:02 PM ET'

beforeEach(() => {
  for (const f of [...Object.values(api), ...Object.values(toast)]) f.mockReset()
  onOpenChange.mockReset()
  onRunNow.mockReset()
  api.getWorkflowExecutions.mockResolvedValue(RUNS)
  api.deleteWorkflowExecutions.mockResolvedValue({ status: 'success', deleted: 2 })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('the runs', () => {
  it('lists each run with its Eastern start, status, duration and error', async () => {
    renderHistory()
    const failed = await row(FAILED)
    expect(within(failed).getByText('Failed')).toBeInTheDocument()
    expect(within(failed).getByText('3s')).toBeInTheDocument()
    expect(
      within(failed).getByText('placeOrder: Order rejected: insufficient funds')
    ).toBeInTheDocument()
    const completed = await row(COMPLETED)
    expect(within(completed).getByText('Completed')).toBeInTheDocument()
    expect(within(completed).getByText('1m 5s')).toBeInTheDocument()
    const running = await row(RUNNING)
    expect(within(running).getByText('Running')).toBeInTheDocument()
    expect(api.getWorkflowExecutions).toHaveBeenCalledWith(7, 200)
    expect(screen.getByText('Opening range breakout')).toBeInTheDocument()
  })

  it('opens a run by double-click, its Open button or Enter, with its log in Eastern time', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.dblClick(await row(FAILED))
    expect(await screen.findByText('2:35:05 PM ET')).toBeInTheDocument()
    expect(screen.getByText('Order rejected: insufficient funds')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Back to runs' }))
    const completed = await row(COMPLETED)
    await user.click(
      within(completed).getByRole('button', { name: `Open the run of ${COMPLETED}` })
    )
    expect(await screen.findByText('Workflow executed successfully')).toBeInTheDocument()
    expect(screen.getByText('2:35:07 PM ET')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Back to runs' }))
    const grid = await screen.findByRole('grid')
    grid.focus()
    await user.keyboard('{ArrowDown}{Enter}')
    expect(await screen.findByText('Starting workflow: ORB')).toBeInTheDocument()
  })

  it('shows a skeleton while the runs load', async () => {
    api.getWorkflowExecutions.mockReturnValue(new Promise(() => {}))
    renderHistory()
    expect(await screen.findByLabelText('Loading runs')).toBeInTheDocument()
  })

  it('says No runs yet, with a Run Now button', async () => {
    api.getWorkflowExecutions.mockResolvedValue([])
    const user = userEvent.setup()
    renderHistory()
    expect(await screen.findByText('No runs yet')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Run Now' }))
    expect(onRunNow).toHaveBeenCalledTimes(1)
  })

  it('shows an inline error with Retry, and Retry reads the runs again', async () => {
    api.getWorkflowExecutions.mockRejectedValueOnce(new Error('Failed to read the run history'))
    const user = userEvent.setup()
    renderHistory()
    expect(await screen.findByText('Could not load the run history')).toBeInTheDocument()
    expect(screen.getByText('Failed to read the run history')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await row(FAILED)).toBeInTheDocument()
  })

  it('reads the runs again when the window gets focus and every 10 seconds', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    renderHistory()
    await row(FAILED)
    expect(api.getWorkflowExecutions).toHaveBeenCalledTimes(1)
    act(() => {
      focusManager.setFocused(false)
      focusManager.setFocused(true)
    })
    await waitFor(() => expect(api.getWorkflowExecutions).toHaveBeenCalledTimes(2))
    await act(async () => {
      vi.advanceTimersByTime(10_000)
    })
    await waitFor(() => expect(api.getWorkflowExecutions).toHaveBeenCalledTimes(3))
  })
})

describe('the selection', () => {
  it('selects by a click anywhere on a row, Shift ranges and Cmd toggles, with the attributes', async () => {
    const user = userEvent.setup()
    renderHistory()
    const failed = await row(FAILED)
    expect(failed).toHaveAttribute('data-item-id', '33')
    await user.click(within(failed).getByText('3s'))
    expect(failed).toHaveAttribute('aria-selected', 'true')
    expect(failed).toHaveAttribute('data-selected', 'true')
    expect(within(failed).getByRole('checkbox')).toBeChecked()
    await user.keyboard('{Shift>}')
    await user.click(await row(RUNNING))
    await user.keyboard('{/Shift}')
    expect(barCount()).toBe('3 runs selected')
    await user.keyboard('{Meta>}')
    await user.click(await row(COMPLETED))
    await user.keyboard('{/Meta}')
    expect(await row(COMPLETED)).toHaveAttribute('aria-selected', 'false')
    expect(barCount()).toBe('2 runs selected')
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe('2 runs selected')
  })

  it('selects all from the header checkbox, which is indeterminate for some', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    const all = screen.getByRole('checkbox', { name: 'Select all runs' })
    expect(all).toHaveAttribute('data-select-all', 'true')
    expect((all as HTMLInputElement).indeterminate).toBe(true)
    await user.click(all)
    expect(barCount()).toBe('3 runs selected')
    expect((all as HTMLInputElement).checked).toBe(true)
    await user.click(screen.getByRole('button', { name: 'Clear selection' }))
    expect(barCount()).toBeNull()
  })

  // The review's finding: a key typed on a control inside the table (a
  // checkbox, an Open button) is that control's, not the focused row's.
  it("keys on the table's own controls act on those controls", async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    const all = screen.getByRole('checkbox', { name: 'Select all runs' })
    all.focus()
    await user.keyboard(' ')
    expect(barCount()).toBe('3 runs selected')
    within(await row(RUNNING))
      .getByRole('button', { name: `Open the run of ${RUNNING}` })
      .focus()
    await user.keyboard('{Enter}')
    expect(await screen.findByText('This run has no log lines.')).toBeInTheDocument()
  })

  it('Escape clears the selection first, and closes the window when nothing is selected', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    await user.keyboard('{Escape}')
    expect(barCount()).toBeNull()
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
    await user.keyboard('{Escape}')
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('copies the chosen runs as text: Eastern time, status, duration, error and log lines', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    await user.keyboard('{Shift>}')
    await user.click(await row(COMPLETED))
    await user.keyboard('{/Shift}')
    const bar = document.querySelector('[data-action-bar="true"]') as HTMLElement
    await user.click(within(bar).getByRole('button', { name: 'Copy' }))
    // userEvent.setup() gives the page a clipboard to read back.
    expect(await navigator.clipboard.readText()).toBe(
      [
        'Nov 12, 2:35:02 PM ET · Failed · 3s',
        'Error: placeOrder: Order rejected: insufficient funds',
        '2:35:02 PM ET  info  Starting workflow: ORB',
        '2:35:05 PM ET  error  Order rejected: insufficient funds',
        '',
        'Nov 12, 2:34:02 PM ET · Completed · 1m 5s',
        '2:34:02 PM ET  info  Starting workflow: ORB',
        '2:35:07 PM ET  info  Workflow executed successfully',
      ].join('\n')
    )
    expect(toast.success).toHaveBeenCalledWith('2 runs copied', 'clipboard')
  })

  it('shows Move disabled, with the reason', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    const bar = document.querySelector('[data-action-bar="true"]') as HTMLElement
    const move = within(bar).getByRole('button', { name: 'Move' })
    expect(move).toBeDisabled()
    expect(move).toHaveAccessibleDescription('Runs stay with their workflow')
    // Delete is the rightmost action.
    const buttons = within(bar).getAllByRole('button')
    expect(buttons[buttons.length - 1]).toHaveTextContent('Delete')
  })

  it('deletes the chosen runs after asking, whatever the count, in one call', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    const bar = document.querySelector('[data-action-bar="true"]') as HTMLElement
    await user.click(within(bar).getByRole('button', { name: 'Delete' }))
    let dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByRole('heading', { name: 'Delete 1 run?' })).toBeInTheDocument()
    expect(within(dialog).getByText("This can't be undone.")).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    expect(api.deleteWorkflowExecutions).not.toHaveBeenCalled()

    await user.keyboard('{Shift>}')
    await user.click(await row(COMPLETED))
    await user.keyboard('{/Shift}')
    await user.click(within(bar).getByRole('button', { name: 'Delete' }))
    dialog = await screen.findByRole('alertdialog')
    expect(within(dialog).getByRole('heading', { name: 'Delete 2 runs?' })).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))
    await waitFor(() => expect(api.deleteWorkflowExecutions).toHaveBeenCalledWith(7, [33, 32]))
    expect(api.deleteWorkflowExecutions).toHaveBeenCalledTimes(1)
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('2 runs deleted', 'flow'))
    await waitFor(() => expect(api.getWorkflowExecutions).toHaveBeenCalledTimes(2))
  })

  it("names the server's refusal when a run is still going", async () => {
    api.deleteWorkflowExecutions.mockRejectedValue(
      new Error('Run 31 is still going, so nothing was deleted. Delete it once it has ended.')
    )
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(RUNNING))
    await user.click(screen.getByRole('button', { name: 'Delete' }))
    const dialog = await screen.findByRole('alertdialog')
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        'Run 31 is still going, so nothing was deleted. Delete it once it has ended.',
        'flow'
      )
    )
  })

  it('opens the right-click menu on a row: an unselected row becomes the selection', async () => {
    const user = userEvent.setup()
    renderHistory()
    await user.click(await row(FAILED))
    fireEvent.contextMenu(await row(COMPLETED), { clientX: 200, clientY: 300 })
    expect(await row(COMPLETED)).toHaveAttribute('aria-selected', 'true')
    expect(await row(FAILED)).toHaveAttribute('aria-selected', 'false')
    const menu = await screen.findByRole('menu', { name: 'Run actions' })
    const items = within(menu)
      .getAllByRole('menuitem')
      .map((i) => i.textContent)
    expect(items).toEqual([
      'Copy',
      'Move to…Runs stay with their workflow',
      'Delete',
      'Select All',
      'Deselect All',
      'Open',
      "DuplicateRuns can't be duplicated",
    ])
    expect(within(menu).getByRole('menuitem', { name: /Duplicate/ })).toBeDisabled()
    await user.click(within(menu).getByRole('menuitem', { name: 'Select All' }))
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(barCount()).toBe('3 runs selected')
    fireEvent.contextMenu(await row(FAILED), { clientX: 200, clientY: 300 })
    expect(barCount()).toBe('3 runs selected')
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(barCount()).toBe('3 runs selected')
  })
})
