import { readFileSync, writeFileSync } from 'node:fs'
import { renderToBuffer } from '@react-pdf/renderer'
import { describe, expect, it } from 'vitest'
import { GoldmanReport } from './GoldmanReport'
import type { GoldmanBrief } from './types'

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
