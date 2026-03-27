import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { Loader2, Plus, Search, ShoppingCart, TrendingDown, TrendingUp } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { providerCommands } from '@/api/tauri-client'
import { tradingApi } from '@/api/trading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { cn } from '@/lib/utils'

interface SearchResult {
  symbol: string
  name: string
  exchange: string | null
  assetType: string | null
  exchangeDisplay: string | null
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

interface TrendProjection {
  dailyRate: number       // avg daily price change ($)
  dailyRatePct: number    // avg daily change (%)
  trendDirection: 'up' | 'down' | 'flat'
  projections: { label: string; days: number; price: number; changePct: number }[]
  r2: number              // R² goodness of fit (0-1)
  dataPoints: number
}

/** Simple linear regression on closing prices → projected future prices */
function computeTrendProjection(
  historicalData: [string, number, number, number, number, number][],
  currentPrice: number
): TrendProjection | null {
  // historicalData = [date, open, high, low, close, volume]
  if (historicalData.length < 20) return null

  const closes = historicalData.map((d) => d[4])
  const n = closes.length

  // Linear regression: price = slope * day + intercept
  // x = day index (0, 1, 2, ...), y = closing price
  let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0, sumY2 = 0
  for (let i = 0; i < n; i++) {
    sumX += i
    sumY += closes[i]
    sumXY += i * closes[i]
    sumX2 += i * i
    sumY2 += closes[i] * closes[i]
  }
  const slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX)
  const intercept = (sumY - slope * sumX) / n

  // R² calculation
  const meanY = sumY / n
  let ssTot = 0, ssRes = 0
  for (let i = 0; i < n; i++) {
    ssTot += (closes[i] - meanY) ** 2
    ssRes += (closes[i] - (slope * i + intercept)) ** 2
  }
  const r2 = ssTot > 0 ? 1 - ssRes / ssTot : 0

  const dailyRate = slope
  const dailyRatePct = currentPrice > 0 ? (slope / currentPrice) * 100 : 0
  const trendDirection = slope > 0.01 ? 'up' : slope < -0.01 ? 'down' : 'flat'

  const horizons = [
    { label: '1 Month', days: 21 },
    { label: '3 Months', days: 63 },
    { label: '6 Months', days: 126 },
  ]

  const projections = horizons.map((h) => {
    const projected = currentPrice + slope * h.days
    const changePct = currentPrice > 0 ? ((projected - currentPrice) / currentPrice) * 100 : 0
    return { ...h, price: Math.max(0, projected), changePct }
  })

  return { dailyRate, dailyRatePct, trendDirection, projections, r2, dataPoints: n }
}

interface PlaceOrderDialogProps {
  /** Optional pre-filled symbol */
  defaultSymbol?: string
  /** Custom trigger element. If omitted, renders a default button. */
  trigger?: React.ReactNode
  /** Called after a successful order placement */
  onOrderPlaced?: () => void
}

export function PlaceOrderDialog({ defaultSymbol, trigger, onOrderPlaced }: PlaceOrderDialogProps) {
  const [open, setOpen] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)

  // Symbol search
  const [symbolQuery, setSymbolQuery] = useState('')
  const [searchResults, setSearchResults] = useState<SearchResult[]>([])
  const [isSearching, setIsSearching] = useState(false)
  const [showResults, setShowResults] = useState(false)
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)

  // Selected symbol quote
  const [quote, setQuote] = useState<YahooQuote | null>(null)
  const [isLoadingQuote, setIsLoadingQuote] = useState(false)

  // Trend projection
  const [trend, setTrend] = useState<TrendProjection | null>(null)
  const [isLoadingTrend, setIsLoadingTrend] = useState(false)

  // Order form
  const [form, setForm] = useState({
    symbol: '',
    exchange: 'US',
    side: 'BUY' as 'BUY' | 'SELL',
    orderType: 'MARKET' as 'MARKET' | 'LIMIT' | 'SL' | 'SL-M',
    quantity: 1,
    price: 0,
    triggerPrice: 0,
    product: 'CNC',
    validity: 'DAY',
  })

  // Reset form when dialog opens
  useEffect(() => {
    if (open) {
      const sym = defaultSymbol || ''
      setForm({
        symbol: sym,
        exchange: 'US',
        side: 'BUY',
        orderType: 'MARKET',
        quantity: 1,
        price: 0,
        triggerPrice: 0,
        product: 'CNC',
        validity: 'DAY',
      })
      setSymbolQuery(sym)
      setSearchResults([])
      setShowResults(false)
      setQuote(null)
      setTrend(null)
      if (sym) {
        fetchQuote(sym)
        fetchTrend(sym)
      }
    }
  }, [open, defaultSymbol])

  const fetchQuote = async (symbol: string) => {
    setIsLoadingQuote(true)
    try {
      const data = await invoke<YahooQuote[]>('get_generic_quote', { symbols: [symbol] })
      if (data.length > 0) {
        setQuote(data[0])
        // Pre-fill price from current market price for limit orders
        setForm((prev) => ({
          ...prev,
          price: data[0].price,
        }))
      }
    } catch (err) {
      console.error('Failed to fetch quote:', err)
    } finally {
      setIsLoadingQuote(false)
    }
  }

  const fetchTrend = async (symbol: string) => {
    setIsLoadingTrend(true)
    setTrend(null)
    try {
      // Fetch 6 months of daily data
      const data = await providerCommands.getYahooHistorical(symbol, '1d', '6mo')
      if (data && data.length > 0) {
        // Get current price from the latest close or from quote
        const latestClose = data[data.length - 1][4]
        const projection = computeTrendProjection(data, latestClose)
        setTrend(projection)
      }
    } catch (err) {
      console.error('Failed to fetch trend data:', err)
    } finally {
      setIsLoadingTrend(false)
    }
  }

  const handleSymbolSearch = useCallback(async (query: string) => {
    setSymbolQuery(query)
    if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current)
    if (query.length < 1) {
      setSearchResults([])
      setShowResults(false)
      return
    }
    setShowResults(true)
    searchTimeoutRef.current = setTimeout(async () => {
      setIsSearching(true)
      try {
        const results = await invoke<SearchResult[]>('search_global_symbols', {
          query,
          limit: 6,
        })
        setSearchResults(results)
      } catch (err) {
        console.error('Symbol search failed:', err)
        setSearchResults([])
      } finally {
        setIsSearching(false)
      }
    }, 300)
  }, [])

  const selectSymbol = (result: SearchResult) => {
    setForm((prev) => ({
      ...prev,
      symbol: result.symbol,
      exchange: result.exchange || 'US',
    }))
    setSymbolQuery(result.symbol)
    setShowResults(false)
    setSearchResults([])
    fetchQuote(result.symbol)
    fetchTrend(result.symbol)
  }

  const handleSubmit = async () => {
    if (!form.symbol) {
      toast.error('Please select a symbol')
      return
    }
    if (form.quantity <= 0) {
      toast.error('Quantity must be greater than 0')
      return
    }
    if (form.orderType === 'LIMIT' && form.price <= 0) {
      toast.error('Price must be greater than 0 for limit orders')
      return
    }

    setIsSubmitting(true)
    try {
      const response = await tradingApi.placeOrder({
        symbol: form.symbol,
        exchange: form.exchange,
        action: form.side,
        product: form.product,
        pricetype: form.orderType,
        price: form.orderType === 'MARKET' ? 0 : form.price,
        quantity: form.quantity,
        trigger_price: form.orderType === 'SL' || form.orderType === 'SL-M' ? form.triggerPrice : undefined,
      })

      if (response.status === 'success') {
        toast.success(`Order placed: ${form.side} ${form.quantity} ${form.symbol}`, {
          description: `Order ID: ${response.data?.orderid}`,
        })
        // Emit event so OrderBook, Positions, and Holdings auto-refresh
        emit('order_event', {
          symbol: form.symbol,
          action: form.side,
          orderid: response.data?.orderid ?? '',
        })
        setOpen(false)
        onOrderPlaced?.()
      } else {
        toast.error(response.message || 'Failed to place order')
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to place order')
    } finally {
      setIsSubmitting(false)
    }
  }

  const formatPrice = (val: number | null | undefined) => {
    if (val == null) return '--'
    return val.toLocaleString('en-US', { style: 'currency', currency: 'USD' })
  }

  const estimatedCost = form.orderType === 'MARKET'
    ? (quote?.price ?? 0) * form.quantity
    : form.price * form.quantity

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        {trigger || (
          <Button size="sm">
            <Plus className="h-4 w-4 mr-2" />
            Place Order
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-[520px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ShoppingCart className="h-5 w-5" />
            Place Order
          </DialogTitle>
          <DialogDescription>
            Place a new buy or sell order through your connected broker.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* Symbol Search */}
          <div className="space-y-1.5">
            <Label>Symbol</Label>
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
              <input
                ref={searchInputRef}
                type="text"
                value={symbolQuery}
                onChange={(e) => handleSymbolSearch(e.target.value)}
                onFocus={() => {
                  if (searchResults.length > 0) setShowResults(true)
                }}
                placeholder="Search by name or ticker..."
                className="w-full h-10 pl-8 pr-3 text-sm rounded-md border bg-background focus:outline-none focus:ring-1 focus:ring-primary"
              />
              {form.symbol && (
                <Badge variant="secondary" className="absolute right-2 top-1/2 -translate-y-1/2 text-xs">
                  {form.symbol}
                </Badge>
              )}
            </div>
            {showResults && (searchResults.length > 0 || isSearching) && (
              <div className="relative z-10 mt-1 bg-popover border rounded-lg shadow-lg overflow-hidden">
                {isSearching ? (
                  <div className="px-3 py-2 text-xs text-muted-foreground">Searching...</div>
                ) : (
                  searchResults.map((r) => (
                    <button
                      key={r.symbol}
                      onClick={() => selectSymbol(r)}
                      className="w-full flex items-center justify-between px-3 py-2 text-left text-sm hover:bg-muted transition-colors"
                    >
                      <div className="min-w-0 flex-1">
                        <span className="font-medium">{r.symbol}</span>
                        <span className="text-muted-foreground ml-2 text-xs truncate">
                          {r.name || ''}
                        </span>
                      </div>
                      <span className="text-[10px] text-muted-foreground flex-shrink-0 ml-2">
                        {r.exchangeDisplay || r.exchange || ''}
                      </span>
                    </button>
                  ))
                )}
              </div>
            )}
          </div>

          {/* Quote Info */}
          {quote && (
            <div className="rounded-lg border bg-muted/50 p-3">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm font-semibold">{quote.symbol}</span>
                  {quote.name && (
                    <span className="text-xs text-muted-foreground ml-2">{quote.name}</span>
                  )}
                </div>
                {isLoadingQuote && <Loader2 className="h-4 w-4 animate-spin" />}
              </div>
              <div className="grid grid-cols-4 gap-3 mt-2 text-xs">
                <div>
                  <span className="text-muted-foreground">Price</span>
                  <div className="font-medium tabular-nums">{formatPrice(quote.price)}</div>
                </div>
                <div>
                  <span className="text-muted-foreground">Change</span>
                  <div
                    className={cn(
                      'font-medium tabular-nums',
                      quote.change != null && quote.change >= 0 ? 'text-green-500' : 'text-red-500'
                    )}
                  >
                    {quote.change != null
                      ? `${quote.change >= 0 ? '+' : ''}${quote.change.toFixed(2)}`
                      : '--'}
                  </div>
                </div>
                <div>
                  <span className="text-muted-foreground">High</span>
                  <div className="font-medium tabular-nums">{formatPrice(quote.high)}</div>
                </div>
                <div>
                  <span className="text-muted-foreground">Low</span>
                  <div className="font-medium tabular-nums">{formatPrice(quote.low)}</div>
                </div>
              </div>
            </div>
          )}

          {/* Trend Projection */}
          {(trend || isLoadingTrend) && (
            <div className="rounded-lg border bg-muted/50 p-3 space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  {trend?.trendDirection === 'up' ? (
                    <TrendingUp className="h-3.5 w-3.5 text-green-500" />
                  ) : trend?.trendDirection === 'down' ? (
                    <TrendingDown className="h-3.5 w-3.5 text-red-500" />
                  ) : null}
                  <span className="text-xs font-semibold">Trend Projection</span>
                </div>
                {isLoadingTrend && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
                {trend && (
                  <span className="text-[10px] text-muted-foreground">
                    R² {(trend.r2 * 100).toFixed(0)}% · {trend.dataPoints} days
                  </span>
                )}
              </div>
              {trend && (
                <>
                  <div className="grid grid-cols-3 gap-2">
                    {trend.projections.map((p) => (
                      <div key={p.label} className="text-center rounded-md bg-background/60 border px-2 py-1.5">
                        <div className="text-[10px] text-muted-foreground font-medium">{p.label}</div>
                        <div className="text-sm font-semibold tabular-nums">{formatPrice(p.price)}</div>
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
                  </div>
                  <p className="text-[10px] text-muted-foreground text-center leading-tight">
                    Based on 6-month linear trend · Speculative, not financial advice
                  </p>
                </>
              )}
            </div>
          )}

          {/* Side Toggle */}
          <div className="space-y-1.5">
            <Label>Side</Label>
            <div className="grid grid-cols-2 gap-2">
              <button
                onClick={() => setForm((prev) => ({ ...prev, side: 'BUY' }))}
                className={cn(
                  'h-10 rounded-md font-semibold text-sm transition-colors border',
                  form.side === 'BUY'
                    ? 'bg-green-500 text-white border-green-500'
                    : 'bg-background text-green-500 border-green-500/30 hover:bg-green-500/10'
                )}
              >
                BUY
              </button>
              <button
                onClick={() => setForm((prev) => ({ ...prev, side: 'SELL' }))}
                className={cn(
                  'h-10 rounded-md font-semibold text-sm transition-colors border',
                  form.side === 'SELL'
                    ? 'bg-red-500 text-white border-red-500'
                    : 'bg-background text-red-500 border-red-500/30 hover:bg-red-500/10'
                )}
              >
                SELL
              </button>
            </div>
          </div>

          {/* Order Type + Quantity Row */}
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label>Order Type</Label>
              <Select
                value={form.orderType}
                onValueChange={(val) =>
                  setForm((prev) => ({ ...prev, orderType: val as typeof form.orderType }))
                }
              >
                <SelectTrigger className="h-10">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="MARKET">Market</SelectItem>
                  <SelectItem value="LIMIT">Limit</SelectItem>
                  <SelectItem value="SL">Stop Loss</SelectItem>
                  <SelectItem value="SL-M">Stop Loss Market</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Quantity</Label>
              <Input
                type="number"
                min={1}
                step={1}
                value={form.quantity}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, quantity: parseInt(e.target.value) || 0 }))
                }
                className="h-10 tabular-nums"
              />
            </div>
          </div>

          {/* Price (conditional) */}
          {(form.orderType === 'LIMIT' || form.orderType === 'SL') && (
            <div className="space-y-1.5">
              <Label>Price</Label>
              <Input
                type="number"
                min={0}
                step={0.01}
                value={form.price}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, price: parseFloat(e.target.value) || 0 }))
                }
                className="h-10 tabular-nums"
              />
            </div>
          )}

          {/* Trigger Price (conditional) */}
          {(form.orderType === 'SL' || form.orderType === 'SL-M') && (
            <div className="space-y-1.5">
              <Label>Trigger Price</Label>
              <Input
                type="number"
                min={0}
                step={0.01}
                value={form.triggerPrice}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, triggerPrice: parseFloat(e.target.value) || 0 }))
                }
                className="h-10 tabular-nums"
              />
            </div>
          )}

          {/* Estimated Cost */}
          {form.symbol && form.quantity > 0 && (
            <div className="rounded-lg border p-3 flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Estimated {form.side === 'BUY' ? 'Cost' : 'Proceeds'}</span>
              <span className="text-sm font-semibold tabular-nums">
                {formatPrice(estimatedCost)}
              </span>
            </div>
          )}
        </div>

        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleSubmit}
            disabled={isSubmitting || !form.symbol || form.quantity <= 0}
            className={cn(
              form.side === 'BUY'
                ? 'bg-green-500 hover:bg-green-600'
                : 'bg-red-500 hover:bg-red-600',
              'text-white'
            )}
          >
            {isSubmitting ? (
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            ) : null}
            {form.side === 'BUY' ? 'Buy' : 'Sell'} {form.symbol || 'Symbol'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
