import { invoke } from '@tauri-apps/api/core'
import {
  Activity,
  Loader2,
  RefreshCw,
  ShoppingCart,
  TrendingDown,
  TrendingUp,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { providerCommands } from '@/api/tauri-client'
import { PlaceOrderDialog } from '@/components/trading/PlaceOrderDialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'

interface WatchlistItem {
  id: number
  symbol: string
  addedAt: string
}

interface SymbolAnalysis {
  symbol: string
  currentPrice: number
  dailyRate: number
  r2: number
  projections: { label: string; days: number; price: number; changePct: number }[]
  sentiment: 'strong_up' | 'up' | 'flat' | 'down' | 'strong_down'
}

function computeTrend(
  historicalData: [string, number, number, number, number, number][],
  currentPrice: number
): Omit<SymbolAnalysis, 'symbol'> | null {
  if (historicalData.length < 20) return null

  const closes = historicalData.map((d) => d[4])
  const n = closes.length

  let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0
  for (let i = 0; i < n; i++) {
    sumX += i
    sumY += closes[i]
    sumXY += i * closes[i]
    sumX2 += i * i
  }
  const slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX)
  const intercept = (sumY - slope * sumX) / n
  const meanY = sumY / n
  let ssTot = 0, ssRes = 0
  for (let i = 0; i < n; i++) {
    ssTot += (closes[i] - meanY) ** 2
    ssRes += (closes[i] - (slope * i + intercept)) ** 2
  }
  const r2 = ssTot > 0 ? 1 - ssRes / ssTot : 0

  const horizons = [
    { label: '3M', days: 63 },
    { label: '6M', days: 126 },
    { label: '12M', days: 252 },
  ]

  const projections = horizons.map((h) => {
    const projected = currentPrice + slope * h.days
    const changePct = currentPrice > 0 ? ((projected - currentPrice) / currentPrice) * 100 : 0
    return { ...h, price: Math.max(0, projected), changePct }
  })

  const annualPct = currentPrice > 0 ? ((slope * 252) / currentPrice) * 100 : 0
  let sentiment: SymbolAnalysis['sentiment']
  if (annualPct > 20) sentiment = 'strong_up'
  else if (annualPct > 5) sentiment = 'up'
  else if (annualPct < -20) sentiment = 'strong_down'
  else if (annualPct < -5) sentiment = 'down'
  else sentiment = 'flat'

  return { currentPrice, dailyRate: slope, r2, projections, sentiment }
}

function formatPrice(val: number): string {
  return val.toLocaleString('en-US', { style: 'currency', currency: 'USD' })
}

const sentimentConfig = {
  strong_up: { label: 'Strong Up', color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/30' },
  up: { label: 'Uptrend', color: 'text-green-500', bg: 'bg-green-500/10 border-green-500/20' },
  flat: { label: 'Flat', color: 'text-yellow-500', bg: 'bg-yellow-500/10 border-yellow-500/20' },
  down: { label: 'Downtrend', color: 'text-red-500', bg: 'bg-red-500/10 border-red-500/20' },
  strong_down: { label: 'Strong Down', color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/30' },
}

type TrendRange = '6mo' | '1y' | '5y'

const rangeOptions: { value: TrendRange; label: string }[] = [
  { value: '6mo', label: '6M' },
  { value: '1y', label: '1Y' },
  { value: '5y', label: '5Y' },
]

export function WatchlistAnalysisCard() {
  const navigate = useNavigate()
  const [analyses, setAnalyses] = useState<SymbolAnalysis[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [hasWatchlist, setHasWatchlist] = useState<boolean | null>(null)
  const [range, setRange] = useState<TrendRange>('6mo')

  const fetchAnalysis = useCallback(async (dataRange: TrendRange) => {
    setIsLoading(true)
    try {
      const items = await invoke<WatchlistItem[]>('get_watchlist_symbols')
      if (items.length === 0) {
        setHasWatchlist(false)
        setAnalyses([])
        return
      }
      setHasWatchlist(true)

      // Fetch historical data for all symbols in parallel
      const results = await Promise.allSettled(
        items.map(async (item) => {
          const historical = await providerCommands.getYahooHistorical(item.symbol, '1d', dataRange)
          if (historical.length === 0) return null
          const currentPrice = historical[historical.length - 1][4]
          const trend = computeTrend(historical, currentPrice)
          if (!trend) return null
          return { symbol: item.symbol, ...trend }
        })
      )

      const valid = results
        .filter((r): r is PromiseFulfilledResult<SymbolAnalysis | null> => r.status === 'fulfilled')
        .map((r) => r.value)
        .filter((a): a is SymbolAnalysis => a !== null)

      // Sort by 12M projected change descending
      valid.sort((a, b) => {
        const aChange = a.projections[2]?.changePct ?? 0
        const bChange = b.projections[2]?.changePct ?? 0
        return bChange - aChange
      })

      setAnalyses(valid)
    } catch (err) {
      console.error('Failed to fetch watchlist analysis:', err)
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchAnalysis(range)
  }, [fetchAnalysis, range])

  if (hasWatchlist === false) return null

  return (
    <Card>
      <CardContent className="p-4">
        {/* Header */}
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Activity className="h-4 w-4 text-muted-foreground" />
            <h3 className="text-sm font-semibold">Watchlist Trend Analysis</h3>
            {analyses.length > 0 && (
              <span className="text-[10px] text-muted-foreground font-medium">
                ({analyses.length})
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            {/* Range toggle */}
            <div className="flex rounded-md border overflow-hidden">
              {rangeOptions.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setRange(opt.value)}
                  disabled={isLoading}
                  className={cn(
                    'px-2.5 py-1 text-[11px] font-medium transition-colors',
                    range === opt.value
                      ? 'bg-primary text-primary-foreground'
                      : 'hover:bg-muted text-muted-foreground'
                  )}
                >
                  {opt.label}
                </button>
              ))}
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => fetchAnalysis(range)}
              disabled={isLoading}
              aria-label="Refresh analysis"
            >
              {isLoading ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RefreshCw className="h-3.5 w-3.5" />
              )}
            </Button>
          </div>
        </div>

        {isLoading && analyses.length === 0 ? (
          <div className="space-y-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="flex items-center gap-3 px-3 py-2">
                <Skeleton className="h-4 w-16" />
                <Skeleton className="h-4 w-12 ml-auto" />
                <Skeleton className="h-4 w-12" />
                <Skeleton className="h-4 w-12" />
                <Skeleton className="h-4 w-12" />
              </div>
            ))}
          </div>
        ) : analyses.length === 0 ? (
          <div className="text-center py-6">
            <Activity className="h-8 w-8 mx-auto text-muted-foreground/40 mb-2" />
            <p className="text-sm text-muted-foreground">Add symbols to your watchlist to see trend analysis</p>
          </div>
        ) : (
          <ScrollArea className="max-h-[400px] overflow-auto">
            {/* Table header */}
            <div className="grid grid-cols-[1fr_80px_80px_80px_80px_80px_40px] gap-2 px-3 py-1.5 border-b text-[10px] text-muted-foreground font-semibold uppercase tracking-wider">
              <div>Symbol</div>
              <div className="text-right">Current</div>
              <div className="text-right">3 Month</div>
              <div className="text-right">6 Month</div>
              <div className="text-right">12 Month</div>
              <div className="text-center">Trend</div>
              <div />
            </div>

            {/* Rows */}
            <div className="divide-y divide-border">
              {analyses.map((a) => {
                const config = sentimentConfig[a.sentiment]
                const isUp = a.dailyRate >= 0

                return (
                  <div
                    key={a.symbol}
                    className="grid grid-cols-[1fr_80px_80px_80px_80px_80px_40px] gap-2 items-center px-3 py-2 hover:bg-muted/50 transition-colors cursor-pointer"
                    onClick={() => navigate(`/fundamentals/${a.symbol}`)}
                  >
                    {/* Symbol */}
                    <div className="flex items-center gap-1.5 min-w-0">
                      {isUp ? (
                        <TrendingUp className="h-3 w-3 text-green-500 flex-shrink-0" />
                      ) : (
                        <TrendingDown className="h-3 w-3 text-red-500 flex-shrink-0" />
                      )}
                      <span className="text-sm font-semibold truncate">{a.symbol}</span>
                      <span className="text-[10px] text-muted-foreground tabular-nums">
                        R²{(a.r2 * 100).toFixed(0)}%
                      </span>
                    </div>

                    {/* Current price */}
                    <div className="text-right text-xs font-medium tabular-nums">
                      {formatPrice(a.currentPrice)}
                    </div>

                    {/* 3M projection */}
                    {a.projections.map((p) => (
                      <div key={p.label} className="text-right">
                        <div className="text-xs font-medium tabular-nums">
                          {formatPrice(p.price)}
                        </div>
                        <div
                          className={cn(
                            'text-[10px] font-medium tabular-nums',
                            p.changePct >= 0 ? 'text-green-500' : 'text-red-500'
                          )}
                        >
                          {p.changePct >= 0 ? '+' : ''}{p.changePct.toFixed(1)}%
                        </div>
                      </div>
                    ))}

                    {/* Sentiment badge */}
                    <div className="flex justify-center">
                      <Badge
                        variant="outline"
                        className={cn('text-[9px] px-1 py-0 whitespace-nowrap', config.color, config.bg)}
                      >
                        {config.label}
                      </Badge>
                    </div>

                    {/* Trade button */}
                    <div className="flex justify-center" onClick={(e) => e.stopPropagation()}>
                      <PlaceOrderDialog
                        defaultSymbol={a.symbol}
                        trigger={
                          <Button
                            variant="ghost"
                            size="icon"
                            className="h-6 w-6 text-green-500 hover:text-green-600"
                          >
                            <ShoppingCart className="h-3 w-3" />
                          </Button>
                        }
                      />
                    </div>
                  </div>
                )
              })}
            </div>

            {/* Disclaimer */}
            <p className="text-[10px] text-muted-foreground text-center py-2 border-t">
              Linear trend projection based on {range === '6mo' ? '6-month' : range === '1y' ? '1-year' : '5-year'} historical data · Not financial advice
            </p>
          </ScrollArea>
        )}
      </CardContent>
    </Card>
  )
}
