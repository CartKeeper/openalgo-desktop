import { create } from 'zustand'
import type { BasketOrderResult, OrderRecommendation } from '@/types/actionQueue'

interface ActionQueueStore {
  items: OrderRecommendation[]
  isReviewOpen: boolean
  isSubmitting: boolean
  lastResults: BasketOrderResult[] | null

  setItemsAndOpen: (items: OrderRecommendation[]) => void
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

  setItemsAndOpen: (items) => set({ items, isReviewOpen: true, lastResults: null }),

  updateItem: (id, patch) =>
    set((state) => ({
      items: state.items.map((item) => (item.id === id ? { ...item, ...patch } : item)),
    })),

  removeItem: (id) =>
    set((state) => ({
      items: state.items.filter((item) => item.id !== id),
    })),

  close: () => set({ items: [], isReviewOpen: false, isSubmitting: false, lastResults: null }),

  setSubmitting: (v) => set({ isSubmitting: v }),

  setResults: (results) => set({ lastResults: results, isSubmitting: false }),

  clearResults: () => set({ lastResults: null }),
}))
