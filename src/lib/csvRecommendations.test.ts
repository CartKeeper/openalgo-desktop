import { describe, expect, it } from 'vitest'
import type { OrderRecommendation } from '@/types/actionQueue'
import { formatRecommendationsAsCsv } from './csvRecommendations'

function item(overrides: Partial<OrderRecommendation> = {}): OrderRecommendation {
  return {
    id: 'a',
    symbol: 'AAPL',
    exchange: 'NASDAQ',
    side: 'BUY',
    quantity: 10,
    orderType: 'MARKET',
    product: 'CNC',
    price: 180,
    rationale: 'Strong Q2 earnings',
    source: 'copilot',
    ...overrides,
  }
}

const HEADER = 'symbol,exchange,side,quantity,price,rationale'

describe('formatRecommendationsAsCsv', () => {
  it('renders header + one row per item with \\r\\n separators', () => {
    const csv = formatRecommendationsAsCsv([
      item({ symbol: 'AAPL', price: 180 }),
      item({ symbol: 'TSLA', side: 'SELL', quantity: 5, price: 240, rationale: 'Reducing risk' }),
    ])
    expect(csv).toBe(
      `${HEADER}\r\nAAPL,NASDAQ,BUY,10,180.00,Strong Q2 earnings\r\nTSLA,NASDAQ,SELL,5,240.00,Reducing risk\r\n`
    )
  })

  it('returns header alone when items is empty', () => {
    const csv = formatRecommendationsAsCsv([])
    expect(csv).toBe(`${HEADER}\r\n`)
  })

  it('formats integer prices to two decimals', () => {
    const csv = formatRecommendationsAsCsv([item({ price: 180 })])
    expect(csv).toContain(',180.00,')
  })

  it('formats fractional prices to two decimals', () => {
    const csv = formatRecommendationsAsCsv([item({ price: 180.5 })])
    expect(csv).toContain(',180.50,')
  })

  it('renders empty rationale as an empty field (not empty quotes)', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: '' })])
    expect(csv.endsWith(',\r\n')).toBe(true)
    expect(csv).not.toContain(',""\r\n')
  })

  it('quotes and escapes rationale containing a comma', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: 'hedge, short term' })])
    expect(csv).toContain(',"hedge, short term"\r\n')
  })

  it('quotes and doubles embedded quotes in rationale', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: 'She said "hi".' })])
    expect(csv).toContain(',"She said ""hi""."\r\n')
  })

  it('quotes rationale containing a newline', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: 'line one\nline two' })])
    expect(csv).toContain(',"line one\nline two"\r\n')
  })
})
