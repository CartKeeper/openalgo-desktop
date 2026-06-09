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
// Yahoo symbols for the sparkline history (free, no API key). Gold/BTC differ
// from their FMP quote symbols.
const YAHOO_SYMBOL: Record<string, string> = {
  '^GSPC': '^GSPC',
  '^DJI': '^DJI',
  '^IXIC': '^IXIC',
  GCUSD: 'GC=F',
  BTCUSD: 'BTC-USD',
}
const POLL_MS = 60000

interface Tile {
  key: string
  label: string
  value: string
  changePct: number | null
  series: number[]
  isYield: boolean
}

function fmtNum(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n)) return '--'
  return n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

// Today's change, formatted per instrument (basis points for the 10Y yield).
function fmtChange(t: Pick<Tile, 'changePct' | 'isYield'>): string {
  if (t.changePct == null) return '--'
  const sign = t.changePct >= 0 ? '+' : ''
  return t.isYield
    ? `${sign}${(t.changePct * 100).toFixed(0)} bp`
    : `${sign}${t.changePct.toFixed(2)}%`
}

// Inline SVG sparkline — small (rail) or large (detail), colored by direction.
function Sparkline({
  data,
  positive,
  className,
  area = false,
}: {
  data: number[]
  positive: boolean
  className?: string
  area?: boolean
}) {
  if (data.length < 2) return <div className={className} />
  const w = 100
  const h = 32
  const min = Math.min(...data)
  const max = Math.max(...data)
  const range = max - min || 1
  const coords = data.map((v, i) => {
    const x = (i / (data.length - 1)) * w
    const y = h - ((v - min) / range) * h
    return [x, y] as const
  })
  const line = coords.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(' ')
  const areaPath = `M0,${h} L${line.replace(/ /g, ' L')} L${w},${h} Z`
  const stroke = positive ? 'stroke-green-500' : 'stroke-red-500'
  const fill = positive ? 'fill-green-500/10' : 'fill-red-500/10'
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" className={className} aria-hidden="true">
      {area && <path d={areaPath} className={cn('stroke-none', fill)} />}
      <polyline
        points={line}
        fill="none"
        strokeWidth={1.5}
        vectorEffect="non-scaling-stroke"
        className={stroke}
      />
    </svg>
  )
}

// Build a plain-language read on the market from the numbers alone.
function describe(t: Tile): string {
  const name = t.label === 'US 10Y' ? 'The US 10-year Treasury yield' : t.label
  const today = t.changePct
  const dir = (n: number | null) => (n == null ? 'flat' : n > 0 ? 'up' : n < 0 ? 'down' : 'flat')

  let rangePhrase = ''
  let monthPhrase = ''
  if (t.series.length >= 2) {
    const first = t.series[0]
    const last = t.series[t.series.length - 1]
    const min = Math.min(...t.series)
    const max = Math.max(...t.series)
    const pos = (last - min) / (max - min || 1)
    const where = pos < 0.33 ? 'the low end' : pos > 0.66 ? 'the high end' : 'the middle'
    rangePhrase = `, near ${where} of its 30-day range`

    if (t.isYield) {
      const bp = (last - first) * 100
      monthPhrase = ` and ${dir(bp)} ${Math.abs(bp).toFixed(0)} bp over the past month`
    } else if (first > 0) {
      const pct = ((last - first) / first) * 100
      monthPhrase = ` and ${dir(pct)} ${Math.abs(pct).toFixed(1)}% over the past month`
    }
  }

  const todayPhrase = t.isYield
    ? `${dir(today)} ${today != null ? Math.abs(today * 100).toFixed(0) : '--'} bp today`
    : `${dir(today)} ${today != null ? Math.abs(today).toFixed(2) : '--'}% today`

  return `${name} is ${todayPhrase}${monthPhrase}${rangePhrase}.`
}

// One stat in the detail pane.
function Stat({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <div className="rounded-lg border bg-muted/40 px-3 py-2">
      <div className="text-[10px] uppercase tracking-[0.05em] text-muted-foreground">{label}</div>
      <div className={cn('mt-0.5 text-sm font-semibold tabular-nums', color)}>{value}</div>
    </div>
  )
}

export function MarketsTickerCard() {
  const [tiles, setTiles] = useState<Tile[]>([])
  const [selectedKey, setSelectedKey] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const load = useCallback(async () => {
    try {
      const isoStart = new Date(Date.now() - 30 * 86400000).toISOString().slice(0, 10)
      // This FMP plan only allows single-symbol quotes (multi-symbol batch is
      // gated), so fetch each ticker individually. Sparkline history comes from
      // Yahoo (free); the 10Y reuses its FRED series below.
      const [quoteLists, histLists, fred] = await Promise.all([
        Promise.all(
          QUOTE_SYMBOLS.map((s) => providerCommands.getBatchQuote(s).catch(() => [] as FmpQuote[]))
        ),
        Promise.all(
          QUOTE_SYMBOLS.map((s) =>
            providerCommands
              .getYahooHistorical(YAHOO_SYMBOL[s], '1d', '1mo')
              .catch(() => [] as [string, number, number, number, number, number][])
          )
        ),
        providerCommands.getFredSeries('DGS10', isoStart).catch(() => [] as unknown[]),
      ])

      const quotes = quoteLists.flat()
      const bySym = new Map(quotes.map((q) => [q.symbol, q]))
      const out: Tile[] = QUOTE_SYMBOLS.map((s, i) => {
        const q = bySym.get(s)
        const closes = histLists[i].map((row) => row[4]).filter((v) => v != null && !Number.isNaN(v))
        return {
          key: s,
          label: LABELS[s] ?? s,
          value: fmtNum(q?.price),
          changePct: q?.change_percent ?? null,
          series: closes,
          isYield: false,
        }
      })

      // US 10-year Treasury yield from FRED (full 30-day series for the sparkline).
      const obs = (fred as { date: string; value: number | null }[]).filter((o) => o.value != null)
      if (obs.length > 0) {
        const vals = obs.map((o) => o.value as number)
        const last = vals[vals.length - 1]
        const prev = vals.length > 1 ? vals[vals.length - 2] : last
        out.push({
          key: 'US10Y',
          label: 'US 10Y',
          value: `${last.toFixed(2)}%`,
          changePct: last - prev,
          series: vals,
          isYield: true,
        })
      }
      setTiles(out)
      setSelectedKey((prev) => (prev && out.some((t) => t.key === prev) ? prev : out[0]?.key ?? null))
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

  const selected = tiles.find((t) => t.key === selectedKey) ?? tiles[0] ?? null

  // 1-month change of the selected market (percent, or basis points for the yield).
  const monthChange = (() => {
    if (!selected || selected.series.length < 2) return null
    const first = selected.series[0]
    const last = selected.series[selected.series.length - 1]
    if (selected.isYield) {
      const bp = (last - first) * 100
      return { text: `${bp >= 0 ? '+' : ''}${bp.toFixed(0)} bp`, val: bp }
    }
    if (first <= 0) return null
    const pct = ((last - first) / first) * 100
    return { text: `${pct >= 0 ? '+' : ''}${pct.toFixed(1)}%`, val: pct }
  })()

  return (
    <Card className="h-140">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Globe className="h-4 w-4 text-primary" />
          Markets
        </CardTitle>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 p-0">
        {isLoading && tiles.length === 0 ? (
          <div className="flex flex-1 items-center justify-center text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : (
          <div className="flex min-h-0 flex-1">
            {/* Vertical market tabs */}
            <div className="no-scrollbar w-36 shrink-0 overflow-y-auto border-r border-border">
              {tiles.map((t) => {
                const isSelected = t.key === selected?.key
                return (
                  <button
                    key={t.key}
                    type="button"
                    onClick={() => setSelectedKey(t.key)}
                    className={cn(
                      'w-full border-l-2 px-3 py-2.5 text-left transition-colors',
                      isSelected
                        ? 'border-l-primary bg-primary/10'
                        : 'border-l-transparent hover:bg-muted/50'
                    )}
                  >
                    <div className="truncate text-xs text-muted-foreground">{t.label}</div>
                    <div className="truncate text-sm font-semibold tabular-nums">{t.value}</div>
                    <div
                      className={cn(
                        'text-[11px] font-medium tabular-nums',
                        pnlColorClass(t.changePct)
                      )}
                    >
                      {fmtChange(t)}
                    </div>
                    <Sparkline
                      data={t.series}
                      positive={(t.changePct ?? 0) >= 0}
                      className="mt-1 h-5 w-full"
                    />
                  </button>
                )
              })}
            </div>

            {/* Detail pane for the selected market */}
            {selected && (
              <div className="flex min-w-0 flex-1 flex-col gap-3 p-4">
                <div>
                  <div className="text-sm text-muted-foreground">{selected.label}</div>
                  <div className="mt-0.5 flex items-baseline gap-3">
                    <span className="text-3xl font-bold tabular-nums">{selected.value}</span>
                    <span
                      className={cn(
                        'text-sm font-semibold tabular-nums',
                        pnlColorClass(selected.changePct)
                      )}
                    >
                      {fmtChange(selected)}
                    </span>
                  </div>
                </div>

                <Sparkline
                  data={selected.series}
                  positive={(selected.changePct ?? 0) >= 0}
                  area
                  className="h-20 w-full"
                />

                <div className="grid grid-cols-2 gap-2">
                  <Stat
                    label="Today"
                    value={fmtChange(selected)}
                    color={pnlColorClass(selected.changePct)}
                  />
                  <Stat
                    label="30-Day High"
                    value={
                      selected.series.length
                        ? selected.isYield
                          ? `${Math.max(...selected.series).toFixed(2)}%`
                          : fmtNum(Math.max(...selected.series))
                        : '--'
                    }
                  />
                  <Stat
                    label="30-Day Low"
                    value={
                      selected.series.length
                        ? selected.isYield
                          ? `${Math.min(...selected.series).toFixed(2)}%`
                          : fmtNum(Math.min(...selected.series))
                        : '--'
                    }
                  />
                  <Stat
                    label="1 Month"
                    value={monthChange?.text ?? '--'}
                    color={pnlColorClass(monthChange?.val ?? null)}
                  />
                </div>

                <p className="text-sm leading-relaxed text-muted-foreground">{describe(selected)}</p>
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
