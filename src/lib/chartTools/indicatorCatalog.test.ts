import { describe, expect, test } from 'vitest'
import { isOverlay, listIndicators, OVERLAY_IDS, searchIndicators } from './indicatorCatalog'

describe('indicatorCatalog', () => {
  test('catalog is large and well-formed', () => {
    const all = listIndicators()
    expect(all.length).toBeGreaterThan(1000)
    expect(all.every((e) => e.id && e.name && e.group)).toBe(true)
    expect(all.every((e) => Array.isArray(e.inputs))).toBe(true)
  })

  test('catalog is sorted by group, then name', () => {
    const all = listIndicators()
    for (let i = 1; i < all.length; i++) {
      const a = all[i - 1]
      const b = all[i]
      const byGroup = a.group.localeCompare(b.group)
      expect(byGroup < 0 || (byGroup === 0 && a.name.localeCompare(b.name) <= 0)).toBe(true)
    }
  })

  test('inputs carry only the editable types with their defaults', () => {
    const sma = listIndicators().find((e) => e.id === 'sma')
    expect(sma).toBeTruthy()
    const len = sma!.inputs.find((i) => i.id === 'len')
    expect(len).toEqual(expect.objectContaining({ label: 'Length', type: 'int', default: 9 }))
    expect(sma!.inputs.every((i) => ['int', 'float', 'bool', 'string', 'source'].includes(i.type)))
      .toBe(true)
  })

  test('search finds RSI first', () => {
    expect(searchIndicators('rsi')[0].id).toMatch(/rsi/i)
    expect(searchIndicators('rsi')[0].id).toBe('rsi')
  })

  test('search is case-insensitive over id and name and respects the limit', () => {
    expect(searchIndicators('MACD', 3)).toHaveLength(3)
    expect(searchIndicators('moving average convergence')[0].id).toBe('macd')
    expect(searchIndicators('')).toHaveLength(50)
    expect(searchIndicators('zzzz-no-such-indicator')).toHaveLength(0)
  })

  test('overlay classification', () => {
    expect(isOverlay({ id: 'SMA' })).toBe(true)
    expect(isOverlay({ id: 'RSI' })).toBe(false)
    expect(isOverlay({ id: 'macd' })).toBe(false)
    // Unknown ids fall back to the normalized overlay set.
    expect(isOverlay({ id: 'Bollinger Bands' })).toBe(true)
    expect(isOverlay({ id: 'unknown-thing' })).toBe(false)
    expect(OVERLAY_IDS.has('bollingerbands')).toBe(true)
  })
})
