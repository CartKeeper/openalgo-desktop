import type { OrderRecommendation, StrategyRole } from '@/types/actionQueue'

const STRATEGY_ROLES: readonly StrategyRole[] = ['cancel', 'fund', 'buy', 'protect', 'standalone']

/** A `protect` leg is a sell-side stop of one of these types; anything else is skipped. */
const PROTECT_ORDER_TYPES: ReadonlySet<string> = new Set(['TRAILING_STOP', 'SL', 'SL-M'])

const ACTIONS_REGEX = /<!--\s*ACTIONS_JSON\s*\n([\s\S]*?)\n\s*-->/g

/**
 * Make a stop order valid regardless of how the AI ordered the numbers.
 *
 * For a stop order the stop level lives in `triggerPrice`; for a stop-limit
 * (`SL`) the limit lives in `price`. The fill rule is direction-dependent:
 *   - SELL stop (protective on a long): when triggered the limit must be AT or
 *     BELOW the stop, else it can never fill (e.g. stop 82 / limit 98 = broken).
 *   - BUY stop (breakout entry): the limit must be AT or ABOVE the stop.
 *
 * The AI gets this backwards (it hands in two technical levels in arbitrary
 * order), so we never trust it with a live order: we put the stop in
 * `triggerPrice` and clamp the limit to the fillable side of it. If no usable
 * limit was given, we fall back to a stop-market (`SL-M`) which always fills —
 * the right default for a protective stop.
 */
function normalizeStopOrder(rec: OrderRecommendation): OrderRecommendation {
  if (rec.orderType !== 'SL' && rec.orderType !== 'SL-M') return rec

  // The stop is triggerPrice; if the AI only supplied one value (in price),
  // that value IS the stop.
  const stop = rec.triggerPrice && rec.triggerPrice > 0 ? rec.triggerPrice : rec.price
  const limit = rec.triggerPrice && rec.triggerPrice > 0 ? rec.price : 0
  if (!stop || stop <= 0) return rec // no usable stop level — let validation reject it

  if (rec.orderType === 'SL-M') {
    return { ...rec, triggerPrice: stop, price: 0 }
  }

  // SL (stop-limit): need a limit on the fillable side of the stop.
  if (!limit || limit <= 0) {
    // No usable limit → safest valid protective stop is stop-market.
    return { ...rec, orderType: 'SL-M', triggerPrice: stop, price: 0 }
  }
  const fillableLimit = rec.side === 'SELL' ? Math.min(limit, stop) : Math.max(limit, stop)
  return { ...rec, triggerPrice: stop, price: fillableLimit }
}

/**
 * Extract structured order recommendations from AI markdown output.
 * The AI emits `<!-- ACTIONS_JSON [...] -->` blocks that are invisible
 * when rendered as markdown but parseable here.
 */
export function parseActionsFromMarkdown(
  text: string,
  source: OrderRecommendation['source']
): OrderRecommendation[] {
  const results: OrderRecommendation[] = []

  // matchAll iterates a copy of the regex, so every call starts from the top.
  for (const match of text.matchAll(ACTIONS_REGEX)) {
    try {
      const parsed = JSON.parse(match[1])
      if (!Array.isArray(parsed)) continue

      for (const item of parsed) {
        const role = STRATEGY_ROLES.find((r) => r === item.role)
        const cancelOrderId =
          item.cancelOrderId !== undefined && item.cancelOrderId !== null
            ? String(item.cancelOrderId)
            : ''
        const isCancel = role === 'cancel'

        // A cancel leg has no size, so it passes without a quantity — but only with an order id.
        if (isCancel) {
          if (!item.symbol || !item.side || !cancelOrderId) continue
        } else if (!item.symbol || !item.side || !item.quantity) {
          continue
        }

        const orderType = item.orderType || 'MARKET'
        if (role === 'protect' && !PROTECT_ORDER_TYPES.has(orderType)) continue
        // Trailing stops only make sense as GTC; otherwise honor an explicit GTC, else DAY.
        const validity = orderType === 'TRAILING_STOP' || item.validity === 'GTC' ? 'GTC' : 'DAY'

        results.push(
          normalizeStopOrder({
            id: crypto.randomUUID(),
            symbol: String(item.symbol).toUpperCase(),
            exchange: item.exchange || 'US',
            // A protect always protects a held (or just-bought) long, so it always sells.
            side: role === 'protect' || item.side === 'SELL' ? 'SELL' : 'BUY',
            quantity: isCancel ? 0 : Math.max(1, Math.floor(Number(item.quantity) || 1)),
            orderType,
            product: item.product || 'CNC',
            price: Number(item.price) || 0,
            triggerPrice: item.triggerPrice ? Number(item.triggerPrice) : undefined,
            trailPrice: item.trailPrice ? Number(item.trailPrice) : undefined,
            trailPercent: item.trailPercent ? Number(item.trailPercent) : undefined,
            validity,
            group: typeof item.group === 'string' && item.group ? item.group : undefined,
            role,
            cancelOrderId: cancelOrderId || undefined,
            rationale: item.rationale || '',
            source,
          })
        )
      }
    } catch {
      // Malformed JSON block — skip silently
    }
  }

  return results
}
