import React from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  ArrowLeft,
  Loader2,
  TrendingDown,
  TrendingUp,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Link, useParams, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { Client } from '@/types/clients'
import type {
  ClientScenario,
  EnrichedScenarioPosition,
  GenericQuote,
  ScenarioPosition,
  ScenarioSummary,
} from '@/types/client-scenarios'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err)
    return (err as { message: string }).message
  return String(err)
}

const fmt = (n: number) =>
  new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
  }).format(n)

const pctFmt = (n: number) => `${n >= 0 ? '+' : ''}${n.toFixed(2)}%`

interface ScenarioData {
  scenario: ClientScenario
  positions: ScenarioPosition[]
  enriched: EnrichedScenarioPosition[]
  summary: ScenarioSummary
}

// Color palette for scenario columns
const SCENARIO_COLORS = [
  'border-blue-500',
  'border-emerald-500',
  'border-amber-500',
]

const SCENARIO_BG = [
  'bg-blue-500/10',
  'bg-emerald-500/10',
  'bg-amber-500/10',
]

function enrichPositions(
  positions: ScenarioPosition[],
  quotes: Map<string, GenericQuote>
): EnrichedScenarioPosition[] {
  return positions.map((pos) => {
    const quote = quotes.get(pos.symbol.toUpperCase())
    const currentPrice = quote?.price ?? null
    const costBasis = pos.quantity * pos.avg_price
    const marketValue =
      currentPrice != null ? pos.quantity * currentPrice : null
    const unrealizedPnl = marketValue != null ? marketValue - costBasis : null
    const pnlPct =
      unrealizedPnl != null && costBasis > 0
        ? (unrealizedPnl / costBasis) * 100
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

function computeSummary(enriched: EnrichedScenarioPosition[]): ScenarioSummary {
  let totalValue = 0
  let totalCost = 0

  for (const p of enriched) {
    if (p.current_price != null) {
      totalValue += p.market_value ?? 0
    }
    totalCost += p.cost_basis
  }

  const totalPnl = totalValue - totalCost
  const pnlPct = totalCost > 0 ? (totalPnl / totalCost) * 100 : 0

  return {
    total_value: totalValue,
    total_cost: totalCost,
    total_pnl: totalPnl,
    pnl_pct: pnlPct,
    position_count: enriched.length,
  }
}

export default function ScenarioCompare() {
  const { id } = useParams<{ id: string }>()
  const clientId = Number(id)
  const [searchParams] = useSearchParams()

  const scenarioIds = (searchParams.get('scenarios') || '')
    .split(',')
    .map(Number)
    .filter((n) => !isNaN(n) && n > 0)

  const [client, setClient] = useState<Client | null>(null)
  const [scenarioData, setScenarioData] = useState<ScenarioData[]>([])
  const [isLoading, setIsLoading] = useState(true)

  const loadClient = useCallback(async () => {
    try {
      const data = await invoke<Client>('get_client', { id: clientId })
      setClient(data)
    } catch {
      // non-critical
    }
  }, [clientId])

  const loadComparison = useCallback(async () => {
    if (scenarioIds.length < 2) return
    setIsLoading(true)

    try {
      // Load all scenarios and their positions in parallel
      const results = await Promise.all(
        scenarioIds.map(async (scId) => {
          const [scenario, positions] = await Promise.all([
            invoke<ClientScenario>('get_client_scenario', { id: scId }),
            invoke<ScenarioPosition[]>('get_scenario_positions', {
              scenarioId: scId,
            }),
          ])
          return { scenario, positions }
        })
      )

      // Collect all unique symbols for pricing
      const allSymbols = new Set<string>()
      for (const r of results) {
        for (const p of r.positions) {
          allSymbols.add(p.symbol.toUpperCase())
        }
      }

      // Fetch pricing for all symbols at once
      let quoteMap = new Map<string, GenericQuote>()
      if (allSymbols.size > 0) {
        try {
          const quotes = await invoke<GenericQuote[]>('get_generic_quote', {
            symbols: [...allSymbols],
          })
          for (const q of quotes) {
            quoteMap.set(q.symbol.toUpperCase(), q)
          }
        } catch {
          // Pricing failure is non-critical
        }
      }

      // Enrich all scenarios with pricing
      const data: ScenarioData[] = results.map((r) => {
        const enriched = enrichPositions(r.positions, quoteMap)
        return {
          scenario: r.scenario,
          positions: r.positions,
          enriched,
          summary: computeSummary(enriched),
        }
      })

      setScenarioData(data)
    } catch (err) {
      toast.error(`Failed to load comparison: ${errMsg(err)}`)
    } finally {
      setIsLoading(false)
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scenarioIds.join(',')])

  useEffect(() => {
    loadClient()
    loadComparison()
  }, [loadClient, loadComparison])

  if (scenarioIds.length < 2) {
    return (
      <div className="space-y-4">
        <Link
          to={`/clients/${clientId}/scenarios`}
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" /> Back to Scenarios
        </Link>
        <p className="text-muted-foreground">
          Select at least 2 scenarios to compare.
        </p>
      </div>
    )
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  // Build combined symbol list across all scenarios
  const allPositionKeys = new Set<string>()
  for (const sd of scenarioData) {
    for (const p of sd.enriched) {
      allPositionKeys.add(`${p.symbol}|${p.exchange}`)
    }
  }
  const sortedKeys = [...allPositionKeys].sort()

  // Build lookup maps: scenario index → symbol key → position
  const positionMaps: Map<string, EnrichedScenarioPosition>[] = scenarioData.map(
    (sd) => {
      const m = new Map<string, EnrichedScenarioPosition>()
      for (const p of sd.enriched) {
        m.set(`${p.symbol}|${p.exchange}`, p)
      }
      return m
    }
  )

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="space-y-1">
        <Link
          to={`/clients/${clientId}/scenarios`}
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" /> Back to Scenarios
        </Link>
        <h1 className="text-2xl font-bold tracking-tight">
          Compare Scenarios
        </h1>
        <p className="text-sm text-muted-foreground">
          Side-by-side comparison for {client?.name}
        </p>
      </div>

      {/* Summary Comparison Cards */}
      <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${scenarioData.length}, minmax(0, 1fr))` }}>
        {scenarioData.map((sd, idx) => (
          <Card
            key={sd.scenario.id}
            className={`border-t-4 ${SCENARIO_COLORS[idx] || ''}`}
          >
            <CardHeader className="pb-2">
              <div className="flex items-center gap-2">
                <CardTitle className="text-base truncate">
                  {sd.scenario.name}
                </CardTitle>
                {sd.scenario.is_baseline && (
                  <Badge variant="secondary" className="shrink-0">
                    Baseline
                  </Badge>
                )}
              </div>
              {sd.scenario.description && (
                <p className="text-xs text-muted-foreground line-clamp-1">
                  {sd.scenario.description}
                </p>
              )}
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Market Value
                  </p>
                  <p className="text-lg font-bold font-mono tabular-nums">
                    {fmt(sd.summary.total_value)}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Cost Basis
                  </p>
                  <p className="text-lg font-bold font-mono tabular-nums">
                    {fmt(sd.summary.total_cost)}
                  </p>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Unrealized P&L
                  </p>
                  <div className="flex items-center gap-1">
                    {sd.summary.total_pnl >= 0 ? (
                      <TrendingUp className="h-3.5 w-3.5 text-green-500" />
                    ) : (
                      <TrendingDown className="h-3.5 w-3.5 text-red-500" />
                    )}
                    <p
                      className={`text-lg font-bold font-mono tabular-nums ${
                        sd.summary.total_pnl >= 0
                          ? 'text-green-500'
                          : 'text-red-500'
                      }`}
                    >
                      {fmt(sd.summary.total_pnl)}
                    </p>
                  </div>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Return
                  </p>
                  <p
                    className={`text-lg font-bold font-mono tabular-nums ${
                      sd.summary.pnl_pct >= 0
                        ? 'text-green-500'
                        : 'text-red-500'
                    }`}
                  >
                    {pctFmt(sd.summary.pnl_pct)}
                  </p>
                </div>
              </div>
              <div>
                <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Positions
                </p>
                <p className="text-lg font-bold font-mono tabular-nums">
                  {sd.summary.position_count}
                </p>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Position-by-Position Comparison Table */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">
            Position Comparison ({sortedKeys.length} symbols)
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {sortedKeys.length === 0 ? (
            <p className="px-4 pb-4 text-sm text-muted-foreground">
              No positions in any selected scenario.
            </p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground sticky left-0 bg-muted/50">
                      Symbol
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Current
                    </th>
                    {scenarioData.map((sd, idx) => (
                      <th
                        key={sd.scenario.id}
                        colSpan={3}
                        className={`h-10 px-4 text-center text-xs font-semibold uppercase tracking-wider ${SCENARIO_BG[idx] || ''}`}
                      >
                        {sd.scenario.name}
                      </th>
                    ))}
                  </tr>
                  <tr className="border-b bg-muted/30">
                    <th className="h-8 px-4 sticky left-0 bg-muted/30" />
                    <th className="h-8 px-4" />
                    {scenarioData.map((sd, idx) => (
                      <React.Fragment key={sd.scenario.id}>
                        <th className={`h-8 px-3 text-right text-[10px] font-semibold uppercase tracking-wider text-muted-foreground ${SCENARIO_BG[idx] || ''}`}>
                          Qty
                        </th>
                        <th className={`h-8 px-3 text-right text-[10px] font-semibold uppercase tracking-wider text-muted-foreground ${SCENARIO_BG[idx] || ''}`}>
                          Value
                        </th>
                        <th className={`h-8 px-3 text-right text-[10px] font-semibold uppercase tracking-wider text-muted-foreground ${SCENARIO_BG[idx] || ''}`}>
                          P&L %
                        </th>
                      </React.Fragment>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {sortedKeys.map((key) => {
                    const [symbol] = key.split('|')
                    // Get current price from first scenario that has this position
                    let currentPrice: number | null = null
                    for (const pm of positionMaps) {
                      const p = pm.get(key)
                      if (p?.current_price != null) {
                        currentPrice = p.current_price
                        break
                      }
                    }

                    return (
                      <tr key={key} className="border-b">
                        <td className="h-12 px-4 font-medium sticky left-0 bg-background">
                          {symbol}
                        </td>
                        <td className="h-12 px-4 text-right font-mono tabular-nums text-sm">
                          {currentPrice != null ? fmt(currentPrice) : '—'}
                        </td>
                        {positionMaps.map((pm, idx) => {
                          const pos = pm.get(key)
                          if (!pos) {
                            return (
                              <React.Fragment key={idx}>
                                <td className={`h-12 px-3 text-right text-muted-foreground text-sm ${SCENARIO_BG[idx] || ''}`}>
                                  —
                                </td>
                                <td className={`h-12 px-3 text-right text-muted-foreground text-sm ${SCENARIO_BG[idx] || ''}`}>
                                  —
                                </td>
                                <td className={`h-12 px-3 text-right text-muted-foreground text-sm ${SCENARIO_BG[idx] || ''}`}>
                                  —
                                </td>
                              </React.Fragment>
                            )
                          }
                          return (
                            <React.Fragment key={idx}>
                              <td className={`h-12 px-3 text-right font-mono tabular-nums text-sm ${SCENARIO_BG[idx] || ''}`}>
                                {pos.quantity.toFixed(pos.quantity % 1 === 0 ? 0 : 2)}
                              </td>
                              <td className={`h-12 px-3 text-right font-mono tabular-nums text-sm ${SCENARIO_BG[idx] || ''}`}>
                                {pos.market_value != null
                                  ? fmt(pos.market_value)
                                  : fmt(pos.cost_basis)}
                              </td>
                              <td
                                className={`h-12 px-3 text-right font-mono tabular-nums text-sm ${SCENARIO_BG[idx] || ''} ${
                                  pos.pnl_pct != null
                                    ? pos.pnl_pct >= 0
                                      ? 'text-green-500'
                                      : 'text-red-500'
                                    : 'text-muted-foreground'
                                }`}
                              >
                                {pos.pnl_pct != null
                                  ? pctFmt(pos.pnl_pct)
                                  : '—'}
                              </td>
                            </React.Fragment>
                          )
                        })}
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

