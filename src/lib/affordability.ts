import { fundsCommands, quoteCommands } from '@/api/tauri-client'
import type { OrderRecommendation } from '@/types/actionQueue'

/**
 * Result of capping a set of recommendations to the account's available cash.
 */
export interface AffordabilityResult {
  /** Recommendations after capping/dropping BUY quantities. Non-BUY items pass through untouched. */
  items: OrderRecommendation[]
  /** Available cash the cap was computed against, or null if it couldn't be read. */
  availableCash: number | null
  /** A user-facing notice describing any adjustments, or null when nothing changed and funds were verified. */
  notice: string | null
}

const usd = (n: number) =>
  n.toLocaleString('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 2 })

/**
 * Per-share price a BUY will fill at, for affordability purposes.
 *   - LIMIT / SL: the supplied limit price.
 *   - MARKET / SL-M / TRAILING_STOP (or any 0-priced order): needs a live quote.
 * Returns null when no usable price is known.
 */
function knownUnitPrice(rec: OrderRecommendation): number | null {
  if ((rec.orderType === 'LIMIT' || rec.orderType === 'SL') && rec.price > 0) {
    return rec.price
  }
  return null
}

/**
 * Confirm the account can afford each BUY before its quantity is suggested.
 *
 * Treats available cash as ONE shared budget and walks the recommendations in
 * order, subtracting each BUY's cost (whole shares only). When a BUY would push
 * the running total past available cash, its quantity is reduced to the most
 * whole shares the remaining cash allows; if not even one share fits, the BUY is
 * dropped. SELL and stop orders never consume the budget and pass through
 * unchanged.
 *
 * Pricing: LIMIT/SL buys use their limit price. MARKET-style buys are priced
 * from a live quote (LTP). A buy that still can't be priced is left as-is and
 * flagged in the notice rather than fabricated or silently dropped.
 *
 * If available cash itself can't be read, no quantities are changed but the
 * notice makes clear the funds weren't verified.
 */
export async function capBuysToAvailableCash(
  recs: OrderRecommendation[]
): Promise<AffordabilityResult> {
  const buys = recs.filter((r) => r.side === 'BUY')
  if (buys.length === 0) {
    return { items: recs, availableCash: null, notice: null }
  }

  // 1. Read available cash. If this fails we cannot confirm anything.
  let availableCash: number
  try {
    const funds = await fundsCommands.getFunds()
    availableCash = funds.available_cash
  } catch {
    return {
      items: recs,
      availableCash: null,
      notice: "Couldn't verify your available cash — confirm funds before placing these buys.",
    }
  }

  // 2. Price any MARKET-style buys from live quotes (LIMIT/SL already carry a price).
  const needQuote = buys.filter((r) => knownUnitPrice(r) === null)
  const quotePrices = new Map<string, number>()
  if (needQuote.length > 0) {
    try {
      const quotes = await quoteCommands.getQuote(
        needQuote.map((r) => ({ exchange: r.exchange, symbol: r.symbol }))
      )
      for (const q of quotes) {
        if (q.ltp > 0) quotePrices.set(q.symbol.toUpperCase(), q.ltp)
      }
    } catch {
      // Leave unpriced; handled below as "unverified".
    }
  }

  const unitPriceFor = (rec: OrderRecommendation): number | null =>
    knownUnitPrice(rec) ?? quotePrices.get(rec.symbol.toUpperCase()) ?? null

  // 3. Walk the recommendations, spending the shared budget on BUYs in order.
  let remaining = availableCash
  const adjusted: string[] = []
  const dropped: string[] = []
  const unverified: string[] = []
  const out: OrderRecommendation[] = []

  for (const rec of recs) {
    if (rec.side !== 'BUY') {
      out.push(rec)
      continue
    }

    const unit = unitPriceFor(rec)
    if (unit === null || unit <= 0) {
      // Can't confirm a price — keep the pick but don't pretend it's verified,
      // and don't let an unknown cost consume the shared budget.
      unverified.push(rec.symbol)
      out.push(rec)
      continue
    }

    const affordableQty = Math.floor(remaining / unit)
    if (affordableQty <= 0) {
      dropped.push(rec.symbol)
      continue
    }

    if (affordableQty < rec.quantity) {
      adjusted.push(`${rec.symbol} ${rec.quantity}→${affordableQty}`)
      out.push({ ...rec, quantity: affordableQty })
      remaining -= affordableQty * unit
    } else {
      out.push(rec)
      remaining -= rec.quantity * unit
    }
  }

  // 4. Build a notice only when something actually warrants surfacing.
  const parts: string[] = []
  if (adjusted.length > 0) {
    parts.push(`reduced ${adjusted.join(', ')} to fit your available cash (${usd(availableCash)})`)
  }
  if (dropped.length > 0) {
    parts.push(`removed ${dropped.join(', ')} — not enough cash for a single share`)
  }
  if (unverified.length > 0) {
    parts.push(`couldn't confirm a live price for ${unverified.join(', ')}; verify funds before placing`)
  }
  const notice = parts.length > 0 ? `Quantities checked against available cash: ${parts.join('; ')}.` : null

  return { items: out, availableCash, notice }
}
