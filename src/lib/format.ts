/**
 * Shared money / P&L formatting helpers (USD).
 *
 * The app trades a US (Alpaca) account, so all monetary values render in USD.
 * Use these instead of locale-specific helpers to keep formatting uniform.
 */

const USD = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
})

const USD_COMPACT = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  notation: 'compact',
  maximumFractionDigits: 2,
})

/** Format a value as USD: 1234.5 -> "$1,234.50". null/NaN -> "--". */
export function formatUSD(value: number | string | null | undefined): string {
  const n = typeof value === 'string' ? parseFloat(value) : value
  if (n == null || Number.isNaN(n)) return '--'
  return USD.format(n)
}

/** Compact USD for large figures: 530420000000 -> "$530.42B". */
export function formatUSDCompact(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return '--'
  return USD_COMPACT.format(value)
}

/** Signed USD with explicit +/- for P&L: 12.3 -> "+$12.30", -4 -> "-$4.00". */
export function formatSignedUSD(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return '--'
  const sign = value > 0 ? '+' : value < 0 ? '-' : ''
  return `${sign}${USD.format(Math.abs(value))}`
}

/** 0.3477 -> "34.77%". Pass alreadyPercent=true when the value is already 0-100. */
export function formatPercent(
  value: number | null | undefined,
  alreadyPercent = false
): string {
  if (value == null || Number.isNaN(value)) return '--'
  const pct = alreadyPercent ? value : value * 100
  const sign = pct > 0 ? '+' : ''
  return `${sign}${pct.toFixed(2)}%`
}

/** Tailwind text color class for a P&L value (green up / red down / muted flat). */
export function pnlColorClass(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value) || value === 0) return 'text-muted-foreground'
  return value > 0
    ? 'text-green-600 dark:text-green-400'
    : 'text-red-600 dark:text-red-400'
}
