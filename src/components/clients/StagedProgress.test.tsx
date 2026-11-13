// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { applyProgress, initialStages, STAGE_LABELS } from '@/api/stagedAnalysis'
import { StagedProgress } from '@/components/clients/StagedProgress'

describe('StagedProgress', () => {
  it('renders the nine stages in order with the icon for each status', () => {
    let stages = initialStages()
    stages = applyProgress(stages, {
      scenario_id: 12,
      stage: 'dossier',
      status: 'done',
      detail: '5 holdings, 5 priced',
    })
    stages = applyProgress(stages, { scenario_id: 12, stage: 'analysts', status: 'running', detail: null })
    stages = applyProgress(stages, {
      scenario_id: 12,
      stage: 'bull',
      status: 'failed',
      detail: 'Claude declined to write this report.',
    })

    render(<StagedProgress stages={stages} />)

    const list = screen.getByRole('list', { name: 'Full analysis progress' })
    const rows = within(list).getAllByRole('listitem')
    expect(rows).toHaveLength(9)
    expect(rows.map((row) => row.getAttribute('data-stage'))).toEqual(STAGE_LABELS.map((s) => s.key))
    for (const [i, row] of rows.entries()) {
      expect(within(row).getByText(STAGE_LABELS[i].label)).toBeInTheDocument()
    }
    expect(STAGE_LABELS.map((s) => s.label)).toEqual([
      'Dossier',
      'Analysts',
      'Bull case',
      'Bear case',
      'Research manager',
      'Risk panel',
      'Portfolio manager',
      'Checks',
      'Report',
    ])

    expect(rows[0]).toHaveAttribute('data-status', 'done')
    expect(within(rows[0]).getByTestId('stage-icon-done')).toBeInTheDocument()
    expect(within(rows[0]).getByText('5 holdings, 5 priced')).toBeInTheDocument()

    expect(rows[1]).toHaveAttribute('data-status', 'running')
    expect(within(rows[1]).getByTestId('stage-icon-running')).toBeInTheDocument()

    expect(rows[2]).toHaveAttribute('data-status', 'failed')
    expect(within(rows[2]).getByTestId('stage-icon-failed')).toBeInTheDocument()
    expect(within(rows[2]).getByText('Claude declined to write this report.')).toBeInTheDocument()

    for (const row of rows.slice(3)) {
      expect(row).toHaveAttribute('data-status', 'pending')
      expect(within(row).getByTestId('stage-icon-pending')).toBeInTheDocument()
    }
  })

  it('applyProgress changes only the named stage', () => {
    const before = initialStages()
    const after = applyProgress(before, { scenario_id: 12, stage: 'checks', status: 'done', detail: '3 kept, 1 dropped' })
    expect(after.find((s) => s.key === 'checks')).toEqual({
      key: 'checks',
      label: 'Checks',
      status: 'done',
      detail: '3 kept, 1 dropped',
    })
    expect(after.filter((s) => s.key !== 'checks')).toEqual(before.filter((s) => s.key !== 'checks'))
    expect(before.every((s) => s.status === 'pending')).toBe(true)
  })
})
