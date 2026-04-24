import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import {
  HelpCircle,
  Info,
  Loader2,
  Plus,
  Search,
  ShoppingCart,
  TrendingDown,
  TrendingUp,
} from 'lucide-react'
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
import { Switch } from '@/components/ui/switch'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import { useAuthStore } from '@/stores/authStore'

// ============================================================================
// Types
// ============================================================================

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
  dailyRate: number
  dailyRatePct: number
  trendDirection: 'up' | 'down' | 'flat'
  projections: { label: string; days: number; price: number; changePct: number }[]
  r2: number
  dataPoints: number
}

type OrderType = 'MARKET' | 'LIMIT' | 'SL' | 'SL-M' | 'TRAILING_STOP'
type Validity = 'DAY' | 'GTC' | 'IOC' | 'FOK'

// ============================================================================
// Broker Capabilities
// ============================================================================

const BROKER_CAPABILITIES: Record<
  string,
  { orderTypes: OrderType[]; validity: Validity[]; extendedHours: boolean }
> = {
  alpaca: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M', 'TRAILING_STOP'],
    validity: ['DAY', 'GTC', 'IOC', 'FOK'],
    extendedHours: true,
  },
  tradier: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M', 'TRAILING_STOP'],
    validity: ['DAY', 'GTC'],
    extendedHours: false,
  },
  schwab: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M', 'TRAILING_STOP'],
    validity: ['DAY', 'GTC'],
    extendedHours: true,
  },
  ibkr: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M', 'TRAILING_STOP'],
    validity: ['DAY', 'GTC', 'IOC'],
    extendedHours: true,
  },
  angel: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M'],
    validity: ['DAY'],
    extendedHours: true,
  },
  zerodha: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M'],
    validity: ['DAY', 'IOC'],
    extendedHours: true,
  },
  fyers: {
    orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M'],
    validity: ['DAY', 'IOC'],
    extendedHours: true,
  },
}

const DEFAULT_CAPABILITIES = {
  orderTypes: ['MARKET', 'LIMIT', 'SL', 'SL-M', 'TRAILING_STOP'] as OrderType[],
  validity: ['DAY', 'GTC', 'IOC', 'FOK'] as Validity[],
  extendedHours: true,
}

// ============================================================================
// Order Type Labels
// ============================================================================

const ORDER_TYPE_LABELS: Record<OrderType, string> = {
  MARKET: 'Market',
  LIMIT: 'Limit',
  SL: 'Stop Loss Limit',
  'SL-M': 'Stop Loss Market',
  TRAILING_STOP: 'Trailing Stop',
}

const VALIDITY_LABELS: Record<Validity, string> = {
  DAY: 'DAY',
  GTC: 'GTC',
  IOC: 'IOC',
  FOK: 'FOK',
}

// ============================================================================
// Glossary Content
// ============================================================================

const ORDER_GLOSSARY = {
  orderTypes: {
    MARKET: {
      label: 'Market Order',
      desc: 'Executes immediately at the best available price. You are guaranteed a fill but not a specific price. Best for liquid stocks where you want in or out now.',
    },
    LIMIT: {
      label: 'Limit Order',
      desc: 'Executes only at your specified price or better. Buy limits fill at or below your price; sell limits fill at or above. May not fill if the market never reaches your price.',
    },
    SL: {
      label: 'Stop Loss Limit',
      desc: 'A two-price order: when the market hits your trigger price, a limit order is placed at your limit price. Protects against losses but the limit order may not fill if the market moves too fast past your limit.',
    },
    'SL-M': {
      label: 'Stop Loss Market',
      desc: 'When the market hits your trigger price, a market order is placed. Guarantees execution once triggered but the fill price may differ from the trigger in fast-moving markets.',
    },
    TRAILING_STOP: {
      label: 'Trailing Stop',
      desc: 'A dynamic stop that follows the market price by a fixed dollar amount or percentage. As the price rises, the stop rises with it. If the price drops by the trail amount, a market order triggers. Locks in profits while letting winners run.',
    },
  },
  validity: {
    DAY: 'Expires at market close today if not filled.',
    GTC: 'Good Till Cancelled \u2014 stays active until filled or you cancel it (up to 90 days).',
    IOC: 'Immediate or Cancel \u2014 fills what it can right now, cancels the rest.',
    FOK: 'Fill or Kill \u2014 the entire quantity must fill immediately or the whole order is cancelled.',
  },
  parameters: {
    extendedHours:
      'Execute during pre-market (4\u20139:30 AM ET) and after-hours (4\u20138 PM ET). Only limit orders are allowed in extended hours.',
    trailAmount: 'The dollar amount the stop trails behind the highest price reached.',
    trailPercent: 'The percentage the stop trails behind the highest price reached.',
    triggerPrice: 'The price that activates a stop loss order. Once hit, your order becomes live.',
    limitPrice:
      'The maximum price you will pay (buy) or the minimum you will accept (sell). The order only fills at this price or better.',
  },
} as const

// ============================================================================
// Trend Projection Utility
// ============================================================================

function computeTrendProjection(
  historicalData: [string, number, number, number, number, number][],
  currentPrice: number
): TrendProjection | null {
  if (historicalData.length < 20) return null

  const closes = historicalData.map((d) => d[4])
  const n = closes.length

  let sumX = 0,
    sumY = 0,
    sumXY = 0,
    sumX2 = 0
  for (let i = 0; i < n; i++) {
    sumX += i
    sumY += closes[i]
    sumXY += i * closes[i]
    sumX2 += i * i
  }
  const slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX)
  const intercept = (sumY - slope * sumX) / n

  const meanY = sumY / n
  let ssTot = 0,
    ssRes = 0
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

// ============================================================================
// Inline Tooltip Helper
// ============================================================================

function InfoTip({ text }: { text: string }) {
  return (
    <TooltipProvider delayDuration={200}>
      <Tooltip>
        <TooltipTrigger asChild>
          <Info className="h-3 w-3 text-muted-foreground inline-block ml-1 cursor-help" />
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-[280px] text-xs">
          {text}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

// ============================================================================
// Glossary Dialog
// ============================================================================

function GlossaryDialog() {
  const [glossaryOpen, setGlossaryOpen] = useState(false)

  return (
    <Dialog open={glossaryOpen} onOpenChange={setGlossaryOpen}>
      <DialogTrigger asChild>
        <button className="text-muted-foreground hover:text-foreground transition-colors">
          <HelpCircle className="h-4 w-4" />
        </button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-[520px] h-[75vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>Order Types Glossary</DialogTitle>
          <DialogDescription>
            Learn about each order type and when to use it.
          </DialogDescription>
        </DialogHeader>
        <div className="flex-1 overflow-y-auto space-y-6 py-2">
          {/* Order Types */}
          <div className="space-y-3">
            <h3 className="text-sm font-semibold">Order Types</h3>
            {Object.entries(ORDER_GLOSSARY.orderTypes).map(([key, val]) => (
              <div key={key} className="rounded-lg border p-3 space-y-1">
                <div className="text-sm font-semibold">{val.label}</div>
                <div className="text-xs text-muted-foreground leading-relaxed">{val.desc}</div>
              </div>
            ))}
          </div>

          {/* Time in Force */}
          <div className="space-y-3">
            <h3 className="text-sm font-semibold">Time in Force (Validity)</h3>
            {Object.entries(ORDER_GLOSSARY.validity).map(([key, desc]) => (
              <div key={key} className="rounded-lg border p-3 space-y-1">
                <div className="text-sm font-semibold">{key}</div>
                <div className="text-xs text-muted-foreground leading-relaxed">{desc}</div>
              </div>
            ))}
          </div>

          {/* Parameters */}
          <div className="space-y-3">
            <h3 className="text-sm font-semibold">Parameters</h3>
            {Object.entries(ORDER_GLOSSARY.parameters).map(([key, desc]) => (
              <div key={key} className="rounded-lg border p-3 space-y-1">
                <div className="text-sm font-semibold capitalize">
                  {key.replace(/([A-Z])/g, ' $1').trim()}
                </div>
                <div className="text-xs text-muted-foreground leading-relaxed">{desc}</div>
              </div>
            ))}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

// ============================================================================
// PlaceOrderDialog
// ============================================================================

interface PlaceOrderDialogProps {
  defaultSymbol?: string
  defaultSide?: 'BUY' | 'SELL'
  defaultOrderType?: OrderType
  defaultQuantity?: number
  defaultPrice?: number
  defaultTriggerPrice?: number
  trigger?: React.ReactNode
  onOrderPlaced?: () => void
  /** Controlled mode: if provided, external state controls open/close */
  open?: boolean
  onOpenChange?: (open: boolean) => void
}

export function PlaceOrderDialog({
  defaultSymbol,
  defaultSide,
  defaultOrderType,
  defaultQuantity,
  defaultPrice,
  defaultTriggerPrice,
  trigger,
  onOrderPlaced,
  open: controlledOpen,
  onOpenChange: controlledOnOpenChange,
}: PlaceOrderDialogProps) {
  const [internalOpen, setInternalOpen] = useState(false)
  const isControlled = controlledOpen !== undefined
  const open = isControlled ? controlledOpen : internalOpen
  const setOpen = isControlled
    ? (val: boolean) => controlledOnOpenChange?.(val)
    : setInternalOpen

  const [isSubmitting, setIsSubmitting] = useState(false)

  // Broker capabilities
  const brokerId = useAuthStore((s) => s.user?.brokerId)
  const capabilities = brokerId
    ? BROKER_CAPABILITIES[brokerId] || DEFAULT_CAPABILITIES
    : DEFAULT_CAPABILITIES

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
    orderType: 'MARKET' as OrderType,
    quantity: 1,
    price: 0,
    triggerPrice: 0,
    product: 'CNC',
    validity: 'DAY' as Validity,
    extendedHours: false,
    trailPrice: 0,
    trailPercent: 0,
    trailMode: 'price' as 'price' | 'percent',
  })

  // Reset form when dialog opens
  useEffect(() => {
    if (open) {
      const sym = defaultSymbol || ''
      const initialOrderType = defaultOrderType && capabilities.orderTypes.includes(defaultOrderType)
        ? defaultOrderType
        : 'MARKET'

      setForm({
        symbol: sym,
        exchange: 'US',
        side: defaultSide || 'BUY',
        orderType: initialOrderType,
        quantity: defaultQuantity || 1,
        price: defaultPrice || 0,
        triggerPrice: defaultTriggerPrice || 0,
        product: 'CNC',
        validity: 'DAY',
        extendedHours: false,
        trailPrice: 0,
        trailPercent: 0,
        trailMode: 'price',
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
  }, [open, defaultSymbol, defaultSide, defaultOrderType, defaultQuantity, defaultPrice, defaultTriggerPrice])

  const fetchQuote = async (symbol: string) => {
    setIsLoadingQuote(true)
    try {
      const data = await invoke<YahooQuote[]>('get_generic_quote', { symbols: [symbol] })
      if (data.length > 0) {
        setQuote(data[0])
        setForm((prev) => ({
          ...prev,
          price: prev.price || data[0].price,
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
      const data = await providerCommands.getYahooHistorical(symbol, '1d', '6mo')
      if (data && data.length > 0) {
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
    if ((form.orderType === 'SL' || form.orderType === 'SL-M') && form.triggerPrice <= 0) {
      toast.error('Trigger price must be greater than 0 for stop loss orders')
      return
    }
    if (form.orderType === 'SL' && form.price <= 0) {
      toast.error('Limit price must be greater than 0 for stop loss limit orders')
      return
    }
    if (form.orderType === 'TRAILING_STOP') {
      if (form.trailMode === 'price' && form.trailPrice <= 0) {
        toast.error('Trail amount must be greater than 0')
        return
      }
      if (form.trailMode === 'percent' && (form.trailPercent <= 0 || form.trailPercent > 100)) {
        toast.error('Trail percent must be between 0 and 100')
        return
      }
    }

    setIsSubmitting(true)
    try {
      const response = await tradingApi.placeOrder({
        symbol: form.symbol,
        exchange: form.exchange,
        action: form.side,
        product: form.product,
        pricetype: form.orderType,
        price: form.orderType === 'MARKET' || form.orderType === 'TRAILING_STOP' ? 0 : form.price,
        quantity: form.quantity,
        trigger_price:
          form.orderType === 'SL' || form.orderType === 'SL-M' ? form.triggerPrice : undefined,
        validity: form.validity,
        amo: form.extendedHours,
        trail_price: form.orderType === 'TRAILING_STOP' && form.trailMode === 'price'
          ? form.trailPrice
          : undefined,
        trail_percent: form.orderType === 'TRAILING_STOP' && form.trailMode === 'percent'
          ? form.trailPercent
          : undefined,
      })

      if (response.status === 'success') {
        toast.success(`Order placed: ${form.side} ${form.quantity} ${form.symbol}`, {
          description: `Order ID: ${response.data?.orderid}`,
        })
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

  const estimatedCost =
    form.orderType === 'MARKET' || form.orderType === 'TRAILING_STOP'
      ? (quote?.price ?? 0) * form.quantity
      : form.price * form.quantity

  // Determine if price/trigger fields are needed
  const showLimitPrice = form.orderType === 'LIMIT' || form.orderType === 'SL'
  const showTriggerPrice = form.orderType === 'SL' || form.orderType === 'SL-M'
  const showTrailFields = form.orderType === 'TRAILING_STOP'

  // Build the dialog content — shared between controlled and uncontrolled modes
  const dialogContent = (
    <DialogContent className="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <ShoppingCart className="h-5 w-5" />
          Place Order
          <span className="ml-auto">
            <GlossaryDialog />
          </span>
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
              <Badge
                variant="secondary"
                className="absolute right-2 top-1/2 -translate-y-1/2 text-xs"
              >
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
                  R\u00B2 {(trend.r2 * 100).toFixed(0)}% \u00B7 {trend.dataPoints} days
                </span>
              )}
            </div>
            {trend && (
              <>
                <div className="grid grid-cols-3 gap-2">
                  {trend.projections.map((p) => (
                    <div
                      key={p.label}
                      className="text-center rounded-md bg-background/60 border px-2 py-1.5"
                    >
                      <div className="text-[10px] text-muted-foreground font-medium">
                        {p.label}
                      </div>
                      <div className="text-sm font-semibold tabular-nums">
                        {formatPrice(p.price)}
                      </div>
                      <div
                        className={cn(
                          'text-[10px] font-medium tabular-nums',
                          p.changePct >= 0 ? 'text-green-500' : 'text-red-500'
                        )}
                      >
                        {p.changePct >= 0 ? '+' : ''}
                        {p.changePct.toFixed(1)}%
                      </div>
                    </div>
                  ))}
                </div>
                <p className="text-[10px] text-muted-foreground text-center leading-tight">
                  Based on 6-month linear trend \u00B7 Speculative, not financial advice
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
            <Label>
              Order Type
              <InfoTip
                text={
                  ORDER_GLOSSARY.orderTypes[form.orderType]?.desc || 'Select an order type.'
                }
              />
            </Label>
            <Select
              value={form.orderType}
              onValueChange={(val) =>
                setForm((prev) => ({ ...prev, orderType: val as OrderType }))
              }
            >
              <SelectTrigger className="h-10">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {capabilities.orderTypes.map((ot) => (
                  <SelectItem key={ot} value={ot}>
                    {ORDER_TYPE_LABELS[ot]}
                  </SelectItem>
                ))}
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

        {/* Validity + Extended Hours Row */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <Label>
              Time in Force
              <InfoTip text={ORDER_GLOSSARY.validity[form.validity]} />
            </Label>
            <div className="flex gap-1">
              {capabilities.validity.map((v) => (
                <button
                  key={v}
                  onClick={() => setForm((prev) => ({ ...prev, validity: v }))}
                  className={cn(
                    'flex-1 h-10 rounded-md text-xs font-semibold transition-colors border',
                    form.validity === v
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-background text-muted-foreground border-border hover:bg-muted'
                  )}
                >
                  {VALIDITY_LABELS[v]}
                </button>
              ))}
            </div>
          </div>
          {capabilities.extendedHours && (
            <div className="space-y-1.5">
              <Label>
                Extended Hours
                <InfoTip text={ORDER_GLOSSARY.parameters.extendedHours} />
              </Label>
              <div className="flex items-center h-10 gap-2">
                <Switch
                  checked={form.extendedHours}
                  onCheckedChange={(checked) =>
                    setForm((prev) => ({ ...prev, extendedHours: checked }))
                  }
                />
                <span className="text-xs text-muted-foreground">
                  {form.extendedHours ? 'On' : 'Off'}
                </span>
              </div>
            </div>
          )}
        </div>

        {/* Price (conditional) */}
        {showLimitPrice && (
          <div className="space-y-1.5">
            <Label>
              Limit Price
              <InfoTip text={ORDER_GLOSSARY.parameters.limitPrice} />
            </Label>
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
        {showTriggerPrice && (
          <div className="space-y-1.5">
            <Label>
              Trigger Price
              <InfoTip text={ORDER_GLOSSARY.parameters.triggerPrice} />
            </Label>
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

        {/* Trailing Stop Fields (conditional) */}
        {showTrailFields && (
          <div className="space-y-1.5">
            <Label>
              Trail Amount
              <InfoTip
                text={
                  form.trailMode === 'price'
                    ? ORDER_GLOSSARY.parameters.trailAmount
                    : ORDER_GLOSSARY.parameters.trailPercent
                }
              />
            </Label>
            <div className="flex gap-2">
              {/* Mode toggle: $ or % */}
              <div className="flex border rounded-md overflow-hidden flex-shrink-0">
                <button
                  onClick={() => setForm((prev) => ({ ...prev, trailMode: 'price' }))}
                  className={cn(
                    'h-10 px-3 text-xs font-semibold transition-colors',
                    form.trailMode === 'price'
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-background text-muted-foreground hover:bg-muted'
                  )}
                >
                  $
                </button>
                <button
                  onClick={() => setForm((prev) => ({ ...prev, trailMode: 'percent' }))}
                  className={cn(
                    'h-10 px-3 text-xs font-semibold transition-colors',
                    form.trailMode === 'percent'
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-background text-muted-foreground hover:bg-muted'
                  )}
                >
                  %
                </button>
              </div>
              <Input
                type="number"
                min={0}
                step={form.trailMode === 'price' ? 0.01 : 0.1}
                value={form.trailMode === 'price' ? form.trailPrice : form.trailPercent}
                onChange={(e) => {
                  const val = parseFloat(e.target.value) || 0
                  if (form.trailMode === 'price') {
                    setForm((prev) => ({ ...prev, trailPrice: val }))
                  } else {
                    setForm((prev) => ({ ...prev, trailPercent: val }))
                  }
                }}
                placeholder={form.trailMode === 'price' ? 'Trail $' : 'Trail %'}
                className="h-10 tabular-nums flex-1"
              />
            </div>
          </div>
        )}

        {/* Estimated Cost */}
        {form.symbol && form.quantity > 0 && (
          <div className="rounded-lg border p-3 flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Estimated {form.side === 'BUY' ? 'Cost' : 'Proceeds'}
            </span>
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
          {isSubmitting ? <Loader2 className="h-4 w-4 mr-2 animate-spin" /> : null}
          {form.side === 'BUY' ? 'Buy' : 'Sell'} {form.symbol || 'Symbol'}
        </Button>
      </DialogFooter>
    </DialogContent>
  )

  // Controlled mode: no trigger, dialog open/close managed externally
  if (isControlled) {
    return (
      <Dialog open={open} onOpenChange={setOpen}>
        {dialogContent}
      </Dialog>
    )
  }

  // Uncontrolled mode: with trigger
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
      {dialogContent}
    </Dialog>
  )
}
