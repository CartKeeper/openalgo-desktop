import { useCallback, useEffect, useState } from 'react'
import { analyticsApi, type LotMethod, type LotReport, type Term } from '@/api/analytics'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { cn, formatQty } from '@/lib/utils'

const METHODS: ReadonlyArray<{ value: LotMethod; label: string }> = [
  { value: 'fifo', label: 'FIFO' },
  { value: 'lifo', label: 'LIFO' },
  { value: 'hifo', label: 'HIFO' },
  { value: 'average', label: 'Average' },
]

const money = (n: number) =>
  new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2 }).format(n)

/** `YYYY-MM-DD` as `Jan 5, 2026`; the time suffix keeps the date out of UTC shifting. */
const day = (iso: string) =>
  new Date(`${iso}T00:00:00`).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })

const pnlClass = (n: number) => (n >= 0 ? 'text-green-500' : 'text-red-500')

const HEAD = 'h-10 px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground'
const HEAD_NUM = `${HEAD} text-right`
const CELL = 'h-12 px-4'
const NUM = `${CELL} text-right font-mono tabular-nums`

function TermBadge({ term }: { term: Term }) {
  return <Badge variant="outline">{term === 'long' ? 'Long' : 'Short'}</Badge>
}

function Metric({ label, value, className }: { label: string; value: string; className?: string }) {
  return (
    <div className="space-y-1">
      <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{label}</p>
      <p className={cn('text-sm font-semibold font-mono tabular-nums', className)}>{value}</p>
    </div>
  )
}

function LotsSkeleton() {
  return (
    <div className="space-y-4 px-4 pb-4">
      <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-6">
        {METHODS.concat(METHODS.slice(0, 2)).map((_, i) => (
          <Skeleton key={i} className="h-10" />
        ))}
      </div>
      <Skeleton className="h-10" />
      <Skeleton className="h-12" />
      <Skeleton className="h-12" />
    </div>
  )
}

/**
 * Tax lots of a client's trade history: open lots and realized sales under the
 * chosen matching method, with short/long-term classification.
 */
export function LotsTable({ clientId }: { clientId: number }) {
  const [method, setMethod] = useState<LotMethod>('fifo')
  const [report, setReport] = useState<LotReport | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    setIsLoading(true)
    setError(null)
    try {
      setReport(await analyticsApi.clientLots(clientId, method))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }, [clientId, method])

  useEffect(() => {
    load()
  }, [load])

  const isEmpty = report != null && report.open_lots.length === 0 && report.disposals.length === 0

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div className="space-y-1.5">
          <CardTitle className="text-base">Tax Lots</CardTitle>
          <CardDescription>
            Sells matched to purchase lots from the trade history. Long term = held more than 365
            days. Lots marked "est." predate the imported history and use the holding's average
            cost.
          </CardDescription>
        </div>
        <Select value={method} onValueChange={(value) => setMethod(value as LotMethod)}>
          <SelectTrigger size="sm" className="w-32 shrink-0" aria-label="Lot method">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {METHODS.map((m) => (
              <SelectItem key={m.value} value={m.value}>
                {m.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </CardHeader>
      <CardContent className="p-0">
        {error ? (
          <div className="flex items-center justify-between gap-4 px-4 pb-4">
            <p className="text-sm text-red-500">{error}</p>
            <Button variant="outline" size="sm" onClick={() => load()}>
              Retry
            </Button>
          </div>
        ) : report == null ? (
          <LotsSkeleton />
        ) : isEmpty ? (
          <p className="px-4 pb-4 text-sm text-muted-foreground">
            No trades yet. Import or add trades to see tax lots.
          </p>
        ) : (
          <div className={cn('space-y-4 pb-4', isLoading && 'opacity-60')} aria-busy={isLoading}>
            {report.warnings.length > 0 && (
              <ul className="mx-4 space-y-1 rounded-lg border border-amber-300 bg-amber-50/60 px-4 py-3 dark:border-amber-900 dark:bg-amber-950/30">
                {report.warnings.map((warning) => (
                  <li key={warning} className="text-xs text-amber-700 dark:text-amber-400">
                    {warning}
                  </li>
                ))}
              </ul>
            )}

            <div className="grid grid-cols-2 gap-4 px-4 md:grid-cols-3 lg:grid-cols-6">
              <Metric label="Cost basis" value={money(report.totals.cost_basis)} />
              <Metric label="Market value" value={money(report.totals.market_value)} />
              <Metric
                label="Unrealized short"
                value={money(report.totals.unrealized_short)}
                className={pnlClass(report.totals.unrealized_short)}
              />
              <Metric
                label="Unrealized long"
                value={money(report.totals.unrealized_long)}
                className={pnlClass(report.totals.unrealized_long)}
              />
              <Metric
                label="Realized YTD short"
                value={money(report.totals.realized_short_ytd)}
                className={pnlClass(report.totals.realized_short_ytd)}
              />
              <Metric
                label="Realized YTD long"
                value={money(report.totals.realized_long_ytd)}
                className={pnlClass(report.totals.realized_long_ytd)}
              />
            </div>

            <section>
              <h3 className="px-4 pb-3 text-sm font-semibold">Open lots</h3>
              <Table aria-label="Open lots">
                <TableHeader>
                  <TableRow className="bg-muted/50">
                    <TableHead className={HEAD}>Symbol</TableHead>
                    <TableHead className={HEAD}>Opened</TableHead>
                    <TableHead className={HEAD_NUM}>Qty</TableHead>
                    <TableHead className={HEAD_NUM}>Cost/sh</TableHead>
                    <TableHead className={HEAD_NUM}>Cost</TableHead>
                    <TableHead className={HEAD_NUM}>Value</TableHead>
                    <TableHead className={HEAD_NUM}>Unrealized</TableHead>
                    <TableHead className={HEAD}>Term</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {report.open_lots.length === 0 ? (
                    <TableRow>
                      <TableCell className={`${CELL} text-muted-foreground`} colSpan={8}>
                        No open lots.
                      </TableCell>
                    </TableRow>
                  ) : (
                    report.open_lots.map((lot, i) => (
                      <TableRow key={`${lot.symbol}-${lot.opened}-${i}`}>
                        <TableCell className={`${CELL} font-semibold`}>
                          {lot.symbol}
                          {lot.estimated && (
                            <Badge
                              variant="secondary"
                              className="ml-2"
                              title="Predates the imported history; priced at the holding's average cost"
                            >
                              est.
                            </Badge>
                          )}
                        </TableCell>
                        <TableCell className={CELL}>{day(lot.opened)}</TableCell>
                        <TableCell className={NUM}>{formatQty(lot.quantity)}</TableCell>
                        <TableCell className={NUM}>{money(lot.cost_per_share)}</TableCell>
                        <TableCell className={NUM}>{money(lot.cost_basis)}</TableCell>
                        <TableCell className={NUM}>
                          {lot.market_value == null ? '—' : money(lot.market_value)}
                        </TableCell>
                        <TableCell
                          className={cn(NUM, lot.unrealized_pnl != null && pnlClass(lot.unrealized_pnl))}
                        >
                          {lot.unrealized_pnl == null ? '—' : money(lot.unrealized_pnl)}
                        </TableCell>
                        <TableCell className={CELL}>
                          <TermBadge term={lot.term} />
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </section>

            <section>
              <h3 className="px-4 pb-3 text-sm font-semibold">Realized</h3>
              <Table aria-label="Realized">
                <TableHeader>
                  <TableRow className="bg-muted/50">
                    <TableHead className={HEAD}>Symbol</TableHead>
                    <TableHead className={HEAD}>Opened</TableHead>
                    <TableHead className={HEAD}>Closed</TableHead>
                    <TableHead className={HEAD_NUM}>Qty</TableHead>
                    <TableHead className={HEAD_NUM}>Proceeds</TableHead>
                    <TableHead className={HEAD_NUM}>Cost</TableHead>
                    <TableHead className={HEAD_NUM}>P/L</TableHead>
                    <TableHead className={HEAD_NUM}>Days</TableHead>
                    <TableHead className={HEAD}>Term</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {report.disposals.length === 0 ? (
                    <TableRow>
                      <TableCell className={`${CELL} text-muted-foreground`} colSpan={9}>
                        No sales yet.
                      </TableCell>
                    </TableRow>
                  ) : (
                    report.disposals.map((sale, i) => (
                      <TableRow key={`${sale.symbol}-${sale.closed}-${i}`}>
                        <TableCell className={`${CELL} font-semibold`}>
                          {sale.symbol}
                          {sale.estimated && (
                            <Badge
                              variant="secondary"
                              className="ml-2"
                              title="Matched against an estimated opening lot"
                            >
                              est.
                            </Badge>
                          )}
                        </TableCell>
                        <TableCell className={CELL}>{day(sale.opened)}</TableCell>
                        <TableCell className={CELL}>{day(sale.closed)}</TableCell>
                        <TableCell className={NUM}>{formatQty(sale.quantity)}</TableCell>
                        <TableCell className={NUM}>{money(sale.proceeds)}</TableCell>
                        <TableCell className={NUM}>{money(sale.cost_basis)}</TableCell>
                        <TableCell className={cn(NUM, pnlClass(sale.realized_pnl))}>
                          {money(sale.realized_pnl)}
                        </TableCell>
                        <TableCell className={NUM}>{sale.holding_days}</TableCell>
                        <TableCell className={CELL}>
                          <TermBadge term={sale.term} />
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </section>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
