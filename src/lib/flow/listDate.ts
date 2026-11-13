/**
 * Times on the Flow pages, in America/New_York, 12-hour with AM/PM and the
 * "ET" label, whatever this computer's time zone (WP8 Phase D, Task D4;
 * CLAUDE.md's display rule). Upstream showed the list's dates in IST and the
 * Execution Log in 24-hour time.
 *
 * Two kinds of stored time reach the pages:
 * - Workflow and run times (`updated_at`, `started_at`, `completed_at`) are
 *   UTC without an offset, as the web's `isoformat()` writes them: read as
 *   UTC, then shown in Eastern time.
 * - The `time` of a run's log line is already Eastern wall-clock time
 *   without an offset (the engine writes `iso_market`): shown as it is,
 *   never shifted. A line the editor writes itself carries its zone (`Z`)
 *   and is converted.
 */

const MARKET_TZ = 'America/New_York'
const HAS_ZONE = /(?:Z|[+-]\d{2}:?\d{2})$/i
const NAIVE = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?$/

/** A stored time as an instant: naive text is UTC. */
function utcInstant(stored: string): Date {
  return new Date(HAS_ZONE.test(stored) ? stored : `${stored}Z`)
}

/** The Eastern wall-clock parts of `at`. */
function easternParts(at: Date, withYear: boolean): Record<string, string> {
  const parts = new Intl.DateTimeFormat('en-US', {
    timeZone: MARKET_TZ,
    month: 'short',
    day: 'numeric',
    ...(withYear ? { year: 'numeric' as const } : {}),
    hour: 'numeric',
    minute: '2-digit',
    second: '2-digit',
    hour12: true,
  }).formatToParts(at)
  return Object.fromEntries(parts.map((p) => [p.type, p.value]))
}

/** `9:05` or `9:05:07` with AM/PM, from a 24-hour hour. */
function clock(hour24: number, minute: string, second?: string): string {
  const period = hour24 < 12 ? 'AM' : 'PM'
  const hour = hour24 % 12 === 0 ? 12 : hour24 % 12
  return `${hour}:${minute}${second === undefined ? '' : `:${second}`} ${period}`
}

/**
 * A workflow's date on the /flow list: `Nov 12, 2026, 2:35 PM ET`.
 */
export function listDate(updatedAt: string): string {
  const p = easternParts(utcInstant(updatedAt), true)
  return `${p.month} ${p.day}, ${p.year}, ${p.hour}:${p.minute} ${p.dayPeriod} ET`
}

/**
 * When a run started or ended (stored naive UTC): `Nov 12, 2:35:02 PM ET`,
 * or a dash when there is no time.
 */
export function runTime(stored: string | null | undefined): string {
  if (!stored) return '—'
  const p = easternParts(utcInstant(stored), false)
  return `${p.month} ${p.day}, ${p.hour}:${p.minute}:${p.second} ${p.dayPeriod} ET`
}

/**
 * A log line's time: `9:20:05 AM ET`. Naive text is Eastern already and is
 * shown as it is; text with a zone is converted. What cannot be read shows
 * "Invalid Date", as the panel always did.
 */
export function logTime(time: string): string {
  const naive = NAIVE.exec(time)
  if (naive) {
    return `${clock(Number(naive[4]), naive[5], naive[6])} ET`
  }
  if (!HAS_ZONE.test(time)) return 'Invalid Date'
  const at = new Date(time)
  if (Number.isNaN(at.getTime())) return 'Invalid Date'
  const p = easternParts(at, false)
  return `${p.hour}:${p.minute}:${p.second} ${p.dayPeriod} ET`
}

/**
 * How long a run took, from its stored start and end (naive UTC): `3s`,
 * `1m 5s`, `1h 2m`; a dash for a run without an end (still going) or with
 * an end before its start.
 */
export function runDuration(
  started: string | null | undefined,
  completed: string | null | undefined
): string {
  if (!started || !completed) return '—'
  const ms = utcInstant(completed).getTime() - utcInstant(started).getTime()
  if (!Number.isFinite(ms) || ms < 0) return '—'
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`
  return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`
}
