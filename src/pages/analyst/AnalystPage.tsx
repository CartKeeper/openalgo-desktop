import {
  AlertCircle,
  ArrowLeft,
  ArrowUpRight,
  ArrowDownRight,
  BarChart3,
  Loader2,
  Search,
  TrendingUp,
  Users,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import {
  providerCommands,
  type CompanyProfile,
  type GlobalSymbolSearchResult,
} from '@/api/tauri-client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

// ============================================================================
// Types
// ============================================================================

interface AnalystEstimate {
  symbol: string
  date: string
  estimated_revenue_avg: number | null
  estimated_revenue_high: number | null
  estimated_revenue_low: number | null
  estimated_eps_avg: number | null
  estimated_eps_high: number | null
  estimated_eps_low: number | null
  number_of_analysts: number | null
}

interface PriceTarget {
  symbol: string
  published_date: string
  analyst_name: string | null
  analyst_company: string | null
  price_target: number | null
  adj_price_target: number | null
  price_when_posted: number | null
  news_title: string | null
}

// ============================================================================
// Helpers
// ============================================================================

function formatCurrency(value: number | null | undefined, precision = 2): string {
  if (value == null || isNaN(value)) return '--'
  return `$${value.toLocaleString('en-US', {
    minimumFractionDigits: precision,
    maximumFractionDigits: precision,
  })}`
}

function formatLargeNumber(value: number | null | undefined): string {
  if (value == null || isNaN(value)) return '--'
  const abs = Math.abs(value)
  const sign = value < 0 ? '-' : ''
  if (abs >= 1e12) return `${sign}$${(abs / 1e12).toFixed(2)}T`
  if (abs >= 1e9) return `${sign}$${(abs / 1e9).toFixed(2)}B`
  if (abs >= 1e6) return `${sign}$${(abs / 1e6).toFixed(2)}M`
  if (abs >= 1e3) return `${sign}$${(abs / 1e3).toFixed(2)}K`
  return `${sign}$${abs.toFixed(2)}`
}

function formatPercent(value: number | null | undefined): string {
  if (value == null || isNaN(value)) return '--'
  const sign = value >= 0 ? '+' : ''
  return `${sign}${value.toFixed(2)}%`
}

function formatDate(dateStr: string | null | undefined): string {
  if (!dateStr) return '--'
  try {
    const d = new Date(dateStr)
    return d.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' })
  } catch {
    return dateStr
  }
}

// ============================================================================
// Sub-components
// ============================================================================

function PriceTargetBar({
  currentPrice,
  lowTarget,
  avgTarget,
  highTarget,
}: {
  currentPrice: number
  lowTarget: number
  avgTarget: number
  highTarget: number
}) {
  const rangeMin = Math.min(lowTarget, currentPrice) * 0.95
  const rangeMax = Math.max(highTarget, currentPrice) * 1.05
  const totalRange = rangeMax - rangeMin

  const toPercent = (val: number) => ((val - rangeMin) / totalRange) * 100

  const lowPct = toPercent(lowTarget)
  const highPct = toPercent(highTarget)
  const avgPct = toPercent(avgTarget)
  const currentPct = toPercent(currentPrice)

  return (
    <div className="relative w-full h-12 mt-4 mb-2">
      {/* Track background */}
      <div className="absolute top-1/2 -translate-y-1/2 left-0 right-0 h-2 bg-muted rounded-full" />

      {/* Colored range bar: low to high */}
      <div
        className="absolute top-1/2 -translate-y-1/2 h-2 bg-primary/20 rounded-full"
        style={{
          left: `${lowPct}%`,
          width: `${highPct - lowPct}%`,
        }}
      />

      {/* Low target marker */}
      <div
        className="absolute top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-red-500 border-2 border-background"
        style={{ left: `${lowPct}%`, transform: 'translate(-50%, -50%)' }}
      />
      <div
        className="absolute top-full text-[10px] font-semibold text-red-500 whitespace-nowrap"
        style={{ left: `${lowPct}%`, transform: 'translateX(-50%)' }}
      >
        Low {formatCurrency(lowTarget)}
      </div>

      {/* Average target marker */}
      <div
        className="absolute top-1/2 -translate-y-1/2 w-4 h-4 rounded-full bg-primary border-2 border-background z-10"
        style={{ left: `${avgPct}%`, transform: 'translate(-50%, -50%)' }}
      />
      <div
        className="absolute bottom-full text-[10px] font-semibold text-primary whitespace-nowrap mb-0.5"
        style={{ left: `${avgPct}%`, transform: 'translateX(-50%)' }}
      >
        Avg {formatCurrency(avgTarget)}
      </div>

      {/* High target marker */}
      <div
        className="absolute top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-emerald-500 border-2 border-background"
        style={{ left: `${highPct}%`, transform: 'translate(-50%, -50%)' }}
      />
      <div
        className="absolute top-full text-[10px] font-semibold text-emerald-500 whitespace-nowrap"
        style={{ left: `${highPct}%`, transform: 'translateX(-50%)' }}
      >
        High {formatCurrency(highTarget)}
      </div>

      {/* Current price line */}
      <div
        className="absolute top-0 bottom-0 w-0.5 bg-foreground z-20"
        style={{ left: `${currentPct}%`, transform: 'translateX(-50%)' }}
      />
      <div
        className="absolute bottom-full text-[10px] font-semibold text-foreground whitespace-nowrap mb-0.5"
        style={{ left: `${currentPct}%`, transform: 'translateX(-50%)' }}
      >
        Current {formatCurrency(currentPrice)}
      </div>
    </div>
  )
}

function LoadingSkeleton() {
  return (
    <div className="space-y-6">
      {/* Price target card skeleton */}
      <Card>
        <CardHeader>
          <Skeleton className="h-5 w-48" />
        </CardHeader>
        <CardContent className="space-y-4">
          <Skeleton className="h-10 w-32" />
          <Skeleton className="h-12 w-full" />
          <div className="flex gap-4">
            <Skeleton className="h-6 w-24" />
            <Skeleton className="h-6 w-24" />
            <Skeleton className="h-6 w-24" />
          </div>
        </CardContent>
      </Card>

      {/* Earnings table skeleton */}
      <Card>
        <CardHeader>
          <Skeleton className="h-5 w-40" />
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-10 w-full" />
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Price targets table skeleton */}
      <Card>
        <CardHeader>
          <Skeleton className="h-5 w-44" />
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-10 w-full" />
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

// ============================================================================
// Main Component
// ============================================================================

export default function AnalystPage() {
  // --- Search state ---
  const [query, setQuery] = useState('')
  const [searchResults, setSearchResults] = useState<GlobalSymbolSearchResult[]>([])
  const [showDropdown, setShowDropdown] = useState(false)
  const [isSearching, setIsSearching] = useState(false)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  // --- Data state ---
  const [selectedSymbol, setSelectedSymbol] = useState<string | null>(null)
  const [companyProfile, setCompanyProfile] = useState<CompanyProfile | null>(null)
  const [estimates, setEstimates] = useState<AnalystEstimate[]>([])
  const [priceTargets, setPriceTargets] = useState<PriceTarget[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [estimatePeriod, setEstimatePeriod] = useState<'quarterly' | 'annual'>('quarterly')

  // --- Close dropdown on outside click ---
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowDropdown(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  // --- Debounced search ---
  const handleSearchChange = useCallback((value: string) => {
    setQuery(value)
    if (debounceRef.current) clearTimeout(debounceRef.current)

    if (value.trim().length < 1) {
      setSearchResults([])
      setShowDropdown(false)
      return
    }

    debounceRef.current = setTimeout(async () => {
      setIsSearching(true)
      try {
        const results = await providerCommands.searchGlobalSymbols(value.trim(), 8)
        setSearchResults(results as GlobalSymbolSearchResult[])
        setShowDropdown(true)
      } catch {
        setSearchResults([])
      } finally {
        setIsSearching(false)
      }
    }, 300)
  }, [])

  // --- Select symbol and fetch data ---
  const handleSelectSymbol = useCallback(async (symbol: string) => {
    setQuery(symbol)
    setShowDropdown(false)
    setSelectedSymbol(symbol)
    setIsLoading(true)
    setError(null)
    setEstimates([])
    setPriceTargets([])
    setCompanyProfile(null)

    try {
      const [profileResult, estimatesResult, targetsResult] = await Promise.allSettled([
        providerCommands.getCompanyProfile(symbol),
        providerCommands.getAnalystEstimates(symbol, 12),
        providerCommands.getPriceTargets(symbol),
      ])

      if (profileResult.status === 'fulfilled') {
        setCompanyProfile(profileResult.value as CompanyProfile | null)
      }
      if (estimatesResult.status === 'fulfilled') {
        setEstimates((estimatesResult.value as AnalystEstimate[]) || [])
      }
      if (targetsResult.status === 'fulfilled') {
        setPriceTargets((targetsResult.value as PriceTarget[]) || [])
      }

      // Check if all failed (likely API key issue)
      if (
        profileResult.status === 'rejected' &&
        estimatesResult.status === 'rejected' &&
        targetsResult.status === 'rejected'
      ) {
        const msg = String(profileResult.reason || '')
        if (
          msg.toLowerCase().includes('api key') ||
          msg.toLowerCase().includes('api_key') ||
          msg.toLowerCase().includes('unauthorized') ||
          msg.toLowerCase().includes('forbidden')
        ) {
          setError('api_key')
        } else {
          setError(msg || 'Failed to fetch analyst data')
        }
      }
    } catch (err) {
      const msg = String(err)
      if (
        msg.toLowerCase().includes('api key') ||
        msg.toLowerCase().includes('unauthorized')
      ) {
        setError('api_key')
      } else {
        setError(msg || 'An unexpected error occurred')
        toast.error('Failed to fetch analyst coverage data')
      }
    } finally {
      setIsLoading(false)
    }
  }, [])

  // --- Derived data ---
  const currentPrice = companyProfile?.price ?? null

  // Price target consensus
  const validTargets = priceTargets.filter(
    (t) => t.price_target != null && t.price_target > 0
  )
  const targetPrices = validTargets.map((t) => t.price_target as number)
  const lowTarget = targetPrices.length > 0 ? Math.min(...targetPrices) : null
  const highTarget = targetPrices.length > 0 ? Math.max(...targetPrices) : null
  const avgTarget =
    targetPrices.length > 0
      ? targetPrices.reduce((sum, p) => sum + p, 0) / targetPrices.length
      : null
  const analystCount = validTargets.length

  const upsidePercent =
    currentPrice != null && currentPrice > 0 && avgTarget != null
      ? ((avgTarget - currentPrice) / currentPrice) * 100
      : null

  // Filter estimates by period
  const filteredEstimates = estimates.filter((e) => {
    if (!e.date) return false
    // Quarterly dates are typically month-specific (e.g., 2024-03-31, 2024-06-30)
    // Annual dates tend to be year-end (e.g., 2024-12-31)
    // FMP returns both mixed; we separate by checking if the date falls on common fiscal year-end months
    // A simple heuristic: annual = December date or year-only
    const month = new Date(e.date).getMonth() // 0-indexed
    if (estimatePeriod === 'annual') {
      // Show entries that are likely annual (Dec year-end or any entry with full-year data)
      return month === 11 // December
    }
    // Quarterly: show non-December entries
    return month !== 11
  })

  const displayEstimates =
    estimatePeriod === 'quarterly'
      ? filteredEstimates.slice(0, 4)
      : filteredEstimates.slice(0, 2)

  // Sort price targets by most recent
  const sortedPriceTargets = [...priceTargets].sort((a, b) => {
    const da = new Date(a.published_date || 0).getTime()
    const db = new Date(b.published_date || 0).getTime()
    return db - da
  })

  const hasData = estimates.length > 0 || priceTargets.length > 0

  // ============================================================================
  // Render
  // ============================================================================

  return (
    <div className="py-6 max-w-5xl mx-auto space-y-6 px-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <BarChart3 className="h-6 w-6" />
            Analyst Coverage
          </h1>
        </div>
        <p className="text-muted-foreground text-sm">
          View price targets, earnings estimates, and analyst consensus
        </p>
      </div>

      {/* Symbol Search */}
      <div className="relative" ref={dropdownRef}>
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => handleSearchChange(e.target.value)}
            onFocus={() => {
              if (searchResults.length > 0) setShowDropdown(true)
            }}
            placeholder="Search for a symbol (e.g., AAPL, MSFT, GOOGL)"
            className="pl-9 h-10 text-base"
          />
          {isSearching && (
            <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 animate-spin text-muted-foreground" />
          )}
        </div>

        {/* Autocomplete dropdown */}
        {showDropdown && searchResults.length > 0 && (
          <div className="absolute z-50 top-full mt-1 w-full rounded-lg border bg-popover shadow-lg overflow-hidden">
            {searchResults.map((result) => (
              <button
                key={`${result.symbol}-${result.exchange}`}
                type="button"
                className="w-full flex items-center gap-3 px-4 py-2.5 text-sm hover:bg-accent transition-colors text-left cursor-pointer"
                onClick={() => handleSelectSymbol(result.symbol)}
              >
                <span className="font-semibold tabular-nums min-w-[64px]">
                  {result.symbol}
                </span>
                <span className="text-muted-foreground truncate flex-1">{result.name}</span>
                {result.exchange && (
                  <Badge variant="secondary" className="text-[10px] shrink-0">
                    {result.exchange}
                  </Badge>
                )}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* No symbol selected state */}
      {!selectedSymbol && !isLoading && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <Search className="h-12 w-12 text-muted-foreground/40 mb-4" />
          <p className="text-muted-foreground text-sm">
            Search for a symbol to view analyst coverage
          </p>
        </div>
      )}

      {/* Loading state */}
      {isLoading && <LoadingSkeleton />}

      {/* Error: API key */}
      {error === 'api_key' && !isLoading && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <AlertCircle className="h-12 w-12 text-destructive/60 mb-4" />
            <p className="text-sm font-semibold mb-1">FMP API Key Required</p>
            <p className="text-muted-foreground text-sm mb-4">
              Analyst coverage data requires a Financial Modeling Prep API key.
            </p>
            <Button asChild variant="outline" size="sm">
              <Link to="/generic-setup">Configure FMP API Key</Link>
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Error: generic */}
      {error && error !== 'api_key' && !isLoading && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <AlertCircle className="h-12 w-12 text-destructive/60 mb-4" />
            <p className="text-sm font-semibold mb-1">Something went wrong</p>
            <p className="text-muted-foreground text-sm mb-4">{error}</p>
            {selectedSymbol && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => handleSelectSymbol(selectedSymbol)}
              >
                Retry
              </Button>
            )}
          </CardContent>
        </Card>
      )}

      {/* Empty state */}
      {selectedSymbol && !isLoading && !error && !hasData && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Users className="h-12 w-12 text-muted-foreground/40 mb-4" />
            <p className="text-sm font-semibold mb-1">No analyst coverage found</p>
            <p className="text-muted-foreground text-sm">
              No analyst coverage data is available for{' '}
              <span className="font-semibold">{selectedSymbol}</span>.
            </p>
          </CardContent>
        </Card>
      )}

      {/* Data sections */}
      {selectedSymbol && !isLoading && !error && hasData && (
        <div className="space-y-6">
          {/* 1. Price Target Consensus */}
          {validTargets.length > 0 && currentPrice != null && (
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="flex items-center gap-2 text-base">
                    <TrendingUp className="h-4 w-4" />
                    Price Target Consensus
                  </CardTitle>
                  <Badge variant="secondary" className="text-[10px]">
                    {analystCount} Analyst{analystCount !== 1 ? 's' : ''}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                {/* Current price hero */}
                <div className="flex items-baseline gap-3">
                  <span className="text-3xl font-bold tabular-nums">
                    {formatCurrency(currentPrice)}
                  </span>
                  {upsidePercent != null && (
                    <span
                      className={`flex items-center gap-1 text-sm font-semibold ${
                        upsidePercent >= 0 ? 'text-emerald-500' : 'text-red-500'
                      }`}
                    >
                      {upsidePercent >= 0 ? (
                        <ArrowUpRight className="h-4 w-4" />
                      ) : (
                        <ArrowDownRight className="h-4 w-4" />
                      )}
                      {formatPercent(upsidePercent)} to avg target
                    </span>
                  )}
                </div>

                {/* Visual bar */}
                {lowTarget != null && highTarget != null && avgTarget != null && (
                  <div className="px-2 pt-6 pb-6">
                    <PriceTargetBar
                      currentPrice={currentPrice}
                      lowTarget={lowTarget}
                      avgTarget={avgTarget}
                      highTarget={highTarget}
                    />
                  </div>
                )}

                {/* Summary stats row */}
                <div className="grid grid-cols-3 gap-4 pt-2 border-t">
                  <div className="text-center">
                    <p className="text-[12px] font-semibold text-muted-foreground uppercase tracking-wider">
                      Low
                    </p>
                    <p className="text-base font-bold tabular-nums text-red-500">
                      {formatCurrency(lowTarget)}
                    </p>
                  </div>
                  <div className="text-center">
                    <p className="text-[12px] font-semibold text-muted-foreground uppercase tracking-wider">
                      Average
                    </p>
                    <p className="text-base font-bold tabular-nums text-primary">
                      {formatCurrency(avgTarget)}
                    </p>
                  </div>
                  <div className="text-center">
                    <p className="text-[12px] font-semibold text-muted-foreground uppercase tracking-wider">
                      High
                    </p>
                    <p className="text-base font-bold tabular-nums text-emerald-500">
                      {formatCurrency(highTarget)}
                    </p>
                  </div>
                </div>
              </CardContent>
            </Card>
          )}

          {/* 2. Earnings Estimates */}
          {estimates.length > 0 && (
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="text-base">Earnings Estimates</CardTitle>
                  <Tabs
                    value={estimatePeriod}
                    onValueChange={(v) => setEstimatePeriod(v as 'quarterly' | 'annual')}
                  >
                    <TabsList className="h-8">
                      <TabsTrigger value="quarterly" className="text-xs px-3">
                        Quarterly
                      </TabsTrigger>
                      <TabsTrigger value="annual" className="text-xs px-3">
                        Annual
                      </TabsTrigger>
                    </TabsList>
                  </Tabs>
                </div>
              </CardHeader>
              <CardContent>
                {displayEstimates.length > 0 ? (
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Date</TableHead>
                        <TableHead className="text-right">Revenue Avg</TableHead>
                        <TableHead className="text-right">Revenue High</TableHead>
                        <TableHead className="text-right">Revenue Low</TableHead>
                        <TableHead className="text-right">EPS Avg</TableHead>
                        <TableHead className="text-right">EPS High</TableHead>
                        <TableHead className="text-right">EPS Low</TableHead>
                        <TableHead className="text-right"># Analysts</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {displayEstimates.map((est) => (
                        <TableRow key={est.date}>
                          <TableCell className="font-semibold">
                            {formatDate(est.date)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {formatLargeNumber(est.estimated_revenue_avg)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {formatLargeNumber(est.estimated_revenue_high)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {formatLargeNumber(est.estimated_revenue_low)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {formatCurrency(est.estimated_eps_avg)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {formatCurrency(est.estimated_eps_high)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {formatCurrency(est.estimated_eps_low)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {est.number_of_analysts ?? '--'}
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                ) : (
                  <p className="text-sm text-muted-foreground text-center py-6">
                    No {estimatePeriod} estimates available. Try switching the period.
                  </p>
                )}
              </CardContent>
            </Card>
          )}

          {/* 3. Individual Price Targets */}
          {priceTargets.length > 0 && (
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="text-base">Individual Price Targets</CardTitle>
                  <Badge variant="secondary" className="text-[10px]">
                    {priceTargets.length} Rating{priceTargets.length !== 1 ? 's' : ''}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Date</TableHead>
                      <TableHead>Analyst / Firm</TableHead>
                      <TableHead className="text-right">Price Target</TableHead>
                      <TableHead className="text-right">Price When Posted</TableHead>
                      <TableHead className="text-right">Implied Change</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {sortedPriceTargets.map((target, idx) => {
                      const impliedChange =
                        currentPrice != null &&
                        currentPrice > 0 &&
                        target.price_target != null
                          ? ((target.price_target - currentPrice) / currentPrice) * 100
                          : null
                      const isPositive = impliedChange != null && impliedChange >= 0

                      return (
                        <TableRow key={`${target.published_date}-${target.analyst_name}-${idx}`}>
                          <TableCell className="tabular-nums">
                            {formatDate(target.published_date)}
                          </TableCell>
                          <TableCell>
                            <div className="flex flex-col">
                              <span className="font-semibold text-sm truncate max-w-[200px]">
                                {target.analyst_name || 'Unknown'}
                              </span>
                              {target.analyst_company && (
                                <span className="text-[12px] text-muted-foreground truncate max-w-[200px]">
                                  {target.analyst_company}
                                </span>
                              )}
                            </div>
                          </TableCell>
                          <TableCell className="text-right tabular-nums font-semibold">
                            {formatCurrency(target.price_target)}
                          </TableCell>
                          <TableCell className="text-right tabular-nums text-muted-foreground">
                            {formatCurrency(target.price_when_posted)}
                          </TableCell>
                          <TableCell className="text-right">
                            {impliedChange != null ? (
                              <span
                                className={`inline-flex items-center gap-1 text-sm font-semibold tabular-nums ${
                                  isPositive ? 'text-emerald-500' : 'text-red-500'
                                }`}
                              >
                                {isPositive ? (
                                  <ArrowUpRight className="h-3.5 w-3.5" />
                                ) : (
                                  <ArrowDownRight className="h-3.5 w-3.5" />
                                )}
                                {formatPercent(impliedChange)}
                              </span>
                            ) : (
                              <span className="text-muted-foreground">--</span>
                            )}
                          </TableCell>
                        </TableRow>
                      )
                    })}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          )}
        </div>
      )}
    </div>
  )
}
