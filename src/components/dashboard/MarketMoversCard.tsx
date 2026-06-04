import { ExternalLink, Flame, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { type GenericQuote, type MarketMover, providerCommands } from '@/api/tauri-client'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { formatPercent, formatUSD, formatUSDCompact, pnlColorClass } from '@/lib/format'
import { cn } from '@/lib/utils'

const TABS = [
  { key: 'gainers', label: 'Gainers', fetch: providerCommands.getMarketGainers },
  { key: 'losers', label: 'Losers', fetch: providerCommands.getMarketLosers },
  { key: 'active', label: 'Most Active', fetch: providerCommands.getMarketMostActive },
] as const

type TabKey = (typeof TABS)[number]['key']

const POLL_MS = 60000
const MAX_ROWS = 50
// Exclude penny / low-priced names — only show movers trading at this price or higher.
const MIN_PRICE = 50

const COMPACT_NUM = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 })
const formatCompactNum = (v: number | null | undefined) =>
  v == null || Number.isNaN(v) ? '--' : COMPACT_NUM.format(v)

// ---------- Quick briefing snapshot (popover content) ----------

function SnapshotStat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-[10px] uppercase tracking-[0.05em] text-muted-foreground">{label}</div>
      <div className="tabular-nums text-sm">{value}</div>
    </div>
  )
}

function MoverBriefing({ symbol, onFullAnalysis }: { symbol: string; onFullAnalysis: () => void }) {
  const [quote, setQuote] = useState<GenericQuote | null>(null)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')

  useEffect(() => {
    let cancelled = false
    setStatus('loading')
    providerCommands
      .getGenericQuote([symbol])
      .then((data) => {
        if (cancelled) return
        if (data.length > 0) {
          setQuote(data[0])
          setStatus('ready')
        } else {
          setStatus('error')
        }
      })
      .catch(() => {
        if (!cancelled) setStatus('error')
      })
    return () => {
      cancelled = true
    }
  }, [symbol])

  return (
    <div className="space-y-3">
      {status === 'loading' && (
        <div className="flex items-center justify-center py-6 text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
        </div>
      )}

      {status === 'error' && (
        <div className="py-4 text-center text-sm text-muted-foreground">
          Couldn't load a quote for {symbol}.
        </div>
      )}

      {status === 'ready' && quote && (
        <>
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <div className="text-sm font-semibold">{quote.symbol}</div>
              {quote.name && (
                <div className="truncate text-xs text-muted-foreground">{quote.name}</div>
              )}
            </div>
            <div className="shrink-0 text-right">
              <div className="tabular-nums text-sm font-semibold">{formatUSD(quote.price)}</div>
              <div
                className={cn(
                  'text-xs font-medium tabular-nums',
                  pnlColorClass(quote.change_percent)
                )}
              >
                {formatPercent(quote.change_percent, true)}
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-x-4 gap-y-2 border-t border-border pt-3">
            <SnapshotStat label="Market Cap" value={formatUSDCompact(quote.market_cap)} />
            <SnapshotStat
              label="P/E"
              value={quote.pe_ratio != null ? quote.pe_ratio.toFixed(2) : '--'}
            />
            <SnapshotStat
              label="52W Range"
              value={`${formatUSD(quote.fifty_two_week_low)} – ${formatUSD(quote.fifty_two_week_high)}`}
            />
            <SnapshotStat label="Volume" value={formatCompactNum(quote.volume)} />
            <SnapshotStat label="Day Low" value={formatUSD(quote.low)} />
            <SnapshotStat label="Day High" value={formatUSD(quote.high)} />
          </div>
        </>
      )}

      <Button size="sm" className="h-8 w-full" onClick={onFullAnalysis}>
        <ExternalLink className="mr-1.5 h-3.5 w-3.5" />
        Full Analysis
      </Button>
    </div>
  )
}

// ---------- Single mover row (popover trigger) ----------

function MoverRow({
  mover,
  open,
  onOpenChange,
  onFullAnalysis,
}: {
  mover: MarketMover
  open: boolean
  onOpenChange: (open: boolean) => void
  onFullAnalysis: (symbol: string) => void
}) {
  return (
    <li>
      <Popover open={open} onOpenChange={onOpenChange}>
        <PopoverTrigger asChild>
          <button
            type="button"
            className={cn(
              'flex w-full items-center justify-between px-4 py-2.5 text-left transition-colors',
              open ? 'bg-primary/10' : 'hover:bg-muted/50'
            )}
          >
            <div className="min-w-0">
              <div className="font-semibold">{mover.symbol}</div>
              <div className="truncate text-xs text-muted-foreground">{mover.name}</div>
            </div>
            <div className="ml-3 shrink-0 text-right">
              <div className="tabular-nums">{formatUSD(mover.price)}</div>
              <div
                className={cn(
                  'text-xs font-medium tabular-nums',
                  pnlColorClass(mover.change_percent)
                )}
              >
                {mover.change_percent != null
                  ? `${mover.change_percent >= 0 ? '+' : ''}${mover.change_percent.toFixed(2)}%`
                  : '--'}
              </div>
            </div>
          </button>
        </PopoverTrigger>
        <PopoverContent side="right" align="start" sideOffset={8} className="w-72">
          <MoverBriefing symbol={mover.symbol} onFullAnalysis={() => onFullAnalysis(mover.symbol)} />
        </PopoverContent>
      </Popover>
    </li>
  )
}

export function MarketMoversCard() {
  const navigate = useNavigate()
  const [tab, setTab] = useState<TabKey>('gainers')
  const [rows, setRows] = useState<MarketMover[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [openSymbol, setOpenSymbol] = useState<string | null>(null)

  const active = TABS.find((t) => t.key === tab) ?? TABS[0]

  const load = useCallback(async () => {
    try {
      const data = await active.fetch()
      const filtered = data.filter((m) => m.price != null && m.price >= MIN_PRICE)
      setRows(filtered.slice(0, MAX_ROWS))
    } catch {
      // transient; keep last
    } finally {
      setIsLoading(false)
    }
  }, [active])

  useEffect(() => {
    setIsLoading(true)
    setOpenSymbol(null)
    load()
    const id = setInterval(load, POLL_MS)
    return () => clearInterval(id)
  }, [load])

  return (
    <Card className="h-140">
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
              onClick={() => {
                setTab(t.key)
                setOpenSymbol(null)
              }}
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
      <CardContent className="flex min-h-0 flex-1 flex-col p-0">
        {isLoading && rows.length === 0 ? (
          <div className="flex flex-1 items-center justify-center text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : rows.length === 0 ? (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            No data available
          </div>
        ) : (
          <ul
            className="h-full divide-y divide-border overflow-y-auto"
            onScroll={() => setOpenSymbol(null)}
          >
            {rows.map((r) => (
              <MoverRow
                key={r.symbol}
                mover={r}
                open={openSymbol === r.symbol}
                onOpenChange={(o) => setOpenSymbol(o ? r.symbol : null)}
                onFullAnalysis={(symbol) => navigate(`/fundamentals/${symbol}`)}
              />
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}
