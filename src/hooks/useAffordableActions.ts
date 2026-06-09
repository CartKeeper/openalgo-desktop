import { useEffect, useState } from 'react'
import { reconcileRecommendationsWithAccount } from '@/lib/affordability'
import type { OrderRecommendation } from '@/types/actionQueue'

export interface AffordableActions {
  /** Recommendations after capping BUY quantities to the account's available cash. */
  actions: OrderRecommendation[]
  /** User-facing notice describing any reduced/dropped buys, or null when nothing changed. */
  notice: string | null
  /** True while the cash/quote check is in flight. */
  checking: boolean
}

/**
 * Reconcile a message's parsed trade recommendations against the account BEFORE
 * they are shown to the user — so an unaffordable buy (a $360 share on $67 of
 * cash) or a sell of a symbol the user doesn't hold is removed/flagged in the
 * chat itself, not just at review time.
 *
 * Mirrors the hard cap the review modal applies, keeping the displayed list and
 * the order that would actually be placed in agreement. Falls back to the raw
 * recommendations (no notice) if the account lookups fail, so a transient error
 * never hides a pick outright.
 */
export function useAffordableActions(rawActions: OrderRecommendation[]): AffordableActions {
  const [state, setState] = useState<AffordableActions>({
    actions: rawActions,
    notice: null,
    checking: rawActions.length > 0,
  })

  // Recompute only when the actual recommendations change, not on every render.
  const key = JSON.stringify(
    rawActions.map((a) => [a.symbol, a.side, a.quantity, a.orderType, a.price])
  )

  useEffect(() => {
    if (rawActions.length === 0) {
      setState({ actions: [], notice: null, checking: false })
      return
    }

    let cancelled = false
    setState((s) => ({ ...s, checking: true }))

    reconcileRecommendationsWithAccount(rawActions)
      .then(({ items, notice }) => {
        if (!cancelled) setState({ actions: items, notice, checking: false })
      })
      .catch(() => {
        if (!cancelled) setState({ actions: rawActions, notice: null, checking: false })
      })

    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])

  return state
}
