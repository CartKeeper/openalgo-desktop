import { invoke } from '@tauri-apps/api/core'
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Copy,
  ExternalLink,
  Loader2,
  Minus,
  Plus,
  RefreshCw,
  Sparkles,
  Trash2,
  TrendingDown,
  TrendingUp,
} from 'lucide-react'
import { Fragment, useCallback, useEffect, useMemo, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { useClientScenarioStore } from '@/stores/clientScenarioStore'
import type { Client } from '@/types/clients'
import { isShortSellingAllowed } from '@/types/clients'
import type { ClientScenario, ScenarioPosition } from '@/types/client-scenarios'
import { HelpButton } from '@/components/HelpButton'
import AddTradeDialog from './AddTradeDialog'
import AnalyzeWithClaudeDialog from './AnalyzeWithClaudeDialog'

interface PositionDiff {
  type: 'modified' | 'new' | 'removed'
  sourceQty?: number
  sourceAvgPrice?: number
  sourceSide?: string
  qtyDelta?: number
  priceDelta?: number
  rationale?: string
}

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

const pctFmt = (n: number) =>
  `${n >= 0 ? '+' : ''}${n.toFixed(2)}%`

export default function ScenarioDetail() {
  const { id, scenarioId } = useParams<{ id: string; scenarioId: string }>()
  const clientId = Number(id)
  const scId = Number(scenarioId)
  const navigate = useNavigate()

  const {
    activeScenario,
    enrichedPositions,
    summary,
    isLoading,
    isPricingLoading,
    setActiveScenario,
    loadPositions,
    deletePosition,
    applyTrade,
    cloneScenario,
    enrichWithPricing,
    startPricingPolling,
    stopPricingPolling,
  } = useClientScenarioStore()

  const [client, setClient] = useState<Client | null>(null)
  const [showTradeDialog, setShowTradeDialog] = useState(false)
  const [showCloneDialog, setShowCloneDialog] = useState(false)
  const [showAnalyzeDialog, setShowAnalyzeDialog] = useState(false)
  const [deletePositionTarget, setDeletePositionTarget] = useState<number | null>(null)
  const [cloneForm, setCloneForm] = useState({ name: '', description: '' })
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [sourcePositions, setSourcePositions] = useState<ScenarioPosition[]>([])
  const sourceScenarioId = activeScenario?.cloned_from_id ?? null

  const loadScenario = useCallback(async () => {
    try {
      const scenario = await invoke<ClientScenario>(
        'get_client_scenario',
        { id: scId }
      )
      setActiveScenario(scenario)
    } catch (err) {
      toast.error(`Failed to load scenario: ${errMsg(err)}`)
    }
  }, [scId, setActiveScenario])

  const loadClient = useCallback(async () => {
    try {
      const data = await invoke<Client>('get_client', { id: clientId })
      setClient(data)
    } catch {
      // non-critical
    }
  }, [clientId])

  useEffect(() => {
    loadClient()
    loadScenario()
    loadPositions(scId)
    startPricingPolling()

    return () => {
      stopPricingPolling()
    }
  }, [scId, clientId, loadClient, loadScenario, loadPositions, startPricingPolling, stopPricingPolling])

  // Load source positions when this is a cloned scenario
  useEffect(() => {
    if (sourceScenarioId != null) {
      invoke<ScenarioPosition[]>('get_scenario_positions', {
        scenarioId: sourceScenarioId,
      })
        .then(setSourcePositions)
        .catch(() => setSourcePositions([]))
    } else {
      setSourcePositions([])
    }
  }, [sourceScenarioId])

  // Compute diff map: symbol → PositionDiff
  const diffMap = useMemo(() => {
    const map = new Map<string, PositionDiff>()
    if (sourcePositions.length === 0) return map

    const sourceBySymbol = new Map<string, ScenarioPosition>()
    for (const sp of sourcePositions) {
      sourceBySymbol.set(sp.symbol.toUpperCase(), sp)
    }

    for (const pos of enrichedPositions) {
      const key = pos.symbol.toUpperCase()
      const src = sourceBySymbol.get(key)
      if (!src) {
        // Position exists now but didn't exist in source — new
        map.set(key, {
          type: 'new',
          rationale: pos.notes ?? undefined,
        })
      } else {
        const qtyChanged = Math.abs(pos.quantity - src.quantity) > 0.001
        const priceChanged = Math.abs(pos.avg_price - src.avg_price) > 0.01
        const sideChanged = pos.side !== src.side
        if (qtyChanged || priceChanged || sideChanged) {
          map.set(key, {
            type: 'modified',
            sourceQty: src.quantity,
            sourceAvgPrice: src.avg_price,
            sourceSide: src.side,
            qtyDelta: pos.quantity - src.quantity,
            priceDelta: pos.avg_price - src.avg_price,
            rationale: pos.notes ?? undefined,
          })
        }
      }
    }

    // Positions that were in source but removed from current
    const currentSymbols = new Set(
      enrichedPositions.map((p) => p.symbol.toUpperCase())
    )
    for (const sp of sourcePositions) {
      const key = sp.symbol.toUpperCase()
      if (!currentSymbols.has(key)) {
        map.set(key, {
          type: 'removed',
          sourceQty: sp.quantity,
          sourceAvgPrice: sp.avg_price,
          sourceSide: sp.side,
        })
      }
    }

    return map
  }, [enrichedPositions, sourcePositions])

  const accountType = activeScenario?.account_type ?? client?.account_type ?? null
  const shortsAllowed = isShortSellingAllowed(accountType)
  const restrictedShorts = useMemo(() => {
    if (shortsAllowed) return [] as typeof enrichedPositions
    return enrichedPositions.filter((p) => p.side === 'short')
  }, [shortsAllowed, enrichedPositions])
  const longQtyBySymbol = useMemo(() => {
    const map: Record<string, number> = {}
    for (const p of enrichedPositions) {
      if (p.side !== 'long') continue
      const key = p.symbol.toUpperCase()
      map[key] = (map[key] ?? 0) + p.quantity
    }
    return map
  }, [enrichedPositions])

  const handleApplyTrade = async (trade: {
    symbol: string
    exchange: string
    side: string
    quantity: number
    price: number
  }) => {
    try {
      await applyTrade(
        scId,
        trade.symbol,
        trade.exchange,
        trade.side,
        trade.quantity,
        trade.price
      )
      toast.success(
        `Applied ${trade.side} ${trade.quantity} ${trade.symbol} @ ${fmt(trade.price)}`
      )
    } catch (err) {
      toast.error(`Failed to apply trade: ${errMsg(err)}`)
      throw err
    }
  }

  const handleDeletePosition = async () => {
    if (deletePositionTarget == null) return
    try {
      await deletePosition(deletePositionTarget)
      toast.success('Position removed')
      setDeletePositionTarget(null)
    } catch (err) {
      toast.error(`Failed to delete position: ${errMsg(err)}`)
    }
  }

  const handleClone = async () => {
    if (!cloneForm.name.trim()) {
      toast.error('Name is required')
      return
    }
    setIsSubmitting(true)
    try {
      const cloned = await cloneScenario(
        scId,
        clientId,
        cloneForm.name.trim(),
        cloneForm.description.trim() || undefined
      )
      toast.success(`Cloned as: ${cloned.name}`)
      setShowCloneDialog(false)
      setCloneForm({ name: '', description: '' })
      navigate(`/clients/${clientId}/scenarios/${cloned.id}`)
    } catch (err) {
      toast.error(`Failed to clone: ${errMsg(err)}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  const openCloneDialog = () => {
    setCloneForm({
      name: `${activeScenario?.name || 'Scenario'} (Copy)`,
      description: activeScenario?.description || '',
    })
    setShowCloneDialog(true)
  }

  if (isLoading && enrichedPositions.length === 0 && !activeScenario) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="space-y-1">
          <Link
            to={`/clients/${clientId}/scenarios`}
            className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
          >
            <ArrowLeft className="h-4 w-4" /> Back to Scenarios
          </Link>
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-bold tracking-tight">
              {activeScenario?.name || 'Scenario'}
            </h1>
            {activeScenario?.is_baseline && (
              <Badge variant="secondary">Baseline</Badge>
            )}
          </div>
          {activeScenario?.description && (
            <p className="text-sm text-muted-foreground">
              {activeScenario.description}
            </p>
          )}
          <p className="text-xs text-muted-foreground">
            Client: {client?.name}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => enrichWithPricing()}
            disabled={isPricingLoading}
          >
            {isPricingLoading ? (
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            ) : (
              <RefreshCw className="h-4 w-4 mr-2" />
            )}
            {isPricingLoading ? 'Refreshing...' : 'Refresh Prices'}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowAnalyzeDialog(true)}
          >
            <Sparkles className="h-4 w-4 mr-2" />
            Analyze with Claude
          </Button>
          <Button variant="outline" size="sm" onClick={openCloneDialog}>
            <Copy className="h-4 w-4 mr-2" />
            Clone
          </Button>
          <Button size="sm" onClick={() => setShowTradeDialog(true)}>
            <Plus className="h-4 w-4 mr-2" />
            Apply Trade
          </Button>
        </div>
      </div>

      {/* Summary Cards */}
      {summary && summary.position_count > 0 && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <Card>
            <CardContent className="pt-4 pb-4">
              <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Market Value
              </p>
              <p className="text-xl font-bold font-mono tabular-nums mt-1">
                {fmt(summary.total_value)}
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4 pb-4">
              <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Cost Basis
              </p>
              <p className="text-xl font-bold font-mono tabular-nums mt-1">
                {fmt(summary.total_cost)}
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4 pb-4">
              <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Unrealized P&L
              </p>
              <div className="flex items-center gap-2 mt-1">
                {summary.total_pnl >= 0 ? (
                  <TrendingUp className="h-4 w-4 text-green-500" />
                ) : (
                  <TrendingDown className="h-4 w-4 text-red-500" />
                )}
                <p
                  className={`text-xl font-bold font-mono tabular-nums ${
                    summary.total_pnl >= 0 ? 'text-green-500' : 'text-red-500'
                  }`}
                >
                  {fmt(summary.total_pnl)}
                </p>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4 pb-4">
              <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Return
              </p>
              <p
                className={`text-xl font-bold font-mono tabular-nums mt-1 ${
                  summary.pnl_pct >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                {pctFmt(summary.pnl_pct)}
              </p>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Restricted-Account Short-Position Warning */}
      {restrictedShorts.length > 0 && (
        <Card className="border-red-500/40 bg-red-500/5">
          <CardContent className="py-3 px-4">
            <div className="flex items-start gap-3">
              <AlertTriangle className="h-4 w-4 text-red-500 shrink-0 mt-0.5" />
              <div className="space-y-1 text-sm">
                <p className="font-semibold text-red-500">
                  Short positions in restricted account
                </p>
                <p className="text-muted-foreground text-xs leading-relaxed">
                  This account type does not permit short selling, but the
                  baseline contains {restrictedShorts.length} short position
                  {restrictedShorts.length === 1 ? '' : 's'}:{' '}
                  <span className="font-mono font-semibold text-foreground">
                    {restrictedShorts.map((p) => p.symbol).join(', ')}
                  </span>
                  . Verify the account type and source data — these positions
                  should not exist in this account.
                </p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Change Summary Banner */}
      {diffMap.size > 0 && (
        <Card className="border-primary/30 bg-primary/5">
          <CardContent className="py-3 px-4">
            <div className="flex items-center gap-3">
              <Sparkles className="h-4 w-4 text-primary shrink-0" />
              <div className="flex items-center gap-4 text-xs">
                {(() => {
                  const modified = [...diffMap.values()].filter((d) => d.type === 'modified').length
                  const added = [...diffMap.values()].filter((d) => d.type === 'new').length
                  const removed = [...diffMap.values()].filter((d) => d.type === 'removed').length
                  return (
                    <>
                      <span className="text-sm font-medium">
                        AI changes vs source scenario:
                      </span>
                      {modified > 0 && (
                        <span className="inline-flex items-center gap-1 text-primary">
                          <ArrowUp className="h-3 w-3" />
                          {modified} modified
                        </span>
                      )}
                      {added > 0 && (
                        <span className="inline-flex items-center gap-1 text-green-500">
                          <Plus className="h-3 w-3" />
                          {added} new
                        </span>
                      )}
                      {removed > 0 && (
                        <span className="inline-flex items-center gap-1 text-red-500">
                          <Minus className="h-3 w-3" />
                          {removed} closed
                        </span>
                      )}
                      <span className="text-muted-foreground">
                        See explanations below each highlighted row.
                      </span>
                    </>
                  )
                })()}
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Positions Table */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle className="text-base">
            Positions ({enrichedPositions.length})
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {enrichedPositions.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <p className="text-sm text-muted-foreground mb-4">
                No positions yet. Apply a trade to add positions.
              </p>
              <Button
                size="sm"
                onClick={() => setShowTradeDialog(true)}
              >
                <Plus className="h-4 w-4 mr-2" />
                Apply Trade
              </Button>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Symbol
                    </th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Side
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Qty
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Avg Price
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Current
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Mkt Value
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      P&L
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      P&L %
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Day Chg
                    </th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Actions
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {enrichedPositions.map((pos) => {
                    const diff = diffMap.get(pos.symbol.toUpperCase())
                    const isChanged = diff != null

                    // Parse AI notes for the explanation sub-row
                    const notes = diff?.rationale || ''
                    const aiMatch = notes.match(
                      /^AI:\s*(BUY|SELL)\s+([\d.]+)\s*@\s*\$?([\d.]+)\s*(?:—\s*(.+))?$/i
                    )
                    const tradeAction = aiMatch
                      ? `${aiMatch[1].toUpperCase()} ${parseFloat(aiMatch[2]).toFixed(0)} shares @ ${fmt(parseFloat(aiMatch[3]))}`
                      : null
                    const rationale = aiMatch?.[4] || (aiMatch ? null : (notes || null)) || null

                    return (
                      <Fragment key={pos.id}>
                        {/* Position row */}
                        <tr
                          className={`transition-colors ${
                            isChanged
                              ? diff.type === 'new'
                                ? 'bg-green-500/5 border-l-[3px] border-l-green-500'
                                : 'bg-primary/5 border-l-[3px] border-l-primary'
                              : 'border-b'
                          }`}
                        >
                          <td className="h-12 px-4">
                            <div className="flex items-center gap-2">
                              <div>
                                <span className="font-medium">{pos.symbol}</span>
                                {pos.company_name && (
                                  <p className="text-xs text-muted-foreground truncate max-w-[160px]">
                                    {pos.company_name}
                                  </p>
                                )}
                              </div>
                              {isChanged && (
                                <Badge
                                  variant="outline"
                                  className={`text-[10px] shrink-0 ${
                                    diff.type === 'new'
                                      ? 'text-green-500 border-green-500/30'
                                      : 'text-primary border-primary/30'
                                  }`}
                                >
                                  {diff.type === 'new' ? 'NEW' : 'MODIFIED'}
                                </Badge>
                              )}
                            </div>
                          </td>
                          <td className="h-12 px-4">
                            <Badge
                              variant={
                                pos.side === 'long' ? 'default' : 'secondary'
                              }
                            >
                              {pos.side.toUpperCase()}
                            </Badge>
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">
                            {pos.quantity.toFixed(2)}
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">
                            {fmt(pos.avg_price)}
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">
                            {pos.current_price != null
                              ? fmt(pos.current_price)
                              : '—'}
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">
                            {pos.market_value != null
                              ? fmt(pos.market_value)
                              : '—'}
                          </td>
                          <td
                            className={`h-12 px-4 text-right font-mono tabular-nums ${
                              pos.unrealized_pnl != null
                                ? pos.unrealized_pnl >= 0
                                  ? 'text-green-500'
                                  : 'text-red-500'
                                : 'text-muted-foreground'
                            }`}
                          >
                            {pos.unrealized_pnl != null
                              ? fmt(pos.unrealized_pnl)
                              : '—'}
                          </td>
                          <td
                            className={`h-12 px-4 text-right font-mono tabular-nums ${
                              pos.pnl_pct != null
                                ? pos.pnl_pct >= 0
                                  ? 'text-green-500'
                                  : 'text-red-500'
                                : 'text-muted-foreground'
                            }`}
                          >
                            {pos.pnl_pct != null ? pctFmt(pos.pnl_pct) : '—'}
                          </td>
                          <td
                            className={`h-12 px-4 text-right font-mono tabular-nums ${
                              pos.day_change_pct != null
                                ? pos.day_change_pct >= 0
                                  ? 'text-green-500'
                                  : 'text-red-500'
                                : 'text-muted-foreground'
                            }`}
                          >
                            {pos.day_change_pct != null
                              ? pctFmt(pos.day_change_pct)
                              : '—'}
                          </td>
                          <td className="h-12 px-4 text-right">
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-destructive hover:text-destructive"
                              onClick={() =>
                                pos.id != null && setDeletePositionTarget(pos.id)
                              }
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </td>
                        </tr>

                        {/* Explanation sub-row for AI-changed positions */}
                        {isChanged && (tradeAction || rationale || diff.type === 'modified') && (
                          <tr
                            className={`border-b ${
                              diff.type === 'new'
                                ? 'bg-green-500/[0.03] border-l-[3px] border-l-green-500'
                                : 'bg-primary/[0.03] border-l-[3px] border-l-primary'
                            }`}
                          >
                            <td colSpan={10} className="px-4 py-2.5">
                              <div className="flex items-start gap-4 text-xs ml-1">
                                {/* Change description */}
                                <div className="flex items-center gap-3 shrink-0">
                                  {diff.type === 'modified' && diff.sourceQty != null && (
                                    <span className="inline-flex items-center gap-1.5 font-mono tabular-nums text-muted-foreground">
                                      <span>{diff.sourceQty.toFixed(0)} {diff.sourceSide?.toUpperCase()}</span>
                                      <ArrowRight className="h-3 w-3" />
                                      <span className="text-foreground font-medium">{pos.quantity.toFixed(0)} {pos.side.toUpperCase()}</span>
                                    </span>
                                  )}
                                  {diff.type === 'new' && tradeAction && (
                                    <span className="font-mono tabular-nums font-medium text-green-500">
                                      {tradeAction}
                                    </span>
                                  )}
                                </div>

                                {/* Rationale */}
                                {rationale && (
                                  <span className="text-muted-foreground leading-relaxed">
                                    {rationale}
                                  </span>
                                )}

                                {/* Link to source scenario */}
                                {sourceScenarioId != null && (
                                  <Link
                                    to={`/clients/${clientId}/scenarios/${sourceScenarioId}`}
                                    className="inline-flex items-center gap-1 text-primary hover:underline shrink-0 ml-auto"
                                    onClick={(e) => e.stopPropagation()}
                                  >
                                    <ExternalLink className="h-3 w-3" />
                                    <span>Source</span>
                                  </Link>
                                )}
                              </div>
                            </td>
                          </tr>
                        )}
                      </Fragment>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Removed Positions (positions that were closed by AI trades) */}
      {(() => {
        const removed = [...diffMap.entries()].filter(([, d]) => d.type === 'removed')
        if (removed.length === 0) return null
        return (
          <Card className="border-red-500/20">
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base flex items-center gap-2">
                <Minus className="h-4 w-4 text-red-500" />
                Closed Positions ({removed.length})
              </CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b bg-muted/50">
                      <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Symbol
                      </th>
                      <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Was
                      </th>
                      <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Prev Qty
                      </th>
                      <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Prev Avg Price
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {removed.map(([symbol, diff]) => (
                      <tr
                        key={symbol}
                        className="border-b bg-red-500/5 border-l-[3px] border-l-red-500 text-muted-foreground"
                      >
                        <td className="h-12 px-4">
                          <span className="font-medium line-through">{symbol}</span>
                        </td>
                        <td className="h-12 px-4">
                          <Badge variant="secondary">
                            {diff.sourceSide?.toUpperCase()}
                          </Badge>
                        </td>
                        <td className="h-12 px-4 text-right font-mono tabular-nums">
                          {diff.sourceQty?.toFixed(2)}
                        </td>
                        <td className="h-12 px-4 text-right font-mono tabular-nums">
                          {fmt(diff.sourceAvgPrice ?? 0)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </CardContent>
          </Card>
        )
      })()}

      {/* Apply Trade Dialog */}
      <AddTradeDialog
        open={showTradeDialog}
        onOpenChange={setShowTradeDialog}
        shortSellingAllowed={isShortSellingAllowed(activeScenario?.account_type ?? client?.account_type ?? null)}
        onSubmit={handleApplyTrade}
      />

      {/* Clone Dialog */}
      <Dialog open={showCloneDialog} onOpenChange={setShowCloneDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Clone Scenario</DialogTitle>
            <DialogDescription>
              Create a copy with all current positions.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="clone-name">Name *</Label>
              <Input
                id="clone-name"
                value={cloneForm.name}
                onChange={(e) =>
                  setCloneForm({ ...cloneForm, name: e.target.value })
                }
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="clone-desc">Description</Label>
              <Input
                id="clone-desc"
                value={cloneForm.description}
                onChange={(e) =>
                  setCloneForm({ ...cloneForm, description: e.target.value })
                }
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowCloneDialog(false)}
            >
              Cancel
            </Button>
            <Button onClick={handleClone} disabled={isSubmitting}>
              {isSubmitting && (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              )}
              Clone
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Position Confirmation */}
      <Dialog
        open={deletePositionTarget != null}
        onOpenChange={() => setDeletePositionTarget(null)}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Remove Position</DialogTitle>
            <DialogDescription>
              This will remove this position from the scenario. This cannot be
              undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeletePositionTarget(null)}
            >
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleDeletePosition}>
              Remove
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Analyze with Claude Dialog */}
      <AnalyzeWithClaudeDialog
        open={showAnalyzeDialog}
        onOpenChange={setShowAnalyzeDialog}
        clientId={clientId}
        scenarioId={scId}
        scenarioName={activeScenario?.name || 'Scenario'}
        isBaseline={activeScenario?.is_baseline ?? false}
        accountType={accountType}
        longQtyBySymbol={longQtyBySymbol}
        onTradesApplied={() => loadPositions(scId)}
      />

      {/* Contextual Help */}
      <HelpButton
        pageTitle="Scenario Detail"
        items={[
          {
            title: 'What is a scenario?',
            description:
              'A scenario is a sandbox portfolio. You can add, remove, or modify positions without affecting real accounts. Use scenarios to model "what-if" changes before committing.',
          },
          {
            title: 'Baseline vs cloned scenarios',
            description:
              'Baseline scenarios are auto-generated from your imported trades. Cloned scenarios copy a baseline so you can experiment. AI Recommendations are clones with AI-suggested trades applied.',
          },
          {
            title: 'Market Value',
            description:
              'The total current value of all positions in this scenario, calculated as quantity x current price for each holding.',
          },
          {
            title: 'Cost Basis',
            description:
              'What you originally paid for all positions combined. This is quantity x average purchase price for each holding.',
          },
          {
            title: 'Unrealized P&L',
            description:
              'The gain or loss if you sold everything now. Green = profit, red = loss. Calculated as (current price - avg price) x quantity for each position.',
          },
          {
            title: 'Return %',
            description:
              'Your total percentage return: (market value - cost basis) / cost basis. A 37% return means your portfolio is worth 37% more than what you paid.',
          },
          {
            title: 'AI-highlighted rows',
            description:
              'Blue-bordered rows were modified by AI recommendations. Green-bordered rows are new positions the AI added. Each highlighted row has an explanation underneath showing what changed and why.',
          },
          {
            title: 'Day Chg',
            description:
              "Today's price change as a percentage. This reflects the stock's movement today, not your P&L on the position.",
          },
          {
            title: 'Refresh Prices',
            description:
              'Fetches live prices for all positions. Prices auto-refresh periodically, but you can force an update with this button.',
          },
          {
            title: 'Analyze with Claude',
            description:
              'Ask Claude AI to review this portfolio and suggest changes. Claude will create a cloned scenario with its recommended trades applied, so your original stays untouched.',
          },
        ]}
      />
    </div>
  )
}
