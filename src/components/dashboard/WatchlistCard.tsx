import { invoke } from '@tauri-apps/api/core'
import {
  Activity,
  BarChart3,
  Eye,
  ExternalLink,
  Loader2,
  Minus,
  Plus,
  RefreshCw,
  Search,
  ShoppingCart,
  Trash2,
  TrendingDown,
  TrendingUp,
  X,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { providerCommands } from '@/api/tauri-client'
import { tradingApi } from '@/api/trading'
import { PlaceOrderDialog } from '@/components/trading/PlaceOrderDialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'
import type { Holding, Order, Position } from '@/types/trading'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WatchlistItem {
  id: number
  symbol: string
  addedAt: string
}

interface YahooQuote {
  symbol: string
  name: string | null
  price: number
  change: number | null
  changePercent: number | null
  open: number | null
  high: number | null
  low: number | null
  previousClose: number | null
  volume: number | null
  marketCap: number | null
  peRatio: number | null
  fiftyTwoWeekHigh: number | null
  fiftyTwoWeekLow: number | null
}

interface SearchResult {
  symbol: string
  name: string
  exchange: string | null
  assetType: string | null
  exchangeDisplay: string | null
}

interface TrendProjection {
  label: string
  days: number
  price: number
  changePct: number
}

interface TechnicalSignals {
  sma20: number | null
  sma50: number | null
  sma200: number | null
  rsi14: number | null
  priceVsSma20: 'above' | 'below' | null
  priceVsSma50: 'above' | 'below' | null
  priceVsSma200: 'above' | 'below' | null
  smaCrossover: 'golden_cross' | 'death_cross' | null
}

interface SymbolTrend {
  dailyRate: number
  r2: number
  projections: TrendProjection[]
  sentiment: 'strong_up' | 'up' | 'flat' | 'down' | 'strong_down'
  technicals: TechnicalSignals
}

type TrendRange = '6mo' | '1y' | '5y'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function computeSMA(closes: number[], period: number): number | null {
  if (closes.length < period) return null
  const slice = closes.slice(-period)
  return slice.reduce((a, b) => a + b, 0) / period
}

function computeRSI(closes: number[], period = 14): number | null {
  if (closes.length < period + 1) return null
  let gains = 0
  let losses = 0
  const start = closes.length - period - 1
  for (let i = start + 1; i <= start + period; i++) {
    const diff = closes[i] - closes[i - 1]
    if (diff >= 0) gains += diff
    else losses -= diff
  }
  let avgGain = gains / period
  let avgLoss = losses / period
  for (let i = start + period + 1; i < closes.length; i++) {
    const diff = closes[i] - closes[i - 1]
    avgGain = (avgGain * (period - 1) + Math.max(diff, 0)) / period
    avgLoss = (avgLoss * (period - 1) + Math.max(-diff, 0)) / period
  }
  if (avgLoss === 0) return 100
  const rs = avgGain / avgLoss
  return 100 - 100 / (1 + rs)
}

function computeTrend(
  historicalData: [string, number, number, number, number, number][],
  currentPrice: number
): SymbolTrend | null {
  if (historicalData.length < 20) return null

  const closes = historicalData.map((d) => d[4])
  const n = closes.length

  // Linear regression
  let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0
  for (let i = 0; i < n; i++) {
    sumX += i; sumY += closes[i]; sumXY += i * closes[i]; sumX2 += i * i
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

  // Projections
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

  // Sentiment
  const annualPct = currentPrice > 0 ? ((slope * 252) / currentPrice) * 100 : 0
  let sentiment: SymbolTrend['sentiment']
  if (annualPct > 20) sentiment = 'strong_up'
  else if (annualPct > 5) sentiment = 'up'
  else if (annualPct < -20) sentiment = 'strong_down'
  else if (annualPct < -5) sentiment = 'down'
  else sentiment = 'flat'

  // Technical signals
  const sma20 = computeSMA(closes, 20)
  const sma50 = computeSMA(closes, 50)
  const sma200 = computeSMA(closes, 200)
  const rsi14 = computeRSI(closes)

  // SMA crossover detection (20 vs 50)
  let smaCrossover: TechnicalSignals['smaCrossover'] = null
  if (closes.length >= 52) {
    const prevCloses = closes.slice(0, -1)
    const prevSma20 = computeSMA(prevCloses, 20)
    const prevSma50 = computeSMA(prevCloses, 50)
    if (sma20 != null && sma50 != null && prevSma20 != null && prevSma50 != null) {
      if (prevSma20 < prevSma50 && sma20 >= sma50) smaCrossover = 'golden_cross'
      else if (prevSma20 > prevSma50 && sma20 <= sma50) smaCrossover = 'death_cross'
    }
  }

  const technicals: TechnicalSignals = {
    sma20,
    sma50,
    sma200,
    rsi14,
    priceVsSma20: sma20 != null ? (currentPrice >= sma20 ? 'above' : 'below') : null,
    priceVsSma50: sma50 != null ? (currentPrice >= sma50 ? 'above' : 'below') : null,
    priceVsSma200: sma200 != null ? (currentPrice >= sma200 ? 'above' : 'below') : null,
    smaCrossover,
  }

  return { dailyRate: slope, r2, projections, sentiment, technicals }
}

function formatPrice(val: number | null | undefined): string {
  if (val == null) return '--'
  return val.toLocaleString('en-US', { style: 'currency', currency: 'USD' })
}

function formatPct(val: number | null | undefined): string {
  if (val == null) return ''
  const sign = val >= 0 ? '+' : ''
  return `${sign}${val.toFixed(2)}%`
}

function formatMarketCap(val: number | null | undefined): string {
  if (val == null) return '--'
  if (val >= 1e12) return `$${(val / 1e12).toFixed(1)}T`
  if (val >= 1e9) return `$${(val / 1e9).toFixed(1)}B`
  if (val >= 1e6) return `$${(val / 1e6).toFixed(1)}M`
  return `$${val.toLocaleString()}`
}

function formatVolume(val: number | null | undefined): string {
  if (val == null) return '--'
  if (val >= 1e9) return `${(val / 1e9).toFixed(1)}B`
  if (val >= 1e6) return `${(val / 1e6).toFixed(1)}M`
  if (val >= 1e3) return `${(val / 1e3).toFixed(0)}K`
  return val.toLocaleString()
}

const sentimentConfig = {
  strong_up: { label: 'Strong Up', color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/30', barColor: 'bg-green-500' },
  up: { label: 'Uptrend', color: 'text-green-500', bg: 'bg-green-500/10 border-green-500/20', barColor: 'bg-green-500' },
  flat: { label: 'Flat', color: 'text-yellow-500', bg: 'bg-yellow-500/10 border-yellow-500/20', barColor: 'bg-yellow-500' },
  down: { label: 'Downtrend', color: 'text-red-500', bg: 'bg-red-500/10 border-red-500/20', barColor: 'bg-red-500' },
  strong_down: { label: 'Strong Down', color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/30', barColor: 'bg-red-500' },
}

const rangeOptions: { value: TrendRange; label: string }[] = [
  { value: '6mo', label: '6M' },
  { value: '1y', label: '1Y' },
  { value: '5y', label: '5Y' },
]

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** 52-week range bar showing where current price sits */
function FiftyTwoWeekBar({ low, high, current }: { low: number | null; high: number | null; current: number }) {
  if (low == null || high == null || high === low) return null
  const pct = Math.max(0, Math.min(100, ((current - low) / (high - low)) * 100))
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-[10px] text-muted-foreground">
        <span>52W Low {formatPrice(low)}</span>
        <span>52W High {formatPrice(high)}</span>
      </div>
      <div className="relative h-1.5 rounded-full bg-muted overflow-hidden">
        <div className="absolute inset-0 rounded-full bg-gradient-to-r from-red-500/40 via-yellow-500/40 to-green-500/40" />
        <div
          className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 rounded-full bg-primary border-2 border-background shadow-sm"
          style={{ left: `calc(${pct}% - 5px)` }}
        />
      </div>
    </div>
  )
}

/** Signal indicator chip */
function SignalChip({ label, signal, value }: { label: string; signal: 'bullish' | 'bearish' | 'neutral'; value?: string }) {
  const colors = {
    bullish: 'text-green-500 bg-green-500/10 border-green-500/20',
    bearish: 'text-red-500 bg-red-500/10 border-red-500/20',
    neutral: 'text-yellow-500 bg-yellow-500/10 border-yellow-500/20',
  }
  return (
    <div className={cn('flex items-center justify-between px-2.5 py-1.5 rounded-md border', colors[signal])}>
      <span className="text-[11px] font-medium">{label}</span>
      <span className="text-[10px] font-medium tabular-nums">{value || signal.charAt(0).toUpperCase() + signal.slice(1)}</span>
    </div>
  )
}

/** Projection row with visual bar */
function ProjectionRow({ label, price, changePct }: { label: string; price: number; changePct: number }) {
  const maxBar = 60 // max width percent for the bar
  const barWidth = Math.min(maxBar, Math.abs(changePct) * 1.5)
  const isPositive = changePct >= 0
  return (
    <div className="space-y-0.5">
      <div className="flex items-center justify-between">
        <span className="text-[11px] text-muted-foreground font-medium">{label}</span>
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium tabular-nums">{formatPrice(price)}</span>
          <span className={cn('text-[11px] font-semibold tabular-nums min-w-[52px] text-right', isPositive ? 'text-green-500' : 'text-red-500')}>
            {isPositive ? '+' : ''}{changePct.toFixed(1)}%
          </span>
        </div>
      </div>
      <div className="h-1 rounded-full bg-muted overflow-hidden">
        <div
          className={cn('h-full rounded-full transition-all duration-300', isPositive ? 'bg-green-500' : 'bg-red-500')}
          style={{ width: `${barWidth}%` }}
        />
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function WatchlistCard() {
  const navigate = useNavigate()
  const [watchlist, setWatchlist] = useState<WatchlistItem[]>([])
  const [quotes, setQuotes] = useState<Record<string, YahooQuote>>({})
  const [trends, setTrends] = useState<Record<string, SymbolTrend>>({})
  const [isLoading, setIsLoading] = useState(true)
  const [isLoadingTrends, setIsLoadingTrends] = useState(false)
  const [searchOpen, setSearchOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<SearchResult[]>([])
  const [isSearching, setIsSearching] = useState(false)
  const [selectedSymbol, setSelectedSymbol] = useState<string | null>(null)
  const [range, setRange] = useState<TrendRange>('6mo')
  const [positions, setPositions] = useState<Position[]>([])
  const [holdings, setHoldings] = useState<Holding[]>([])
  const [orders, setOrders] = useState<Order[]>([])
  const searchInputRef = useRef<HTMLInputElement>(null)
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // ---- Data fetching ----

  const fetchWatchlist = useCallback(async () => {
    try {
      const items = await invoke<WatchlistItem[]>('get_watchlist_symbols')
      setWatchlist(items)
    } catch (err) {
      console.error('Failed to fetch watchlist:', err)
    } finally {
      setIsLoading(false)
    }
  }, [])

  const fetchQuotes = useCallback(async (symbols: string[]) => {
    if (symbols.length === 0) return
    try {
      const data = await invoke<YahooQuote[]>('get_generic_quote', { symbols })
      const map: Record<string, YahooQuote> = {}
      for (const q of data) map[q.symbol] = q
      setQuotes((prev) => ({ ...prev, ...map }))
    } catch (err) {
      console.error('Failed to fetch quotes:', err)
    }
  }, [])

  const fetchTrends = useCallback(async (symbols: string[], dataRange: TrendRange) => {
    if (symbols.length === 0) return
    setIsLoadingTrends(true)
    try {
      const results = await Promise.allSettled(
        symbols.map(async (symbol) => {
          const historical = await providerCommands.getYahooHistorical(symbol, '1d', dataRange)
          if (historical.length === 0) return { symbol, trend: null }
          const currentPrice = historical[historical.length - 1][4]
          return { symbol, trend: computeTrend(historical, currentPrice) }
        })
      )
      const map: Record<string, SymbolTrend> = {}
      for (const r of results) {
        if (r.status === 'fulfilled' && r.value.trend) {
          map[r.value.symbol] = r.value.trend
        }
      }
      setTrends(map)
    } catch (err) {
      console.error('Failed to fetch trends:', err)
    } finally {
      setIsLoadingTrends(false)
    }
  }, [])

  const fetchPortfolioData = useCallback(async () => {
    try {
      const [posRes, holdRes, ordRes] = await Promise.allSettled([
        tradingApi.getPositions(''),
        tradingApi.getHoldings(''),
        tradingApi.getOrders(''),
      ])
      if (posRes.status === 'fulfilled' && posRes.value.status === 'success' && posRes.value.data) {
        setPositions(posRes.value.data)
      }
      if (holdRes.status === 'fulfilled' && holdRes.value.status === 'success' && holdRes.value.data) {
        setHoldings(holdRes.value.data.holdings || [])
      }
      if (ordRes.status === 'fulfilled' && ordRes.value.status === 'success' && ordRes.value.data) {
        setOrders(ordRes.value.data.orders || [])
      }
    } catch (err) {
      console.error('Failed to fetch portfolio data:', err)
    }
  }, [])

  useEffect(() => { fetchWatchlist() }, [fetchWatchlist])

  useEffect(() => {
    fetchPortfolioData()
    const interval = setInterval(fetchPortfolioData, 30000)
    return () => clearInterval(interval)
  }, [fetchPortfolioData])

  useEffect(() => {
    const symbols = watchlist.map((w) => w.symbol)
    if (symbols.length > 0) {
      fetchQuotes(symbols)
      fetchTrends(symbols, range)
      const interval = setInterval(() => fetchQuotes(symbols), 60000)
      return () => clearInterval(interval)
    }
  }, [watchlist, fetchQuotes, fetchTrends, range])

  useEffect(() => {
    if (searchOpen && searchInputRef.current) searchInputRef.current.focus()
  }, [searchOpen])

  // ---- Search ----

  const handleSearch = useCallback(async (query: string) => {
    setSearchQuery(query)
    if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current)
    if (query.length < 1) { setSearchResults([]); return }
    searchTimeoutRef.current = setTimeout(async () => {
      setIsSearching(true)
      try {
        const results = await invoke<SearchResult[]>('search_global_symbols', { query, limit: 6 })
        setSearchResults(results)
      } catch (err) {
        console.error('Symbol search failed:', err)
        setSearchResults([])
      } finally {
        setIsSearching(false)
      }
    }, 300)
  }, [])

  const addSymbol = async (symbol: string) => {
    try {
      await invoke('add_watchlist_symbol', { symbol })
      setSearchOpen(false)
      setSearchQuery('')
      setSearchResults([])
      fetchWatchlist()
    } catch (err) {
      console.error('Failed to add symbol:', err)
    }
  }

  const removeSymbol = async (symbol: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation()
    try {
      await invoke('remove_watchlist_symbol', { symbol })
      if (selectedSymbol === symbol) setSelectedSymbol(null)
      setWatchlist((prev) => prev.filter((w) => w.symbol !== symbol))
      setQuotes((prev) => { const next = { ...prev }; delete next[symbol]; return next })
      setTrends((prev) => { const next = { ...prev }; delete next[symbol]; return next })
    } catch (err) {
      console.error('Failed to remove symbol:', err)
    }
  }

  // ---- Sorted list (by 12M projection descending) ----

  const sortedWatchlist = [...watchlist].sort((a, b) => {
    const aTrend = trends[a.symbol]
    const bTrend = trends[b.symbol]
    const aChange = aTrend?.projections[2]?.changePct ?? 0
    const bChange = bTrend?.projections[2]?.changePct ?? 0
    return bChange - aChange
  })

  // ---- Detail panel data ----

  const selectedQuote = selectedSymbol ? quotes[selectedSymbol] : null
  const selectedTrend = selectedSymbol ? trends[selectedSymbol] : null

  // Filter portfolio data for selected symbol
  const symbolPositions = selectedSymbol
    ? positions.filter((p) => p.symbol.toUpperCase() === selectedSymbol.toUpperCase())
    : []
  const symbolHoldings = selectedSymbol
    ? holdings.filter((h) => h.symbol.toUpperCase() === selectedSymbol.toUpperCase())
    : []
  const symbolOrders = selectedSymbol
    ? orders
        .filter((o) => o.symbol.toUpperCase() === selectedSymbol.toUpperCase())
        .sort((a, b) => (b.timestamp || '').localeCompare(a.timestamp || ''))
        .slice(0, 5)
    : []
  const hasPortfolioData = symbolPositions.length > 0 || symbolHoldings.length > 0 || symbolOrders.length > 0

  // Compute overall technical score from signals
  function getTechnicalSummary(t: SymbolTrend | null): { label: string; signal: 'bullish' | 'bearish' | 'neutral'; score: number } {
    if (!t) return { label: 'No Data', signal: 'neutral', score: 0 }
    let score = 0
    const tech = t.technicals
    if (tech.priceVsSma20 === 'above') score++; else if (tech.priceVsSma20 === 'below') score--
    if (tech.priceVsSma50 === 'above') score++; else if (tech.priceVsSma50 === 'below') score--
    if (tech.priceVsSma200 === 'above') score++; else if (tech.priceVsSma200 === 'below') score--
    if (tech.rsi14 != null) {
      if (tech.rsi14 < 30) score++ // oversold = bullish signal
      else if (tech.rsi14 > 70) score-- // overbought = bearish signal
    }
    if (tech.smaCrossover === 'golden_cross') score += 2
    else if (tech.smaCrossover === 'death_cross') score -= 2

    // Trend direction weight
    if (t.sentiment === 'strong_up') score += 2
    else if (t.sentiment === 'up') score += 1
    else if (t.sentiment === 'down') score -= 1
    else if (t.sentiment === 'strong_down') score -= 2

    if (score >= 4) return { label: 'Strong Buy', signal: 'bullish', score }
    if (score >= 2) return { label: 'Buy', signal: 'bullish', score }
    if (score <= -4) return { label: 'Strong Sell', signal: 'bearish', score }
    if (score <= -2) return { label: 'Sell', signal: 'bearish', score }
    return { label: 'Neutral', signal: 'neutral', score }
  }

  return (
    <Card>
      <CardContent className="p-4">
        {/* Header */}
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Eye className="h-4 w-4 text-muted-foreground" />
            <h3 className="text-sm font-semibold">Watchlist</h3>
            {watchlist.length > 0 && (
              <span className="text-[10px] text-muted-foreground font-medium">
                ({watchlist.length})
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            {/* Range toggle */}
            {watchlist.length > 0 && (
              <div className="flex rounded-md border overflow-hidden">
                {rangeOptions.map((opt) => (
                  <button
                    key={opt.value}
                    onClick={() => setRange(opt.value)}
                    disabled={isLoadingTrends}
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
            )}
            {watchlist.length > 0 && (
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={() => {
                  const symbols = watchlist.map((w) => w.symbol)
                  fetchQuotes(symbols)
                  fetchTrends(symbols, range)
                }}
                disabled={isLoadingTrends}
                aria-label="Refresh"
              >
                {isLoadingTrends ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="h-3.5 w-3.5" />
                )}
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => setSearchOpen(!searchOpen)}
              aria-label="Add symbol"
            >
              {searchOpen ? <X className="h-3.5 w-3.5" /> : <Plus className="h-3.5 w-3.5" />}
            </Button>
          </div>
        </div>

        {/* Search */}
        {searchOpen && (
          <div className="mb-3 relative">
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
              <input
                ref={searchInputRef}
                type="text"
                value={searchQuery}
                onChange={(e) => handleSearch(e.target.value)}
                placeholder="Search by name or ticker..."
                className="w-full h-8 pl-8 pr-3 text-sm rounded-md border bg-background focus:outline-none focus:ring-1 focus:ring-primary"
              />
            </div>
            {(searchResults.length > 0 || isSearching) && (
              <div className="absolute z-10 left-0 right-0 mt-1 bg-popover border rounded-lg shadow-lg overflow-hidden">
                {isSearching ? (
                  <div className="px-3 py-2 text-xs text-muted-foreground">Searching...</div>
                ) : (
                  searchResults.map((r) => {
                    const alreadyAdded = watchlist.some((w) => w.symbol === r.symbol)
                    return (
                      <button
                        key={r.symbol}
                        disabled={alreadyAdded}
                        onClick={() => addSymbol(r.symbol)}
                        className={cn(
                          'w-full flex items-center justify-between px-3 py-2 text-left text-sm hover:bg-muted transition-colors',
                          alreadyAdded && 'opacity-50 cursor-not-allowed'
                        )}
                      >
                        <div className="min-w-0 flex-1">
                          <span className="font-medium">{r.symbol}</span>
                          <span className="text-muted-foreground ml-2 text-xs truncate">{r.name || ''}</span>
                        </div>
                        <span className="text-[10px] text-muted-foreground flex-shrink-0 ml-2">
                          {alreadyAdded ? 'Added' : r.exchangeDisplay || r.exchange || ''}
                        </span>
                      </button>
                    )
                  })
                )}
              </div>
            )}
          </div>
        )}

        {/* Main content area */}
        {isLoading ? (
          <div className="space-y-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-12 rounded-lg bg-muted/50 animate-pulse" />
            ))}
          </div>
        ) : watchlist.length === 0 ? (
          <div className="text-center py-6">
            <Eye className="h-8 w-8 mx-auto text-muted-foreground/40 mb-2" />
            <p className="text-sm text-muted-foreground">No symbols watched</p>
            <Button
              variant="ghost"
              size="sm"
              className="mt-2 text-xs h-7"
              onClick={() => setSearchOpen(true)}
            >
              <Plus className="h-3 w-3 mr-1" /> Add Symbol
            </Button>
          </div>
        ) : (
          <div className="flex gap-4">
            {/* LEFT: Compact watchlist table */}
            <div className={cn(
              'flex-shrink-0 transition-all duration-200',
              selectedSymbol ? 'w-[340px]' : 'w-full'
            )}>
              <ScrollArea className={cn(watchlist.length > 15 ? 'max-h-[600px] overflow-auto' : '')}>
                {/* Table header */}
                <div className={cn(
                  'grid gap-1 px-3 py-1.5 border-b text-[10px] text-muted-foreground font-semibold uppercase tracking-wider',
                  selectedSymbol
                    ? 'grid-cols-[minmax(0,1fr)_60px_52px]'
                    : 'grid-cols-[minmax(0,1fr)_72px_72px_72px_72px_72px]'
                )}>
                  <div>Symbol</div>
                  <div className="text-right">Price</div>
                  {selectedSymbol ? (
                    <div className="text-center">Trend</div>
                  ) : (
                    <>
                      <div className="text-right">3M</div>
                      <div className="text-right">6M</div>
                      <div className="text-right">12M</div>
                      <div className="text-center">Trend</div>
                    </>
                  )}
                </div>

                {/* Rows */}
                <div className="divide-y divide-border">
                  {sortedWatchlist.map((item) => {
                    const q = quotes[item.symbol]
                    const t = trends[item.symbol]
                    const change = q?.change ?? null
                    const changePct = q?.changePercent ?? null
                    const isUp = t ? t.dailyRate >= 0 : (change != null && change >= 0)
                    const isSelected = selectedSymbol === item.symbol
                    const config = t ? sentimentConfig[t.sentiment] : null

                    return (
                      <button
                        key={item.symbol}
                        onClick={() => setSelectedSymbol(isSelected ? null : item.symbol)}
                        className={cn(
                          'w-full grid gap-1 items-center px-3 py-2 text-left transition-colors',
                          selectedSymbol
                            ? 'grid-cols-[minmax(0,1fr)_60px_52px]'
                            : 'grid-cols-[minmax(0,1fr)_72px_72px_72px_72px_72px]',
                          isSelected
                            ? 'bg-primary/10 border-l-2 border-l-primary'
                            : 'hover:bg-muted/50 border-l-2 border-l-transparent'
                        )}
                      >
                        {/* Symbol + change */}
                        <div className="flex items-center gap-1.5 min-w-0">
                          {isUp ? (
                            <TrendingUp className="h-3 w-3 text-green-500 flex-shrink-0" />
                          ) : (
                            <TrendingDown className="h-3 w-3 text-red-500 flex-shrink-0" />
                          )}
                          <span className="text-sm font-semibold truncate">{item.symbol}</span>
                          {changePct != null && (
                            <span className={cn(
                              'text-[10px] font-medium tabular-nums flex-shrink-0',
                              change != null && change >= 0 ? 'text-green-500' : 'text-red-500'
                            )}>
                              {formatPct(changePct)}
                            </span>
                          )}
                        </div>

                        {/* Current price */}
                        <div className="text-right text-xs font-medium tabular-nums">
                          {formatPrice(q?.price)}
                        </div>

                        {/* When collapsed: show full projections; when detail open: just sentiment */}
                        {selectedSymbol ? (
                          <div className="flex justify-center">
                            {config ? (
                              <Badge
                                variant="outline"
                                className={cn('text-[9px] px-1 py-0 whitespace-nowrap', config.color, config.bg)}
                              >
                                {config.label}
                              </Badge>
                            ) : (
                              <span className="text-[10px] text-muted-foreground">--</span>
                            )}
                          </div>
                        ) : (
                          <>
                            {/* Projections */}
                            {t ? t.projections.map((p) => (
                              <div key={p.label} className="text-right">
                                <div className="text-xs font-medium tabular-nums">{formatPrice(p.price)}</div>
                                <div className={cn(
                                  'text-[10px] font-medium tabular-nums',
                                  p.changePct >= 0 ? 'text-green-500' : 'text-red-500'
                                )}>
                                  {p.changePct >= 0 ? '+' : ''}{p.changePct.toFixed(1)}%
                                </div>
                              </div>
                            )) : (
                              <>
                                <div className="text-right text-[10px] text-muted-foreground">--</div>
                                <div className="text-right text-[10px] text-muted-foreground">--</div>
                                <div className="text-right text-[10px] text-muted-foreground">--</div>
                              </>
                            )}
                            {/* Sentiment */}
                            <div className="flex justify-center">
                              {config ? (
                                <Badge
                                  variant="outline"
                                  className={cn('text-[9px] px-1 py-0 whitespace-nowrap', config.color, config.bg)}
                                >
                                  {config.label}
                                </Badge>
                              ) : (
                                <span className="text-[10px] text-muted-foreground">--</span>
                              )}
                            </div>
                          </>
                        )}
                      </button>
                    )
                  })}
                </div>

                {/* Disclaimer */}
                <p className="text-[10px] text-muted-foreground text-center py-2 border-t">
                  Linear trend based on {range === '6mo' ? '6-month' : range === '1y' ? '1-year' : '5-year'} data · Not financial advice
                </p>
              </ScrollArea>
            </div>

            {/* RIGHT: Analysis detail panel */}
            {selectedSymbol && (
              <div className="flex-1 min-w-0 border-l pl-4">
                  {/* Symbol header */}
                  <div className="flex items-start justify-between mb-3">
                    <div>
                      <div className="flex items-center gap-2">
                        <h3 className="text-lg font-bold">{selectedSymbol}</h3>
                        {selectedTrend && (() => {
                          const config = sentimentConfig[selectedTrend.sentiment]
                          return (
                            <Badge variant="outline" className={cn('text-[10px]', config.color, config.bg)}>
                              {config.label}
                            </Badge>
                          )
                        })()}
                      </div>
                      {selectedQuote?.name && (
                        <p className="text-xs text-muted-foreground mt-0.5">{selectedQuote.name}</p>
                      )}
                    </div>
                    <div className="text-right">
                      <div className="text-lg font-bold tabular-nums">{formatPrice(selectedQuote?.price)}</div>
                      {selectedQuote?.change != null && (
                        <div className={cn(
                          'text-xs font-medium tabular-nums',
                          selectedQuote.change >= 0 ? 'text-green-500' : 'text-red-500'
                        )}>
                          {selectedQuote.change >= 0 ? '+' : ''}
                          {formatPrice(selectedQuote.change)} ({formatPct(selectedQuote.changePercent)})
                        </div>
                      )}
                    </div>
                  </div>

                  {/* 52-Week Range */}
                  {selectedQuote && (
                    <div className="mb-4">
                      <FiftyTwoWeekBar
                        low={selectedQuote.fiftyTwoWeekLow}
                        high={selectedQuote.fiftyTwoWeekHigh}
                        current={selectedQuote.price}
                      />
                    </div>
                  )}

                  {/* Key Metrics */}
                  <div className="grid grid-cols-4 gap-2 mb-4">
                    {[
                      { label: 'Mkt Cap', value: formatMarketCap(selectedQuote?.marketCap) },
                      { label: 'P/E', value: selectedQuote?.peRatio != null ? selectedQuote.peRatio.toFixed(1) : '--' },
                      { label: 'Volume', value: formatVolume(selectedQuote?.volume) },
                      { label: 'Prev Close', value: formatPrice(selectedQuote?.previousClose) },
                    ].map((m) => (
                      <div key={m.label} className="bg-muted/50 rounded-md px-2.5 py-2 text-center">
                        <div className="text-[10px] text-muted-foreground font-medium">{m.label}</div>
                        <div className="text-xs font-semibold tabular-nums mt-0.5">{m.value}</div>
                      </div>
                    ))}
                  </div>

                  {/* Trend Projections */}
                  <div className="mb-4">
                    <div className="flex items-center gap-1.5 mb-2">
                      <BarChart3 className="h-3.5 w-3.5 text-muted-foreground" />
                      <span className="text-xs font-semibold">Price Projections</span>
                    </div>
                    <div className="space-y-2">
                      {selectedTrend ? selectedTrend.projections.map((p) => (
                        <ProjectionRow key={p.label} label={p.label} price={p.price} changePct={p.changePct} />
                      )) : (
                        <p className="text-xs text-muted-foreground">No trend data available</p>
                      )}
                    </div>
                    {selectedTrend && (
                      <div className="flex items-center gap-3 mt-2 text-[10px] text-muted-foreground">
                        <span>Daily rate: <span className={cn('font-medium', selectedTrend.dailyRate >= 0 ? 'text-green-500' : 'text-red-500')}>
                          {selectedTrend.dailyRate >= 0 ? '+' : ''}{formatPrice(selectedTrend.dailyRate)}/day
                        </span></span>
                        <span>R² Fit: <span className="font-medium text-foreground">{(selectedTrend.r2 * 100).toFixed(0)}%</span></span>
                      </div>
                    )}
                  </div>

                  {/* Technical Signals */}
                  {selectedTrend && (
                    <div className="mb-4">
                      <div className="flex items-center gap-1.5 mb-2">
                        <Activity className="h-3.5 w-3.5 text-muted-foreground" />
                        <span className="text-xs font-semibold">Technical Signals</span>
                      </div>
                      <div className="grid grid-cols-2 gap-1.5">
                        <SignalChip
                          label="Price vs SMA20"
                          signal={selectedTrend.technicals.priceVsSma20 === 'above' ? 'bullish' : selectedTrend.technicals.priceVsSma20 === 'below' ? 'bearish' : 'neutral'}
                          value={selectedTrend.technicals.sma20 != null ? formatPrice(selectedTrend.technicals.sma20) : undefined}
                        />
                        <SignalChip
                          label="Price vs SMA50"
                          signal={selectedTrend.technicals.priceVsSma50 === 'above' ? 'bullish' : selectedTrend.technicals.priceVsSma50 === 'below' ? 'bearish' : 'neutral'}
                          value={selectedTrend.technicals.sma50 != null ? formatPrice(selectedTrend.technicals.sma50) : undefined}
                        />
                        <SignalChip
                          label="Price vs SMA200"
                          signal={selectedTrend.technicals.priceVsSma200 === 'above' ? 'bullish' : selectedTrend.technicals.priceVsSma200 === 'below' ? 'bearish' : 'neutral'}
                          value={selectedTrend.technicals.sma200 != null ? formatPrice(selectedTrend.technicals.sma200) : undefined}
                        />
                        <SignalChip
                          label="RSI (14)"
                          signal={
                            selectedTrend.technicals.rsi14 != null
                              ? selectedTrend.technicals.rsi14 < 30
                                ? 'bullish'
                                : selectedTrend.technicals.rsi14 > 70
                                  ? 'bearish'
                                  : 'neutral'
                              : 'neutral'
                          }
                          value={selectedTrend.technicals.rsi14 != null ? selectedTrend.technicals.rsi14.toFixed(1) : '--'}
                        />
                        {selectedTrend.technicals.smaCrossover && (
                          <SignalChip
                            label="SMA Crossover"
                            signal={selectedTrend.technicals.smaCrossover === 'golden_cross' ? 'bullish' : 'bearish'}
                            value={selectedTrend.technicals.smaCrossover === 'golden_cross' ? 'Golden Cross' : 'Death Cross'}
                          />
                        )}
                      </div>

                      {/* Overall Assessment */}
                      {(() => {
                        const summary = getTechnicalSummary(selectedTrend)
                        const colors = {
                          bullish: 'bg-green-500/10 border-green-500/30 text-green-500',
                          bearish: 'bg-red-500/10 border-red-500/30 text-red-500',
                          neutral: 'bg-yellow-500/10 border-yellow-500/30 text-yellow-500',
                        }
                        return (
                          <div className={cn(
                            'mt-2 flex items-center justify-between px-3 py-2 rounded-md border',
                            colors[summary.signal]
                          )}>
                            <span className="text-xs font-semibold">Overall Assessment</span>
                            <span className="text-sm font-bold">{summary.label}</span>
                          </div>
                        )
                      })()}
                    </div>
                  )}

                  {/* Day Range */}
                  {selectedQuote && selectedQuote.low != null && selectedQuote.high != null && (
                    <div className="mb-4">
                      <div className="flex justify-between text-[10px] text-muted-foreground mb-1">
                        <span>Day Low {formatPrice(selectedQuote.low)}</span>
                        <span>Day High {formatPrice(selectedQuote.high)}</span>
                      </div>
                      <div className="h-1.5 rounded-full bg-muted overflow-hidden">
                        {(() => {
                          const range = selectedQuote.high! - selectedQuote.low!
                          const pct = range > 0 ? ((selectedQuote.price - selectedQuote.low!) / range) * 100 : 50
                          return (
                            <div
                              className="h-full rounded-full bg-primary/60"
                              style={{ width: `${Math.max(0, Math.min(100, pct))}%` }}
                            />
                          )
                        })()}
                      </div>
                    </div>
                  )}

                  {/* Portfolio: Positions, Holdings, Orders */}
                  {hasPortfolioData && (
                    <div className="mb-4">
                      {/* Active Positions */}
                      {symbolPositions.length > 0 && (
                        <div className="mb-3">
                          <div className="flex items-center gap-1.5 mb-1.5">
                            <Activity className="h-3.5 w-3.5 text-blue-500" />
                            <span className="text-xs font-semibold">Open Position</span>
                          </div>
                          {symbolPositions.map((pos, i) => (
                            <div key={i} className="bg-muted/50 rounded-md border px-3 py-2 space-y-1.5">
                              <div className="flex items-center justify-between">
                                <div className="flex items-center gap-2">
                                  <Badge variant="outline" className="text-[9px] py-0">
                                    {pos.product}
                                  </Badge>
                                  <span className={cn(
                                    'text-xs font-semibold',
                                    pos.quantity > 0 ? 'text-green-500' : pos.quantity < 0 ? 'text-red-500' : 'text-muted-foreground'
                                  )}>
                                    {pos.quantity > 0 ? 'LONG' : pos.quantity < 0 ? 'SHORT' : 'FLAT'} {Math.abs(pos.quantity)} shares
                                  </span>
                                </div>
                                <span className={cn(
                                  'text-xs font-bold tabular-nums',
                                  pos.pnl >= 0 ? 'text-green-500' : 'text-red-500'
                                )}>
                                  {pos.pnl >= 0 ? '+' : ''}{formatPrice(pos.pnl)}
                                </span>
                              </div>
                              <div className="grid grid-cols-3 gap-2 text-[10px]">
                                <div>
                                  <span className="text-muted-foreground">Avg Price</span>
                                  <div className="font-medium tabular-nums">{formatPrice(pos.average_price)}</div>
                                </div>
                                <div>
                                  <span className="text-muted-foreground">LTP</span>
                                  <div className="font-medium tabular-nums">{formatPrice(pos.ltp)}</div>
                                </div>
                                <div className="text-right">
                                  <span className="text-muted-foreground">P&L %</span>
                                  <div className={cn('font-medium tabular-nums', pos.pnlpercent >= 0 ? 'text-green-500' : 'text-red-500')}>
                                    {pos.pnlpercent >= 0 ? '+' : ''}{pos.pnlpercent.toFixed(2)}%
                                  </div>
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}

                      {/* Holdings */}
                      {symbolHoldings.length > 0 && (
                        <div className="mb-3">
                          <div className="flex items-center gap-1.5 mb-1.5">
                            <BarChart3 className="h-3.5 w-3.5 text-violet-500" />
                            <span className="text-xs font-semibold">Holdings</span>
                          </div>
                          {symbolHoldings.map((hold, i) => (
                            <div key={i} className="bg-muted/50 rounded-md border px-3 py-2 space-y-1.5">
                              <div className="flex items-center justify-between">
                                <span className="text-xs font-semibold">
                                  {hold.quantity} shares
                                </span>
                                <span className={cn(
                                  'text-xs font-bold tabular-nums',
                                  hold.pnl >= 0 ? 'text-green-500' : 'text-red-500'
                                )}>
                                  {hold.pnl >= 0 ? '+' : ''}{formatPrice(hold.pnl)} ({hold.pnlpercent >= 0 ? '+' : ''}{hold.pnlpercent.toFixed(2)}%)
                                </span>
                              </div>
                              <div className="grid grid-cols-2 gap-2 text-[10px]">
                                <div>
                                  <span className="text-muted-foreground">Avg Price</span>
                                  <div className="font-medium tabular-nums">{hold.average_price != null ? formatPrice(hold.average_price) : '--'}</div>
                                </div>
                                <div className="text-right">
                                  <span className="text-muted-foreground">LTP</span>
                                  <div className="font-medium tabular-nums">{hold.ltp != null ? formatPrice(hold.ltp) : '--'}</div>
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}

                      {/* Recent Orders */}
                      {symbolOrders.length > 0 && (
                        <div className="mb-3">
                          <div className="flex items-center gap-1.5 mb-1.5">
                            <ShoppingCart className="h-3.5 w-3.5 text-amber-500" />
                            <span className="text-xs font-semibold">Recent Orders</span>
                          </div>
                          <div className="space-y-1">
                            {symbolOrders.map((ord) => {
                              const statusColors: Record<string, string> = {
                                complete: 'text-green-500 bg-green-500/10 border-green-500/20',
                                open: 'text-blue-500 bg-blue-500/10 border-blue-500/20',
                                pending: 'text-yellow-500 bg-yellow-500/10 border-yellow-500/20',
                                'trigger pending': 'text-yellow-500 bg-yellow-500/10 border-yellow-500/20',
                                rejected: 'text-red-500 bg-red-500/10 border-red-500/20',
                                cancelled: 'text-muted-foreground bg-muted/50 border-border',
                              }
                              const colorClass = statusColors[ord.order_status] || statusColors.cancelled
                              return (
                                <div
                                  key={ord.orderid}
                                  className="flex items-center justify-between px-2.5 py-1.5 rounded-md bg-muted/30 border text-[11px]"
                                >
                                  <div className="flex items-center gap-2">
                                    <Badge
                                      variant="outline"
                                      className={cn('text-[9px] px-1 py-0', ord.action === 'BUY' ? 'text-green-500 border-green-500/30' : 'text-red-500 border-red-500/30')}
                                    >
                                      {ord.action}
                                    </Badge>
                                    <span className="font-medium tabular-nums">{ord.quantity} @ {formatPrice(ord.price || ord.trigger_price)}</span>
                                  </div>
                                  <div className="flex items-center gap-2">
                                    <Badge variant="outline" className={cn('text-[9px] px-1 py-0', colorClass)}>
                                      {ord.order_status}
                                    </Badge>
                                    {ord.timestamp && (
                                      <span className="text-[10px] text-muted-foreground tabular-nums">
                                        {(() => {
                                          try { return new Date(ord.timestamp).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' }) } catch { return '' }
                                        })()}
                                      </span>
                                    )}
                                  </div>
                                </div>
                              )
                            })}
                          </div>
                        </div>
                      )}
                    </div>
                  )}

                  {/* Actions */}
                  <div className="flex items-center gap-2 pt-2 border-t">
                    <PlaceOrderDialog
                      defaultSymbol={selectedSymbol}
                      trigger={
                        <Button variant="default" size="sm" className="h-8 text-xs gap-1.5 bg-green-600 hover:bg-green-700">
                          <ShoppingCart className="h-3.5 w-3.5" /> Trade
                        </Button>
                      }
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-8 text-xs gap-1.5"
                      onClick={() => navigate(`/fundamentals/${selectedSymbol}`)}
                    >
                      <ExternalLink className="h-3.5 w-3.5" /> Full Research
                    </Button>
                    <div className="flex-1" />
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-8 text-xs gap-1 text-destructive hover:text-destructive"
                      onClick={(e) => removeSymbol(selectedSymbol, e)}
                    >
                      <Trash2 className="h-3.5 w-3.5" /> Remove
                    </Button>
                  </div>
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
