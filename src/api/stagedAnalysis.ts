/**
 * Staged "Analyze with Claude" (WP3): the full-pipeline command and its
 * progress events.
 *
 * Thin wrappers over the `staged_run_analysis` Tauri command and the
 * `staged-analysis-progress` event. The types mirror
 * `src-tauri/src/services/staged_analysis/mod.rs`.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type StageKey =
  | 'dossier'
  | 'analysts'
  | 'bull'
  | 'bear'
  | 'research_manager'
  | 'risk_panel'
  | 'portfolio_manager'
  | 'checks'
  | 'render'

/** A stage's state in the dialog; the backend reports the last three. */
export type StageStatus = 'pending' | 'running' | 'done' | 'failed'

/** One `staged-analysis-progress` event. */
export interface StageProgress {
  scenario_id: number
  stage: StageKey
  status: Exclude<StageStatus, 'pending'>
  detail: string | null
}

/** One row of the stage list. */
export interface StageRow {
  key: StageKey
  label: string
  status: StageStatus
  detail?: string | null
}

/** The command's result: the shape `copilot_send_message` returns. */
export interface StagedAnalysisResponse {
  response_text: string
  tool_calls_made: Array<{ tool_name: string; tool_use_id: string; success: boolean; summary: string }>
  conversation: Array<{ role: 'user' | 'assistant'; content: string }>
}

export const PROGRESS_EVENT = 'staged-analysis-progress'

/** The nine stages, in the order the backend runs them. */
export const STAGE_LABELS: ReadonlyArray<{ key: StageKey; label: string }> = [
  { key: 'dossier', label: 'Dossier' },
  { key: 'analysts', label: 'Analysts' },
  { key: 'bull', label: 'Bull case' },
  { key: 'bear', label: 'Bear case' },
  { key: 'research_manager', label: 'Research manager' },
  { key: 'risk_panel', label: 'Risk panel' },
  { key: 'portfolio_manager', label: 'Portfolio manager' },
  { key: 'checks', label: 'Checks' },
  { key: 'render', label: 'Report' },
]

/** Every stage pending, for the start of a run. */
export function initialStages(): StageRow[] {
  return STAGE_LABELS.map((stage) => ({ key: stage.key, label: stage.label, status: 'pending' }))
}

/** The stage list after one progress event; other stages are untouched. */
export function applyProgress(stages: StageRow[], progress: StageProgress): StageRow[] {
  return stages.map((stage) =>
    stage.key === progress.stage ? { ...stage, status: progress.status, detail: progress.detail } : stage
  )
}

function message(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

/**
 * Run the full staged analysis of a scenario. `focus` (the dialog's input
 * text) is handed to the portfolio manager. About nine Claude calls; the
 * backend gives up after ten minutes and names the stage.
 */
export async function runStagedAnalysis(scenarioId: number, focus?: string): Promise<StagedAnalysisResponse> {
  const trimmed = focus?.trim()
  try {
    return await invoke<StagedAnalysisResponse>('staged_run_analysis', {
      scenarioId,
      focus: trimmed ? trimmed : null,
    })
  } catch (error) {
    throw new Error(message(error))
  }
}

/** Progress events for one scenario's run; call the returned function to stop listening. */
export function onStagedProgress(
  scenarioId: number,
  callback: (progress: StageProgress) => void
): Promise<UnlistenFn> {
  return listen<StageProgress>(PROGRESS_EVENT, (event) => {
    if (event.payload.scenario_id === scenarioId) callback(event.payload)
  })
}
