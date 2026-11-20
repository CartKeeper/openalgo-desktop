import { DrawingManager } from 'lightweight-charts-drawing'
import { SMA } from 'lightweight-charts-indicators'
import { expect, test } from 'vitest'

const bars = [1, 2, 3, 4, 5].map((c, i) => ({
  time: i + 1,
  open: c,
  high: c,
  low: c,
  close: c,
  volume: 1,
}))

test('chart tool libraries load and compute', () => {
  expect(typeof DrawingManager).toBe('function')
  expect(SMA.calculate(bars, { len: 3 })).toBeTruthy()
})
