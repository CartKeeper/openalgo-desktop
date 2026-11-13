/**
 * WP8 Phase B follow-up: a new node starts on this app's US market. A new
 * Start node runs daily at 9:35 AM ET inside the 9:30 AM to 4:00 PM ET
 * session on the US calendar; a new Time Window is that session; every
 * equity or order node starts on the `US` venue, where its product follows
 * the exchange to CNC. The option, index and margin nodes keep OpenAlgo
 * web's defaults until Phase D's US lookups, and the Indian exchanges stay
 * selectable. The run reads the same defaults for a field that is missing
 * (src-tauri flow::node_contracts::{default_exchange, default_schedule_time,
 * default_time_window}).
 */

import { describe, expect, it } from 'vitest'
import { DEFAULT_NODE_DATA, defaultProductForExchange, EXCHANGES } from './constants'

const defaults = DEFAULT_NODE_DATA as Record<string, Record<string, unknown>>

/** The equity and order nodes whose default exchange was the web's NSE. */
const EQUITY_AND_ORDER_NODES = [
  'priceAlert',
  'placeOrder',
  'smartOrder',
  'subscribeLtp',
  'subscribeQuote',
  'subscribeDepth',
  'unsubscribe',
  'closePositions',
  'splitOrder',
  'positionCheck',
  'priceCondition',
  'getQuote',
  'getDepth',
  'history',
  'indicator',
  'priorPeriodOhlc',
  'barOffset',
  'openPosition',
  'symbol',
  'multiQuotes',
]

describe('a new node starts on the US market (WP8)', () => {
  it('a new Start node runs daily at 09:35 inside 09:30 to 16:00 on the US calendar', () => {
    expect(DEFAULT_NODE_DATA.start).toEqual({
      scheduleType: 'daily',
      time: '09:35',
      marketHoursOnly: true,
      marketHoursStart: '09:30',
      marketHoursEnd: '16:00',
      marketHoursExchange: 'US',
    })
  })

  it('a new Time Window is the US regular session', () => {
    expect(DEFAULT_NODE_DATA.timeWindow).toEqual({ startTime: '09:30', endTime: '16:00' })
  })

  it.each(EQUITY_AND_ORDER_NODES)('%s starts on US, its product following to CNC', (node) => {
    expect(defaults[node].exchange).toBe('US')
    expect(defaults[node].product).toBeUndefined()
    expect(defaultProductForExchange(defaults[node].exchange as string)).toBe('CNC')
  })

  it.each([
    ['optionsOrder', 'NSE_INDEX', 'NRML'],
    ['optionsMultiOrder', 'NSE_INDEX', 'NRML'],
    ['optionSymbol', 'NSE_INDEX', undefined],
    ['optionChain', 'NSE_INDEX', undefined],
    ['syntheticFuture', 'NSE_INDEX', undefined],
    ['expiry', 'NFO', undefined],
    ['margin', 'NSE', undefined],
  ])('%s keeps the web default until Phase D (%s)', (node, exchange, product) => {
    expect(defaults[node].exchange).toBe(exchange)
    expect(defaults[node].product).toBe(product)
  })

  it('keeps the Indian exchanges selectable beside the US venues', () => {
    const offered = EXCHANGES.map((e) => e.value)
    for (const exchange of ['NSE', 'BSE', 'NFO', 'BFO', 'MCX', 'US', 'NASDAQ', 'NYSE']) {
      expect(offered).toContain(exchange)
    }
  })
})
