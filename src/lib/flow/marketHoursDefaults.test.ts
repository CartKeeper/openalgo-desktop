/**
 * The schedule trigger's market-hours defaults (parity N-marketHours-07):
 * a new Start node is gated to the market's session. This app (WP8): the US
 * regular session, 09:30 to 16:00 ET, on the US calendar (the web's editor
 * wrote 09:15 to 15:15 on NSE). The backend reads these keys from the saved
 * node on every run; a node saved without them runs ungated.
 */

import { describe, expect, it } from 'vitest'
import { DEFAULT_NODE_DATA } from './constants'

describe('start node market-hours defaults', () => {
  it('gate a new schedule to the 09:30-16:00 US session', () => {
    const start = DEFAULT_NODE_DATA.start
    expect(start.marketHoursOnly).toBe(true)
    expect(start.marketHoursStart).toBe('09:30')
    expect(start.marketHoursEnd).toBe('16:00')
    expect(start.marketHoursExchange).toBe('US')
  })
})
