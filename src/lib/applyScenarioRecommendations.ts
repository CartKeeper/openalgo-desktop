import type { InvokeArgs } from '@tauri-apps/api/core'
import type { OrderRecommendation } from '@/types/actionQueue'
import type { ClientScenario } from '@/types/client-scenarios'

export interface ApplyScenarioRecommendationsDeps {
  /** Tauri invoke — injected so the logic is testable without Tauri. */
  invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>
  /** Store method that wraps the `clone_client_scenario` Tauri command. */
  cloneScenario: (
    sourceId: number,
    clientId: number,
    name: string,
    description?: string
  ) => Promise<ClientScenario>
}

export interface ApplyScenarioRecommendationsParams extends ApplyScenarioRecommendationsDeps {
  items: OrderRecommendation[]
  clientId: number
  currentScenarioId: number
  isBaseline: boolean
  /** Required when isBaseline === true; names the new clone. */
  cloneName: string | undefined
}

export interface ApplyScenarioRecommendationsResult {
  /** ID the trades were applied against — either the current scenario or the new clone. */
  targetScenarioId: number
  /** True when a new clone was created as part of this apply. */
  cloned: boolean
  /** Count of successfully applied trades. */
  applied: number
  /** Per-trade failures (loop does not abort on individual failures). */
  failures: Array<{ symbol: string; message: string }>
}

export async function applyScenarioRecommendations(
  params: ApplyScenarioRecommendationsParams
): Promise<ApplyScenarioRecommendationsResult> {
  const { items, clientId, currentScenarioId, isBaseline, cloneName, invoke, cloneScenario } =
    params

  let targetScenarioId = currentScenarioId
  let cloned = false

  if (isBaseline) {
    const trimmed = (cloneName ?? '').trim()
    if (trimmed.length === 0) {
      throw new Error('A clone name is required when applying to a baseline scenario.')
    }
    const clone = await cloneScenario(currentScenarioId, clientId, trimmed, undefined)
    if (clone.id == null) {
      throw new Error('Clone succeeded but returned no id.')
    }
    targetScenarioId = clone.id
    cloned = true
  }

  const failures: Array<{ symbol: string; message: string }> = []
  let applied = 0

  for (const item of items) {
    try {
      await invoke('apply_scenario_trade', {
        scenarioId: targetScenarioId,
        symbol: item.symbol,
        exchange: item.exchange,
        side: item.side === 'BUY' ? 'long' : 'short',
        quantity: item.quantity,
        price: item.price,
      })
      applied++
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      failures.push({ symbol: item.symbol, message })
    }
  }

  return { targetScenarioId, cloned, applied, failures }
}
