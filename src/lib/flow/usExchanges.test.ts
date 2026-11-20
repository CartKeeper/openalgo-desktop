/**
 * WP8 Phase B: the editor offers this app's US venues, as the run accepts
 * them. The run's list is `flow::support::US_EXCHANGES` (Phase A Task A5),
 * and its default product on those venues is CNC
 * (`node_contracts::default_product_for_exchange`). Without them in the
 * editor, a US trader could not pick NASDAQ for a Get Quote or Place Order
 * node, and a node imported on NASDAQ showed MIS while the run sent CNC.
 */

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { defaultProductForExchange, EXCHANGES } from './constants'

/** The run's US venues, read from the Rust source. */
function runUsExchanges(): string[] {
  const rust = readFileSync(join(process.cwd(), 'src-tauri/src/flow/support/mod.rs'), 'utf8')
  const start = rust.indexOf('pub const US_EXCHANGES')
  expect(start).toBeGreaterThanOrEqual(0)
  const declaration = rust.slice(start, rust.indexOf(';', start))
  return [...declaration.matchAll(/"([^"]+)"/g)].map((m) => m[1])
}

const US = ['NYSE', 'NASDAQ', 'AMEX', 'ARCA', 'BATS', 'OTC', 'US']

describe("the editor's US venues (WP8)", () => {
  it("lists the run's US venues, after the web's exchanges, in the run's order", () => {
    expect(runUsExchanges()).toEqual(US)
    const offered = EXCHANGES.map((e) => e.value)
    expect(offered.slice(-US.length)).toEqual(US)
    expect(new Set(offered).size).toBe(offered.length)
  })

  it.each(US)('%s defaults to CNC, as the run does', (exchange) => {
    expect(defaultProductForExchange(exchange)).toBe('CNC')
    expect(defaultProductForExchange(` ${exchange.toLowerCase()} `)).toBe('CNC')
  })

  it("leaves the web's rule alone elsewhere", () => {
    expect(defaultProductForExchange('NSE')).toBe('MIS')
    expect(defaultProductForExchange('NFO')).toBe('NRML')
    expect(defaultProductForExchange('CRYPTO')).toBe('MIS')
    expect(defaultProductForExchange('')).toBe('MIS')
  })
})
