import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { renderMarkdown } from './markdown'

function render(text: string): string {
  return renderToStaticMarkup(<>{renderMarkdown(text)}</>)
}

describe('renderMarkdown', () => {
  it('never shows the hidden ACTIONS_JSON block', () => {
    const text =
      'Sell the leveraged ETF.\n\n<!-- ACTIONS_JSON\n[\n{ "symbol": "TQQQ", "side": "SELL", "quantity": 100 }\n]\n-->'
    const html = render(text)
    expect(html).toContain('Sell the leveraged ETF.')
    expect(html).not.toContain('ACTIONS_JSON')
    expect(html).not.toContain('TQQQ')
  })

  it('keeps prose written after the block', () => {
    const html = render('Before.\n<!-- ACTIONS_JSON\n[]\n-->\nAfter.')
    expect(html).toContain('Before.')
    expect(html).toContain('After.')
  })
})
