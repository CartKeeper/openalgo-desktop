import { create } from 'zustand'
import { capBuysToAvailableCash } from '@/lib/affordability'
import type { BasketOrderResult, OrderRecommendation } from '@/types/actionQueue'

interface ActionQueueStore {
  items: OrderRecommendation[]
  isReviewOpen: boolean
  isSubmitting: boolean
  lastResults: BasketOrderResult[] | null
  /** Notice describing any cash-affordability adjustments applied to the current items. */
  fundsNotice: string | null

  setItemsAndOpen: (items: OrderRecommendation[]) => void
  /**
   * Live-account path: confirm the account's available cash covers the BUY
   * recommendations (capping quantities cumulatively) BEFORE opening review.
   * Use this for any flow that places real/paper orders on the user's account.
   */
  setItemsAndOpenWithFundsCheck: (items: OrderRecommendation[]) => Promise<void>
  updateItem: (id: string, patch: Partial<OrderRecommendation>) => void
  removeItem: (id: string) => void
  close: () => void
  setSubmitting: (v: boolean) => void
  setResults: (results: BasketOrderResult[]) => void
  clearResults: () => void
}

export const useActionQueueStore = create<ActionQueueStore>()((set) => ({
  items: [],
  isReviewOpen: false,
  isSubmitting: false,
  lastResults: null,
  fundsNotice: null,

  // No funds check (e.g. client-scenario flows that don't touch the live account).
  setItemsAndOpen: (items) =>
    set({ items, isReviewOpen: true, lastResults: null, fundsNotice: null }),

  setItemsAndOpenWithFundsCheck: async (items) => {
    const { items: capped, notice } = await capBuysToAvailableCash(items)
    set({ items: capped, isReviewOpen: true, lastResults: null, fundsNotice: notice })
  },

  updateItem: (id, patch) =>
    set((state) => ({
      items: state.items.map((item) => (item.id === id ? { ...item, ...patch } : item)),
    })),

  removeItem: (id) =>
    set((state) => ({
      items: state.items.filter((item) => item.id !== id),
    })),

  close: () =>
    set({ items: [], isReviewOpen: false, isSubmitting: false, lastResults: null, fundsNotice: null }),

  setSubmitting: (v) => set({ isSubmitting: v }),

  setResults: (results) => set({ lastResults: results, isSubmitting: false }),

  clearResults: () => set({ lastResults: null }),
}))
