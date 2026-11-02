import { describe, expect, it } from 'vitest'
import { parseActionsFromMarkdown } from './parseActions'

function block(items: unknown[]): string {
  return `Briefing text\n\n<!-- ACTIONS_JSON\n${JSON.stringify(items)}\n-->\n`
}

const protectItem = {
  symbol: 'BAC',
  exchange: 'NYSE',
  side: 'SELL',
  quantity: 13,
  orderType: 'TRAILING_STOP',
  product: 'CNC',
  price: 0,
  trailPercent: 4.2,
  validity: 'DAY',
  group: 'rotation-1',
  role: 'protect',
  rationale: 'Trail 4.2%',
}

const fundItem = {
  symbol: 'PFE',
  exchange: 'NYSE',
  side: 'SELL',
  quantity: 11,
  orderType: 'MARKET',
  product: 'CNC',
  price: 0,
  validity: 'DAY',
  group: 'rotation-1',
  role: 'fund',
  rationale: 'Dormant',
}

const cancelItem = {
  symbol: 'NVDA',
  exchange: 'NASDAQ',
  side: 'SELL',
  group: 'protect-NVDA',
  role: 'cancel',
  cancelOrderId: '3f9c2b1e-7a4d-4c8e-9b0f-5d6e7a8b9c01',
  rationale: 'Cancel the DAY stop',
}

describe('parseActionsFromMarkdown strategy fields', () => {
  it('forces GTC on trailing stops and keeps group/role', () => {
    const [rec] = parseActionsFromMarkdown(block([protectItem]), 'briefing')
    expect(rec.orderType).toBe('TRAILING_STOP')
    expect(rec.validity).toBe('GTC')
    expect(rec.group).toBe('rotation-1')
    expect(rec.role).toBe('protect')
    expect(rec.trailPercent).toBe(4.2)
  })

  it('passes DAY through for a market fund sell', () => {
    const [rec] = parseActionsFromMarkdown(block([fundItem]), 'briefing')
    expect(rec.validity).toBe('DAY')
    expect(rec.group).toBe('rotation-1')
    expect(rec.role).toBe('fund')
    expect(rec.quantity).toBe(11)
  })

  it('keeps a cancel leg with cancelOrderId and drops one without', () => {
    const noId = { ...cancelItem, cancelOrderId: undefined }
    const emptyId = { ...cancelItem, cancelOrderId: '' }
    const recs = parseActionsFromMarkdown(block([cancelItem, noId, emptyId]), 'briefing')
    expect(recs).toHaveLength(1)
    expect(recs[0].cancelOrderId).toBe(cancelItem.cancelOrderId)
  })

  it('emits a cancel leg with quantity 0, its role, cancelOrderId and group', () => {
    const [rec] = parseActionsFromMarkdown(block([cancelItem]), 'briefing')
    expect(rec.quantity).toBe(0)
    expect(rec.role).toBe('cancel')
    expect(rec.cancelOrderId).toBe(cancelItem.cancelOrderId)
    expect(rec.group).toBe('protect-NVDA')
    expect(rec.price).toBe(0)
    expect(rec.orderType).toBe('MARKET')
  })

  it('defaults validity to DAY and leaves role/group unset for an ordinary item', () => {
    const plain = {
      symbol: 'aapl',
      side: 'BUY',
      quantity: 3,
      orderType: 'LIMIT',
      price: 100,
      rationale: 'x',
    }
    const [rec] = parseActionsFromMarkdown(block([plain]), 'copilot')
    expect(rec.validity).toBe('DAY')
    expect(rec.role).toBeUndefined()
    expect(rec.group).toBeUndefined()
    expect(rec.cancelOrderId).toBeUndefined()
  })

  it('honors GTC on non-trailing items and ignores an unknown role', () => {
    const item = { ...fundItem, validity: 'GTC', role: 'bogus' }
    const [rec] = parseActionsFromMarkdown(block([item]), 'briefing')
    expect(rec.validity).toBe('GTC')
    expect(rec.role).toBeUndefined()
  })

  it('always emits a protect as a SELL', () => {
    const [rec] = parseActionsFromMarkdown(block([{ ...protectItem, side: 'BUY' }]), 'briefing')
    expect(rec.role).toBe('protect')
    expect(rec.side).toBe('SELL')
  })

  it('skips a protect that is not a TRAILING_STOP, SL or SL-M order', () => {
    const market = { ...protectItem, symbol: 'KO', orderType: 'MARKET' }
    const limit = { ...protectItem, symbol: 'T', orderType: 'LIMIT', price: 20 }
    const untyped = { ...protectItem, symbol: 'F', orderType: undefined }
    const slm = { ...protectItem, symbol: 'PFE', orderType: 'SL-M', triggerPrice: 25, trailPercent: undefined }
    const recs = parseActionsFromMarkdown(block([market, limit, untyped, protectItem, slm]), 'briefing')
    expect(recs.map((r) => `${r.symbol}:${r.orderType}`)).toEqual(['BAC:TRAILING_STOP', 'PFE:SL-M'])
  })

  it('reads every ACTIONS_JSON block and skips a block that is not an array', () => {
    const text = `${block([fundItem])}<!-- ACTIONS_JSON\n{"symbol":"X"}\n-->\n${block([protectItem])}`
    const recs = parseActionsFromMarkdown(text, 'briefing')
    expect(recs.map((r) => r.symbol)).toEqual(['PFE', 'BAC'])
    // Repeat calls start from the top of the text each time.
    expect(parseActionsFromMarkdown(text, 'briefing')).toHaveLength(2)
  })
})
