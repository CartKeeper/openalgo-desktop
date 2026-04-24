import type { OrderRecommendation } from '@/types/actionQueue'

const HEADER = 'symbol,exchange,side,quantity,price,rationale'
const ROW_TERMINATOR = '\r\n'

/**
 * Wraps a field in double quotes and doubles embedded quotes when the value
 * contains a comma, double quote, or newline. Per RFC 4180.
 * Empty strings pass through unquoted (rendered as an empty field).
 */
function escapeCsvField(value: string): string {
  if (value === '') return ''
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

/**
 * Format a list of order recommendations as RFC 4180 CSV.
 * Always returns the header row; each item contributes one data row.
 * Price is always rendered with two decimal places.
 */
export function formatRecommendationsAsCsv(items: OrderRecommendation[]): string {
  const rows = items.map((item) => {
    const fields = [
      item.symbol,
      item.exchange,
      item.side,
      String(item.quantity),
      item.price.toFixed(2),
      item.rationale,
    ].map(escapeCsvField)
    return fields.join(',')
  })

  return [HEADER, ...rows].join(ROW_TERMINATOR) + ROW_TERMINATOR
}
