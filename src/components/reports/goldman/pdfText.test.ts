import { describe, expect, it } from 'vitest'
import { pdfSafe, pdfSafeDeep } from './pdfText'

describe('pdfSafe', () => {
  it('keeps the sign of a loss written with a true minus', () => {
    expect(pdfSafe('VLO (−0.88%), PSX (+4.48%)')).toBe('VLO (-0.88%), PSX (+4.48%)')
  })

  it('spells out comparison and arrow characters', () => {
    expect(pdfSafe('upside ≥ 10 pts → swap')).toBe('upside >= 10 pts -> swap')
  })

  it('leaves characters the fonts can draw alone', () => {
    const text = 'Goldman Sax & Violins · “Andante” — 1,234.56 • ±2% – é'
    expect(pdfSafe(text)).toBe(text)
  })
})

describe('pdfSafeDeep', () => {
  it('cleans every string in a brief and keeps other values', () => {
    const brief = {
      subtitle: 'down −52%',
      facts: { positions: 46, unrealized_pct: null, unpriced: ['YOURF'] },
      movements: [{ bullets: ['AG (−20.84%)'], pl_positive: false }],
    }
    expect(pdfSafeDeep(brief)).toEqual({
      subtitle: 'down -52%',
      facts: { positions: 46, unrealized_pct: null, unpriced: ['YOURF'] },
      movements: [{ bullets: ['AG (-20.84%)'], pl_positive: false }],
    })
  })
})
