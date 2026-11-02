import {
  fundsCommands,
  holdingsCommands,
  orderCommands,
  positionCommands,
  quoteCommands,
} from '@/api/tauri-client'
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

/** Whole-dollar currency for the funded-buy notice, e.g. 334.18 → "$334". */
const usdWhole = (n: number) =>
  Math.round(n).toLocaleString('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  })

/** Broker order statuses (uppercase, as the Rust order book serializes them) that can still be cancelled. */
const OPEN_ORDER_STATUSES = new Set(['OPEN', 'PARTIALLY_FILLED', 'PENDING'])

/** A strategy `cancel` leg: carries a broker order id, no quantity, never a real sell. */
const isCancel = (rec: OrderRecommendation): boolean => rec.role === 'cancel'

/** A sell that turns shares into cash (MARKET/LIMIT). Stops and cancel legs do not. */
const isLiquidatingSell = (rec: OrderRecommendation): boolean =>
  rec.side === 'SELL' && (rec.orderType === 'MARKET' || rec.orderType === 'LIMIT') && !isCancel(rec)

/** A sell-side protective stop. */
const isSellStop = (rec: OrderRecommendation): boolean =>
  rec.side === 'SELL' &&
  (rec.orderType === 'SL' || rec.orderType === 'SL-M' || rec.orderType === 'TRAILING_STOP')

/** Key a group's buy by symbol so its protect stop can be sized to it. */
const groupSymbolKey = (group: string, symbol: string): string => `${group}|${symbol.toUpperCase()}`

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

/** Live prices (uppercase symbol → ltp) for the given items; empty on any failure. */
async function readQuotePrices(items: OrderRecommendation[]): Promise<Map<string, number>> {
  const prices = new Map<string, number>()
  if (items.length === 0) return prices
  try {
    const quotes = await quoteCommands.getQuote(
      items.map((r) => ({ exchange: r.exchange, symbol: r.symbol }))
    )
    for (const q of quotes) if (q.ltp > 0) prices.set(q.symbol.toUpperCase(), q.ltp)
  } catch {
    // Unpriced items are flagged / given no proceeds by the walk.
  }
  return prices
}

/**
 * Ids of broker orders that are still open, or null if the open orders couldn't be read.
 * Reads the broker's open orders (not the recent-orders window, where an old working
 * GTC stop can scroll out of view and look closed).
 */
async function readOpenOrderIds(): Promise<Set<string> | null> {
  try {
    const orders = await orderCommands.getOpenOrders()
    if (!Array.isArray(orders)) return null
    return new Set(orders.filter((o) => OPEN_ORDER_STATUSES.has(o.status)).map((o) => o.order_id))
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
 *  - Liquidating sells (MARKET/LIMIT) add their proceeds (post-cap quantity x limit
 *    price or live quote) to the same budget as the walk reaches them, so a sell
 *    earlier in the batch can fund a later buy; a sell after a buy does not. Stops
 *    never add proceeds, and an unpriceable sell adds nothing.
 *  - A `protect` stop whose group has an earlier surviving `buy` of the same symbol
 *    is capped to that buy's final quantity instead of to held shares.
 *  - A TRAILING_STOP with no trail distance is dropped. A `cancel` leg is dropped
 *    when its order is no longer open in the order book (kept, flagged, if the
 *    order book can't be read); cancel legs skip the held/budget logic entirely.
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
  const cancels = recs.filter(isCancel)
  const sells = recs.filter((r) => r.side === 'SELL' && !isCancel(r))

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

  // Price MARKET-style buys and liquidating sells from live quotes (LIMIT/SL already carry a price).
  const needQuote = recs.filter(
    (r) => (r.side === 'BUY' || isLiquidatingSell(r)) && knownUnitPrice(r) === null
  )
  const quotePromise = readQuotePrices(needQuote)

  // Only cancel legs need the order book.
  const openOrderIdsPromise: Promise<Set<string> | null> =
    cancels.length > 0 ? readOpenOrderIds() : Promise.resolve(null)

  const [availableCash, heldBySymbol, quotePrices, openOrderIds] = await Promise.all([
    cashPromise,
    heldPromise,
    quotePromise,
    openOrderIdsPromise,
  ])

  const unitPriceFor = (rec: OrderRecommendation): number | null =>
    knownUnitPrice(rec) ?? quotePrices.get(rec.symbol.toUpperCase()) ?? null

  // --- Walk recommendations in order ---------------------------------------
  let remaining = availableCash ?? 0
  // Sale proceeds credited to `remaining` so far (drives the cap wording).
  let proceedsCredited = 0
  const buyAdjusted: string[] = []
  // Reduced buys whose cap included sale proceeds, keyed by the cash + proceeds budget at that point.
  const buyAdjustedWithProceeds = new Map<number, string[]>()
  const buyDropped: string[] = []
  const buyUnverified: string[] = []
  const sellAdjusted: string[] = []
  const sellDropped: string[] = []
  const protectAdjusted: string[] = []
  const trailDropped: string[] = []
  const cancelDropped: string[] = []
  const fundedNotes: string[] = []
  // Shares not yet claimed by an earlier kept sell/stop, per symbol: two sells of the
  // same 11 shares must not both be kept (nor both credit proceeds).
  const heldLeft: Record<string, number> = { ...(heldBySymbol ?? {}) }
  // Final (post-cap) quantity of each surviving buy, keyed by group + symbol.
  const boughtQty = new Map<string, number>()
  // Kept, priced fund sells per group, for the funded-buy notice.
  const fundSells = new Map<string, { symbols: string[]; proceeds: number }>()
  const out: OrderRecommendation[] = []

  for (const rec of recs) {
    if (isCancel(rec)) {
      // Cancel legs only need their target order to still be open.
      if (openOrderIds !== null && !openOrderIds.has(rec.cancelOrderId ?? '')) {
        cancelDropped.push(rec.symbol)
      } else {
        out.push(rec)
      }
      continue
    }

    if (rec.side === 'SELL') {
      if (rec.orderType === 'TRAILING_STOP' && !((rec.trailPercent ?? 0) > 0)) {
        trailDropped.push(rec.symbol)
        continue
      }

      // A protect stop is sized to its group's surviving buy, not to held shares.
      const boughtKey = rec.role === 'protect' && rec.group ? groupSymbolKey(rec.group, rec.symbol) : null
      const bought = boughtKey !== null && isSellStop(rec) ? boughtQty.get(boughtKey) : undefined
      if (bought !== undefined) {
        if (rec.quantity > bought) {
          protectAdjusted.push(`${rec.symbol} ${rec.quantity}→${bought}`)
          out.push({ ...rec, quantity: bought })
        } else {
          out.push(rec)
        }
        continue
      }

      // Cap sells/stops to actually-held shares.
      if (heldBySymbol === null) {
        // Couldn't read holdings — keep the pick but flag it below (no proceeds counted).
        out.push(rec)
        continue
      }
      const heldKey = rec.symbol.toUpperCase()
      const held = Math.floor(heldLeft[heldKey] ?? 0)
      if (held <= 0) {
        sellDropped.push(rec.symbol)
        continue
      }
      let sellQty = rec.quantity
      if (rec.quantity > held) {
        sellAdjusted.push(`${rec.symbol} ${rec.quantity}→${held}`)
        sellQty = held
        out.push({ ...rec, quantity: held })
      } else {
        out.push(rec)
      }
      heldLeft[heldKey] = held - sellQty

      // A liquidating sell funds later buys with its (post-cap) proceeds.
      if (isLiquidatingSell(rec)) {
        const unit = unitPriceFor(rec)
        if (unit !== null && unit > 0) {
          const proceeds = sellQty * unit
          remaining += proceeds
          proceedsCredited += proceeds
          if (rec.role === 'fund' && rec.group) {
            const entry = fundSells.get(rec.group) ?? { symbols: [], proceeds: 0 }
            entry.symbols.push(rec.symbol)
            entry.proceeds += proceeds
            fundSells.set(rec.group, entry)
          }
        }
      }
      continue
    }

    // BUY: cap cumulatively to available cash.
    const rememberBuy = (qty: number) => {
      if (rec.group) boughtQty.set(groupSymbolKey(rec.group, rec.symbol), qty)
    }
    if (availableCash === null) {
      // Couldn't read cash — keep the pick but flag the buys as unverified.
      buyUnverified.push(rec.symbol)
      rememberBuy(rec.quantity)
      out.push(rec)
      continue
    }
    const unit = unitPriceFor(rec)
    if (unit === null || unit <= 0) {
      buyUnverified.push(rec.symbol)
      rememberBuy(rec.quantity)
      out.push(rec)
      continue
    }
    const affordableQty = Math.floor(remaining / unit + 1e-9)
    if (affordableQty <= 0) {
      buyDropped.push(rec.symbol)
      continue
    }
    const remainingBefore = remaining
    const keptQty = Math.min(rec.quantity, affordableQty)
    if (keptQty < rec.quantity) {
      const label = `${rec.symbol} ${rec.quantity}→${keptQty}`
      if (proceedsCredited > 0) {
        // The wording must say when sale proceeds (not just cash) set the cap.
        const budget = availableCash + proceedsCredited
        buyAdjustedWithProceeds.set(budget, [...(buyAdjustedWithProceeds.get(budget) ?? []), label])
      } else {
        buyAdjusted.push(label)
      }
      out.push({ ...rec, quantity: keptQty })
    } else {
      out.push(rec)
    }
    remaining -= keptQty * unit
    rememberBuy(keptQty)

    const funding = rec.group ? fundSells.get(rec.group) : undefined
    if (funding) {
      const cashPart = Math.max(0, remainingBefore - funding.proceeds)
      fundedNotes.push(
        `${rec.symbol} ×${keptQty} is funded by selling ${funding.symbols.join(', ')} (about ${usdWhole(funding.proceeds)}) plus ${usdWhole(cashPart)} cash`
      )
    }
  }

  // --- Build a single notice -----------------------------------------------
  const parts: string[] = []
  if (buyAdjusted.length > 0 && availableCash !== null) {
    parts.push(`reduced ${buyAdjusted.join(', ')} to fit your available cash (${usd(availableCash)})`)
  }
  for (const [budget, labels] of buyAdjustedWithProceeds) {
    parts.push(`reduced ${labels.join(', ')} to fit your available cash plus sale proceeds (${usd(budget)})`)
  }
  if (buyDropped.length > 0) {
    parts.push(`removed ${buyDropped.join(', ')} — not enough cash for a single share`)
  }
  parts.push(...fundedNotes)
  if (sellAdjusted.length > 0) {
    parts.push(`reduced ${sellAdjusted.join(', ')} to the shares you actually hold`)
  }
  if (protectAdjusted.length > 0) {
    parts.push(`reduced ${protectAdjusted.join(', ')} to match its buy`)
  }
  if (sellDropped.length > 0) {
    parts.push(`removed sell of ${sellDropped.join(', ')} — you don't hold any shares`)
  }
  if (trailDropped.length > 0) {
    parts.push(`removed trailing stop for ${trailDropped.join(', ')} — no trail distance given`)
  }
  if (cancelDropped.length > 0) {
    parts.push(`removed cancel for ${cancelDropped.join(', ')} — that order is no longer open`)
  }
  if (buyUnverified.length > 0) {
    parts.push(`couldn't confirm cash/price for ${buyUnverified.join(', ')}; verify funds before placing`)
  }
  if (sells.length > 0 && heldBySymbol === null) {
    parts.push(`couldn't confirm your holdings; verify positions before placing any sell`)
  }
  if (cancels.length > 0 && openOrderIds === null) {
    parts.push(`couldn't confirm open orders; verify the Order Book before cancelling`)
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
