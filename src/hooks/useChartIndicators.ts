/**
 * Renders the active indicators on the Historify chart through
 * `lightweight-charts-indicators/render`.
 *
 * One IndicatorRenderer per active entry (by key): overlays draw in pane 0,
 * the others take panes 1.. in order of the active list. Every change to the
 * bars, the inputs or the list recomputes after a 100 ms debounce; a bars or
 * inputs change reuses the renderer's series, a list change redraws fresh so
 * pane numbers stay right after a removal.
 */
import type { IChartApi, ISeriesApi } from 'lightweight-charts'
import type { ChartContext } from 'lightweight-charts-indicators'
import { IndicatorRenderer } from 'lightweight-charts-indicators/render'
import type { Bar } from 'oakscriptjs'
import { useEffect, useRef } from 'react'
import type { ActiveIndicator } from '@/lib/chartTools/activeIndicators'
import { findRegistryEntry } from '@/lib/chartTools/indicatorCatalog'

export const INDICATOR_RENDER_DEBOUNCE_MS = 100

export interface UseChartIndicatorsOptions {
  chart: IChartApi | null
  mainSeries: ISeriesApi<'Candlestick'> | null
  bars: Bar[]
  active: ActiveIndicator[]
  context: ChartContext
  /** An indicator that cannot be resolved or computed; the others still render. */
  onError?: (key: string, message: string) => void
}

interface Slot {
  renderer: IndicatorRenderer
  chart: IChartApi
  pane: number
}

type RenderState = Omit<UseChartIndicatorsOptions, 'onError'>

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}

function safeClear(renderer: IndicatorRenderer) {
  try {
    renderer.clear()
  } catch {
    // the chart may already be gone
  }
}

function renderAll(
  slots: Map<string, Slot>,
  structureRef: { current: string },
  state: RenderState,
  onError: UseChartIndicatorsOptions['onError']
) {
  const { chart, mainSeries, bars, active, context } = state

  // Renderers of removed indicators, or bound to a chart that was replaced, go away.
  for (const [key, slot] of slots) {
    const keep = chart !== null && slot.chart === chart && active.some((a) => a.key === key)
    if (!keep) {
      safeClear(slot.renderer)
      slots.delete(key)
    }
  }
  if (!chart) {
    structureRef.current = ''
    return
  }

  const structure = active.map((a) => a.key).join('|')
  const structureChanged = structure !== structureRef.current
  structureRef.current = structure

  let nextPane = 1
  for (const item of active) {
    const entry = findRegistryEntry(item.id)
    if (!entry) {
      onError?.(item.key, `Unknown indicator "${item.id}"`)
      continue
    }
    const pane = entry.overlay ? 0 : nextPane++
    let slot = slots.get(item.key)
    const fresh = !slot
    if (!slot) {
      slot = {
        renderer: new IndicatorRenderer(chart, {
          paneIndex: pane,
          mainSeries: mainSeries ?? undefined,
        }),
        chart,
        pane,
      }
      slots.set(item.key, slot)
    } else if (slot.pane !== pane) {
      slot.renderer.setPaneIndex(pane)
      slot.pane = pane
    }
    const inputs = { ...(entry.defaultInputs ?? {}), ...item.inputs }
    try {
      const result = entry.calculate(bars, inputs, context)
      slot.renderer.render(entry, result, bars, {
        inputs,
        reuseSeries: !fresh && !structureChanged,
      })
    } catch (error) {
      onError?.(item.key, errorMessage(error))
    }
  }
}

export function useChartIndicators(opts: UseChartIndicatorsOptions): void {
  const { chart, mainSeries, bars, active, context, onError } = opts
  const slotsRef = useRef<Map<string, Slot>>(new Map())
  const structureRef = useRef('')
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const onErrorRef = useRef(onError)
  onErrorRef.current = onError

  useEffect(() => {
    // Every change resets the timer, so the closure the timer runs is always the latest state.
    const state: RenderState = { chart, mainSeries, bars, active, context }
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      renderAll(slotsRef.current, structureRef, state, onErrorRef.current)
    }, INDICATOR_RENDER_DEBOUNCE_MS)
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current)
        timerRef.current = null
      }
    }
  }, [chart, mainSeries, bars, active, context])

  useEffect(() => {
    const slots = slotsRef.current
    return () => {
      for (const slot of slots.values()) safeClear(slot.renderer)
      slots.clear()
    }
  }, [])
}
