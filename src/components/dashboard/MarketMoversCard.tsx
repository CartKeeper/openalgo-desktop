import { Flame, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { type MarketMover, providerCommands } from '@/api/tauri-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { formatUSD, pnlColorClass } from '@/lib/format'
import { cn } from '@/lib/utils'

const TABS = [
  { key: 'gainers', label: 'Gainers', fetch: providerCommands.getMarketGainers },
  { key: 'losers', label: 'Losers', fetch: providerCommands.getMarketLosers },
  { key: 'active', label: 'Most Active', fetch: providerCommands.getMarketMostActive },
] as const

type TabKey = (typeof TABS)[number]['key']

const POLL_MS = 60000
const MAX_ROWS = 7

export function MarketMoversCard() {
  const [tab, setTab] = useState<TabKey>('gainers')
  const [rows, setRows] = useState<MarketMover[]>([])
  const [isLoading, setIsLoading] = useState(true)

  const active = TABS.find((t) => t.key === tab) ?? TABS[0]

  const load = useCallback(async () => {
    try {
      const data = await active.fetch()
      setRows(data.slice(0, MAX_ROWS))
    } catch {
      // transient; keep last
    } finally {
      setIsLoading(false)
    }
  }, [active])

  useEffect(() => {
    setIsLoading(true)
    load()
    const id = setInterval(load, POLL_MS)
    return () => clearInterval(id)
  }, [load])

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Flame className="h-4 w-4 text-primary" />
          Market Movers
        </CardTitle>
        <div className="mt-2 flex h-8 items-center gap-1 rounded-lg border border-border p-0.5">
          {TABS.map((t) => (
            <button
              key={t.key}
              type="button"
              onClick={() => setTab(t.key)}
              className={cn(
                'h-7 flex-1 rounded-md px-2 text-xs font-medium transition-colors',
                tab === t.key
                  ? 'bg-primary text-primary-foreground'
                  : 'text-muted-foreground hover:text-foreground'
              )}
            >
              {t.label}
            </button>
          ))}
        </div>
      </CardHeader>
      <CardContent className="p-0">
        {isLoading && rows.length === 0 ? (
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : rows.length === 0 ? (
          <div className="py-10 text-center text-sm text-muted-foreground">No data available</div>
        ) : (
          <ul className="divide-y divide-border">
            {rows.map((r) => (
              <li key={r.symbol} className="flex items-center justify-between px-4 py-2.5">
                <div className="min-w-0">
                  <div className="font-semibold">{r.symbol}</div>
                  <div className="truncate text-xs text-muted-foreground">{r.name}</div>
                </div>
                <div className="ml-3 shrink-0 text-right">
                  <div className="tabular-nums">{formatUSD(r.price)}</div>
                  <div className={cn('text-xs font-medium tabular-nums', pnlColorClass(r.change_percent))}>
                    {r.change_percent != null
                      ? `${r.change_percent >= 0 ? '+' : ''}${r.change_percent.toFixed(2)}%`
                      : '--'}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}
