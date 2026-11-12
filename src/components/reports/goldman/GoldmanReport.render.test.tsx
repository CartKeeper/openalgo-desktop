import { readFileSync, writeFileSync } from 'node:fs'
import { inflateSync } from 'node:zlib'
import { renderToBuffer } from '@react-pdf/renderer'
import { describe, expect, it } from 'vitest'
import { GoldmanReport } from './GoldmanReport'
import type { GoldmanBrief } from './types'

/** Pages in a rendered PDF: one `/Type /Page` object each (`/Pages` is the tree root). */
function pageCount(pdf: Buffer): number {
  return (pdf.toString('latin1').match(/\/Type\s*\/Page(?![s\w])/g) ?? []).length
}

/**
 * The text a PDF draws, with whitespace removed. react-pdf writes each glyph
 * of the built-in fonts as a `<hex>` string inside deflated content streams,
 * so inflating every stream and decoding those strings gives the page text.
 */
function pdfText(pdf: Buffer): string {
  let streams = ''
  let at = 0
  for (;;) {
    const start = pdf.indexOf('stream', at)
    if (start < 0) break
    let body = start + 'stream'.length
    if (pdf[body] === 0x0d) body++
    if (pdf[body] === 0x0a) body++
    const end = pdf.indexOf('endstream', body)
    if (end < 0) break
    const chunk = pdf.subarray(body, end)
    try {
      streams += inflateSync(chunk).toString('latin1')
    } catch {
      streams += chunk.toString('latin1')
    }
    at = end + 'endstream'.length
  }
  const glyphs = [...streams.matchAll(/<([0-9a-fA-F]+)>/g)].map((m) => Buffer.from(m[1], 'hex').toString('latin1'))
  return glyphs.join('').replace(/\s+/g, '')
}

const squeeze = (s: string) => s.replace(/\s+/g, '')

const brief: GoldmanBrief = {
  client_name: 'Casey Borst',
  document_label: 'Opus 46 / Combined Brief',
  tempo: 'Andante con moto · Q4 2026',
  generated_date: 'November 11, 2026',
  subtitle: 'A forty-six-instrument ensemble.',
  confidential_label: 'Confidential client brief — 401(k)',
  facts: {
    account_type: '401(k)',
    positions: 46,
    market_value: 172553.12,
    cost_basis: 163400.5,
    unrealized_pnl: 9152.62,
    unrealized_pct: 5.6,
    prices_as_of: 'November 11, 2026, 12:06 PM ET',
    unpriced: ['ABILF', 'SMME', 'YOURF'],
  },
  movements: [
    {
      kind: 'restructuring',
      numeral: 'II',
      tempo: 'Allegro moderato',
      title_main: 'The',
      title_accent: 'Restructuring',
      allocation_cards: [
        { percentage: '38%', label: 'Core — index & mega-cap', body: 'b', symbols: ['VOO', 'SPY'], caption: 'of account value today' },
        { percentage: '22%', label: 'Sector sleeve', body: 'b' },
        { percentage: '9%', label: 'Growth / satellite', body: 'b' },
      ],
      proposed_core_table: {
        heading: 'Proposed core',
        rows: [{ ticker: 'VLO', name: 'Valero', pl: '-0.88%', pl_positive: false, action: 'Hold' }],
      },
      immediate_eliminations: {
        heading: 'Immediate eliminations',
        bullets: Array.from({ length: 9 }, (_, i) => ({ lead: `AB${i}`, body: '−98.52% — deep loss.' })),
      },
    },
    {
      kind: 'coda',
      numeral: 'IV',
      tempo: 'Coda',
      title_main: 'The',
      title_accent: 'Coda',
      groups: [{ heading: 'Before next session', items: [{ lead: 'Sell TQQQ.', body: 'Upside ≥ 10 pts.' }] }],
    },
  ],
}

describe('GoldmanReport', () => {
  it('renders a brief with facts, cards, a ledger, a table and a checklist to a PDF', async () => {
    const pdf = await renderToBuffer(<GoldmanReport brief={brief} />)
    expect(pdf.subarray(0, 5).toString()).toBe('%PDF-')
    expect(pdf.length).toBeGreaterThan(2000)
  }, 30000)

  it('renders a comparison report: cover panel, change tables and measures', async () => {
    const comparison: GoldmanBrief = {
      client_name: 'Casey Borst',
      document_label: 'Opus 47 / Comparison',
      tempo: 'Andante',
      generated_date: 'November 11, 2026',
      subtitle: 'What changed.',
      cover_title: { main: 'The Recommendations', accent: 'Before & After' },
      comparison: {
        account_type: '401(k)',
        prices_as_of: 'November 11, 2026, 2:41 PM ET',
        original: { name: 'Current', positions: 2, holdings_value: 18388, cash: null, total_value: null, unpriced: ['YOURF'] },
        recommended: { name: 'Rebalanced', positions: 1, holdings_value: 6272, cash: null, total_value: null, unpriced: [] },
      },
      movements: [
        {
          kind: 'changes',
          numeral: 'I',
          tempo: 'Allegro',
          title_main: 'The',
          title_accent: 'Recommendations',
          rows: [
            { ticker: 'TQQQ', kind: 'sold', change: 'Sold all 3', before: 3, after: 0, price: 6129.4, value_today: 18388, reason: 'Leveraged.' },
            { ticker: 'YOURF', kind: 'sold', change: 'Sold all 5,000', before: 5000, after: 0, price: null, value_today: null, reason: '' },
            { ticker: 'VOO', kind: 'bought', change: 'Bought 9', before: 0, after: 9, price: 696.94, value_today: 6272, reason: 'Core.' },
          ],
        },
        {
          kind: 'measures',
          numeral: 'II',
          tempo: 'Andante',
          title_main: 'Before &',
          title_accent: 'After',
          original_label: 'Current',
          recommended_label: 'Rebalanced',
          rows: [{ label: 'Positions', original: '2', recommended: '1' }],
          note: 'Percentages are shares of holdings value.',
        },
      ],
    }
    const pdf = await renderToBuffer(<GoldmanReport brief={comparison} />)
    expect(pdf.subarray(0, 5).toString()).toBe('%PDF-')
  }, 30000)

  it('renders the metrics and health movements of a comparison report', async () => {
    const fixture = JSON.parse(
      readFileSync(new URL('./__fixtures__/comparison-with-metrics.json', import.meta.url), 'utf8')
    ) as GoldmanBrief
    const kinds = fixture.movements.map((m) => m.kind)
    expect(kinds).toEqual(['changes', 'measures', 'metrics', 'health', 'diagnosis', 'coda'])

    const without: GoldmanBrief = {
      ...fixture,
      movements: fixture.movements.filter((m) => m.kind !== 'metrics' && m.kind !== 'health'),
    }
    const base = await renderToBuffer(<GoldmanReport brief={without} />)
    const full = await renderToBuffer(<GoldmanReport brief={fixture} />)
    expect(full.subarray(0, 5).toString()).toBe('%PDF-')
    // Each new movement starts on its own page.
    expect(pageCount(full)).toBeGreaterThanOrEqual(pageCount(base) + 2)

    const text = pdfText(full)
    expect(text).toContain('MonteCarlo')
    expect(text).toContain('Health')
    // The metrics table, the monthly grid, a drawdown row and the odds line, as the app prints them.
    expect(text).toContain(squeeze('CAGR'))
    expect(text).toContain(squeeze('12.3%'))
    expect(text).toContain(squeeze('Monthly returns'))
    expect(text).toContain(squeeze('Mar 13, 2026'))
    expect(text).toContain(squeeze('odds of a -20% drawdown 23%'))
    expect(text).toContain(squeeze('odds of +10% or better 68%'))
    // Health rows carry both sides and the threshold.
    expect(text).toContain(squeeze('Largest position'))
    expect(text).toContain(squeeze('TQQQ · 74.6%'))
    expect(text).toContain(squeeze('warn above 10%, fail above 25% of holdings value'))
    expect(text).toContain(squeeze('No expense ratio on record for TQQQ'))
  }, 60000)

  // Visual check of a real brief: GOLDMAN_BRIEF_JSON=<brief.json> GOLDMAN_PDF_OUT=<out.pdf>
  it.runIf(!!process.env.GOLDMAN_BRIEF_JSON && !!process.env.GOLDMAN_PDF_OUT)(
    'renders the brief at GOLDMAN_BRIEF_JSON to GOLDMAN_PDF_OUT',
    async () => {
      const real = JSON.parse(readFileSync(process.env.GOLDMAN_BRIEF_JSON as string, 'utf8')) as GoldmanBrief
      writeFileSync(process.env.GOLDMAN_PDF_OUT as string, await renderToBuffer(<GoldmanReport brief={real} />))
    },
    30000
  )
})
