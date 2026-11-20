import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import type { OrderRecommendation } from '@/types/actionQueue'
import type { Client } from '@/types/clients'
import type { ClientScenario, ScenarioPosition } from '@/types/client-scenarios'
import { filterRecommendationsForAccount } from './filterRecommendationsForAccount'

/** What reviewing trades against a client's sandbox scenario needs to know. */
export interface ScenarioReviewContext {
  scenario: ClientScenario
  /** The scenario's account type, falling back to the client's. */
  accountType: string | null
  /** Symbol (upper-cased) → long quantity held in the scenario. */
  longQtyBySymbol: Record<string, number>
}

export function longQtyBySymbol(
  positions: Pick<ScenarioPosition, 'symbol' | 'side' | 'quantity'>[]
): Record<string, number> {
  const map: Record<string, number> = {}
  for (const p of positions) {
    if (p.side !== 'long') continue
    const key = p.symbol.toUpperCase()
    map[key] = (map[key] ?? 0) + p.quantity
  }
  return map
}

/** Load a scenario, its client's account type, and its long holdings. */
export async function loadScenarioReviewContext(scenarioId: number): Promise<ScenarioReviewContext> {
  const scenario = await invoke<ClientScenario>('get_client_scenario', { id: scenarioId })
  const positions = await invoke<ScenarioPosition[]>('get_scenario_positions', { scenarioId })
  let accountType = scenario.account_type
  if (!accountType) {
    const client = await invoke<Client>('get_client', { id: scenario.client_id })
    accountType = client.account_type
  }
  return { scenario, accountType, longQtyBySymbol: longQtyBySymbol(positions) }
}

/**
 * Drop the trades this account type forbids (e.g. shorting in an IRA) and say
 * which were dropped. Returns the trades left to review; an empty list has
 * already been explained to the user.
 */
export function allowedScenarioTrades(
  actions: OrderRecommendation[],
  ctx: Pick<ScenarioReviewContext, 'accountType' | 'longQtyBySymbol'>
): OrderRecommendation[] {
  if (actions.length === 0) {
    toast.error('No recommended trades found in this message')
    return []
  }
  const { allowed, dropped } = filterRecommendationsForAccount(actions, ctx)
  if (dropped.length > 0) {
    toast.warning(
      `${dropped.length} recommendation${dropped.length === 1 ? '' : 's'} dropped — account type restriction`,
      {
        description: dropped
          .map((d) => `${d.item.side} ${d.item.quantity} ${d.item.symbol}: ${d.reason}`)
          .join('\n'),
        duration: 8000,
      }
    )
  }
  if (allowed.length === 0) {
    toast.error('All recommendations were disallowed for this account type')
  }
  return allowed
}
