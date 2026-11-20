import { Loader2, Plus, Trash2 } from 'lucide-react'
import { useCallback, useEffect, useId, useState } from 'react'
import { toast } from 'sonner'
import {
  analyticsApi,
  type AllocationTarget,
  type DriftReport,
  type DriftStatus,
  type RebalancePlan,
  type TargetScope,
} from '@/api/analytics'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { rebalancePlanToActions } from '@/lib/rebalancePlanToActions'
import { cn, formatQty } from '@/lib/utils'
import { useActionQueueStore } from '@/stores/actionQueueStore'

interface EditorRow {
  uid: string
  scope: TargetScope
  key: string
  target: string
  band: string
}

const DEFAULT_BAND = '2'

const toRow = (target: AllocationTarget): EditorRow => ({
  uid: crypto.randomUUID(),
  scope: target.scope,
  key: target.key,
  target: String(target.target_pct),
  band: String(target.band_pct),
})

const newRow = (): EditorRow => ({
  uid: crypto.randomUUID(),
  scope: 'symbol',
  key: '',
  target: '',
  band: DEFAULT_BAND,
})

const sameRows = (a: EditorRow[], b: EditorRow[]) =>
  a.length === b.length &&
  a.every(
    (row, i) =>
      row.scope === b[i].scope &&
      row.key.trim() === b[i].key.trim() &&
      row.target === b[i].target &&
      row.band === b[i].band
  )

const money = (n: number) =>
  new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2 }).format(n)
const pct = (n: number) => `${n.toFixed(2)}%`
const signedPct = (n: number) => `${n >= 0 ? '+' : ''}${n.toFixed(2)}%`
const errMsg = (err: unknown) => (err instanceof Error ? err.message : String(err))

const STATUS: Record<DriftStatus, { label: string; className: string }> = {
  in_band: { label: 'In band', className: 'border-green-500/40 text-green-600 dark:text-green-400' },
  over: { label: 'Over', className: 'border-amber-500/40 text-amber-700 dark:text-amber-400' },
  under: { label: 'Under', className: 'border-red-500/40 text-red-600 dark:text-red-400' },
}

const HEAD = 'h-10 px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground'
const HEAD_NUM = `${HEAD} text-right`
const CELL = 'h-12 px-4'
const NUM = `${CELL} text-right font-mono tabular-nums`

interface AllocationCardProps {
  clientId: number
  scenarioId: number
  /** Symbols held in the scenario — the symbol picker's suggestions. */
  symbols: string[]
}

/**
 * Allocation targets for a scenario: the client's targets (symbol or sector,
 * with a tolerance band), each target's drift, and a whole-share rebalance plan.
 *
 * "Review plan" hands the plan to the action-queue store; the `ActionReviewModal`
 * already hosted on the Scenario Detail page (scenario mode: applies to this
 * scenario, or to a new clone when it is the baseline) shows it. When that
 * review closes, the drift is fetched again and the plan is dropped.
 */
export function AllocationCard({ clientId, scenarioId, symbols }: AllocationCardProps) {
  const listId = useId()
  const [rows, setRows] = useState<EditorRow[]>([])
  const [saved, setSaved] = useState<EditorRow[]>([])
  const [targetsLoaded, setTargetsLoaded] = useState(false)
  const [targetsError, setTargetsError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [isSaving, setIsSaving] = useState(false)
  const [drift, setDrift] = useState<DriftReport | null>(null)
  const [driftError, setDriftError] = useState<string | null>(null)
  const [plan, setPlan] = useState<RebalancePlan | null>(null)
  const [planError, setPlanError] = useState<string | null>(null)
  const [isBuilding, setIsBuilding] = useState(false)

  const loadTargets = useCallback(async () => {
    try {
      const loaded = (await analyticsApi.listTargets(clientId)).map(toRow)
      setRows(loaded)
      setSaved(loaded)
      setTargetsError(null)
    } catch (err) {
      setTargetsError(errMsg(err))
    } finally {
      setTargetsLoaded(true)
    }
  }, [clientId])

  const loadDrift = useCallback(async () => {
    try {
      setDrift(await analyticsApi.scenarioDrift(scenarioId))
      setDriftError(null)
    } catch (err) {
      setDriftError(errMsg(err))
    }
  }, [scenarioId])

  useEffect(() => {
    loadTargets()
  }, [loadTargets])

  useEffect(() => {
    loadDrift()
  }, [loadDrift])

  // The review modal applies the plan (to this scenario or a clone) and then
  // closes: refresh the drift and drop the plan it was built from.
  useEffect(() => {
    let wasOpen = useActionQueueStore.getState().isReviewOpen
    return useActionQueueStore.subscribe((state) => {
      if (wasOpen && !state.isReviewOpen) {
        setPlan(null)
        loadDrift()
      }
      wasOpen = state.isReviewOpen
    })
  }, [loadDrift])

  const dirty = !sameRows(rows, saved)
  const complete = rows.every((row) => row.key.trim() !== '' && row.target.trim() !== '')
  const canSave = dirty && complete && !isSaving

  const updateRow = (uid: string, patch: Partial<EditorRow>) => {
    setRows((current) => current.map((row) => (row.uid === uid ? { ...row, ...patch } : row)))
    setSaveError(null)
  }

  const handleSave = async () => {
    setIsSaving(true)
    setSaveError(null)
    try {
      const targets: AllocationTarget[] = rows.map((row) => ({
        id: null,
        client_id: clientId,
        scope: row.scope,
        key: row.key.trim(),
        target_pct: Number(row.target),
        band_pct: Number(row.band) || 0,
      }))
      const stored = (await analyticsApi.saveTargets(clientId, targets)).map(toRow)
      setRows(stored)
      setSaved(stored)
      setPlan(null)
      toast.success(`Saved ${stored.length} target${stored.length === 1 ? '' : 's'}`)
      await loadDrift()
    } catch (err) {
      setSaveError(errMsg(err))
    } finally {
      setIsSaving(false)
    }
  }

  const handleBuildPlan = async () => {
    setIsBuilding(true)
    setPlanError(null)
    try {
      setPlan(await analyticsApi.rebalancePlan(scenarioId))
    } catch (err) {
      setPlanError(errMsg(err))
    } finally {
      setIsBuilding(false)
    }
  }

  const handleReview = () => {
    if (!plan) return
    useActionQueueStore.getState().setItemsAndOpen(rebalancePlanToActions(plan))
  }

  const canBuild = saved.length > 0 && !dirty && !isBuilding

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Allocation targets</CardTitle>
        <CardDescription>
          Target weights per symbol or sector with a tolerance band, measured against this
          scenario's positions and cash. The rebalance plan trades whole shares back toward the
          targets and goes through the usual review before anything is applied.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* Targets editor */}
        <section className="space-y-3">
          <h3 className="text-sm font-semibold">Targets</h3>
          {!targetsLoaded ? (
            <Skeleton className="h-9" />
          ) : targetsError ? (
            <div className="flex items-center justify-between gap-4">
              <p className="text-sm text-red-500">{targetsError}</p>
              <Button variant="outline" size="sm" onClick={() => loadTargets()}>
                Retry
              </Button>
            </div>
          ) : (
            <>
              {rows.length > 0 && (
                <div className="hidden grid-cols-[8rem_minmax(0,1fr)_6rem_6rem_2.25rem] gap-2 md:grid">
                  {['Scope', 'Symbol or sector', 'Target %', 'Band ±%', ''].map((label) => (
                    <p
                      key={label || 'remove'}
                      className="text-xs font-semibold uppercase tracking-wider text-muted-foreground"
                    >
                      {label}
                    </p>
                  ))}
                </div>
              )}
              {rows.map((row) => (
                <div
                  key={row.uid}
                  className="grid grid-cols-2 gap-2 md:grid-cols-[8rem_minmax(0,1fr)_6rem_6rem_2.25rem]"
                >
                  <Select
                    value={row.scope}
                    onValueChange={(value) => updateRow(row.uid, { scope: value as TargetScope })}
                  >
                    <SelectTrigger aria-label="Scope">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="symbol">Symbol</SelectItem>
                      <SelectItem value="sector">Sector</SelectItem>
                    </SelectContent>
                  </Select>
                  <Input
                    aria-label="Symbol or sector"
                    list={`${listId}-${row.scope}`}
                    value={row.key}
                    placeholder={row.scope === 'symbol' ? 'VOO' : 'Technology'}
                    onChange={(e) =>
                      updateRow(row.uid, {
                        key: row.scope === 'symbol' ? e.target.value.toUpperCase() : e.target.value,
                      })
                    }
                  />
                  <Input
                    aria-label="Target %"
                    type="number"
                    inputMode="decimal"
                    min={0}
                    max={100}
                    step="any"
                    value={row.target}
                    onChange={(e) => updateRow(row.uid, { target: e.target.value })}
                  />
                  <Input
                    aria-label="Band ±%"
                    type="number"
                    inputMode="decimal"
                    min={0}
                    step="any"
                    value={row.band}
                    onChange={(e) => updateRow(row.uid, { band: e.target.value })}
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="Remove target"
                    onClick={() => {
                      setRows((current) => current.filter((r) => r.uid !== row.uid))
                      setSaveError(null)
                    }}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}
              <datalist id={`${listId}-symbol`}>
                {symbols.map((symbol) => (
                  <option key={symbol} value={symbol} />
                ))}
              </datalist>
              <datalist id={`${listId}-sector`}>
                {(drift?.sectors ?? []).map((sector) => (
                  <option key={sector} value={sector} />
                ))}
              </datalist>
              {rows.length === 0 && (
                <p className="text-sm text-muted-foreground">
                  No targets yet. Add a symbol or sector target to measure drift.
                </p>
              )}
              <div className="flex flex-wrap items-center gap-2">
                <Button variant="outline" size="sm" onClick={() => setRows((c) => [...c, newRow()])}>
                  <Plus className="h-4 w-4" />
                  Add target
                </Button>
                <Button size="sm" onClick={handleSave} disabled={!canSave}>
                  {isSaving && <Loader2 className="h-4 w-4 animate-spin" />}
                  Save
                </Button>
                {saveError && <p className="text-xs font-semibold text-red-500">{saveError}</p>}
              </div>
            </>
          )}
        </section>

        {/* Drift */}
        <section className="space-y-3">
          <h3 className="text-sm font-semibold">Drift</h3>
          {driftError ? (
            <div className="flex items-center justify-between gap-4">
              <p className="text-sm text-red-500">{driftError}</p>
              <Button variant="outline" size="sm" onClick={() => loadDrift()}>
                Retry
              </Button>
            </div>
          ) : drift == null ? (
            <Skeleton className="h-10" />
          ) : drift.rows.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              Save at least one target to see how far this scenario sits from it.
            </p>
          ) : (
            <>
              <Table aria-label="Drift">
                <TableHeader>
                  <TableRow className="bg-muted/50">
                    <TableHead className={HEAD}>Key</TableHead>
                    <TableHead className={HEAD_NUM}>Target</TableHead>
                    <TableHead className={HEAD_NUM}>Actual</TableHead>
                    <TableHead className={HEAD_NUM}>Drift</TableHead>
                    <TableHead className={HEAD}>Status</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {drift.rows.map((row) => (
                    <TableRow key={`${row.scope}-${row.key}`}>
                      <TableCell className={`${CELL} font-semibold`}>
                        {row.key}
                        {row.scope === 'sector' && (
                          <span className="ml-2 text-xs font-normal text-muted-foreground">sector</span>
                        )}
                      </TableCell>
                      <TableCell className={NUM}>
                        {pct(row.target_pct)}
                        <span className="text-muted-foreground"> ±{formatQty(row.band_pct)}</span>
                      </TableCell>
                      <TableCell className={NUM}>{pct(row.actual_pct)}</TableCell>
                      <TableCell className={NUM}>{signedPct(row.drift_pct)}</TableCell>
                      <TableCell className={CELL}>
                        <Badge variant="outline" className={STATUS[row.status].className}>
                          {STATUS[row.status].label}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                  <TableRow>
                    <TableCell className={`${CELL} text-muted-foreground`}>Untargeted (incl. cash)</TableCell>
                    <TableCell className={NUM}>—</TableCell>
                    <TableCell className={`${NUM} text-muted-foreground`}>{pct(drift.untargeted_pct)}</TableCell>
                    <TableCell className={NUM}>—</TableCell>
                    <TableCell className={CELL} />
                  </TableRow>
                </TableBody>
              </Table>
              <p className="text-xs text-muted-foreground">
                Total {money(drift.total_value)} · Cash {money(drift.cash)}
              </p>
            </>
          )}
          {drift != null && drift.warnings.length > 0 && (
            <ul className="space-y-1 rounded-lg border border-amber-300 bg-amber-50/60 px-4 py-3 dark:border-amber-900 dark:bg-amber-950/30">
              {drift.warnings.map((warning) => (
                <li key={warning} className="text-xs text-amber-700 dark:text-amber-400">
                  {warning}
                </li>
              ))}
            </ul>
          )}
        </section>

        {/* Rebalance plan */}
        <section className="space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="secondary" size="lg" onClick={handleBuildPlan} disabled={!canBuild}>
              {isBuilding && <Loader2 className="h-4 w-4 animate-spin" />}
              Build rebalance plan
            </Button>
            {plan && plan.lines.length > 0 && (
              <Button size="lg" onClick={handleReview}>
                Review plan
              </Button>
            )}
            {dirty && saved.length > 0 && (
              <p className="text-xs text-muted-foreground">Save the targets to plan against them.</p>
            )}
          </div>
          {planError && <p className="text-sm text-red-500">{planError}</p>}
          {plan && (
            <div className="space-y-3">
              {plan.lines.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  Nothing to trade: every target is within reach of a whole share.
                </p>
              ) : (
                <Table aria-label="Rebalance plan">
                  <TableHeader>
                    <TableRow className="bg-muted/50">
                      <TableHead className={HEAD}>Symbol</TableHead>
                      <TableHead className={HEAD}>Side</TableHead>
                      <TableHead className={HEAD_NUM}>Qty</TableHead>
                      <TableHead className={HEAD_NUM}>Price</TableHead>
                      <TableHead className={HEAD_NUM}>Value</TableHead>
                      <TableHead className={HEAD_NUM}>Before → After</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {plan.lines.map((line) => (
                      <TableRow key={`${line.symbol}-${line.side}`}>
                        <TableCell className={`${CELL} font-semibold`}>{line.symbol}</TableCell>
                        <TableCell className={CELL}>
                          <Badge
                            variant="outline"
                            className={cn(
                              line.side === 'sell'
                                ? 'border-red-500/40 text-red-600 dark:text-red-400'
                                : 'border-green-500/40 text-green-600 dark:text-green-400'
                            )}
                          >
                            {line.side === 'sell' ? 'Sell' : 'Buy'}
                          </Badge>
                        </TableCell>
                        <TableCell className={NUM}>{formatQty(line.quantity)}</TableCell>
                        <TableCell className={NUM}>{money(line.price)}</TableCell>
                        <TableCell className={NUM}>{money(line.value)}</TableCell>
                        <TableCell className={NUM}>
                          {pct(line.before_pct)} → {pct(line.after_pct)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
              <p className="text-sm font-mono tabular-nums">
                Cash before {money(plan.cash_before)} → after {money(plan.cash_after)}
              </p>
              {plan.unresolved.length > 0 && (
                <ul className="space-y-1 rounded-lg border border-amber-300 bg-amber-50/60 px-4 py-3 dark:border-amber-900 dark:bg-amber-950/30">
                  {plan.unresolved.map((item) => (
                    <li key={item} className="text-xs text-amber-700 dark:text-amber-400">
                      {item}
                    </li>
                  ))}
                </ul>
              )}
              <p className="text-xs text-muted-foreground">{plan.note}</p>
            </div>
          )}
        </section>
      </CardContent>
    </Card>
  )
}
