// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useActionQueueStore } from '@/stores/actionQueueStore'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  reconcile: vi.fn(),
  modalProps: [] as Array<{ onApply?: unknown; cloneNameRequired?: boolean }>,
  pickerProps: [] as Array<{ open: boolean; onPick: (id: number) => Promise<void> }>,
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@/lib/affordability', () => ({ reconcileRecommendationsWithAccount: mocks.reconcile }))
vi.mock('@/components/trading/ActionReviewModal', () => ({
  ActionReviewModal: (props: { onApply?: unknown; cloneNameRequired?: boolean }) => {
    mocks.modalProps.push(props)
    return null
  },
}))
vi.mock('@/components/reports/ScenarioPickerDialog', () => ({
  ScenarioPickerDialog: (props: { open: boolean; onPick: (id: number) => Promise<void> }) => {
    mocks.pickerProps.push(props)
    return null
  },
}))
vi.mock('@/components/reports/ReportBuilderDialog', () => ({ ReportBuilderDialog: () => null }))

import ViewReport from './ViewReport'

const actionsMessage = {
  id: '2',
  role: 'assistant',
  content:
    'Sell the leveraged ETF.\n\n<!-- ACTIONS_JSON\n' +
    JSON.stringify([
      { symbol: 'TQQQ', exchange: 'GENERIC', side: 'SELL', quantity: 3, orderType: 'MARKET', product: 'CNC', price: 0, rationale: 'r' },
      { symbol: 'VOO', exchange: 'GENERIC', side: 'BUY', quantity: 9, orderType: 'MARKET', product: 'CNC', price: 0, rationale: 'r' },
    ]) +
    '\n-->\n',
  timestamp: 1,
}

function report(overrides: Record<string, unknown>) {
  return {
    id: 1,
    title: 'Scenario Analysis — Current Portfolio — 401(k) (Nov 11, 2026)',
    summary: null,
    tags: JSON.stringify(['Scenario Analysis', 'Get Client Scenario']),
    messages_json: JSON.stringify([actionsMessage]),
    tool_calls_json: null,
    scenario_id: null,
    created_at: '2026-11-11 17:00:00',
    updated_at: '2026-11-11 17:00:00',
    ...overrides,
  }
}

const scenario = {
  id: 5,
  client_id: 3,
  name: 'Current Portfolio — 401(k)',
  description: null,
  is_baseline: true,
  cloned_from_id: null,
  account_type: '401k',
  created_at: null,
  updated_at: null,
}

function backend(rep: ReturnType<typeof report>) {
  mocks.invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case 'get_research_report':
        return rep
      case 'get_report_notes':
        return []
      case 'get_client_scenario':
        return scenario
      case 'get_scenario_positions':
        return [{ symbol: 'TQQQ', side: 'long', quantity: 3 }]
      case 'link_research_report_scenario':
        return { ...rep, scenario_id: args?.scenarioId }
      default:
        throw new Error(`unexpected ${cmd}`)
    }
  })
}

function renderPage() {
  render(
    <MemoryRouter initialEntries={['/reports/1']}>
      <Routes>
        <Route path="/reports/:id" element={<ViewReport />} />
      </Routes>
    </MemoryRouter>
  )
}

const lastModal = () => mocks.modalProps[mocks.modalProps.length - 1]
const lastPicker = () => mocks.pickerProps[mocks.pickerProps.length - 1]

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.reconcile.mockReset()
  mocks.modalProps.length = 0
  mocks.pickerProps.length = 0
  useActionQueueStore.getState().close()
})

describe('ViewReport Review Actions', () => {
  it("applies a linked scenario report's trades to its sandbox, with no live funds check", async () => {
    backend(report({ scenario_id: 5 }))
    renderPage()
    await userEvent.click(await screen.findByRole('button', { name: /Review 2 Actions/ }))

    await waitFor(() => expect(useActionQueueStore.getState().isReviewOpen).toBe(true))
    expect(useActionQueueStore.getState().items.map((i) => i.symbol)).toEqual(['TQQQ', 'VOO'])
    expect(mocks.reconcile).not.toHaveBeenCalled()
    expect(lastModal().onApply).toBeTypeOf('function')
    // A baseline scenario is cloned, so the modal asks for the new scenario's name.
    expect(lastModal().cloneNameRequired).toBe(true)
  })

  it('asks which scenario an old, unlinked scenario report belongs to, then remembers it', async () => {
    backend(report({ scenario_id: null }))
    renderPage()
    await userEvent.click(await screen.findByRole('button', { name: /Review 2 Actions/ }))

    await waitFor(() => expect(lastPicker().open).toBe(true))
    expect(useActionQueueStore.getState().isReviewOpen).toBe(false)
    expect(lastModal().onApply).toBeTypeOf('function')

    await lastPicker().onPick(5)
    expect(mocks.invoke).toHaveBeenCalledWith('link_research_report_scenario', { id: 1, scenarioId: 5 })
    await waitFor(() => expect(useActionQueueStore.getState().isReviewOpen).toBe(true))
    expect(mocks.reconcile).not.toHaveBeenCalled()
  })

  it('keeps the live funds check for reports that are not about a client', async () => {
    backend(report({ tags: JSON.stringify(['Copilot']), scenario_id: null }))
    mocks.reconcile.mockImplementation(async (items: unknown[]) => ({ items, notice: null }))
    renderPage()
    await userEvent.click(await screen.findByRole('button', { name: /Review 2 Actions/ }))

    await waitFor(() => expect(mocks.reconcile).toHaveBeenCalledOnce())
    expect(lastModal().onApply).toBeUndefined()
    expect(mocks.pickerProps).toHaveLength(0)
  })
})
