import { Globe, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { type FmpQuote, providerCommands } from '@/api/tauri-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { pnlColorClass } from '@/lib/format'
import { cn } from '@/lib/utils'

const QUOTE_SYMBOLS = ['^GSPC', '^DJI', '^IXIC', 'GCUSD', 'BTCUSD']
const LABELS: Record<string, string> = {
  '^GSPC': 'S&P 500',
  '^DJI': 'Dow Jones',
  '^IXIC': 'Nasdaq',
  GCUSD: 'Gold',
  BTCUSD: 'Bitcoin',
}
const POLL_MS = 60000

interface Tile {
  label: string
  value: string
  changePct: number | null
}

function fmtNum(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n)) return '--'
  return n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

export function MarketsTickerCard() {
  const [tiles, setTiles] = useState<Tile[]>([])
  const [isLoading, setIsLoading] = useState(true)

  const load = useCallback(async () => {
    try {
      const isoStart = new Date(Date.now() - 30 * 86400000).toISOString().slice(0, 10)
      // This FMP plan only allows single-symbol quotes (multi-symbol batch is
      // gated), so fetch each ticker individually.
      const [quoteLists, fred] = await Promise.all([
        Promise.all(
          QUOTE_SYMBOLS.map((s) => providerCommands.getBatchQuote(s).catch(() => [] as FmpQuote[]))
        ),
        providerCommands.getFredSeries('DGS10', isoStart).catch(() => [] as unknown[]),
      ])

      const quotes = quoteLists.flat()
      const bySym = new Map(quotes.map((q) => [q.symbol, q]))
      const out: Tile[] = QUOTE_SYMBOLS.map((s) => {
        const q = bySym.get(s)
        return { label: LABELS[s] ?? s, value: fmtNum(q?.price), changePct: q?.change_percent ?? null }
      })

      // US 10-year Treasury yield from FRED (last two valid observations).
      const obs = (fred as { date: string; value: number | null }[]).filter((o) => o.value != null)
      if (obs.length > 0) {
        const last = obs[obs.length - 1].value as number
        const prev = obs.length > 1 ? (obs[obs.length - 2].value as number) : last
        out.push({
          label: 'US 10Y',
          value: `${last.toFixed(2)}%`,
          // daily change in basis points, expressed as a signed number for coloring
          changePct: last - prev,
        })
      }
      setTiles(out)
    } catch {
      // transient; keep last
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    load()
    const id = setInterval(load, POLL_MS)
    return () => clearInterval(id)
  }, [load])

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Globe className="h-4 w-4 text-primary" />
          Markets
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading && tiles.length === 0 ? (
          <div className="flex items-center justify-center py-8 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-x-4 gap-y-3 sm:grid-cols-3">
            {tiles.map((t) => (
              <div key={t.label}>
                <div className="text-xs text-muted-foreground">{t.label}</div>
                <div className="text-lg font-semibold tabular-nums">{t.value}</div>
                <div className={cn('text-xs font-medium tabular-nums', pnlColorClass(t.changePct))}>
                  {t.changePct != null
                    ? t.label === 'US 10Y'
                      ? `${t.changePct >= 0 ? '+' : ''}${(t.changePct * 100).toFixed(0)} bp`
                      : `${t.changePct >= 0 ? '+' : ''}${t.changePct.toFixed(2)}%`
                    : '--'}
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
