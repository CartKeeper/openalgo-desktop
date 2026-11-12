/**
 * Lifecycle of a `lightweight-charts-drawing` DrawingManager on the Historify chart.
 *
 * One manager per (chart, series, symbol). Drawings are imported from the saved
 * JSON of the symbol, re-imported from the last export when the page re-creates
 * the chart (theme flip, new bars), and saved 500 ms after the last change. A
 * save still pending when the symbol changes is dropped, never written to the
 * new symbol.
 */
import type { IChartApi, ISeriesApi } from 'lightweight-charts'
import { DrawingManager, type OHLC } from 'lightweight-charts-drawing'
import { useCallback, useEffect, useRef, useState } from 'react'

export type ToolKind = Parameters<DrawingManager['setTool']>[0]

export interface DrawingTextEdit {
  id: string
  /** Pane coordinates (px) where the library asks the host to open its text editor. */
  x: number
  y: number
  text: string
}

export interface ChartDrawingsApi {
  activeTool: ToolKind | null
  setTool: (t: ToolKind | null) => void
  allHidden: boolean
  toggleHidden: () => void
  clearAll: () => void
  deleteSelection: () => void
  hasSelection: boolean
  /** A text-family drawing waiting for its text (just placed or double-clicked). */
  textEdit: DrawingTextEdit | null
  commitText: (text: string) => void
  cancelText: () => void
}

export interface UseChartDrawingsOptions {
  chart: IChartApi | null
  series: ISeriesApi<'Candlestick'> | null
  /** `${exchange}:${symbol}`: the key the drawings are saved under. */
  storageKey: string
  /**
   * The saved JSON of `storageKey`: `null` when nothing was saved, `undefined`
   * while it is still loading (no manager is created until it has loaded, so
   * another symbol's drawings never show up on this one).
   */
  initialJson: string | null | undefined
  onSave: (json: string) => void
  onError: (message: string) => void
  /** Bars with volume for the data-driven tools (volume profiles, anchored VWAP). */
  bars?: () => OHLC[]
  /** The chart interval is intraday: date labels on drawings show the time of day. */
  intraday?: boolean
}

export const DRAWINGS_SAVE_DEBOUNCE_MS = 500
export const DRAWINGS_TIME_ZONE = 'America/New_York'

export function useChartDrawings(opts: UseChartDrawingsOptions): ChartDrawingsApi {
  const { chart, series, storageKey, initialJson, onSave, onError, bars, intraday = false } = opts

  const managerRef = useRef<DrawingManager | null>(null)
  /** Last known JSON per key: re-imported when the chart is re-created for the same symbol. */
  const lastJsonRef = useRef<{ key: string; json: string } | null>(null)
  const pendingRef = useRef<{ key: string; timer: ReturnType<typeof setTimeout> } | null>(null)
  const initialJsonRef = useRef(initialJson)
  initialJsonRef.current = initialJson
  const onSaveRef = useRef(onSave)
  onSaveRef.current = onSave
  const onErrorRef = useRef(onError)
  onErrorRef.current = onError
  const barsRef = useRef(bars)
  barsRef.current = bars
  const intradayRef = useRef(intraday)
  intradayRef.current = intraday

  const [activeTool, setActiveTool] = useState<ToolKind | null>(null)
  const [allHidden, setAllHidden] = useState(false)
  const allHiddenRef = useRef(allHidden)
  allHiddenRef.current = allHidden
  const [hasSelection, setHasSelection] = useState(false)
  const [textEdit, setTextEdit] = useState<DrawingTextEdit | null>(null)
  const textEditRef = useRef(textEdit)
  textEditRef.current = textEdit

  const loaded = initialJson !== undefined

  useEffect(() => {
    // A save pending for another symbol is dropped: it must never land on this one.
    if (pendingRef.current && pendingRef.current.key !== storageKey) {
      clearTimeout(pendingRef.current.timer)
      pendingRef.current = null
    }
    if (!chart || !series || !loaded) return

    let manager: DrawingManager
    try {
      manager = new DrawingManager(chart, series, {
        magnet: 'weak',
        bars: barsRef.current ? () => barsRef.current?.() ?? [] : undefined,
        timeInfo: () => ({ timeZone: DRAWINGS_TIME_ZONE, intraday: intradayRef.current }),
      })
    } catch {
      onErrorRef.current('Drawing tools could not start')
      return
    }
    managerRef.current = manager

    const remembered =
      lastJsonRef.current && lastJsonRef.current.key === storageKey
        ? lastJsonRef.current.json
        : null
    const json = remembered ?? initialJsonRef.current ?? '[]'
    let imported = true
    try {
      manager.importJSON(json)
    } catch {
      imported = false
      onErrorRef.current('Saved drawings could not be loaded')
    }
    if (imported) {
      let exported = json
      try {
        exported = manager.exportJSON()
      } catch {
        // keep the JSON we imported
      }
      lastJsonRef.current = { key: storageKey, json: exported }
    } else {
      // Continue with an empty canvas; the next change overwrites the unreadable JSON.
      lastJsonRef.current = { key: storageKey, json: '[]' }
    }

    setActiveTool(manager.tool())
    setAllHidden(!manager.drawingsVisible())
    setHasSelection(manager.selection().length > 0)
    setTextEdit(null)

    const offs = [
      manager.on('change', () => {
        let exported: string
        try {
          exported = manager.exportJSON()
        } catch {
          return
        }
        lastJsonRef.current = { key: storageKey, json: exported }
        if (pendingRef.current) clearTimeout(pendingRef.current.timer)
        const timer = setTimeout(() => {
          pendingRef.current = null
          onSaveRef.current(exported)
        }, DRAWINGS_SAVE_DEBOUNCE_MS)
        pendingRef.current = { key: storageKey, timer }
      }),
      manager.on('tool', (kind) => setActiveTool(kind)),
      manager.on('selection', (ids) => setHasSelection(ids.length > 0)),
      manager.on('visibility', (visible) => setAllHidden(!visible)),
      manager.on('textEdit', (d, screen) =>
        setTextEdit({ id: d.id, x: screen.x, y: screen.y, text: d.text ?? '' })
      ),
    ]

    return () => {
      for (const off of offs) off()
      if (managerRef.current === manager) managerRef.current = null
      try {
        manager.destroy()
      } catch {
        // the chart may already be gone
      }
    }
  }, [chart, series, storageKey, loaded])

  const setTool = useCallback((t: ToolKind | null) => {
    managerRef.current?.setTool(t)
    setActiveTool(t)
  }, [])

  const toggleHidden = useCallback(() => {
    const manager = managerRef.current
    if (!manager) return
    const show = allHiddenRef.current
    manager.setDrawingsVisible(show)
    setAllHidden(!show)
  }, [])

  const clearAll = useCallback(() => {
    managerRef.current?.clear()
  }, [])

  const deleteSelection = useCallback(() => {
    const manager = managerRef.current
    if (!manager) return
    for (const id of [...manager.selection()]) manager.remove(id)
  }, [])

  const commitText = useCallback((text: string) => {
    const manager = managerRef.current
    const edit = textEditRef.current
    setTextEdit(null)
    if (!manager || !edit) return
    const drawing = manager.get(edit.id)
    if (!drawing) return
    if (!text.trim()) {
      manager.remove(edit.id)
      return
    }
    manager.update({ ...drawing, text })
  }, [])

  const cancelText = useCallback(() => {
    const manager = managerRef.current
    const edit = textEditRef.current
    setTextEdit(null)
    if (!manager || !edit) return
    const drawing = manager.get(edit.id)
    if (drawing && !(drawing.text ?? '').trim()) manager.remove(edit.id)
  }, [])

  return {
    activeTool,
    setTool,
    allHidden,
    toggleHidden,
    clearAll,
    deleteSelection,
    hasSelection,
    textEdit,
    commitText,
    cancelText,
  }
}
