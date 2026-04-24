import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { create } from 'zustand'
import type {
  ClientScenario,
  EnrichedScenarioPosition,
  GenericQuote,
  ScenarioPosition,
  ScenarioSummary,
} from '@/types/client-scenarios'

interface ClientScenarioStore {
  scenarios: ClientScenario[]
  activeScenario: ClientScenario | null
  positions: ScenarioPosition[]
  enrichedPositions: EnrichedScenarioPosition[]
  summary: ScenarioSummary | null
  isLoading: boolean
  isPricingLoading: boolean
  error: string | null
  pricingTimer: ReturnType<typeof setInterval> | null

  // Scenario CRUD
  loadScenarios: (clientId: number) => Promise<void>
  createScenario: (
    clientId: number,
    name: string,
    description?: string
  ) => Promise<ClientScenario>
  updateScenario: (
    id: number,
    name?: string,
    description?: string
  ) => Promise<void>
  deleteScenario: (id: number) => Promise<void>
  cloneScenario: (
    sourceId: number,
    clientId: number,
    name: string,
    description?: string
  ) => Promise<ClientScenario>
  syncBaseline: (clientId: number) => Promise<void>

  // Position CRUD
  loadPositions: (scenarioId: number) => Promise<void>
  addPosition: (
    scenarioId: number,
    symbol: string,
    exchange: string,
    quantity: number,
    avgPrice: number,
    side?: string,
    notes?: string
  ) => Promise<void>
  updatePosition: (
    id: number,
    quantity?: number,
    avgPrice?: number,
    side?: string,
    notes?: string
  ) => Promise<void>
  deletePosition: (id: number) => Promise<void>
  applyTrade: (
    scenarioId: number,
    symbol: string,
    exchange: string,
    side: string,
    quantity: number,
    price: number
  ) => Promise<void>

  // Pricing
  enrichWithPricing: () => Promise<void>
  startPricingPolling: () => void
  stopPricingPolling: () => void

  // Helpers
  setActiveScenario: (scenario: ClientScenario | null) => void
  clearError: () => void
}

function computeSummary(
  enriched: EnrichedScenarioPosition[]
): ScenarioSummary {
  let totalValue = 0
  let totalCost = 0
  let totalPnl = 0
  let count = 0

  for (const p of enriched) {
    if (p.current_price != null) {
      totalValue += Math.abs(p.market_value ?? 0)
    }
    totalCost += Math.abs(p.cost_basis)
    totalPnl += p.unrealized_pnl ?? 0
    count++
  }

  const pnlPct = totalCost !== 0 ? (totalPnl / totalCost) * 100 : 0

  return {
    total_value: totalValue,
    total_cost: totalCost,
    total_pnl: totalPnl,
    pnl_pct: pnlPct,
    position_count: count,
  }
}

function enrichPositions(
  positions: ScenarioPosition[],
  quotes: Map<string, GenericQuote>
): EnrichedScenarioPosition[] {
  return positions.map((pos) => {
    const quote = quotes.get(pos.symbol.toUpperCase())
    const currentPrice = quote?.price ?? null
    const isShort = pos.side === 'short'
    const costBasis = pos.quantity * Math.abs(pos.avg_price)
    const marketValue =
      currentPrice != null ? pos.quantity * currentPrice : null

    // Long: P&L = market_value - cost_basis (profit when price goes up)
    // Short: P&L = cost_basis - market_value (profit when price goes down)
    const unrealizedPnl =
      marketValue != null
        ? isShort
          ? costBasis - marketValue
          : marketValue - costBasis
        : null

    const absCost = Math.abs(costBasis)
    const pnlPct =
      unrealizedPnl != null && absCost > 0
        ? (unrealizedPnl / absCost) * 100
        : null

    return {
      ...pos,
      current_price: currentPrice,
      market_value: marketValue,
      cost_basis: costBasis,
      unrealized_pnl: unrealizedPnl,
      pnl_pct: pnlPct,
      day_change: quote?.change ?? null,
      day_change_pct: quote?.change_percent ?? null,
      company_name: quote?.name ?? null,
    }
  })
}

export const useClientScenarioStore = create<ClientScenarioStore>(
  (set, get) => ({
    scenarios: [],
    activeScenario: null,
    positions: [],
    enrichedPositions: [],
    summary: null,
    isLoading: false,
    isPricingLoading: false,
    error: null,
    pricingTimer: null,

    // -----------------------------------------------------------------------
    // Scenario CRUD
    // -----------------------------------------------------------------------

    loadScenarios: async (clientId) => {
      set({ isLoading: true, error: null })
      try {
        const scenarios = await invoke<ClientScenario[]>(
          'get_client_scenarios',
          { clientId }
        )
        set({ scenarios, isLoading: false })
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ isLoading: false, error: msg })
      }
    },

    createScenario: async (clientId, name, description) => {
      try {
        const scenario = await invoke<ClientScenario>(
          'create_client_scenario',
          {
            clientId,
            name,
            description: description || null,
          }
        )
        set((s) => ({ scenarios: [...s.scenarios, scenario] }))
        return scenario
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    updateScenario: async (id, name, description) => {
      try {
        const updated = await invoke<ClientScenario>(
          'update_client_scenario',
          {
            id,
            name: name || null,
            description: description ?? null,
          }
        )
        set((s) => ({
          scenarios: s.scenarios.map((sc) =>
            sc.id === id ? updated : sc
          ),
          activeScenario:
            s.activeScenario?.id === id ? updated : s.activeScenario,
        }))
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    deleteScenario: async (id) => {
      try {
        await invoke('delete_client_scenario', { id })
        set((s) => ({
          scenarios: s.scenarios.filter((sc) => sc.id !== id),
          activeScenario:
            s.activeScenario?.id === id ? null : s.activeScenario,
        }))
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    cloneScenario: async (sourceId, clientId, name, description) => {
      try {
        const cloned = await invoke<ClientScenario>(
          'clone_client_scenario',
          {
            sourceId,
            clientId,
            name,
            description: description || null,
          }
        )
        set((s) => ({ scenarios: [...s.scenarios, cloned] }))
        return cloned
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    syncBaseline: async (clientId) => {
      try {
        const baselines = await invoke<ClientScenario[]>(
          'sync_baseline_scenario',
          { clientId }
        )
        set((s) => {
          // Remove old baselines, replace with new ones
          const nonBaseline = s.scenarios.filter((sc) => !sc.is_baseline)
          return {
            scenarios: [...baselines, ...nonBaseline],
          }
        })
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    // -----------------------------------------------------------------------
    // Position CRUD
    // -----------------------------------------------------------------------

    loadPositions: async (scenarioId) => {
      set({ isLoading: true, error: null })
      try {
        const positions = await invoke<ScenarioPosition[]>(
          'get_scenario_positions',
          { scenarioId }
        )
        set({ positions, isLoading: false })
        // Enrich with live pricing
        await get().enrichWithPricing()
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ isLoading: false, error: msg })
      }
    },

    addPosition: async (
      scenarioId,
      symbol,
      exchange,
      quantity,
      avgPrice,
      side,
      notes
    ) => {
      try {
        await invoke('add_scenario_position', {
          scenarioId,
          symbol,
          exchange,
          quantity,
          avgPrice,
          side: side || null,
          notes: notes || null,
        })
        await get().loadPositions(scenarioId)
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    updatePosition: async (id, quantity, avgPrice, side, notes) => {
      try {
        await invoke('update_scenario_position', {
          id,
          quantity: quantity ?? null,
          avgPrice: avgPrice ?? null,
          side: side || null,
          notes: notes ?? null,
        })
        const scenarioId = get().activeScenario?.id
        if (scenarioId) await get().loadPositions(scenarioId)
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    deletePosition: async (id) => {
      try {
        await invoke('delete_scenario_position', { id })
        set((s) => {
          const remaining = s.positions.filter((p) => p.id !== id)
          const enriched = s.enrichedPositions.filter(
            (p) => p.id !== id
          )
          return {
            positions: remaining,
            enrichedPositions: enriched,
            summary: computeSummary(enriched),
          }
        })
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    applyTrade: async (
      scenarioId,
      symbol,
      exchange,
      side,
      quantity,
      price
    ) => {
      try {
        await invoke('apply_scenario_trade', {
          scenarioId,
          symbol,
          exchange,
          side,
          quantity,
          price,
        })
        await get().loadPositions(scenarioId)
      } catch (err) {
        const msg =
          err instanceof Error ? err.message : String(err)
        set({ error: msg })
        throw err
      }
    },

    // -----------------------------------------------------------------------
    // Pricing
    // -----------------------------------------------------------------------

    enrichWithPricing: async () => {
      const { positions } = get()
      if (positions.length === 0) {
        set({ enrichedPositions: [], summary: null })
        return
      }

      const symbols = [
        ...new Set(positions.map((p) => p.symbol.toUpperCase())),
      ]

      set({ isPricingLoading: true })
      try {
        const quotes = await invoke<GenericQuote[]>(
          'get_generic_quote',
          { symbols }
        )
        const quoteMap = new Map<string, GenericQuote>()
        for (const q of quotes) {
          quoteMap.set(q.symbol.toUpperCase(), q)
        }

        const enriched = enrichPositions(positions, quoteMap)
        set({
          enrichedPositions: enriched,
          summary: computeSummary(enriched),
          isPricingLoading: false,
        })
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err)
        toast.error(`Failed to fetch prices: ${msg}`)
        // Show positions without live data
        const enriched = enrichPositions(positions, new Map())
        set({
          enrichedPositions: enriched,
          summary: computeSummary(enriched),
          isPricingLoading: false,
        })
      }
    },

    startPricingPolling: () => {
      const existing = get().pricingTimer
      if (existing) clearInterval(existing)

      const timer = setInterval(() => {
        get().enrichWithPricing()
      }, 30_000) // 30 seconds

      set({ pricingTimer: timer })
    },

    stopPricingPolling: () => {
      const timer = get().pricingTimer
      if (timer) {
        clearInterval(timer)
        set({ pricingTimer: null })
      }
    },

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    setActiveScenario: (scenario) => {
      set({ activeScenario: scenario })
    },

    clearError: () => set({ error: null }),
  })
)
