import { create } from 'zustand'
import { rideCommands } from '@/api/tauri-client'
import { reconcileRecommendationsWithAccount } from '@/lib/affordability'
import type { BasketOrderResult, OrderRecommendation } from '@/types/actionQueue'

const sameGroup = (a: OrderRecommendation, b: OrderRecommendation): boolean =>
  !!a.group && !!b.group && a.group === b.group

const sameSymbol = (a: OrderRecommendation, b: OrderRecommendation): boolean =>
  a.symbol.toUpperCase() === b.symbol.toUpperCase()

export interface CascadeResult {
  /** Items left after the removal and its cascade. */
  remaining: OrderRecommendation[]
  /** The item the user removed, or undefined if the id was not found. */
  removed: OrderRecommendation | undefined
  /** True when the removed item itself was a protect (not removed by cascade). */
  protectRemovedDirectly: boolean
}

/**
 * Pure removal rules for grouped strategy legs:
 * - removing a buy also removes the protect(s) for the same group + symbol (fund sells stay);
 * - removing a fund also removes the cancel(s) for the same group + symbol;
 * - removing a protect, cancel or standalone leg cascades nothing.
 */
export function cascadeRemoval(items: OrderRecommendation[], removedId: string): CascadeResult {
  const removed = items.find((item) => item.id === removedId)
  if (!removed) return { remaining: items, removed: undefined, protectRemovedDirectly: false }

  const cascadeRole = removed.role === 'buy' ? 'protect' : removed.role === 'fund' ? 'cancel' : null
  const remaining = items.filter((item) => {
    if (item.id === removedId) return false
    return !(
      cascadeRole !== null &&
      item.role === cascadeRole &&
      sameGroup(item, removed) &&
      sameSymbol(item, removed)
    )
  })
  return { remaining, removed, protectRemovedDirectly: removed.role === 'protect' }
}

/** Header copy for a strategy group in the review modal; '' when the group has no items. */
export function groupHeader(items: OrderRecommendation[], group: string): string {
  const inGroup = items.filter((item) => item.group === group)
  const funds = inGroup.filter((item) => item.role === 'fund')
  const buy = inGroup.find((item) => item.role === 'buy')
  const protect = inGroup.find((item) => item.role === 'protect')
  const cancel = inGroup.find((item) => item.role === 'cancel')

  if (funds.length > 0 && buy) {
    return `Rotation: sell ${funds.map((f) => f.symbol).join(', ')} → buy ${buy.symbol}`
  }
  if (funds.length > 0) return `Sell ${funds.map((f) => f.symbol).join(', ')} to cash`
  if (buy) return `Buy ${buy.symbol}`
  if (protect) return `Protect ${protect.symbol}`
  if (cancel) return `Cancel order on ${cancel.symbol}`
  return ''
}

/** Monotonic token so a slow reconcile from an earlier removal never overwrites newer state. */
let removalToken = 0

interface ActionQueueStore {
  items: OrderRecommendation[]
  isReviewOpen: boolean
  isSubmitting: boolean
  lastResults: BasketOrderResult[] | null
  /** Notice describing any cash-affordability adjustments applied to the current items. */
  fundsNotice: string | null

  setItemsAndOpen: (items: OrderRecommendation[]) => void
  /**
   * Live-account path: reconcile recommendations against the account BEFORE
   * opening review — cap BUYs cumulatively to available cash and cap SELLs/stops
   * to actually-held shares. Use this for any flow that places real/paper orders.
   */
  setItemsAndOpenWithFundsCheck: (items: OrderRecommendation[]) => Promise<void>
  updateItem: (id: string, patch: Partial<OrderRecommendation>) => void
  removeItem: (id: string) => void
  close: () => void
  setSubmitting: (v: boolean) => void
  setResults: (results: BasketOrderResult[]) => void
  clearResults: () => void
}

export const useActionQueueStore = create<ActionQueueStore>()((set, get) => ({
  items: [],
  isReviewOpen: false,
  isSubmitting: false,
  lastResults: null,
  fundsNotice: null,

  // No funds check (e.g. client-scenario flows that don't touch the live account).
  setItemsAndOpen: (items) =>
    set({ items, isReviewOpen: true, lastResults: null, fundsNotice: null }),

  setItemsAndOpenWithFundsCheck: async (items) => {
    const { items: capped, notice } = await reconcileRecommendationsWithAccount(items)
    set({ items: capped, isReviewOpen: true, lastResults: null, fundsNotice: notice })
  },

  updateItem: (id, patch) =>
    set((state) => ({
      items: state.items.map((item) => (item.id === id ? { ...item, ...patch } : item)),
    })),

  removeItem: (id) => {
    const { remaining, removed, protectRemovedDirectly } = cascadeRemoval(get().items, id)
    if (!removed) return
    const token = ++removalToken
    set({ items: remaining })

    if (protectRemovedDirectly) {
      // Fire-and-forget: remember the user declined this protection. Errors are swallowed.
      void Promise.resolve(rideCommands.recordProtectionDismissal(removed.symbol, removed.quantity)).catch(
        () => undefined,
      )
    }

    if (remaining.some((item) => item.group)) {
      reconcileRecommendationsWithAccount(remaining)
        .then(({ items, notice }) => {
          if (token !== removalToken) return
          set({ items, fundsNotice: notice })
        })
        .catch(() => undefined)
    }
  },

  close: () =>
    set({ items: [], isReviewOpen: false, isSubmitting: false, lastResults: null, fundsNotice: null }),

  setSubmitting: (v) => set({ isSubmitting: v }),

  setResults: (results) => set({ lastResults: results, isSubmitting: false }),

  clearResults: () => set({ lastResults: null }),
}))
