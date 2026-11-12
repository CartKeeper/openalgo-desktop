/**
 * Indicator catalog over `lightweight-charts-indicators`'s `indicatorRegistry`.
 *
 * Pure module: no chart, no DOM. The picker searches it, the chips read labels
 * from it, and the renderer hook resolves registry entries through it.
 */
import { type IndicatorRegistryEntry, indicatorRegistry } from 'lightweight-charts-indicators'

export type CatalogInputType = 'int' | 'float' | 'bool' | 'string' | 'source'

export interface CatalogInput {
  id: string
  label: string
  type: CatalogInputType
  default: number | boolean | string
  min?: number
  max?: number
  step?: number
  /** Allowed values of a `string` input shown as a dropdown by the library. */
  options?: string[]
}

export interface CatalogEntry {
  id: string
  name: string
  /** Display group: the registry's category ("Oscillators", "Trend", ...). */
  group: string
  overlay: boolean
  inputs: CatalogInput[]
  /** Where the registry sourced the indicator from. */
  origin: 'standard' | 'community' | 'candlestick'
}

/** Normalized (lowercase, no spaces/dashes/underscores) ids of the classic price-pane overlays. */
export const OVERLAY_IDS: ReadonlySet<string> = new Set([
  'sma',
  'ema',
  'wma',
  'vwap',
  'bollingerbands',
  'keltnerchannels',
  'donchianchannels',
  'ichimokucloud',
  'parabolicsar',
  'supertrend',
  'pivotpoints',
])

const EDITABLE_TYPES: ReadonlySet<string> = new Set(['int', 'float', 'bool', 'string', 'source'])

export function normalizeIndicatorId(id: string): string {
  return id.toLowerCase().replace(/[\s_-]+/g, '')
}

function isScalar(v: unknown): v is number | boolean | string {
  return typeof v === 'number' || typeof v === 'boolean' || typeof v === 'string'
}

function toCatalogInputs(entry: IndicatorRegistryEntry): CatalogInput[] {
  const inputs: CatalogInput[] = []
  for (const cfg of entry.inputConfig) {
    if (!EDITABLE_TYPES.has(cfg.type)) continue
    const fallback = entry.defaultInputs?.[cfg.id]
    const value = cfg.defval !== undefined ? cfg.defval : fallback
    if (!isScalar(value)) continue
    const input: CatalogInput = {
      id: cfg.id,
      label: cfg.title || cfg.id,
      type: cfg.type as CatalogInputType,
      default: value,
    }
    if (typeof cfg.min === 'number') input.min = cfg.min
    if (typeof cfg.max === 'number') input.max = cfg.max
    if (typeof cfg.step === 'number') input.step = cfg.step
    if (Array.isArray(cfg.options) && cfg.options.length > 0) input.options = [...cfg.options]
    inputs.push(input)
  }
  return inputs
}

function toCatalogEntry(entry: IndicatorRegistryEntry): CatalogEntry {
  return {
    id: entry.id,
    name: entry.name,
    group: entry.category,
    overlay: entry.overlay,
    inputs: toCatalogInputs(entry),
    origin: entry.group,
  }
}

let catalogCache: CatalogEntry[] | null = null
let registryById: Map<string, IndicatorRegistryEntry> | null = null
let registryByNormalizedId: Map<string, IndicatorRegistryEntry> | null = null

function registryIndex(): {
  byId: Map<string, IndicatorRegistryEntry>
  byNormalizedId: Map<string, IndicatorRegistryEntry>
} {
  if (!registryById || !registryByNormalizedId) {
    registryById = new Map()
    registryByNormalizedId = new Map()
    for (const entry of indicatorRegistry) {
      registryById.set(entry.id, entry)
      const normalized = normalizeIndicatorId(entry.id)
      if (!registryByNormalizedId.has(normalized)) registryByNormalizedId.set(normalized, entry)
    }
  }
  return { byId: registryById, byNormalizedId: registryByNormalizedId }
}

/** The registry entry behind a catalog id (exact id first, then the normalized form). */
export function findRegistryEntry(id: string): IndicatorRegistryEntry | undefined {
  const { byId, byNormalizedId } = registryIndex()
  return byId.get(id) ?? byNormalizedId.get(normalizeIndicatorId(id))
}

/** Every indicator of the registry, sorted by group then name. */
export function listIndicators(): CatalogEntry[] {
  if (!catalogCache) {
    catalogCache = indicatorRegistry.map(toCatalogEntry).sort((a, b) => {
      const byGroup = a.group.localeCompare(b.group)
      if (byGroup !== 0) return byGroup
      const byName = a.name.localeCompare(b.name)
      if (byName !== 0) return byName
      return a.id.localeCompare(b.id)
    })
  }
  return catalogCache
}

/** The catalog entry of an id, if the registry knows it. */
export function getCatalogEntry(id: string): CatalogEntry | undefined {
  const entry = findRegistryEntry(id)
  if (!entry) return undefined
  return listIndicators().find((e) => e.id === entry.id)
}

const ORIGIN_RANK: Record<CatalogEntry['origin'], number> = {
  standard: 0,
  candlestick: 1,
  community: 2,
}

function matchScore(entry: CatalogEntry, q: string, qNormalized: string): number {
  const id = entry.id.toLowerCase()
  const idNormalized = normalizeIndicatorId(entry.id)
  const name = entry.name.toLowerCase()
  const nameNormalized = normalizeIndicatorId(entry.name)
  if (id === q || idNormalized === qNormalized || nameNormalized === qNormalized) return 0
  if (id.startsWith(q) || idNormalized.startsWith(qNormalized)) return 1
  if (name.startsWith(q) || nameNormalized.startsWith(qNormalized)) return 2
  if (id.includes(q) || idNormalized.includes(qNormalized)) return 3
  if (name.includes(q) || nameNormalized.includes(qNormalized)) return 4
  return -1
}

/**
 * Case-insensitive search over id and name. Exact matches rank first, then id
 * prefixes, name prefixes and substrings; standard indicators rank before
 * community ports at equal relevance. An empty query lists the catalog order.
 */
export function searchIndicators(query: string, limit = 50): CatalogEntry[] {
  const all = listIndicators()
  const q = query.trim().toLowerCase()
  if (!q) return all.slice(0, limit)
  const qNormalized = normalizeIndicatorId(q)
  const scored: { entry: CatalogEntry; score: number }[] = []
  for (const entry of all) {
    const score = matchScore(entry, q, qNormalized)
    if (score >= 0) scored.push({ entry, score })
  }
  scored.sort((a, b) => {
    if (a.score !== b.score) return a.score - b.score
    const byOrigin = ORIGIN_RANK[a.entry.origin] - ORIGIN_RANK[b.entry.origin]
    if (byOrigin !== 0) return byOrigin
    return a.entry.name.localeCompare(b.entry.name)
  })
  return scored.slice(0, limit).map((s) => s.entry)
}

/**
 * Whether an indicator draws on the price pane. The registry's own `overlay`
 * flag decides when the id is known; otherwise the normalized id is looked up
 * in `OVERLAY_IDS`.
 */
export function isOverlay(entry: Pick<CatalogEntry, 'id'> & Partial<CatalogEntry>): boolean {
  if (typeof entry.overlay === 'boolean') return entry.overlay
  const known = findRegistryEntry(entry.id)
  if (known) return known.overlay
  return OVERLAY_IDS.has(normalizeIndicatorId(entry.id))
}
