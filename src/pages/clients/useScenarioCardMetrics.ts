import { invoke } from '@tauri-apps/api/core'
import { useCallback, useEffect, useState } from 'react'
import { computeSummary, enrichPositions } from '@/stores/clientScenarioStore'
import type {
  ClientScenario,
  GenericQuote,
  ScenarioPosition,
} from '@/types/client-scenarios'

/** One card's numbers — the same math as the Scenario page header. */
export interface CardMetrics {
  marketValue: number
  cash: number | null
  pnl: number
  /** Return on cost basis, in percent */
  returnPct: number
  positionCount: number
  pricedCount: number
}

export type PricesStatus = 'loading' | 'ready' | 'error'

/**
 * The baseline a clone is measured against: the client's baseline for the same
 * account type. A baseline has no deltas of its own.
 */
export function baselineFor(
  scenario: ClientScenario,
  scenarios: ClientScenario[]
): ClientScenario | null {
  if (scenario.is_baseline) return null
  return (
    scenarios.find((s) => s.is_baseline && s.account_type === scenario.account_type) ?? null
  )
}

export function metricsFrom(
  scenario: ClientScenario,
  positions: ScenarioPosition[],
  quotes: Map<string, GenericQuote>
): CardMetrics {
  const enriched = enrichPositions(positions, quotes)
  const summary = computeSummary(enriched)
  return {
    marketValue: summary.total_value,
    cash: scenario.cash,
    pnl: summary.total_pnl,
    returnPct: summary.pnl_pct,
    positionCount: summary.position_count,
    pricedCount: enriched.filter((p) => p.current_price != null).length,
  }
}

/**
 * Loads every scenario's positions and prices them with one quote call, so the
 * cards can show live metrics and how each clone differs from its baseline.
 */
export function useScenarioCardMetrics(scenarios: ClientScenario[]) {
  const [metrics, setMetrics] = useState<Map<number, CardMetrics>>(new Map())
  const [status, setStatus] = useState<PricesStatus>('loading')
  const [error, setError] = useState<string | null>(null)
  const [attempt, setAttempt] = useState(0)

  const key = scenarios.map((s) => `${s.id}:${s.updated_at}:${s.cash}`).join('|')

  useEffect(() => {
    let cancelled = false
    const withIds = scenarios.filter((s): s is ClientScenario & { id: number } => s.id != null)
    if (withIds.length === 0) {
      setMetrics(new Map())
      setStatus('ready')
      return
    }
    setStatus('loading')
    setError(null)
    ;(async () => {
      try {
        const positionsById = new Map<number, ScenarioPosition[]>()
        await Promise.all(
          withIds.map(async (s) => {
            const positions = await invoke<ScenarioPosition[]>('get_scenario_positions', {
              scenarioId: s.id,
            })
            positionsById.set(s.id, positions)
          })
        )
        const symbols = [
          ...new Set([...positionsById.values()].flat().map((p) => p.symbol.toUpperCase())),
        ]
        const quotes = new Map<string, GenericQuote>()
        if (symbols.length > 0) {
          const list = await invoke<GenericQuote[]>('get_generic_quote', { symbols })
          for (const q of list) quotes.set(q.symbol.toUpperCase(), q)
        }
        if (cancelled) return
        const next = new Map<number, CardMetrics>()
        for (const s of withIds) {
          next.set(s.id, metricsFrom(s, positionsById.get(s.id) ?? [], quotes))
        }
        setMetrics(next)
        setStatus('ready')
      } catch (err) {
        if (cancelled) return
        setError(err instanceof Error ? err.message : String(err))
        setStatus('error')
      }
    })()
    return () => {
      cancelled = true
    }
    // `key` captures the scenario fields that change the numbers.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, attempt])

  const retry = useCallback(() => setAttempt((n) => n + 1), [])

  return { metrics, status, error, retry }
}
