/**
 * The PDF's built-in Times/Helvetica fonts only cover the WinAnsi character set.
 * Anything outside it is dropped silently — "−0.88%" printed as "0.88%".
 * These replacements keep the meaning with characters the fonts can draw.
 */
const REPLACEMENTS: Record<string, string> = {
  '−': '-', // minus sign
  '‐': '-', // hyphen
  '‑': '-', // non-breaking hyphen
  '≤': '<=',
  '≥': '>=',
  '≠': '!=',
  '≈': '~',
  '→': '->',
  '←': '<-',
  '↑': 'up',
  '↓': 'down',
  '▲': 'up',
  '▼': 'down',
  '✓': 'yes',
  '✔': 'yes',
  '✗': 'no',
  '♪': '',
  '♫': '',
  '□': '',
  '☐': '',
  ' ': ' ',
  ' ': ' ',
  ' ': ' ',
}

const PATTERN = new RegExp(`[${Object.keys(REPLACEMENTS).join('')}]`, 'g')

/** A string the PDF fonts can draw in full. */
export function pdfSafe(text: string): string {
  return text.replace(PATTERN, (ch) => REPLACEMENTS[ch] ?? ch)
}

/** Apply `pdfSafe` to every string in a JSON-shaped value. */
export function pdfSafeDeep<T>(value: T): T {
  if (typeof value === 'string') return pdfSafe(value) as T
  if (Array.isArray(value)) return value.map((v) => pdfSafeDeep(v)) as T
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([k, v]) => [k, pdfSafeDeep(v)])
    ) as T
  }
  return value
}
