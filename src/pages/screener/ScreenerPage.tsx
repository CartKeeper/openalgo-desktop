import {
  ArrowLeft,
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ChevronDown,
  ChevronRight,
  Filter,
  Loader2,
  Search,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { providerCommands, type ScreenerFilters } from '@/api/tauri-client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ScreenerResult {
  symbol: string
  company_name: string
  exchange: string
  sector: string
  industry: string
  market_cap: number | null
  price: number | null
  change_percent: number | null
  volume: number | null
  pe_ratio: number | null
  beta: number | null
  dividend_yield: number | null
  country: string
}

type SortField =
  | 'symbol'
  | 'company_name'
  | 'price'
  | 'change_percent'
  | 'market_cap'
  | 'pe_ratio'
  | 'beta'
  | 'dividend_yield'
  | 'sector'
  | 'volume'

type SortDirection = 'asc' | 'desc'

interface FilterState {
  pe_min: string
  pe_max: string
  price_min: string
  price_max: string
  market_cap_min: string
  market_cap_max: string
  market_cap_min_mag: 'M' | 'B'
  market_cap_max_mag: 'M' | 'B'
  beta_min: string
  beta_max: string
  dividend_yield_min: string
  volume_min: string
  sector: string
  country: string
  is_etf: boolean
  limit: string
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

interface Preset {
  label: string
  filters: Partial<FilterState>
}

const PRESETS: Preset[] = [
  {
    label: 'Large Cap Growth',
    filters: {
      market_cap_min: '10',
      market_cap_min_mag: 'B',
      pe_min: '20',
      limit: '50',
    },
  },
  {
    label: 'High Dividend',
    filters: {
      dividend_yield_min: '3',
      market_cap_min: '1',
      market_cap_min_mag: 'B',
      limit: '50',
    },
  },
  {
    label: 'Small Cap Value',
    filters: {
      market_cap_min: '300',
      market_cap_min_mag: 'M',
      market_cap_max: '2',
      market_cap_max_mag: 'B',
      pe_max: '15',
      limit: '50',
    },
  },
  {
    label: 'Tech Giants',
    filters: {
      sector: 'Technology',
      market_cap_min: '100',
      market_cap_min_mag: 'B',
      limit: '50',
    },
  },
  {
    label: 'Low PE',
    filters: {
      pe_min: '1',
      pe_max: '10',
      market_cap_min: '1',
      market_cap_min_mag: 'B',
      limit: '50',
    },
  },
]

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SECTORS = [
  'Technology',
  'Healthcare',
  'Financial Services',
  'Energy',
  'Consumer Cyclical',
  'Consumer Defensive',
  'Industrials',
  'Basic Materials',
  'Communication Services',
  'Real Estate',
  'Utilities',
]

const EMPTY_FILTERS: FilterState = {
  pe_min: '',
  pe_max: '',
  price_min: '',
  price_max: '',
  market_cap_min: '',
  market_cap_max: '',
  market_cap_min_mag: 'B',
  market_cap_max_mag: 'B',
  beta_min: '',
  beta_max: '',
  dividend_yield_min: '',
  volume_min: '',
  sector: '',
  country: 'US',
  is_etf: false,
  limit: '25',
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatMarketCap(value: number | null): string {
  if (value == null) return '-'
  if (value >= 1_000_000_000_000) return `$${(value / 1_000_000_000_000).toFixed(2)}T`
  if (value >= 1_000_000_000) return `$${(value / 1_000_000_000).toFixed(2)}B`
  if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(2)}M`
  return `$${value.toLocaleString()}`
}

function formatPrice(value: number | null): string {
  if (value == null) return '-'
  return `$${value.toFixed(2)}`
}

function formatPercent(value: number | null): string {
  if (value == null) return '-'
  return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`
}

function formatNumber(value: number | null, decimals = 2): string {
  if (value == null) return '-'
  return value.toFixed(decimals)
}

function formatVolume(value: number | null): string {
  if (value == null) return '-'
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return value.toLocaleString()
}

function magMultiplier(mag: 'M' | 'B'): number {
  return mag === 'B' ? 1_000_000_000 : 1_000_000
}

function parseNumericInput(val: string): number | undefined {
  if (val === '') return undefined
  const n = Number(val)
  return Number.isNaN(n) ? undefined : n
}

// ---------------------------------------------------------------------------
// Collapsible filter section
// ---------------------------------------------------------------------------

function FilterSection({
  title,
  defaultOpen = true,
  children,
}: {
  title: string
  defaultOpen?: boolean
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(defaultOpen)

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between py-3 px-1 text-sm font-semibold text-foreground hover:text-foreground/80 transition-colors"
      >
        {title}
        {open ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
      </button>
      {open && <div className="pb-4 px-1 space-y-4">{children}</div>}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Number input helper
// ---------------------------------------------------------------------------

function NumberInput({
  value,
  onChange,
  placeholder,
  className,
}: {
  value: string
  onChange: (val: string) => void
  placeholder?: string
  className?: string
}) {
  return (
    <Input
      type="text"
      inputMode="decimal"
      value={value}
      onChange={(e) => {
        const v = e.target.value
        // Allow empty, digits, one decimal point, and optional leading minus
        if (v === '' || /^-?\d*\.?\d*$/.test(v)) {
          onChange(v)
        }
      }}
      placeholder={placeholder}
      className={className}
    />
  )
}

// ---------------------------------------------------------------------------
// Magnitude selector (M / B)
// ---------------------------------------------------------------------------

function MagSelector({
  value,
  onChange,
}: {
  value: 'M' | 'B'
  onChange: (v: 'M' | 'B') => void
}) {
  return (
    <div className="flex rounded-md border border-border overflow-hidden">
      <button
        type="button"
        onClick={() => onChange('M')}
        className={`px-2 py-1 text-xs font-semibold transition-colors ${
          value === 'M'
            ? 'bg-primary text-primary-foreground'
            : 'bg-transparent text-muted-foreground hover:text-foreground'
        }`}
      >
        M
      </button>
      <button
        type="button"
        onClick={() => onChange('B')}
        className={`px-2 py-1 text-xs font-semibold transition-colors border-l border-border ${
          value === 'B'
            ? 'bg-primary text-primary-foreground'
            : 'bg-transparent text-muted-foreground hover:text-foreground'
        }`}
      >
        B
      </button>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sort icon
// ---------------------------------------------------------------------------

function SortIcon({
  field,
  sortField,
  sortDir,
}: {
  field: SortField
  sortField: SortField | null
  sortDir: SortDirection
}) {
  if (sortField !== field) {
    return <ArrowUpDown className="ml-1 inline h-3 w-3 text-muted-foreground/50" />
  }
  return sortDir === 'asc' ? (
    <ArrowUp className="ml-1 inline h-3 w-3 text-foreground" />
  ) : (
    <ArrowDown className="ml-1 inline h-3 w-3 text-foreground" />
  )
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export default function ScreenerPage() {
  const navigate = useNavigate()

  // Filter state
  const [filters, setFilters] = useState<FilterState>({ ...EMPTY_FILTERS })
  const [activePreset, setActivePreset] = useState<string | null>(null)

  // Results state
  const [results, setResults] = useState<ScreenerResult[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [hasSearched, setHasSearched] = useState(false)

  // Sort state
  const [sortField, setSortField] = useState<SortField | null>(null)
  const [sortDir, setSortDir] = useState<SortDirection>('desc')

  // Mobile filter panel toggle
  const [showFilters, setShowFilters] = useState(false)

  // Update a single filter field
  const setFilter = useCallback(
    <K extends keyof FilterState>(key: K, value: FilterState[K]) => {
      setFilters((prev) => ({ ...prev, [key]: value }))
      setActivePreset(null)
    },
    []
  )

  // Apply preset
  const applyPreset = useCallback((preset: Preset) => {
    setFilters({ ...EMPTY_FILTERS, ...preset.filters })
    setActivePreset(preset.label)
  }, [])

  // Clear filters
  const clearFilters = useCallback(() => {
    setFilters({ ...EMPTY_FILTERS })
    setActivePreset(null)
  }, [])

  // Build ScreenerFilters from form state
  const buildApiFilters = useCallback((): ScreenerFilters => {
    const f: ScreenerFilters = {}

    const peMin = parseNumericInput(filters.pe_min)
    const peMax = parseNumericInput(filters.pe_max)
    if (peMin !== undefined) f.pe_min = peMin
    if (peMax !== undefined) f.pe_max = peMax

    const priceMin = parseNumericInput(filters.price_min)
    const priceMax = parseNumericInput(filters.price_max)
    if (priceMin !== undefined) f.price_min = priceMin
    if (priceMax !== undefined) f.price_max = priceMax

    const mcMin = parseNumericInput(filters.market_cap_min)
    const mcMax = parseNumericInput(filters.market_cap_max)
    if (mcMin !== undefined) f.market_cap_min = mcMin * magMultiplier(filters.market_cap_min_mag)
    if (mcMax !== undefined) f.market_cap_max = mcMax * magMultiplier(filters.market_cap_max_mag)

    const betaMin = parseNumericInput(filters.beta_min)
    const betaMax = parseNumericInput(filters.beta_max)
    if (betaMin !== undefined) f.beta_min = betaMin
    if (betaMax !== undefined) f.beta_max = betaMax

    const divMin = parseNumericInput(filters.dividend_yield_min)
    if (divMin !== undefined) f.dividend_yield_min = divMin

    const volMin = parseNumericInput(filters.volume_min)
    if (volMin !== undefined) f.volume_min = volMin

    if (filters.sector) f.sector = filters.sector
    if (filters.country) f.country = filters.country
    if (filters.is_etf) f.is_etf = true

    const limit = parseInt(filters.limit, 10)
    if (!Number.isNaN(limit)) f.limit = limit

    return f
  }, [filters])

  // Run screener
  const runScreen = useCallback(async () => {
    setLoading(true)
    setError(null)
    setHasSearched(true)
    setSortField(null)

    try {
      const apiFilters = buildApiFilters()
      const data = (await providerCommands.screenStocks(apiFilters)) as ScreenerResult[]
      setResults(data)

      if (data.length === 0) {
        toast.info('No stocks matched your criteria')
      } else {
        toast.success(`Found ${data.length} stock${data.length !== 1 ? 's' : ''}`)
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err)

      if (
        message.toLowerCase().includes('api key') ||
        message.toLowerCase().includes('unauthorized') ||
        message.toLowerCase().includes('401')
      ) {
        setError('FMP API key is not configured. Go to Profile to set it up.')
      } else {
        setError(message)
      }

      toast.error('Screener failed', { description: message })
      setResults(null)
    } finally {
      setLoading(false)
    }
  }, [buildApiFilters])

  // Toggle sort
  const toggleSort = useCallback(
    (field: SortField) => {
      if (sortField === field) {
        setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
      } else {
        setSortField(field)
        setSortDir('desc')
      }
    },
    [sortField]
  )

  // Sorted results
  const sortedResults = useMemo(() => {
    if (!results || !sortField) return results
    const sorted = [...results]
    sorted.sort((a, b) => {
      const aVal = a[sortField]
      const bVal = b[sortField]

      if (aVal == null && bVal == null) return 0
      if (aVal == null) return 1
      if (bVal == null) return -1

      if (typeof aVal === 'string' && typeof bVal === 'string') {
        return sortDir === 'asc'
          ? aVal.localeCompare(bVal)
          : bVal.localeCompare(aVal)
      }

      const numA = aVal as number
      const numB = bVal as number
      return sortDir === 'asc' ? numA - numB : numB - numA
    })
    return sorted
  }, [results, sortField, sortDir])

  // Navigate to fundamentals on row click
  const handleRowClick = useCallback(
    (symbol: string) => {
      navigate(`/fundamentals?symbol=${encodeURIComponent(symbol)}`)
    },
    [navigate]
  )

  // -------------------------------------------------------------------------
  // Filter panel (shared between mobile and desktop)
  // -------------------------------------------------------------------------

  const filterPanel = (
    <div className="space-y-2">
      {/* Presets */}
      <div className="px-1 pb-2">
        <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-3">
          Quick Presets
        </p>
        <div className="flex flex-wrap gap-2">
          {PRESETS.map((preset) => (
            <Button
              key={preset.label}
              variant={activePreset === preset.label ? 'default' : 'outline'}
              size="sm"
              className="h-8 text-xs"
              onClick={() => applyPreset(preset)}
            >
              <Sparkles className="h-3 w-3 mr-1" />
              {preset.label}
            </Button>
          ))}
        </div>
      </div>

      {/* Filter Sections */}
      <FilterSection title="Valuation">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label className="text-xs text-muted-foreground">PE Min</Label>
            <NumberInput
              value={filters.pe_min}
              onChange={(v) => setFilter('pe_min', v)}
              placeholder="0"
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">PE Max</Label>
            <NumberInput
              value={filters.pe_max}
              onChange={(v) => setFilter('pe_max', v)}
              placeholder="Any"
            />
          </div>
        </div>
      </FilterSection>

      <FilterSection title="Price">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label className="text-xs text-muted-foreground">Min ($)</Label>
            <NumberInput
              value={filters.price_min}
              onChange={(v) => setFilter('price_min', v)}
              placeholder="0"
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Max ($)</Label>
            <NumberInput
              value={filters.price_max}
              onChange={(v) => setFilter('price_max', v)}
              placeholder="Any"
            />
          </div>
        </div>
      </FilterSection>

      <FilterSection title="Market Cap">
        <div className="space-y-3">
          <div>
            <Label className="text-xs text-muted-foreground">Min</Label>
            <div className="flex gap-2 items-center">
              <NumberInput
                value={filters.market_cap_min}
                onChange={(v) => setFilter('market_cap_min', v)}
                placeholder="0"
                className="flex-1"
              />
              <MagSelector
                value={filters.market_cap_min_mag}
                onChange={(v) => setFilter('market_cap_min_mag', v)}
              />
            </div>
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Max</Label>
            <div className="flex gap-2 items-center">
              <NumberInput
                value={filters.market_cap_max}
                onChange={(v) => setFilter('market_cap_max', v)}
                placeholder="Any"
                className="flex-1"
              />
              <MagSelector
                value={filters.market_cap_max_mag}
                onChange={(v) => setFilter('market_cap_max_mag', v)}
              />
            </div>
          </div>
        </div>
      </FilterSection>

      <FilterSection title="Fundamentals" defaultOpen={false}>
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label className="text-xs text-muted-foreground">Beta Min</Label>
              <NumberInput
                value={filters.beta_min}
                onChange={(v) => setFilter('beta_min', v)}
                placeholder="0"
              />
            </div>
            <div>
              <Label className="text-xs text-muted-foreground">Beta Max</Label>
              <NumberInput
                value={filters.beta_max}
                onChange={(v) => setFilter('beta_max', v)}
                placeholder="Any"
              />
            </div>
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Div Yield Min (%)</Label>
            <NumberInput
              value={filters.dividend_yield_min}
              onChange={(v) => setFilter('dividend_yield_min', v)}
              placeholder="0"
            />
          </div>
        </div>
      </FilterSection>

      <FilterSection title="Volume" defaultOpen={false}>
        <div>
          <Label className="text-xs text-muted-foreground">Minimum Volume</Label>
          <NumberInput
            value={filters.volume_min}
            onChange={(v) => setFilter('volume_min', v)}
            placeholder="e.g. 1000000"
          />
        </div>
      </FilterSection>

      <FilterSection title="Sector" defaultOpen={false}>
        <Select
          value={filters.sector}
          onValueChange={(v) => setFilter('sector', v === '__all__' ? '' : v)}
        >
          <SelectTrigger>
            <SelectValue placeholder="All Sectors" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">All Sectors</SelectItem>
            {SECTORS.map((s) => (
              <SelectItem key={s} value={s}>
                {s}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </FilterSection>

      <FilterSection title="Country" defaultOpen={false}>
        <Input
          value={filters.country}
          onChange={(e) => setFilter('country', e.target.value)}
          placeholder="US"
        />
      </FilterSection>

      <FilterSection title="Type" defaultOpen={false}>
        <div className="flex items-center gap-2">
          <Checkbox
            id="include-etfs"
            checked={filters.is_etf}
            onCheckedChange={(checked) => setFilter('is_etf', checked === true)}
          />
          <Label htmlFor="include-etfs" className="text-sm cursor-pointer">
            Include ETFs
          </Label>
        </div>
      </FilterSection>

      <FilterSection title="Limit" defaultOpen={false}>
        <Select
          value={filters.limit}
          onValueChange={(v) => setFilter('limit', v)}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="25">25 results</SelectItem>
            <SelectItem value="50">50 results</SelectItem>
            <SelectItem value="100">100 results</SelectItem>
          </SelectContent>
        </Select>
      </FilterSection>

      {/* Action buttons */}
      <div className="space-y-2 pt-2 px-1">
        <Button
          className="w-full h-10"
          onClick={runScreen}
          disabled={loading}
        >
          {loading ? (
            <Loader2 className="h-4 w-4 mr-2 animate-spin" />
          ) : (
            <Search className="h-4 w-4 mr-2" />
          )}
          Screen Stocks
        </Button>
        <Button
          variant="ghost"
          className="w-full h-10"
          onClick={clearFilters}
          disabled={loading}
        >
          <Trash2 className="h-4 w-4 mr-2" />
          Clear Filters
        </Button>
      </div>
    </div>
  )

  // -------------------------------------------------------------------------
  // Results panel
  // -------------------------------------------------------------------------

  const resultsPanel = (
    <div className="flex-1 min-w-0">
      {/* Results header */}
      {hasSearched && !error && results && (
        <div className="mb-4">
          <Badge variant="secondary" className="text-xs">
            {results.length} stock{results.length !== 1 ? 's' : ''} found
          </Badge>
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="rounded-full bg-destructive/10 p-4 mb-4">
            <X className="h-8 w-8 text-destructive" />
          </div>
          <p className="text-sm font-semibold text-foreground mb-1">Screener Error</p>
          <p className="text-sm text-muted-foreground max-w-md">{error}</p>
          {error.toLowerCase().includes('api key') && (
            <Button
              variant="outline"
              size="sm"
              className="mt-4 h-8"
              onClick={() => navigate('/profile')}
            >
              Go to Profile Settings
            </Button>
          )}
        </div>
      )}

      {/* Initial state */}
      {!hasSearched && !error && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="rounded-full bg-muted p-4 mb-4">
            <Filter className="h-8 w-8 text-muted-foreground" />
          </div>
          <p className="text-sm font-semibold text-foreground mb-1">Set filters and click Screen to find stocks</p>
          <p className="text-sm text-muted-foreground">
            Use presets for quick searches or customize your criteria
          </p>
        </div>
      )}

      {/* Loading state */}
      {loading && (
        <div className="space-y-0">
          <div className="border rounded-lg overflow-hidden">
            {/* Skeleton header */}
            <div className="h-10 bg-muted/30 border-b flex items-center gap-4 px-4">
              {Array.from({ length: 7 }).map((_, i) => (
                <Skeleton key={i} className="h-3 w-16" />
              ))}
            </div>
            {/* Skeleton rows */}
            {Array.from({ length: 10 }).map((_, rowIdx) => (
              <div
                key={rowIdx}
                className="h-12 border-b last:border-b-0 flex items-center gap-4 px-4"
              >
                {Array.from({ length: 7 }).map((_, colIdx) => (
                  <Skeleton
                    key={colIdx}
                    className={`h-3 ${colIdx === 0 ? 'w-14' : colIdx === 1 ? 'w-32' : 'w-16'}`}
                  />
                ))}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Empty results */}
      {hasSearched && !loading && !error && results && results.length === 0 && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="rounded-full bg-muted p-4 mb-4">
            <Search className="h-8 w-8 text-muted-foreground" />
          </div>
          <p className="text-sm font-semibold text-foreground mb-1">No stocks match your criteria</p>
          <p className="text-sm text-muted-foreground">
            Try broadening your filters or using a different preset
          </p>
        </div>
      )}

      {/* Results table */}
      {!loading && !error && sortedResults && sortedResults.length > 0 && (
        <div className="border rounded-lg overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead
                  className="cursor-pointer select-none"
                  onClick={() => toggleSort('symbol')}
                >
                  Symbol
                  <SortIcon field="symbol" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none"
                  onClick={() => toggleSort('company_name')}
                >
                  Company
                  <SortIcon field="company_name" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('price')}
                >
                  Price
                  <SortIcon field="price" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('change_percent')}
                >
                  Change%
                  <SortIcon field="change_percent" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('market_cap')}
                >
                  Market Cap
                  <SortIcon field="market_cap" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('pe_ratio')}
                >
                  P/E
                  <SortIcon field="pe_ratio" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('beta')}
                >
                  Beta
                  <SortIcon field="beta" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('dividend_yield')}
                >
                  Div Yield
                  <SortIcon field="dividend_yield" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none"
                  onClick={() => toggleSort('sector')}
                >
                  Sector
                  <SortIcon field="sector" sortField={sortField} sortDir={sortDir} />
                </TableHead>
                <TableHead
                  className="cursor-pointer select-none text-right"
                  onClick={() => toggleSort('volume')}
                >
                  Volume
                  <SortIcon field="volume" sortField={sortField} sortDir={sortDir} />
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sortedResults.map((stock) => (
                <TableRow
                  key={stock.symbol}
                  className="cursor-pointer"
                  onClick={() => handleRowClick(stock.symbol)}
                >
                  <TableCell className="font-semibold text-foreground">
                    {stock.symbol}
                  </TableCell>
                  <TableCell className="max-w-[200px] truncate text-muted-foreground">
                    {stock.company_name || '-'}
                  </TableCell>
                  <TableCell className="text-right font-mono tabular-nums">
                    {formatPrice(stock.price)}
                  </TableCell>
                  <TableCell
                    className={`text-right font-mono tabular-nums ${
                      stock.change_percent != null && stock.change_percent > 0
                        ? 'text-green-500'
                        : stock.change_percent != null && stock.change_percent < 0
                          ? 'text-red-500'
                          : 'text-muted-foreground'
                    }`}
                  >
                    {formatPercent(stock.change_percent)}
                  </TableCell>
                  <TableCell className="text-right font-mono tabular-nums">
                    {formatMarketCap(stock.market_cap)}
                  </TableCell>
                  <TableCell className="text-right font-mono tabular-nums">
                    {formatNumber(stock.pe_ratio)}
                  </TableCell>
                  <TableCell className="text-right font-mono tabular-nums">
                    {formatNumber(stock.beta)}
                  </TableCell>
                  <TableCell className="text-right font-mono tabular-nums">
                    {stock.dividend_yield != null
                      ? `${stock.dividend_yield.toFixed(2)}%`
                      : '-'}
                  </TableCell>
                  <TableCell className="max-w-[120px] truncate text-muted-foreground text-xs">
                    {stock.sector || '-'}
                  </TableCell>
                  <TableCell className="text-right font-mono tabular-nums">
                    {formatVolume(stock.volume)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  )

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <div className="py-6 max-w-[1400px] mx-auto space-y-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Search className="h-6 w-6" />
            Stock Screener
          </h1>
        </div>
        <p className="text-muted-foreground">
          Find stocks matching your investment criteria
        </p>
      </div>

      {/* Mobile filter toggle */}
      <div className="lg:hidden">
        <Button
          variant="outline"
          className="w-full h-10"
          onClick={() => setShowFilters(!showFilters)}
        >
          <Filter className="h-4 w-4 mr-2" />
          {showFilters ? 'Hide Filters' : 'Show Filters'}
          <ChevronDown
            className={`h-4 w-4 ml-auto transition-transform ${showFilters ? 'rotate-180' : ''}`}
          />
        </Button>

        {showFilters && (
          <div className="mt-4 p-4 border rounded-lg bg-card">{filterPanel}</div>
        )}
      </div>

      {/* Two-panel layout */}
      <div className="flex gap-6">
        {/* Desktop sidebar */}
        <div className="hidden lg:block w-[300px] shrink-0">
          <div className="sticky top-6 border rounded-lg bg-card p-4 max-h-[calc(100vh-120px)] overflow-y-auto">
            {filterPanel}
          </div>
        </div>

        {/* Results */}
        {resultsPanel}
      </div>
    </div>
  )
}
