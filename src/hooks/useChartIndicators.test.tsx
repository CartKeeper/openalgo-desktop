// @vitest-environment jsdom
import '@/test/setup'
import { act, renderHook } from '@testing-library/react'
import type { IChartApi, ISeriesApi } from 'lightweight-charts'
import type { Bar } from 'oakscriptjs'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ActiveIndicator } from '@/lib/chartTools/activeIndicators'
import { useChartIndicators } from './useChartIndicators'

const mocks = vi.hoisted(() => {
  class FakeRenderer {
    static instances: FakeRenderer[] = []
    chart: unknown
    opts: { paneIndex?: number; mainSeries?: unknown }
    render = vi.fn()
    clear = vi.fn()
    setPaneIndex = vi.fn()
    constructor(chart: unknown, opts: { paneIndex?: number; mainSeries?: unknown }) {
      this.chart = chart
      this.opts = opts
      FakeRenderer.instances.push(this)
    }
  }
  return { FakeRenderer }
})

vi.mock('lightweight-charts-indicators/render', () => ({ IndicatorRenderer: mocks.FakeRenderer }))

vi.mock('@/lib/chartTools/indicatorCatalog', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/chartTools/indicatorCatalog')>()
  const boom = {
    id: 'boom',
    overlay: false,
    defaultInputs: {},
    inputConfig: [],
    plotConfig: [],
    calculate: () => {
      throw new Error('kaboom')
    },
  }
  return {
    ...actual,
    findRegistryEntry: (id: string) => (id === 'boom' ? boom : actual.findRegistryEntry(id)),
  }
})

const { FakeRenderer } = mocks

const chart = { id: 'chart' } as unknown as IChartApi
const mainSeries = { id: 'candles' } as unknown as ISeriesApi<'Candlestick'>

function makeBars(n: number, seed = 0): Bar[] {
  return Array.from({ length: n }, (_, i) => ({
    time: 1_700_000_000 + i * 86_400,
    open: 100 + i + seed,
    high: 101 + i + seed,
    low: 99 + i + seed,
    close: 100.5 + i + seed,
    volume: 1000,
  }))
}

const sma: ActiveIndicator = { key: 'ksma', id: 'sma', inputs: { len: 9 } }
const rsi: ActiveIndicator = { key: 'krsi', id: 'rsi', inputs: { length: 14 } }
const macd: ActiveIndicator = { key: 'kmacd', id: 'macd', inputs: {} }
const context = { timeframe: '1D', timezone: 'America/New_York' }

type HookProps = Parameters<typeof useChartIndicators>[0]

function setup(over: Partial<HookProps> = {}) {
  const onError = vi.fn()
  const initialProps: HookProps = {
    chart,
    mainSeries,
    bars: makeBars(60),
    active: [sma],
    context,
    onError,
    ...over,
  }
  const hook = renderHook((p: HookProps) => useChartIndicators(p), { initialProps })
  return { ...hook, initialProps, onError }
}

function flush(ms = 100) {
  act(() => {
    vi.advanceTimersByTime(ms)
  })
}

describe('useChartIndicators', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    FakeRenderer.instances = []
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders nothing without a chart', () => {
    setup({ chart: null })
    flush()
    expect(FakeRenderer.instances).toHaveLength(0)
  })

  it('second bars update reuses the renderer', () => {
    const { rerender, initialProps } = setup()
    flush()
    expect(FakeRenderer.instances).toHaveLength(1)
    const r = FakeRenderer.instances[0]
    expect(r.chart).toBe(chart)
    expect(r.opts).toEqual(expect.objectContaining({ paneIndex: 0, mainSeries }))
    expect(r.render).toHaveBeenCalledTimes(1)

    rerender({ ...initialProps, bars: makeBars(61, 1) })
    flush()
    expect(FakeRenderer.instances).toHaveLength(1)
    expect(r.render).toHaveBeenCalledTimes(2)
    expect(r.render.mock.calls[1][3]).toEqual(
      expect.objectContaining({ reuseSeries: true, inputs: expect.objectContaining({ len: 9 }) })
    )
  })

  it('ten bar updates in 50 ms render once', () => {
    const { rerender, initialProps } = setup()
    for (let i = 0; i < 10; i++) {
      rerender({ ...initialProps, bars: makeBars(60 + i, i) })
      flush(5)
    }
    expect(FakeRenderer.instances.reduce((n, r) => n + r.render.mock.calls.length, 0)).toBe(0)
    flush(100)
    expect(FakeRenderer.instances).toHaveLength(1)
    expect(FakeRenderer.instances[0].render).toHaveBeenCalledTimes(1)
  })

  it('overlays draw in pane 0, oscillators take the next panes in order, removal re-packs them', () => {
    const { rerender, initialProps } = setup({ active: [sma, rsi, macd] })
    flush()
    expect(FakeRenderer.instances.map((r) => r.opts.paneIndex)).toEqual([0, 1, 2])
    const [rSma, rRsi, rMacd] = FakeRenderer.instances

    rerender({ ...initialProps, active: [sma, macd] })
    flush()
    expect(rRsi.clear).toHaveBeenCalledTimes(1)
    expect(rMacd.setPaneIndex).toHaveBeenCalledWith(1)
    expect(rMacd.render).toHaveBeenCalledTimes(2)
    expect(rMacd.render.mock.calls[1][3]).toEqual(expect.objectContaining({ reuseSeries: false }))
    expect(rSma.clear).not.toHaveBeenCalled()
    expect(FakeRenderer.instances).toHaveLength(3)
  })

  it('input changes re-render with the merged inputs', () => {
    const { rerender, initialProps } = setup()
    flush()
    rerender({ ...initialProps, active: [{ ...sma, inputs: { len: 21 } }] })
    flush()
    const r = FakeRenderer.instances[0]
    expect(r.render).toHaveBeenCalledTimes(2)
    expect(r.render.mock.calls[1][3].inputs).toEqual(
      expect.objectContaining({ len: 21, src: 'close' })
    )
  })

  it('a throwing indicator reports an error and the others still render', () => {
    const boom: ActiveIndicator = { key: 'kboom', id: 'boom', inputs: {} }
    const { onError } = setup({ active: [boom, sma] })
    flush()
    expect(onError).toHaveBeenCalledWith('kboom', 'kaboom')
    const smaRenderer = FakeRenderer.instances.find((r) => r.opts.paneIndex === 0)
    expect(smaRenderer?.render).toHaveBeenCalledTimes(1)
  })

  it('an unknown indicator id is reported, not rendered', () => {
    const { onError } = setup({ active: [{ key: 'kx', id: 'no-such-indicator', inputs: {} }] })
    flush()
    expect(onError).toHaveBeenCalledWith('kx', expect.stringMatching(/no-such-indicator/))
    expect(FakeRenderer.instances).toHaveLength(0)
  })

  it('unmount clears every renderer', () => {
    const { unmount } = setup({ active: [sma, rsi] })
    flush()
    unmount()
    expect(FakeRenderer.instances).toHaveLength(2)
    for (const r of FakeRenderer.instances) expect(r.clear).toHaveBeenCalledTimes(1)
  })
})
