import {
  AreaSeries,
  ColorType,
  createChart,
  type IChartApi,
  type ISeriesApi,
  type UTCTimestamp,
} from 'lightweight-charts'
import { Loader2, TrendingUp } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { fundsCommands, type PortfolioHistory } from '@/api/tauri-client'
import { Card, CardContent } from '@/components/ui/card'
import { formatPercent, formatSignedUSD, formatUSD, pnlColorClass } from '@/lib/format'
import { cn } from '@/lib/utils'
import { useThemeStore } from '@/stores/themeStore'

// Alpaca period/timeframe pairs behind each toggle.
const RANGES = [
  { key: '1D', period: '1D', timeframe: '5Min', refreshMs: 30000 },
  { key: '1M', period: '1M', timeframe: '1D', refreshMs: 300000 },
  { key: '1Y', period: '1A', timeframe: '1D', refreshMs: 600000 },
  { key: 'All', period: 'all', timeframe: '1D', refreshMs: 600000 },
] as const

type RangeKey = (typeof RANGES)[number]['key']

// lightweight-charts renders timestamps in UTC. Alpaca returns UTC epoch
// seconds, so we shift each point by the America/New_York offset (DST-aware via
// Intl — never a hardcoded -4/-5) so the axis and crosshair read Eastern time.
const ET_FMT = new Intl.DateTimeFormat('en-US', {
  timeZone: 'America/New_York',
  hour12: false,
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
})
function toEtSeconds(utcSeconds: number): number {
  const parts = ET_FMT.formatToParts(new Date(utcSeconds * 1000))
  const p: Record<string, string> = {}
  for (const part of parts) p[part.type] = part.value
  // hour can come back as "24" at midnight in some engines — normalize to 0.
  const hour = p.hour === '24' ? 0 : Number(p.hour)
  const asUtc = Date.UTC(Number(p.year), Number(p.month) - 1, Number(p.day), hour, Number(p.minute), Number(p.second))
  return Math.floor(asUtc / 1000)
}

export function PortfolioChartCard() {
  const { mode } = useThemeStore()
  const isDark = mode === 'dark'
  const [rangeKey, setRangeKey] = useState<RangeKey>('1D')
  const [history, setHistory] = useState<PortfolioHistory | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const seriesRef = useRef<ISeriesApi<'Area'> | null>(null)

  const range = RANGES.find((r) => r.key === rangeKey) ?? RANGES[0]

  // Fetch on range change + refresh on an interval.
  useEffect(() => {
    let cancelled = false
    const load = async () => {
      try {
        const h = await fundsCommands.getPortfolioHistory(range.period, range.timeframe)
        if (!cancelled) setHistory(h)
      } catch {
        // transient; keep last
      } finally {
        if (!cancelled) setIsLoading(false)
      }
    }
    load()
    const id = setInterval(load, range.refreshMs)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [range.period, range.timeframe, range.refreshMs])

  // Pair timestamps (epoch seconds) with finite equity points, ascending.
  const points = useMemo(() => {
    if (!history?.equity?.length) return []
    const out: { time: UTCTimestamp; value: number }[] = []
    for (let i = 0; i < history.equity.length; i++) {
      const v = history.equity[i]
      const t = history.timestamp[i]
      if (typeof v === 'number' && Number.isFinite(v) && v > 0 && typeof t === 'number') {
        out.push({ time: toEtSeconds(t) as UTCTimestamp, value: v })
      }
    }
    return out
  }, [history])

  const { value, change, changePct, positive } = useMemo(() => {
    const base = history?.base_value ?? (points[0]?.value || 0)
    const last = points[points.length - 1]?.value ?? base
    const ch = last - base
    return {
      value: last,
      change: ch,
      changePct: base > 0 ? (ch / base) * 100 : 0,
      positive: ch >= 0,
    }
  }, [history, points])

  // Build / rebuild the chart.
  useEffect(() => {
    if (!containerRef.current) return
    if (chartRef.current) {
      chartRef.current.remove()
      chartRef.current = null
    }
    const container = containerRef.current
    const chart = createChart(container, {
      width: container.offsetWidth || 600,
      height: 220,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: isDark ? '#a6adbb' : '#64748b',
      },
      grid: { vertLines: { visible: false }, horzLines: { visible: false } },
      rightPriceScale: { borderVisible: false },
      timeScale: { borderVisible: false, timeVisible: rangeKey === '1D', secondsVisible: false },
      crosshair: { horzLine: { visible: true }, vertLine: { visible: true } },
      handleScroll: false,
      handleScale: false,
    })
    const color = positive ? '#22c55e' : '#ef4444'
    const series = chart.addSeries(AreaSeries, {
      lineColor: color,
      topColor: positive ? 'rgba(34,197,94,0.35)' : 'rgba(239,68,68,0.35)',
      bottomColor: positive ? 'rgba(34,197,94,0.0)' : 'rgba(239,68,68,0.0)',
      lineWidth: 2,
      priceLineVisible: false,
    })
    if (points.length > 0) {
      series.setData(points)
      chart.timeScale().fitContent()
    }
    chartRef.current = chart
    seriesRef.current = series

    const onResize = () => {
      if (chartRef.current && container.offsetWidth > 0) {
        chartRef.current.applyOptions({ width: container.offsetWidth })
      }
    }
    const ro = new ResizeObserver(onResize)
    ro.observe(container)
    return () => {
      ro.disconnect()
      if (chartRef.current) {
        chartRef.current.remove()
        chartRef.current = null
      }
    }
  }, [points, positive, isDark, rangeKey])

  return (
    <Card>
      <CardContent className="pt-6">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <TrendingUp className="h-4 w-4 text-primary" />
              Portfolio Value
            </div>
            <div className="mt-1 text-3xl font-bold tabular-nums">{formatUSD(value)}</div>
            <div className={cn('mt-1 text-sm font-medium tabular-nums', pnlColorClass(change))}>
              {formatSignedUSD(change)} ({formatPercent(changePct, true)}){' '}
              <span className="text-muted-foreground font-normal">
                {rangeKey === '1D' ? 'today' : `past ${rangeKey}`}
              </span>
            </div>
          </div>
          <div className="flex h-8 shrink-0 items-center gap-1 rounded-lg border border-border p-0.5">
            {RANGES.map((r) => (
              <button
                key={r.key}
                type="button"
                onClick={() => setRangeKey(r.key)}
                className={cn(
                  'h-7 rounded-md px-3 text-xs font-medium transition-colors',
                  rangeKey === r.key
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:text-foreground'
                )}
              >
                {r.key}
              </button>
            ))}
          </div>
        </div>

        <div className="relative mt-4 h-[220px]">
          {isLoading && !history ? (
            <div className="flex h-full items-center justify-center text-muted-foreground">
              <Loader2 className="h-5 w-5 animate-spin" />
            </div>
          ) : points.length === 0 ? (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              No portfolio history for this range yet.
            </div>
          ) : (
            <div ref={containerRef} className="absolute inset-0" />
          )}
        </div>
      </CardContent>
    </Card>
  )
}
