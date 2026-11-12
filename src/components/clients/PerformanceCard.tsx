import { useCallback, useEffect, useState } from 'react'
import { analyticsApi, type PerformanceReport, type PeriodKey } from '@/api/analytics'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table'
import { cn } from '@/lib/utils'

const money = (n: number) =>
  new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2 }).format(n)

/** A fraction (0.1523) as `+15.23%`; `—` when the engine could not compute it. */
const pct = (v: number | null) => (v == null ? '—' : `${v >= 0 ? '+' : ''}${(v * 100).toFixed(2)}%`)

const day = (iso: string) =>
  new Date(`${iso}T00:00:00`).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })

const toneClass = (v: number | null) =>
  v == null ? 'text-muted-foreground' : v >= 0 ? 'text-green-500' : 'text-red-500'

function PerformanceSkeleton() {
  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        {['1M', '3M', '6M', 'YTD', '1Y', 'SI'].map((key) => (
          <Skeleton key={key} className="h-8 w-16" />
        ))}
      </div>
      <Skeleton className="h-12" />
      <Skeleton className="h-12" />
      <Skeleton className="h-12" />
    </div>
  )
}

/**
 * A client's returns over 1M, 3M, 6M, YTD, 1Y and since inception: one period
 * selected at a time, time-weighted and money-weighted side by side.
 */
export function PerformanceCard({ clientId }: { clientId: number }) {
  const [report, setReport] = useState<PerformanceReport | null>(null)
  const [selectedKey, setSelectedKey] = useState<PeriodKey>('YTD')
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    setError(null)
    try {
      setReport(await analyticsApi.clientPerformance(clientId))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [clientId])

  useEffect(() => {
    load()
  }, [load])

  const selected = report?.periods.find((p) => p.key === selectedKey) ?? report?.periods[0] ?? null

  const rows: Array<[string, string, string]> = selected
    ? [
        ['Time-weighted return', pct(selected.twr), toneClass(selected.twr)],
        ['Annualized', pct(selected.twr_annualized), toneClass(selected.twr_annualized)],
        ['Money-weighted (XIRR, annualized)', pct(selected.mwr), toneClass(selected.mwr)],
        ['Start value', money(selected.start_value), ''],
        ['End value', money(selected.end_value), ''],
        ['Net flows', money(selected.net_flows), ''],
      ]
    : []

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Performance</CardTitle>
        <CardDescription>
          Time-weighted return strips out the timing of deposits and withdrawals; money-weighted
          (XIRR) includes it. Valued daily from FMP closes over the imported trade history.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {error ? (
          <div className="flex items-center justify-between gap-4">
            <p className="text-sm text-red-500">{error}</p>
            <Button variant="outline" size="sm" onClick={() => load()}>
              Retry
            </Button>
          </div>
        ) : report == null ? (
          <PerformanceSkeleton />
        ) : report.periods.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No trades yet. Import or add trades to see performance.
          </p>
        ) : (
          <>
            <div className="flex flex-wrap gap-2">
              {report.periods.map((period) => {
                const isSelected = period.key === selected?.key
                return (
                  <Button
                    key={period.key}
                    size="sm"
                    variant={isSelected ? 'default' : 'outline'}
                    aria-pressed={isSelected}
                    onClick={() => setSelectedKey(period.key)}
                  >
                    {period.key}
                  </Button>
                )
              })}
            </div>

            {selected && (
              <>
                <Table aria-label={`${selected.key} performance`}>
                  <TableBody>
                    {rows.map(([label, value, tone]) => (
                      <TableRow key={label}>
                        <TableCell className="h-12 px-4 font-semibold">{label}</TableCell>
                        <TableCell className={cn('h-12 px-4 text-right font-mono tabular-nums', tone)}>
                          {value}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
                <p className="text-xs text-muted-foreground">
                  {day(selected.start)} – {day(selected.end)}
                  {selected.note ? ` · ${selected.note}` : ''}
                  {selected.twr_annualized == null && selected.twr != null
                    ? ' · Windows under a year are not annualized.'
                    : ''}
                </p>
              </>
            )}

            {report.warnings.length > 0 && (
              <ul className="space-y-1 rounded-lg border border-amber-300 bg-amber-50/60 px-4 py-3 dark:border-amber-900 dark:bg-amber-950/30">
                {report.warnings.map((warning) => (
                  <li key={warning} className="text-xs text-amber-700 dark:text-amber-400">
                    {warning}
                  </li>
                ))}
              </ul>
            )}

            <p className="text-xs text-muted-foreground">Prices as of {report.prices_as_of}</p>
          </>
        )}
      </CardContent>
    </Card>
  )
}
