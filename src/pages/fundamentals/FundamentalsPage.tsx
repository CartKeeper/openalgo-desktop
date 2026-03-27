import {
  ArrowLeft,
  Building2,
  DollarSign,
  ExternalLink,
  Globe,
  Loader2,
  Search,
  ShoppingCart,
  TrendingUp,
  Users,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import {
  type CompanyProfile,
  type GlobalSymbolSearchResult,
  providerCommands,
} from '@/api/tauri-client'
import { PlaceOrderDialog } from '@/components/trading/PlaceOrderDialog'
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

// ---------------------------------------------------------------------------
// Types (inline, matching FMP API response shapes)
// ---------------------------------------------------------------------------

interface IncomeStatementRow {
  date: string
  revenue: number | null
  costOfRevenue: number | null
  grossProfit: number | null
  operatingIncome: number | null
  netIncome: number | null
  eps: number | null
  epsdiluted: number | null
  ebitda: number | null
}

interface BalanceSheetRow {
  date: string
  totalAssets: number | null
  totalCurrentAssets: number | null
  cashAndCashEquivalents: number | null
  totalLiabilities: number | null
  totalCurrentLiabilities: number | null
  longTermDebt: number | null
  totalStockholdersEquity: number | null
}

interface CashFlowRow {
  date: string
  operatingCashFlow: number | null
  netCashUsedForInvestingActivites: number | null
  netCashUsedProvidedByFinancingActivities: number | null
  freeCashFlow: number | null
  capitalExpenditure: number | null
  dividendsPaid: number | null
}

interface KeyMetricsRow {
  date: string
  peRatio: number | null
  pbRatio: number | null
  priceToSalesRatio: number | null
  enterpriseValueOverEBITDA: number | null
  roe: number | null
  returnOnTangibleAssets: number | null
  grossProfitMargin: number | null
  operatingProfitMargin: number | null
  netProfitMargin: number | null
  debtToEquity: number | null
  currentRatio: number | null
  revenuePerShare: number | null
  bookValuePerShare: number | null
  freeCashFlowPerShare: number | null
  dividendYield: number | null
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatLargeNumber(value: number | null | undefined): string {
  if (value == null) return '--'
  const abs = Math.abs(value)
  const sign = value < 0 ? '-' : ''
  if (abs >= 1_000_000_000_000) return `${sign}$${(abs / 1_000_000_000_000).toFixed(2)}T`
  if (abs >= 1_000_000_000) return `${sign}$${(abs / 1_000_000_000).toFixed(2)}B`
  if (abs >= 1_000_000) return `${sign}$${(abs / 1_000_000).toFixed(2)}M`
  if (abs >= 1_000) return `${sign}$${(abs / 1_000).toFixed(2)}K`
  return `${sign}$${abs.toFixed(2)}`
}

function formatNumber(value: number | null | undefined, decimals = 2): string {
  if (value == null) return '--'
  return value.toFixed(decimals)
}

function formatPercent(value: number | null | undefined): string {
  if (value == null) return '--'
  return `${(value * 100).toFixed(2)}%`
}

function formatEmployees(value: number | null | undefined): string {
  if (value == null) return '--'
  return value.toLocaleString()
}

// FMP returns camelCase keys. We cast the unknown[] to Record<string, unknown>
// and pluck fields safely.
function pick(obj: Record<string, unknown>, key: string): number | null {
  const v = obj[key]
  if (v == null || typeof v !== 'number') return null
  return v
}

function pickStr(obj: Record<string, unknown>, key: string): string {
  const v = obj[key]
  if (v == null) return ''
  return String(v)
}

import { AnalysisTab } from './AnalysisTab'

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <Card>
      <CardContent className="p-4">
        <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          {label}
        </p>
        <p className="text-xl font-bold mt-1 font-mono tabular-nums">{value}</p>
      </CardContent>
    </Card>
  )
}

function StatCardSkeleton() {
  return (
    <Card>
      <CardContent className="p-4 space-y-2">
        <Skeleton className="h-3 w-20" />
        <Skeleton className="h-6 w-28" />
      </CardContent>
    </Card>
  )
}

function TableSkeleton({ rows = 5, cols = 7 }: { rows?: number; cols?: number }) {
  return (
    <div className="space-y-2 mt-4">
      <div className="flex gap-2">
        {Array.from({ length: cols }).map((_, i) => (
          <Skeleton key={i} className="h-4 flex-1" />
        ))}
      </div>
      {Array.from({ length: rows }).map((_, r) => (
        <div key={r} className="flex gap-2">
          {Array.from({ length: cols }).map((_, c) => (
            <Skeleton key={c} className="h-5 flex-1" />
          ))}
        </div>
      ))}
    </div>
  )
}

function PeriodToggle({
  period,
  onChange,
}: {
  period: 'annual' | 'quarter'
  onChange: (p: 'annual' | 'quarter') => void
}) {
  return (
    <div className="flex gap-2 mb-4">
      <Button
        size="sm"
        variant={period === 'annual' ? 'default' : 'outline'}
        onClick={() => onChange('annual')}
        className="h-8"
      >
        Annual
      </Button>
      <Button
        size="sm"
        variant={period === 'quarter' ? 'default' : 'outline'}
        onClick={() => onChange('quarter')}
        className="h-8"
      >
        Quarterly
      </Button>
    </div>
  )
}

function EmptySymbol() {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center">
      <Search className="h-12 w-12 text-muted-foreground/40 mb-4" />
      <h3 className="text-lg font-semibold text-muted-foreground">No symbol selected</h3>
      <p className="text-sm text-muted-foreground mt-1">
        Search for a stock symbol above to view its fundamentals.
      </p>
    </div>
  )
}

function FmpNotConfigured() {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center">
      <Building2 className="h-12 w-12 text-muted-foreground/40 mb-4" />
      <h3 className="text-lg font-semibold text-muted-foreground">FMP API Key Required</h3>
      <p className="text-sm text-muted-foreground mt-1 max-w-md">
        Fundamentals data is powered by Financial Modeling Prep. Configure your API key in{' '}
        <Link to="/dashboard" className="text-primary underline underline-offset-2">
          Settings
        </Link>{' '}
        to get started.
      </p>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main Page
// ---------------------------------------------------------------------------

export default function FundamentalsPage() {
  const { symbol: urlSymbol } = useParams<{ symbol: string }>()

  // ------ State ------
  const [fmpConfigured, setFmpConfigured] = useState<boolean | null>(null)
  const [query, setQuery] = useState('')
  const [suggestions, setSuggestions] = useState<GlobalSymbolSearchResult[]>([])
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [selectedSymbol, setSelectedSymbol] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState('overview')

  // Data
  const [profile, setProfile] = useState<CompanyProfile | null>(null)
  const [keyMetricsLatest, setKeyMetricsLatest] = useState<Record<string, unknown> | null>(null)
  const [incomeData, setIncomeData] = useState<IncomeStatementRow[]>([])
  const [balanceData, setBalanceData] = useState<BalanceSheetRow[]>([])
  const [cashFlowData, setCashFlowData] = useState<CashFlowRow[]>([])
  const [ratiosData, setRatiosData] = useState<KeyMetricsRow[]>([])

  // Periods
  const [incomePeriod, setIncomePeriod] = useState<'annual' | 'quarter'>('annual')
  const [balancePeriod, setBalancePeriod] = useState<'annual' | 'quarter'>('annual')
  const [cashFlowPeriod, setCashFlowPeriod] = useState<'annual' | 'quarter'>('annual')
  const [ratiosPeriod, setRatiosPeriod] = useState<'annual' | 'quarter'>('annual')

  // Loading states
  const [loadingProfile, setLoadingProfile] = useState(false)
  const [loadingIncome, setLoadingIncome] = useState(false)
  const [loadingBalance, setLoadingBalance] = useState(false)
  const [loadingCashFlow, setLoadingCashFlow] = useState(false)
  const [loadingRatios, setLoadingRatios] = useState(false)

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const suggestionsRef = useRef<HTMLDivElement>(null)

  // ------ Check FMP configured ------
  useEffect(() => {
    providerCommands
      .getConfiguredProviders()
      .then((list) => setFmpConfigured(list.includes('fmp')))
      .catch(() => setFmpConfigured(false))
  }, [])

  // ------ Debounced symbol search ------
  const handleQueryChange = useCallback(
    (value: string) => {
      setQuery(value)
      if (debounceRef.current) clearTimeout(debounceRef.current)
      if (value.trim().length < 1) {
        setSuggestions([])
        setShowSuggestions(false)
        return
      }
      debounceRef.current = setTimeout(async () => {
        try {
          const results = await providerCommands.searchGlobalSymbols(value.trim(), 8)
          setSuggestions(results)
          setShowSuggestions(results.length > 0)
        } catch {
          setSuggestions([])
        }
      }, 300)
    },
    []
  )

  // Close suggestions on click outside
  useEffect(() => {
    function onClickOutside(e: MouseEvent) {
      if (suggestionsRef.current && !suggestionsRef.current.contains(e.target as Node)) {
        setShowSuggestions(false)
      }
    }
    document.addEventListener('mousedown', onClickOutside)
    return () => document.removeEventListener('mousedown', onClickOutside)
  }, [])

  // ------ Select a symbol ------
  const selectSymbol = useCallback((symbol: string) => {
    setSelectedSymbol(symbol)
    setQuery(symbol)
    setShowSuggestions(false)
    setSuggestions([])
    setActiveTab('overview')
  }, [])

  const handleSearchSubmit = useCallback(() => {
    if (query.trim()) {
      selectSymbol(query.trim().toUpperCase())
    }
  }, [query, selectSymbol])

  // ------ Auto-select symbol from URL param ------
  useEffect(() => {
    if (urlSymbol) {
      selectSymbol(urlSymbol.toUpperCase())
    }
  }, [urlSymbol, selectSymbol])

  // ------ Fetch overview data ------
  useEffect(() => {
    if (!selectedSymbol) return
    let cancelled = false

    async function fetchOverview() {
      setLoadingProfile(true)
      setProfile(null)
      setKeyMetricsLatest(null)
      try {
        const [profileResult, metricsResult] = await Promise.all([
          providerCommands.getCompanyProfile(selectedSymbol!),
          providerCommands.getKeyMetrics(selectedSymbol!, 'annual', 1),
        ])
        if (cancelled) return
        setProfile(profileResult)
        if (metricsResult && metricsResult.length > 0) {
          setKeyMetricsLatest(metricsResult[0] as Record<string, unknown>)
        }
      } catch (err) {
        if (!cancelled) {
          toast.error(`Failed to fetch profile: ${err instanceof Error ? err.message : String(err)}`)
        }
      } finally {
        if (!cancelled) setLoadingProfile(false)
      }
    }

    fetchOverview()
    return () => { cancelled = true }
  }, [selectedSymbol])

  // ------ Fetch income statement ------
  useEffect(() => {
    if (!selectedSymbol) return
    let cancelled = false

    async function fetchIncome() {
      setLoadingIncome(true)
      setIncomeData([])
      try {
        const raw = await providerCommands.getIncomeStatement(selectedSymbol!, incomePeriod, 5)
        if (cancelled) return
        setIncomeData(
          (raw as Record<string, unknown>[]).map((r) => ({
            date: pickStr(r, 'date'),
            revenue: pick(r, 'revenue'),
            costOfRevenue: pick(r, 'costOfRevenue'),
            grossProfit: pick(r, 'grossProfit'),
            operatingIncome: pick(r, 'operatingIncome'),
            netIncome: pick(r, 'netIncome'),
            eps: pick(r, 'eps'),
            epsdiluted: pick(r, 'epsdiluted'),
            ebitda: pick(r, 'ebitda'),
          }))
        )
      } catch (err) {
        if (!cancelled) {
          toast.error(`Failed to fetch income statement: ${err instanceof Error ? err.message : String(err)}`)
        }
      } finally {
        if (!cancelled) setLoadingIncome(false)
      }
    }

    fetchIncome()
    return () => { cancelled = true }
  }, [selectedSymbol, incomePeriod])

  // ------ Fetch balance sheet ------
  useEffect(() => {
    if (!selectedSymbol) return
    let cancelled = false

    async function fetchBalance() {
      setLoadingBalance(true)
      setBalanceData([])
      try {
        const raw = await providerCommands.getBalanceSheet(selectedSymbol!, balancePeriod, 5)
        if (cancelled) return
        setBalanceData(
          (raw as Record<string, unknown>[]).map((r) => ({
            date: pickStr(r, 'date'),
            totalAssets: pick(r, 'totalAssets'),
            totalCurrentAssets: pick(r, 'totalCurrentAssets'),
            cashAndCashEquivalents: pick(r, 'cashAndCashEquivalents'),
            totalLiabilities: pick(r, 'totalLiabilities'),
            totalCurrentLiabilities: pick(r, 'totalCurrentLiabilities'),
            longTermDebt: pick(r, 'longTermDebt'),
            totalStockholdersEquity: pick(r, 'totalStockholdersEquity'),
          }))
        )
      } catch (err) {
        if (!cancelled) {
          toast.error(`Failed to fetch balance sheet: ${err instanceof Error ? err.message : String(err)}`)
        }
      } finally {
        if (!cancelled) setLoadingBalance(false)
      }
    }

    fetchBalance()
    return () => { cancelled = true }
  }, [selectedSymbol, balancePeriod])

  // ------ Fetch cash flow ------
  useEffect(() => {
    if (!selectedSymbol) return
    let cancelled = false

    async function fetchCashFlow() {
      setLoadingCashFlow(true)
      setCashFlowData([])
      try {
        const raw = await providerCommands.getCashFlow(selectedSymbol!, cashFlowPeriod, 5)
        if (cancelled) return
        setCashFlowData(
          (raw as Record<string, unknown>[]).map((r) => ({
            date: pickStr(r, 'date'),
            operatingCashFlow: pick(r, 'operatingCashFlow'),
            netCashUsedForInvestingActivites: pick(r, 'netCashUsedForInvestingActivites'),
            netCashUsedProvidedByFinancingActivities: pick(r, 'netCashUsedProvidedByFinancingActivities'),
            freeCashFlow: pick(r, 'freeCashFlow'),
            capitalExpenditure: pick(r, 'capitalExpenditure'),
            dividendsPaid: pick(r, 'dividendsPaid'),
          }))
        )
      } catch (err) {
        if (!cancelled) {
          toast.error(`Failed to fetch cash flow: ${err instanceof Error ? err.message : String(err)}`)
        }
      } finally {
        if (!cancelled) setLoadingCashFlow(false)
      }
    }

    fetchCashFlow()
    return () => { cancelled = true }
  }, [selectedSymbol, cashFlowPeriod])

  // ------ Fetch ratios / key metrics ------
  useEffect(() => {
    if (!selectedSymbol) return
    let cancelled = false

    async function fetchRatios() {
      setLoadingRatios(true)
      setRatiosData([])
      try {
        const raw = await providerCommands.getKeyMetrics(selectedSymbol!, ratiosPeriod, 5)
        if (cancelled) return
        setRatiosData(
          (raw as Record<string, unknown>[]).map((r) => ({
            date: pickStr(r, 'date'),
            peRatio: pick(r, 'peRatio'),
            pbRatio: pick(r, 'pbRatio'),
            priceToSalesRatio: pick(r, 'priceToSalesRatio'),
            enterpriseValueOverEBITDA: pick(r, 'enterpriseValueOverEBITDA'),
            roe: pick(r, 'roe'),
            returnOnTangibleAssets: pick(r, 'returnOnTangibleAssets'),
            grossProfitMargin: pick(r, 'grossProfitMargin'),
            operatingProfitMargin: pick(r, 'operatingProfitMargin'),
            netProfitMargin: pick(r, 'netProfitMargin'),
            debtToEquity: pick(r, 'debtToEquity'),
            currentRatio: pick(r, 'currentRatio'),
            revenuePerShare: pick(r, 'revenuePerShare'),
            bookValuePerShare: pick(r, 'bookValuePerShare'),
            freeCashFlowPerShare: pick(r, 'freeCashFlowPerShare'),
            dividendYield: pick(r, 'dividendYield'),
          }))
        )
      } catch (err) {
        if (!cancelled) {
          toast.error(`Failed to fetch key metrics: ${err instanceof Error ? err.message : String(err)}`)
        }
      } finally {
        if (!cancelled) setLoadingRatios(false)
      }
    }

    fetchRatios()
    return () => { cancelled = true }
  }, [selectedSymbol, ratiosPeriod])

  // ------ Render: loading FMP check ------
  if (fmpConfigured === null) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin" />
      </div>
    )
  }

  // ------ Render: FMP not configured ------
  if (!fmpConfigured) {
    return (
      <div className="py-6 max-w-5xl mx-auto px-6 space-y-6">
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold">Fundamentals</h1>
        </div>
        <FmpNotConfigured />
      </div>
    )
  }

  // ------ Render: main page ------
  return (
    <div className="py-6 max-w-5xl mx-auto px-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-2">
        <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
          <ArrowLeft className="h-4 w-4" />
        </Link>
        <h1 className="text-2xl font-bold">Fundamentals</h1>
      </div>

      {/* Symbol Search */}
      <div className="relative" ref={suggestionsRef}>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search symbol (e.g. AAPL, MSFT, TSLA)"
              value={query}
              onChange={(e) => handleQueryChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSearchSubmit()
                if (e.key === 'Escape') setShowSuggestions(false)
              }}
              onFocus={() => {
                if (suggestions.length > 0) setShowSuggestions(true)
              }}
              className="pl-9 h-10"
            />
          </div>
          <Button onClick={handleSearchSubmit} className="h-10">
            <Search className="h-4 w-4 mr-1" />
            Search
          </Button>
        </div>

        {/* Autocomplete dropdown */}
        {showSuggestions && suggestions.length > 0 && (
          <div className="absolute z-50 top-full mt-1 w-full bg-popover border rounded-lg shadow-lg overflow-hidden">
            {suggestions.map((s) => (
              <button
                key={`${s.symbol}-${s.exchange ?? ''}`}
                type="button"
                className="w-full flex items-center gap-3 px-4 py-2.5 text-left hover:bg-muted/50 transition-colors cursor-pointer"
                onClick={() => selectSymbol(s.symbol)}
              >
                <span className="font-semibold text-sm font-mono tabular-nums min-w-[64px]">
                  {s.symbol}
                </span>
                <span className="text-sm text-muted-foreground truncate flex-1">{s.name}</span>
                {s.exchange_display && (
                  <Badge variant="secondary" className="text-[10px] shrink-0">
                    {s.exchange_display}
                  </Badge>
                )}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Content */}
      {!selectedSymbol ? (
        <EmptySymbol />
      ) : (
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList>
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="analysis">Analysis</TabsTrigger>
            <TabsTrigger value="income">Income</TabsTrigger>
            <TabsTrigger value="balance">Balance Sheet</TabsTrigger>
            <TabsTrigger value="cashflow">Cash Flow</TabsTrigger>
            <TabsTrigger value="ratios">Ratios</TabsTrigger>
          </TabsList>

          {/* ============== OVERVIEW TAB ============== */}
          <TabsContent value="overview" className="space-y-6 mt-4">
            {loadingProfile ? (
              <>
                {/* Profile skeleton */}
                <Card>
                  <CardContent className="p-4 space-y-3">
                    <div className="flex items-center gap-4">
                      <Skeleton className="h-14 w-14 rounded-lg" />
                      <div className="space-y-2 flex-1">
                        <Skeleton className="h-5 w-48" />
                        <Skeleton className="h-3 w-32" />
                      </div>
                    </div>
                    <Skeleton className="h-3 w-full" />
                    <Skeleton className="h-3 w-3/4" />
                  </CardContent>
                </Card>
                {/* Stats skeleton */}
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                  {Array.from({ length: 8 }).map((_, i) => (
                    <StatCardSkeleton key={i} />
                  ))}
                </div>
              </>
            ) : profile ? (
              <>
                {/* Company Profile */}
                <Card>
                  <CardContent className="p-4">
                    <div className="flex items-start gap-4">
                      {profile.image && (
                        <img
                          src={profile.image}
                          alt={profile.company_name}
                          className="h-14 w-14 rounded-lg object-contain bg-white p-1 border"
                          onError={(e) => {
                            ;(e.target as HTMLImageElement).style.display = 'none'
                          }}
                        />
                      )}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <h2 className="text-xl font-bold truncate">{profile.company_name}</h2>
                          <Badge variant="secondary" className="text-xs shrink-0">
                            {profile.symbol}
                          </Badge>
                        </div>
                        <div className="flex items-center gap-3 mt-1 flex-wrap text-sm text-muted-foreground">
                          {profile.exchange && (
                            <span className="flex items-center gap-1">
                              <Globe className="h-3 w-3" />
                              {profile.exchange}
                            </span>
                          )}
                          {profile.sector && (
                            <span className="flex items-center gap-1">
                              <Building2 className="h-3 w-3" />
                              {profile.sector}
                            </span>
                          )}
                          {profile.industry && <span>{profile.industry}</span>}
                        </div>
                        <div className="flex items-center gap-3 mt-1 flex-wrap text-sm text-muted-foreground">
                          {profile.ceo && (
                            <span className="flex items-center gap-1">
                              <Users className="h-3 w-3" />
                              CEO: {profile.ceo}
                            </span>
                          )}
                          {profile.employees != null && (
                            <span>{formatEmployees(profile.employees)} employees</span>
                          )}
                          {profile.website && (
                            <a
                              href={profile.website}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="flex items-center gap-1 text-primary hover:underline"
                              onClick={(e) => e.stopPropagation()}
                            >
                              <ExternalLink className="h-3 w-3" />
                              Website
                            </a>
                          )}
                        </div>
                      </div>
                    </div>
                    {profile.description && (
                      <p className="text-sm text-muted-foreground mt-4 leading-relaxed line-clamp-4">
                        {profile.description}
                      </p>
                    )}
                  </CardContent>
                </Card>

                {/* Price Info */}
                {profile.price != null && (
                  <Card>
                    <CardContent className="p-4">
                      <div className="flex items-center justify-between flex-wrap gap-4">
                        <div className="flex items-center gap-6 flex-wrap">
                          <div>
                            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                              Current Price
                            </p>
                            <p className="text-2xl font-bold font-mono tabular-nums">
                              ${formatNumber(profile.price)}
                            </p>
                          </div>
                          {(profile as unknown as Record<string, unknown>).changes != null && (
                            <div>
                              <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                                Change
                              </p>
                              <p className="text-lg font-bold font-mono tabular-nums">
                                {formatNumber((profile as unknown as Record<string, unknown>).changes as number)}
                              </p>
                            </div>
                          )}
                        </div>
                        <PlaceOrderDialog
                          defaultSymbol={selectedSymbol ?? undefined}
                          trigger={
                            <Button size="sm" className="bg-green-500 hover:bg-green-600 text-white">
                              <ShoppingCart className="h-4 w-4 mr-2" />
                              Trade {selectedSymbol}
                            </Button>
                          }
                        />
                      </div>
                    </CardContent>
                  </Card>
                )}

                {/* Key Stats Grid */}
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                  <StatCard
                    label="Market Cap"
                    value={formatLargeNumber(profile.market_cap)}
                  />
                  <StatCard
                    label="P/E Ratio"
                    value={keyMetricsLatest ? formatNumber(pick(keyMetricsLatest, 'peRatio')) : '--'}
                  />
                  <StatCard
                    label="P/B Ratio"
                    value={keyMetricsLatest ? formatNumber(pick(keyMetricsLatest, 'pbRatio')) : '--'}
                  />
                  <StatCard
                    label="EV/EBITDA"
                    value={
                      keyMetricsLatest
                        ? formatNumber(pick(keyMetricsLatest, 'enterpriseValueOverEBITDA'))
                        : '--'
                    }
                  />
                  <StatCard
                    label="ROE"
                    value={
                      keyMetricsLatest ? formatPercent(pick(keyMetricsLatest, 'roe')) : '--'
                    }
                  />
                  <StatCard
                    label="Gross Margin"
                    value={
                      keyMetricsLatest
                        ? formatPercent(pick(keyMetricsLatest, 'grossProfitMargin'))
                        : '--'
                    }
                  />
                  <StatCard
                    label="Dividend Yield"
                    value={
                      keyMetricsLatest
                        ? formatPercent(pick(keyMetricsLatest, 'dividendYield'))
                        : '--'
                    }
                  />
                  <StatCard label="Beta" value={formatNumber(profile.beta)} />
                </div>
              </>
            ) : (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <TrendingUp className="h-10 w-10 text-muted-foreground/40 mb-3" />
                <p className="text-sm text-muted-foreground">
                  No profile data found for <span className="font-semibold">{selectedSymbol}</span>
                </p>
              </div>
            )}
          </TabsContent>

          {/* ============== ANALYSIS TAB ============== */}
          <TabsContent value="analysis">
            <AnalysisTab symbol={selectedSymbol} />
          </TabsContent>

          {/* ============== INCOME STATEMENT TAB ============== */}
          <TabsContent value="income" className="mt-4">
            <PeriodToggle period={incomePeriod} onChange={setIncomePeriod} />
            {loadingIncome ? (
              <TableSkeleton cols={8} />
            ) : incomeData.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <DollarSign className="h-10 w-10 text-muted-foreground/40 mb-3" />
                <p className="text-sm text-muted-foreground">No income statement data available.</p>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="text-left">Date</TableHead>
                      <TableHead className="text-right">Revenue</TableHead>
                      <TableHead className="text-right">Cost of Revenue</TableHead>
                      <TableHead className="text-right">Gross Profit</TableHead>
                      <TableHead className="text-right">Operating Income</TableHead>
                      <TableHead className="text-right">Net Income</TableHead>
                      <TableHead className="text-right">EPS</TableHead>
                      <TableHead className="text-right">EBITDA</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {incomeData.map((row) => (
                      <TableRow key={row.date}>
                        <TableCell className="font-medium">{row.date}</TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.revenue)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.costOfRevenue)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.grossProfit)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.operatingIncome)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.netIncome)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {row.epsdiluted != null ? `$${formatNumber(row.epsdiluted)}` : '--'}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.ebitda)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            )}
          </TabsContent>

          {/* ============== BALANCE SHEET TAB ============== */}
          <TabsContent value="balance" className="mt-4">
            <PeriodToggle period={balancePeriod} onChange={setBalancePeriod} />
            {loadingBalance ? (
              <TableSkeleton cols={8} />
            ) : balanceData.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <DollarSign className="h-10 w-10 text-muted-foreground/40 mb-3" />
                <p className="text-sm text-muted-foreground">No balance sheet data available.</p>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="text-left">Date</TableHead>
                      <TableHead className="text-right">Total Assets</TableHead>
                      <TableHead className="text-right">Current Assets</TableHead>
                      <TableHead className="text-right">Cash</TableHead>
                      <TableHead className="text-right">Total Liabilities</TableHead>
                      <TableHead className="text-right">Current Liabilities</TableHead>
                      <TableHead className="text-right">Long-term Debt</TableHead>
                      <TableHead className="text-right">Equity</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {balanceData.map((row) => (
                      <TableRow key={row.date}>
                        <TableCell className="font-medium">{row.date}</TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.totalAssets)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.totalCurrentAssets)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.cashAndCashEquivalents)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.totalLiabilities)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.totalCurrentLiabilities)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.longTermDebt)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.totalStockholdersEquity)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            )}
          </TabsContent>

          {/* ============== CASH FLOW TAB ============== */}
          <TabsContent value="cashflow" className="mt-4">
            <PeriodToggle period={cashFlowPeriod} onChange={setCashFlowPeriod} />
            {loadingCashFlow ? (
              <TableSkeleton cols={7} />
            ) : cashFlowData.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <DollarSign className="h-10 w-10 text-muted-foreground/40 mb-3" />
                <p className="text-sm text-muted-foreground">No cash flow data available.</p>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="text-left">Date</TableHead>
                      <TableHead className="text-right">Operating CF</TableHead>
                      <TableHead className="text-right">Investing CF</TableHead>
                      <TableHead className="text-right">Financing CF</TableHead>
                      <TableHead className="text-right">Free Cash Flow</TableHead>
                      <TableHead className="text-right">CapEx</TableHead>
                      <TableHead className="text-right">Dividends</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {cashFlowData.map((row) => (
                      <TableRow key={row.date}>
                        <TableCell className="font-medium">{row.date}</TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.operatingCashFlow)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.netCashUsedForInvestingActivites)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.netCashUsedProvidedByFinancingActivities)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.freeCashFlow)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.capitalExpenditure)}
                        </TableCell>
                        <TableCell className="text-right font-mono tabular-nums">
                          {formatLargeNumber(row.dividendsPaid)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            )}
          </TabsContent>

          {/* ============== RATIOS TAB ============== */}
          <TabsContent value="ratios" className="mt-4">
            <PeriodToggle period={ratiosPeriod} onChange={setRatiosPeriod} />
            {loadingRatios ? (
              <div className="space-y-6">
                {Array.from({ length: 4 }).map((_, i) => (
                  <Card key={i}>
                    <CardContent className="p-4 space-y-3">
                      <Skeleton className="h-4 w-24" />
                      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                        {Array.from({ length: 4 }).map((_, j) => (
                          <div key={j} className="space-y-1">
                            <Skeleton className="h-3 w-16" />
                            <Skeleton className="h-5 w-20" />
                          </div>
                        ))}
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            ) : ratiosData.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <TrendingUp className="h-10 w-10 text-muted-foreground/40 mb-3" />
                <p className="text-sm text-muted-foreground">No key metrics data available.</p>
              </div>
            ) : (
              <div className="space-y-6">
                {ratiosData.map((row) => (
                  <Card key={row.date}>
                    <CardHeader className="pb-3 pt-4 px-4">
                      <CardTitle className="text-sm font-semibold">{row.date}</CardTitle>
                    </CardHeader>
                    <CardContent className="px-4 pb-4 space-y-4">
                      {/* Valuation */}
                      <div>
                        <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                          Valuation
                        </p>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                          <RatioItem label="P/E" value={formatNumber(row.peRatio)} />
                          <RatioItem label="P/B" value={formatNumber(row.pbRatio)} />
                          <RatioItem label="P/S" value={formatNumber(row.priceToSalesRatio)} />
                          <RatioItem label="EV/EBITDA" value={formatNumber(row.enterpriseValueOverEBITDA)} />
                        </div>
                      </div>

                      {/* Profitability */}
                      <div>
                        <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                          Profitability
                        </p>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                          <RatioItem label="ROE" value={formatPercent(row.roe)} />
                          <RatioItem label="ROA" value={formatPercent(row.returnOnTangibleAssets)} />
                          <RatioItem label="Gross Margin" value={formatPercent(row.grossProfitMargin)} />
                          <RatioItem label="Operating Margin" value={formatPercent(row.operatingProfitMargin)} />
                          <RatioItem label="Net Margin" value={formatPercent(row.netProfitMargin)} />
                        </div>
                      </div>

                      {/* Leverage */}
                      <div>
                        <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                          Leverage
                        </p>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                          <RatioItem label="Debt/Equity" value={formatNumber(row.debtToEquity)} />
                          <RatioItem label="Current Ratio" value={formatNumber(row.currentRatio)} />
                        </div>
                      </div>

                      {/* Per Share */}
                      <div>
                        <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                          Per Share
                        </p>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                          <RatioItem label="Revenue/Share" value={`$${formatNumber(row.revenuePerShare)}`} />
                          <RatioItem label="Book Value/Share" value={`$${formatNumber(row.bookValuePerShare)}`} />
                          <RatioItem label="FCF/Share" value={`$${formatNumber(row.freeCashFlowPerShare)}`} />
                          <RatioItem label="Dividend Yield" value={formatPercent(row.dividendYield)} />
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            )}
          </TabsContent>
        </Tabs>
      )}
    </div>
  )
}

function RatioItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-sm font-semibold font-mono tabular-nums">{value}</p>
    </div>
  )
}
