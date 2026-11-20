/**
 * Per-symbol chart preferences (saved drawings and indicators) — WP1.
 *
 * Thin invoke wrappers over the `chart_get_preference` / `chart_save_preference`
 * Tauri commands. Rows are keyed by `exchange` + `symbol` in SQLite.
 */
import { invoke } from '@tauri-apps/api/core'

export interface ChartPreference {
  symbol: string
  exchange: string
  timeframe: string
  chart_type: string
  /** JSON of the active indicator list (see `activeIndicators.ts`), or null. */
  indicators: string | null
  /** `DrawingManager.exportJSON()` output, or null when nothing was saved. */
  drawings: string | null
  updated_at: string
}

export interface ChartPreferenceInput {
  indicators: string | null
  drawings: string | null
}

function message(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw new Error(message(error))
  }
}

/** The saved preference of a symbol, or null when none was saved yet. */
export async function getChartPreference(
  exchange: string,
  symbol: string
): Promise<ChartPreference | null> {
  const pref = await call<ChartPreference | null>('chart_get_preference', { exchange, symbol })
  return pref ?? null
}

/** Upsert the saved indicators and drawings of a symbol. */
export async function saveChartPreference(
  exchange: string,
  symbol: string,
  data: ChartPreferenceInput
): Promise<void> {
  await call<void>('chart_save_preference', {
    exchange,
    symbol,
    indicators: data.indicators,
    drawings: data.drawings,
  })
}
