import { afterEach, describe, expect, it } from 'vitest'
import { listDate, logTime, runTime } from './listDate'

/**
 * WP8 Phase D, Task D4: every time the Flow pages show is America/New_York,
 * 12-hour with AM/PM and the "ET" label (CLAUDE.md). The server stores run
 * and workflow times as naive UTC (`2026-11-12T19:35:02`, no `Z`); the time
 * inside a run's log line is naive Eastern already (the engine writes
 * `iso_market`), so it is shown as it is, never shifted.
 */

const ORIGINAL_TZ = process.env.TZ

afterEach(() => {
  process.env.TZ = ORIGINAL_TZ
})

describe('listDate', () => {
  it('reads the server time as UTC and shows it in Eastern time, 12-hour, with ET', () => {
    // 19:35 UTC on 12 Nov 2026 is 2:35 PM EST (daylight time ended 1 Nov).
    expect(listDate('2026-11-12T19:35:02')).toBe('Nov 12, 2026, 2:35 PM ET')
    // 19:35 UTC on 1 Jul 2026 is 3:35 PM EDT.
    expect(listDate('2026-07-01T19:35:00')).toBe('Jul 1, 2026, 3:35 PM ET')
  })

  it('shows the Eastern date, not the UTC one, in the evening', () => {
    // 01:30 UTC on 13 Nov is 8:30 PM on 12 Nov in New York.
    expect(listDate('2026-11-13T01:30:00')).toBe('Nov 12, 2026, 8:30 PM ET')
  })

  it('keeps a time that carries its own zone', () => {
    expect(listDate('2026-11-12T19:35:02Z')).toBe('Nov 12, 2026, 2:35 PM ET')
    expect(listDate('2026-11-12T14:35:02-05:00')).toBe('Nov 12, 2026, 2:35 PM ET')
  })

  it('shows midnight and noon as 12', () => {
    // 05:00 UTC on 12 Nov is midnight EST; 17:00 UTC is noon.
    expect(listDate('2026-11-12T05:00:00')).toBe('Nov 12, 2026, 12:00 AM ET')
    expect(listDate('2026-11-12T17:00:00')).toBe('Nov 12, 2026, 12:00 PM ET')
  })
})

describe('runTime', () => {
  it("shows a run's stored start (naive UTC) as Eastern month, day and time with seconds", () => {
    expect(runTime('2026-11-12T19:35:02')).toBe('Nov 12, 2:35:02 PM ET')
    // 13:35:00 UTC on 30 Oct 2026 is 9:35 AM EDT.
    expect(runTime('2026-10-30T13:35:00')).toBe('Oct 30, 9:35:00 AM ET')
  })

  it('answers a dash for a run with no time', () => {
    expect(runTime(null)).toBe('—')
    expect(runTime('')).toBe('—')
  })
})

describe('logTime', () => {
  it("shows a log line's naive Eastern time as it is, 12-hour, with ET", () => {
    expect(logTime('2026-10-30T09:20:05')).toBe('9:20:05 AM ET')
    expect(logTime('2026-10-30T15:04:09.123456')).toBe('3:04:09 PM ET')
    expect(logTime('2026-10-30T00:05:00')).toBe('12:05:00 AM ET')
    expect(logTime('2026-10-30T12:00:00')).toBe('12:00:00 PM ET')
  })

  it("never shifts a naive line by this computer's time zone", () => {
    process.env.TZ = 'Asia/Kolkata'
    expect(logTime('2026-10-30T09:20:05')).toBe('9:20:05 AM ET')
    process.env.TZ = 'America/Los_Angeles'
    expect(logTime('2026-10-30T15:04:09')).toBe('3:04:09 PM ET')
  })

  it("converts a line that carries its zone (the editor's own lines) to Eastern", () => {
    // 13:20:05 UTC on 30 Oct 2026 is 9:20:05 AM EDT.
    expect(logTime('2026-10-30T13:20:05.000Z')).toBe('9:20:05 AM ET')
  })

  it('shows "Invalid Date" for a time it cannot read, as before', () => {
    expect(logTime('')).toBe('Invalid Date')
    expect(logTime('not a time')).toBe('Invalid Date')
    expect(logTime('09:20:05')).toBe('Invalid Date')
  })
})
