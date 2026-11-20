// @vitest-environment jsdom
import '@/test/setup'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  runStagedAnalysis: vi.fn(),
  onStagedProgress: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@/api/stagedAnalysis', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/stagedAnalysis')>()),
  runStagedAnalysis: mocks.runStagedAnalysis,
  onStagedProgress: mocks.onStagedProgress,
}))
// The review modal is a heavy component with its own hosts; the dialog only needs it mounted.
vi.mock('@/components/trading/ActionReviewModal', () => ({ ActionReviewModal: () => null }))

import AnalyzeWithClaudeDialog from '@/pages/clients/AnalyzeWithClaudeDialog'

// src/test/setup.ts mocks these with arrow functions, which break under `new`.
class ObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
window.ResizeObserver = ObserverStub as unknown as typeof ResizeObserver
window.IntersectionObserver = ObserverStub as unknown as typeof IntersectionObserver

function renderDialog(open: boolean) {
  return (
    <MemoryRouter>
      <AnalyzeWithClaudeDialog
        open={open}
        onOpenChange={() => {}}
        clientId={1}
        scenarioId={12}
        scenarioName="Casey baseline"
        isBaseline
        accountType="401k"
      />
    </MemoryRouter>
  )
}

describe('AnalyzeWithClaudeDialog — Run full analysis', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
    mocks.runStagedAnalysis.mockReset()
    mocks.onStagedProgress.mockReset()
    // copilot_check_configured
    mocks.invoke.mockResolvedValue(true)
    mocks.onStagedProgress.mockResolvedValue(() => {})
    // A run that is still in flight.
    mocks.runStagedAnalysis.mockReturnValue(new Promise(() => {}))
  })

  it('keeps a running analysis on screen when the dialog is closed and reopened', async () => {
    const user = userEvent.setup()
    const view = render(renderDialog(true))

    await user.click(await screen.findByRole('button', { name: 'Run full analysis' }))
    expect(mocks.runStagedAnalysis).toHaveBeenCalledWith(12, undefined)
    expect(await screen.findByRole('list', { name: 'Full analysis progress' })).toBeInTheDocument()
    expect(screen.getByText('Run a full analysis of this scenario.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Running…' })).toBeDisabled()

    view.rerender(renderDialog(false))
    view.rerender(renderDialog(true))

    expect(await screen.findByRole('list', { name: 'Full analysis progress' })).toBeInTheDocument()
    expect(screen.getByText('Run a full analysis of this scenario.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Running…' })).toBeDisabled()
  })
})
