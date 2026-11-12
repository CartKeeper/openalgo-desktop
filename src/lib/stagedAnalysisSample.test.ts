import { describe, expect, it } from 'vitest'
import { parseActionsFromMarkdown } from '@/lib/parseActions'

/**
 * A report in the shape `staged_analysis::render::render_markdown` produces
 * (src-tauri/src/services/staged_analysis/render.rs): the sections, the
 * closing line, the disclaimer, then the ACTIONS_JSON block built from the
 * kept trades. The block's keys are alphabetical, as serde_json writes them.
 */
const SAMPLE = `## Executive summary
Trim the leveraged fund and add to the index core.

**Rating:** Overweight — for "Casey baseline" as a whole, with prices as of Nov 12, 2026 4:05 PM ET

## Thesis
Lower variance at the same expected return.

## What the analysts found
**Fundamentals** — Valuations are reasonable outside the leveraged fund.
- TQQQ: Outside the guidelines for this account type (bearish, weight 5/5; leveraged ETF)

## Bull vs bear
**Bull:** The core sleeve compounds.
**Bear:** The leveraged fund can halve.
**Research manager:** Hold the core, sell the leveraged fund.
- TQQQ — Sell: Outside the account guidelines.

## Risk panel
**Aggressive:** Add to the core.
**Conservative:** Sell the leveraged fund first.
**Neutral:** Do both, in that order.

## Trades
- **SELL 20 TQQQ** · MARKET · at market · Outside the guidelines for this account type.
- **BUY 2 VOO** · LIMIT · $500.25 · Core exposure.

## Dropped by the app's checks
- BUY 3 SOXL: SOXL is a leveraged ETF — outside the guidelines for this account type

These trades are ready under **Review Recommendations** below; nothing is applied until you review them. This scenario is the baseline, so Review applies them to a new clone and leaves the baseline untouched.

_AI-generated analysis: it can be confidently wrong and is not based on the client's full financial picture. It is not financial advice._

<!-- ACTIONS_JSON
[
  {
    "exchange": "GENERIC",
    "orderType": "MARKET",
    "price": 0.0,
    "product": "CNC",
    "quantity": 20,
    "rationale": "Outside the guidelines for this account type.",
    "side": "SELL",
    "symbol": "TQQQ"
  },
  {
    "exchange": "NYSE",
    "orderType": "LIMIT",
    "price": 500.25,
    "product": "CNC",
    "quantity": 2,
    "rationale": "Core exposure.",
    "side": "BUY",
    "symbol": "VOO"
  }
]
-->
`

describe('staged analysis report', () => {
  it('yields the kept trades through parseActionsFromMarkdown with side and quantity', () => {
    const actions = parseActionsFromMarkdown(SAMPLE, 'copilot')
    expect(actions).toHaveLength(2)
    expect(actions.map((a) => [a.side, a.quantity, a.symbol, a.orderType, a.price, a.exchange])).toEqual([
      ['SELL', 20, 'TQQQ', 'MARKET', 0, 'GENERIC'],
      ['BUY', 2, 'VOO', 'LIMIT', 500.25, 'NYSE'],
    ])
    expect(actions.every((a) => a.source === 'copilot' && a.product === 'CNC')).toBe(true)
    expect(actions[1].rationale).toBe('Core exposure.')
  })

  it('does not turn the dropped-trades section into actions', () => {
    const actions = parseActionsFromMarkdown(SAMPLE, 'copilot')
    expect(actions.some((a) => a.symbol === 'SOXL')).toBe(false)
  })
})
