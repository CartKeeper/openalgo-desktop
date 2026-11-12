// @vitest-environment jsdom
import '@/test/setup'
import { act, renderHook } from '@testing-library/react'
import type { IChartApi, ISeriesApi } from 'lightweight-charts'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useChartDrawings } from './useChartDrawings'

const mocks = vi.hoisted(() => {
  type Handler = (...args: unknown[]) => void
  class FakeManager {
    static instances: FakeManager[] = []
    static importThrows = false
    exported = '[]'
    selectionIds: string[] = []
    handlers: Record<string, Handler[]> = {}
    chart: unknown
    series: unknown
    opts: unknown
    importJSON = vi.fn((_json: string) => {
      if (FakeManager.importThrows) throw new Error('bad json')
      return 0
    })
    exportJSON = vi.fn(() => this.exported)
    destroy = vi.fn()
    setTool = vi.fn()
    tool = vi.fn(() => null)
    setDrawingsVisible = vi.fn()
    drawingsVisible = vi.fn(() => true)
    selection = vi.fn(() => this.selectionIds)
    remove = vi.fn()
    clear = vi.fn()
    cancelPending = vi.fn()
    get = vi.fn()
    update = vi.fn()
    constructor(chart: unknown, series: unknown, opts: unknown) {
      this.chart = chart
      this.series = series
      this.opts = opts
      FakeManager.instances.push(this)
    }
    on(event: string, cb: Handler) {
      const list = this.handlers[event] ?? []
      list.push(cb)
      this.handlers[event] = list
      return () => {
        this.handlers[event] = (this.handlers[event] ?? []).filter((h) => h !== cb)
      }
    }
    emit(event: string, ...args: unknown[]) {
      for (const h of this.handlers[event] ?? []) h(...args)
    }
  }
  return { FakeManager }
})

vi.mock('lightweight-charts-drawing', () => ({ DrawingManager: mocks.FakeManager }))

const { FakeManager } = mocks

const chartA = { id: 'chart-a' } as unknown as IChartApi
const chartB = { id: 'chart-b' } as unknown as IChartApi
const series = { id: 'candles' } as unknown as ISeriesApi<'Candlestick'>

type HookProps = Parameters<typeof useChartDrawings>[0]

function setup(over: Partial<HookProps> = {}) {
  const onSave = vi.fn()
  const onError = vi.fn()
  const initialProps: HookProps = {
    chart: chartA,
    series,
    storageKey: 'NASDAQ:AAPL',
    initialJson: '[]',
    onSave,
    onError,
    ...over,
  }
  const hook = renderHook((p: HookProps) => useChartDrawings(p), { initialProps })
  return { ...hook, initialProps, onSave, onError }
}

describe('useChartDrawings', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    FakeManager.instances = []
    FakeManager.importThrows = false
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('creates one manager with a weak magnet and imports the saved JSON', () => {
    setup({ initialJson: '[{"kind":"trend-line"}]' })
    expect(FakeManager.instances).toHaveLength(1)
    const m = FakeManager.instances[0]
    expect(m.chart).toBe(chartA)
    expect(m.series).toBe(series)
    expect(m.opts).toEqual(expect.objectContaining({ magnet: 'weak' }))
    expect(m.importJSON).toHaveBeenCalledWith('[{"kind":"trend-line"}]')
  })

  it('does not create a manager until the saved JSON has loaded, nor without a chart', () => {
    const { rerender, initialProps } = setup({ initialJson: undefined })
    expect(FakeManager.instances).toHaveLength(0)
    rerender({ ...initialProps, initialJson: null })
    expect(FakeManager.instances).toHaveLength(1)
    expect(FakeManager.instances[0].importJSON).toHaveBeenCalledWith('[]')
    rerender({ ...initialProps, initialJson: null, chart: null })
    expect(FakeManager.instances[0].destroy).toHaveBeenCalledTimes(1)
    expect(FakeManager.instances).toHaveLength(1)
  })

  it('saves the exported JSON 500 ms after a change, once', () => {
    const { onSave } = setup()
    const m = FakeManager.instances[0]
    act(() => {
      m.exported = '[a]'
      m.emit('change', [])
    })
    act(() => {
      m.exported = '[b]'
      m.emit('change', [])
    })
    act(() => {
      vi.advanceTimersByTime(499)
    })
    expect(onSave).not.toHaveBeenCalled()
    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith('[b]')
  })

  it('pending save is cancelled on symbol change', () => {
    const { rerender, initialProps, onSave } = setup()
    const m1 = FakeManager.instances[0]
    act(() => {
      m1.exported = '[x]'
      m1.emit('change', [])
    })
    act(() => {
      vi.advanceTimersByTime(100)
    })
    rerender({ ...initialProps, storageKey: 'NASDAQ:MSFT', initialJson: null })
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(onSave).not.toHaveBeenCalled()
    expect(m1.destroy).toHaveBeenCalledTimes(1)
    const m2 = FakeManager.instances[1]
    expect(m2).toBeTruthy()
    expect(m2.importJSON).toHaveBeenCalledWith('[]')
    expect(m2.importJSON).not.toHaveBeenCalledWith('[x]')
  })

  it('re-created manager re-imports the last JSON', () => {
    const { rerender, initialProps, onSave } = setup()
    const m1 = FakeManager.instances[0]
    act(() => {
      m1.exported = '[x]'
      m1.emit('change', [])
    })
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(onSave).toHaveBeenCalledWith('[x]')
    rerender({ ...initialProps, chart: chartB })
    expect(m1.destroy).toHaveBeenCalledTimes(1)
    const m2 = FakeManager.instances[1]
    expect(m2.chart).toBe(chartB)
    expect(m2.importJSON).toHaveBeenCalledTimes(1)
    expect(m2.importJSON).toHaveBeenCalledWith('[x]')
  })

  it('import error is reported, not thrown', () => {
    FakeManager.importThrows = true
    const { result, onError } = setup({ initialJson: '{bad' })
    expect(onError).toHaveBeenCalledWith('Saved drawings could not be loaded')
    expect(result.current.activeTool).toBeNull()
    expect(FakeManager.instances).toHaveLength(1)
  })

  it('setTool arms the manager and the tool event drives activeTool', () => {
    const { result } = setup()
    const m = FakeManager.instances[0]
    act(() => {
      result.current.setTool('ray')
    })
    expect(m.setTool).toHaveBeenCalledWith('ray')
    act(() => {
      m.emit('tool', 'ray')
    })
    expect(result.current.activeTool).toBe('ray')
    act(() => {
      m.emit('tool', null)
    })
    expect(result.current.activeTool).toBeNull()
  })

  it('selection and visibility events are reflected; deleteSelection removes each id', () => {
    const { result } = setup()
    const m = FakeManager.instances[0]
    expect(result.current.hasSelection).toBe(false)
    act(() => {
      m.selectionIds = ['a', 'b']
      m.emit('selection', ['a', 'b'])
    })
    expect(result.current.hasSelection).toBe(true)
    act(() => {
      result.current.deleteSelection()
    })
    expect(m.remove).toHaveBeenCalledWith('a')
    expect(m.remove).toHaveBeenCalledWith('b')
    act(() => {
      m.emit('visibility', false)
    })
    expect(result.current.allHidden).toBe(true)
    act(() => {
      result.current.toggleHidden()
    })
    expect(m.setDrawingsVisible).toHaveBeenCalledWith(true)
    act(() => {
      result.current.clearAll()
    })
    expect(m.clear).toHaveBeenCalledTimes(1)
  })

  it('text edit requests are exposed and committed back to the drawing', () => {
    const { result } = setup()
    const m = FakeManager.instances[0]
    const drawing = { id: 'd1', kind: 'text', points: [], style: {}, text: '' }
    m.get.mockReturnValue(drawing)
    act(() => {
      m.emit('textEdit', drawing, { x: 10, y: 20 })
    })
    expect(result.current.textEdit).toEqual({ id: 'd1', x: 10, y: 20, text: '' })
    act(() => {
      result.current.commitText('Breakout')
    })
    expect(m.update).toHaveBeenCalledWith(expect.objectContaining({ id: 'd1', text: 'Breakout' }))
    expect(result.current.textEdit).toBeNull()
    act(() => {
      m.emit('textEdit', drawing, { x: 1, y: 2 })
    })
    act(() => {
      result.current.commitText('')
    })
    expect(m.remove).toHaveBeenCalledWith('d1')
  })

  it('unmount destroys the manager', () => {
    const { unmount } = setup()
    unmount()
    expect(FakeManager.instances[0].destroy).toHaveBeenCalledTimes(1)
  })
})
