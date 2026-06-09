import { fundsCommands, holdingsCommands, positionCommands, quoteCommands } from '@/api/tauri-client'
import type { OrderRecommendation } from '@/types/actionQueue'

/**
 * Result of reconciling a set of recommendations against the account: capping
 * BUYs to available cash and SELLs/stops to actually-held shares.
 */
export interface AffordabilityResult {
  /** Recommendations after capping/dropping. */
  items: OrderRecommendation[]
  /** Available cash the buy-cap was computed against, or null if it couldn't be read. */
  availableCash: number | null
  /** A user-facing notice describing any adjustments, or null when nothing changed. */
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

/** Read net long quantity held per symbol (positions + holdings), uppercase keys. */
async function readHeldLongQty(): Promise<Record<string, number> | null> {
  try {
    const [positions, holdings] = await Promise.all([
      positionCommands.getPositions().catch(() => []),
      holdingsCommands.getHoldings().catch(() => []),
    ])
    const held: Record<string, number> = {}
    for (const p of positions) {
      held[p.symbol.toUpperCase()] = (held[p.symbol.toUpperCase()] ?? 0) + p.quantity
    }
    for (const h of holdings) {
      held[h.symbol.toUpperCase()] = (held[h.symbol.toUpperCase()] ?? 0) + h.quantity
    }
    return held
  } catch {
    return null
  }
}

/**
 * Reconcile AI trade recommendations against the actual account BEFORE they are
 * shown or placed — the deterministic backstop for the model's soft rules:
 *
 *  - BUYs are capped cumulatively to available cash (whole shares). A buy that
 *    doesn't fit the remaining cash is reduced; if not even one share fits it is
 *    dropped. SELLs never consume the cash budget.
 *  - SELLs and protective stops (SL/SL-M/TRAILING_STOP on the sell side) are
 *    capped to the shares actually held. A sell on a symbol the account does not
 *    hold is dropped (the app is long-only / delivery); a sell larger than the
 *    held quantity is reduced to what is held.
 *
 * Pricing: LIMIT/SL buys use their limit price; MARKET-style buys are priced from
 * a live quote. A buy that still can't be priced is kept but flagged, never
 * fabricated. If cash or holdings can't be read, those caps are skipped and the
 * notice makes the unverified state explicit rather than hiding a pick.
 */
export async function reconcileRecommendationsWithAccount(
  recs: OrderRecommendation[]
): Promise<AffordabilityResult> {
  if (recs.length === 0) {
    return { items: recs, availableCash: null, notice: null }
  }

  const buys = recs.filter((r) => r.side === 'BUY')
  const sells = recs.filter((r) => r.side === 'SELL')

  // --- Reads (parallel) -----------------------------------------------------
  const cashPromise: Promise<number | null> =
    buys.length > 0
      ? fundsCommands
          .getFunds()
          .then((f) => f.available_cash)
          .catch(() => null)
      : Promise.resolve(null)

  const heldPromise: Promise<Record<string, number> | null> =
    sells.length > 0 ? readHeldLongQty() : Promise.resolve({})

  // Price MARKET-style buys from live quotes (LIMIT/SL already carry a price).
  const needQuote = buys.filter((r) => knownUnitPrice(r) === null)
  const quotePromise: Promise<Map<string, number>> =
    needQuote.length > 0
      ? quoteCommands
          .getQuote(needQuote.map((r) => ({ exchange: r.exchange, symbol: r.symbol })))
          .then((quotes) => {
            const m = new Map<string, number>()
            for (const q of quotes) if (q.ltp > 0) m.set(q.symbol.toUpperCase(), q.ltp)
            return m
          })
          .catch(() => new Map<string, number>())
      : Promise.resolve(new Map<string, number>())

  const [availableCash, heldBySymbol, quotePrices] = await Promise.all([
    cashPromise,
    heldPromise,
    quotePromise,
  ])

  const unitPriceFor = (rec: OrderRecommendation): number | null =>
    knownUnitPrice(rec) ?? quotePrices.get(rec.symbol.toUpperCase()) ?? null

  // --- Walk recommendations in order ---------------------------------------
  let remaining = availableCash ?? 0
  const buyAdjusted: string[] = []
  const buyDropped: string[] = []
  const buyUnverified: string[] = []
  const sellAdjusted: string[] = []
  const sellDropped: string[] = []
  const out: OrderRecommendation[] = []

  for (const rec of recs) {
    if (rec.side === 'SELL') {
      // Cap protective sells/stops to actually-held shares.
      if (heldBySymbol === null) {
        // Couldn't read holdings — keep the pick but flag it below.
        out.push(rec)
        continue
      }
      const held = Math.floor(heldBySymbol[rec.symbol.toUpperCase()] ?? 0)
      if (held <= 0) {
        sellDropped.push(rec.symbol)
        continue
      }
      if (rec.quantity > held) {
        sellAdjusted.push(`${rec.symbol} ${rec.quantity}→${held}`)
        out.push({ ...rec, quantity: held })
      } else {
        out.push(rec)
      }
      continue
    }

    // BUY: cap cumulatively to available cash.
    if (availableCash === null) {
      // Couldn't read cash — keep the pick but flag the buys as unverified.
      buyUnverified.push(rec.symbol)
      out.push(rec)
      continue
    }
    const unit = unitPriceFor(rec)
    if (unit === null || unit <= 0) {
      buyUnverified.push(rec.symbol)
      out.push(rec)
      continue
    }
    const affordableQty = Math.floor(remaining / unit)
    if (affordableQty <= 0) {
      buyDropped.push(rec.symbol)
      continue
    }
    if (affordableQty < rec.quantity) {
      buyAdjusted.push(`${rec.symbol} ${rec.quantity}→${affordableQty}`)
      out.push({ ...rec, quantity: affordableQty })
      remaining -= affordableQty * unit
    } else {
      out.push(rec)
      remaining -= rec.quantity * unit
    }
  }

  // --- Build a single notice -----------------------------------------------
  const parts: string[] = []
  if (buyAdjusted.length > 0 && availableCash !== null) {
    parts.push(`reduced ${buyAdjusted.join(', ')} to fit your available cash (${usd(availableCash)})`)
  }
  if (buyDropped.length > 0) {
    parts.push(`removed ${buyDropped.join(', ')} — not enough cash for a single share`)
  }
  if (sellAdjusted.length > 0) {
    parts.push(`reduced ${sellAdjusted.join(', ')} to the shares you actually hold`)
  }
  if (sellDropped.length > 0) {
    parts.push(`removed sell of ${sellDropped.join(', ')} — you don't hold any shares`)
  }
  if (buyUnverified.length > 0) {
    parts.push(`couldn't confirm cash/price for ${buyUnverified.join(', ')}; verify funds before placing`)
  }
  if (sells.length > 0 && heldBySymbol === null) {
    parts.push(`couldn't confirm your holdings; verify positions before placing any sell`)
  }
  const notice = parts.length > 0 ? `Checked against your account: ${parts.join('; ')}.` : null

  return { items: out, availableCash, notice }
}

/**
 * Back-compat wrapper: cap BUYs to available cash only. Prefer
 * reconcileRecommendationsWithAccount, which also caps sells to held shares.
 */
export async function capBuysToAvailableCash(
  recs: OrderRecommendation[]
): Promise<AffordabilityResult> {
  return reconcileRecommendationsWithAccount(recs)
}
