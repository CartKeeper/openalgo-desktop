import { describe, expect, test } from 'vitest'
import {
  type ActiveIndicator,
  buildChartContext,
  newActiveIndicator,
  parseIndicators,
  serializeIndicators,
  timeframeForInterval,
} from './activeIndicators'

describe('activeIndicators', () => {
  test('serialize/parse round-trip', () => {
    const list: ActiveIndicator[] = [
      { key: 'k1', id: 'sma', inputs: { len: 20 } },
      { key: 'k2', id: 'rsi', inputs: { length: 14, src: 'close', calculateDivergence: false } },
    ]
    expect(parseIndicators(serializeIndicators(list))).toEqual(list)
  })

  test('tolerant parse: invalid or wrong-shaped JSON gives an empty list', () => {
    expect(parseIndicators(null)).toEqual([])
    expect(parseIndicators('')).toEqual([])
    expect(parseIndicators('nope')).toEqual([])
    expect(parseIndicators('{"a":1}')).toEqual([])
    expect(parseIndicators('42')).toEqual([])
  })

  test('parse drops malformed entries, keeps saved keys and invents missing ones', () => {
    const parsed = parseIndicators(
      '[{"key":"saved","id":"sma","inputs":{"len":5}},{"id":"rsi"},{"nope":1},{"key":"k","id":3},{"id":"x","inputs":{"bad":{"nested":true},"ok":1}}]'
    )
    expect(parsed).toHaveLength(3)
    expect(parsed[0]).toEqual({ key: 'saved', id: 'sma', inputs: { len: 5 } })
    expect(parsed[1].id).toBe('rsi')
    expect(parsed[1].inputs).toEqual({})
    expect(typeof parsed[1].key).toBe('string')
    expect(parsed[1].key.length).toBeGreaterThan(0)
    expect(parsed[2].inputs).toEqual({ ok: 1 })
  })

  test('newActiveIndicator seeds the inputs from the catalog defaults with a unique key', () => {
    const entry = {
      id: 'sma',
      name: 'Simple moving average',
      group: 'Moving Averages',
      overlay: true,
      origin: 'standard' as const,
      inputs: [
        { id: 'len', label: 'Length', type: 'int' as const, default: 9 },
        { id: 'src', label: 'Source', type: 'source' as const, default: 'close' },
      ],
    }
    const a = newActiveIndicator(entry)
    const b = newActiveIndicator(entry)
    expect(a.id).toBe('sma')
    expect(a.inputs).toEqual({ len: 9, src: 'close' })
    expect(a.key).not.toBe(b.key)
  })

  test('timeframeForInterval maps the page intervals to PineScript timeframes', () => {
    expect(timeframeForInterval('1m')).toBe('1')
    expect(timeframeForInterval('5m')).toBe('5')
    expect(timeframeForInterval('25m')).toBe('25')
    expect(timeframeForInterval('1h')).toBe('60')
    expect(timeframeForInterval('4h')).toBe('240')
    expect(timeframeForInterval('D')).toBe('1D')
    expect(timeframeForInterval('W')).toBe('1W')
    expect(timeframeForInterval('2W')).toBe('2W')
    expect(timeframeForInterval('M')).toBe('1M')
    expect(timeframeForInterval('3MO')).toBe('3M')
    expect(timeframeForInterval('Q')).toBe('3M')
    expect(timeframeForInterval('Y')).toBe('12M')
    expect(timeframeForInterval('')).toBe('1D')
  })

  test('buildChartContext is Eastern time, regular US session, a stock in USD', () => {
    const context = buildChartContext({ exchange: 'NASDAQ', symbol: 'AAPL', interval: 'D' })
    expect(context).toEqual(
      expect.objectContaining({
        timeframe: '1D',
        timezone: 'America/New_York',
        session: '0930-1600',
        tickerid: 'NASDAQ:AAPL',
        symbolType: 'stock',
        currency: 'USD',
        mintick: 0.01,
        timeUnit: 's',
        lastBarConfirmed: true,
      })
    )
  })
})
