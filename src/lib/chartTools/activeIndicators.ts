/**
 * The list of indicators active on a chart, as saved in `chart_preferences.indicators`.
 *
 * Pure module: (de)serialization, new entries seeded from the catalog defaults,
 * and the chart context the indicators compute in (Eastern time, US session).
 */
import type { ChartContext } from 'lightweight-charts-indicators'
import type { CatalogEntry } from './indicatorCatalog'

export type IndicatorInputValue = number | boolean | string

export interface ActiveIndicator {
  /** Stable per-instance key (two SMAs with different lengths are two entries). */
  key: string
  /** Registry id (`sma`, `rsi`, ...). */
  id: string
  /** User inputs; anything not set here falls back to the registry defaults. */
  inputs: Record<string, IndicatorInputValue>
}

export const INDICATOR_TIME_ZONE = 'America/New_York'
export const US_REGULAR_SESSION = '0930-1600'

function newKey(): string {
  const c = globalThis.crypto
  if (c && typeof c.randomUUID === 'function') return c.randomUUID()
  return `ind-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
}

function isScalar(v: unknown): v is IndicatorInputValue {
  return typeof v === 'number' || typeof v === 'boolean' || typeof v === 'string'
}

function cleanInputs(raw: unknown): Record<string, IndicatorInputValue> {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {}
  const out: Record<string, IndicatorInputValue> = {}
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (isScalar(v)) out[k] = v
  }
  return out
}

/** A new active entry of a catalog indicator with its default inputs and a fresh key. */
export function newActiveIndicator(entry: CatalogEntry): ActiveIndicator {
  const inputs: Record<string, IndicatorInputValue> = {}
  for (const input of entry.inputs) inputs[input.id] = input.default
  return { key: newKey(), id: entry.id, inputs }
}

export function serializeIndicators(list: ActiveIndicator[]): string {
  return JSON.stringify(list.map(({ key, id, inputs }) => ({ key, id, inputs })))
}

/** Saved JSON -> active list. Invalid JSON, a non-array, or malformed entries are dropped. */
export function parseIndicators(json: string | null): ActiveIndicator[] {
  if (!json) return []
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return []
  }
  if (!Array.isArray(parsed)) return []
  const out: ActiveIndicator[] = []
  for (const item of parsed) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) continue
    const { key, id, inputs } = item as Record<string, unknown>
    if (typeof id !== 'string' || !id) continue
    out.push({
      key: typeof key === 'string' && key ? key : newKey(),
      id,
      inputs: cleanInputs(inputs),
    })
  }
  return out
}

/**
 * The page's interval strings ("5m", "1h", "D", "W", "M", "Q", "Y", "25m", "3MO")
 * as PineScript timeframes ("5", "60", "1D", "1W", "1M", "3M", "12M", "25", "3M").
 * Minutes are lowercase `m`, months uppercase `M` (or `MO`). Unknown strings pass through.
 */
export function timeframeForInterval(interval: string): string {
  const s = (interval ?? '').trim()
  if (!s) return '1D'
  const match = /^(\d*)(m|h|D|W|MO|M|Q|Y)$/.exec(s)
  if (!match) return s
  const n = match[1] ? Number.parseInt(match[1], 10) : 1
  switch (match[2]) {
    case 'm':
      return String(n)
    case 'h':
      return String(n * 60)
    case 'D':
      return `${n}D`
    case 'W':
      return `${n}W`
    case 'M':
    case 'MO':
      return `${n}M`
    case 'Q':
      return `${n * 3}M`
    default:
      return `${n * 12}M`
  }
}

export interface ChartContextParams {
  exchange: string
  symbol: string
  interval: string
  /** Chart colours for indicators that read `chart.fg_color` / `chart.bg_color`. */
  foregroundColor?: string
  backgroundColor?: string
}

/** What the indicators know about the chart: a US stock in Eastern time, regular session, bar times in seconds. */
export function buildChartContext(params: ChartContextParams): ChartContext {
  const context: ChartContext = {
    timeframe: timeframeForInterval(params.interval),
    timezone: INDICATOR_TIME_ZONE,
    session: US_REGULAR_SESSION,
    tickerid: `${params.exchange}:${params.symbol}`,
    symbolType: 'stock',
    currency: 'USD',
    mintick: 0.01,
    timeUnit: 's',
    lastBarConfirmed: true,
  }
  if (params.foregroundColor) context.foregroundColor = params.foregroundColor
  if (params.backgroundColor) context.backgroundColor = params.backgroundColor
  return context
}
