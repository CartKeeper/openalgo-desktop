import {
  ArrowLeft,
  Calendar,
  AlertCircle,
  Search,
  Loader2,
  TrendingUp,
  RefreshCw,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { providerCommands } from '@/api/tauri-client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

// ============================================================================
// Types
// ============================================================================

interface EconomicEvent {
  date: string
  country: string
  event: string
  impact: string
  actual: string | number | null
  estimate: string | number | null
  previous: string | number | null
  currency: string
}

interface FredSeriesResult {
  id: string
  title: string
  frequency: string
  units: string
  seasonal_adjustment: string
  last_updated: string
  observation_start: string
  observation_end: string
  popularity: number
}

interface FredObservation {
  date: string
  value: number | null
}

// ============================================================================
// Helpers
// ============================================================================

function formatDateString(iso: string): string {
  try {
    const d = new Date(iso)
    if (isNaN(d.getTime())) return iso
    return d.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  } catch {
    return iso
  }
}

function formatTimeString(iso: string): string {
  try {
    const d = new Date(iso)
    if (isNaN(d.getTime())) return ''
    const hours = d.getUTCHours()
    const minutes = d.getUTCMinutes()
    if (hours === 0 && minutes === 0) return ''
    const h = hours % 12 || 12
    const ampm = hours >= 12 ? 'PM' : 'AM'
    return `${h}:${String(minutes).padStart(2, '0')} ${ampm}`
  } catch {
    return ''
  }
}

function formatNumber(val: string | number | null | undefined): string {
  if (val === null || val === undefined || val === '') return '-'
  const num = typeof val === 'string' ? parseFloat(val) : val
  if (isNaN(num)) return String(val)
  if (Math.abs(num) >= 1_000_000) return `${(num / 1_000_000).toFixed(2)}M`
  if (Math.abs(num) >= 1_000) return `${(num / 1_000).toFixed(2)}K`
  if (Number.isInteger(num)) return num.toLocaleString()
  return num.toFixed(2)
}

function toInputDate(date: Date): string {
  return date.toISOString().split('T')[0]
}

function hasMajorDifference(
  actual: string | number | null | undefined,
  estimate: string | number | null | undefined
): boolean {
  if (actual === null || actual === undefined || actual === '') return false
  if (estimate === null || estimate === undefined || estimate === '') return false
  const a = typeof actual === 'string' ? parseFloat(actual) : actual
  const e = typeof estimate === 'string' ? parseFloat(estimate) : estimate
  if (isNaN(a) || isNaN(e) || e === 0) return false
  return Math.abs((a - e) / e) > 0.1
}

function normalizeImpact(raw: string): 'High' | 'Medium' | 'Low' {
  const lower = (raw || '').toLowerCase()
  if (lower.includes('high')) return 'High'
  if (lower.includes('medium') || lower.includes('med')) return 'Medium'
  return 'Low'
}

// ============================================================================
// SVG Line Chart
// ============================================================================

interface LineChartProps {
  data: FredObservation[]
  width?: number
  height?: number
}

function LineChart({ data, width = 800, height = 300 }: LineChartProps) {
  const filtered = useMemo(
    () => data.filter((d) => d.value !== null) as { date: string; value: number }[],
    [data]
  )

  if (filtered.length === 0) {
    return (
      <div className="flex items-center justify-center h-[300px] text-muted-foreground text-sm">
        No data points available
      </div>
    )
  }

  const padding = { top: 20, right: 20, bottom: 40, left: 70 }
  const chartW = width - padding.left - padding.right
  const chartH = height - padding.top - padding.bottom

  const values = filtered.map((d) => d.value)
  const minVal = Math.min(...values)
  const maxVal = Math.max(...values)
  const valRange = maxVal - minVal || 1

  const xScale = (i: number) => padding.left + (i / Math.max(filtered.length - 1, 1)) * chartW
  const yScale = (v: number) => padding.top + chartH - ((v - minVal) / valRange) * chartH

  const points = filtered.map((d, i) => `${xScale(i)},${yScale(d.value)}`).join(' ')

  // Area fill path
  const areaPath = filtered.length > 1
    ? `M${xScale(0)},${yScale(filtered[0].value)} ` +
      filtered.map((d, i) => `L${xScale(i)},${yScale(d.value)}`).join(' ') +
      ` L${xScale(filtered.length - 1)},${padding.top + chartH} L${xScale(0)},${padding.top + chartH} Z`
    : ''

  // Y-axis labels (5 ticks)
  const yTicks = Array.from({ length: 5 }, (_, i) => {
    const val = minVal + (valRange * i) / 4
    return { y: yScale(val), label: formatNumber(val) }
  })

  // X-axis labels (up to 6 dates)
  const xTickCount = Math.min(6, filtered.length)
  const xTicks = Array.from({ length: xTickCount }, (_, i) => {
    const idx = Math.round((i / Math.max(xTickCount - 1, 1)) * (filtered.length - 1))
    return { x: xScale(idx), label: formatDateString(filtered[idx].date) }
  })

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="w-full h-auto"
      preserveAspectRatio="xMidYMid meet"
    >
      {/* Grid lines */}
      {yTicks.map((tick, i) => (
        <line
          key={`grid-${i}`}
          x1={padding.left}
          y1={tick.y}
          x2={width - padding.right}
          y2={tick.y}
          className="stroke-border"
          strokeWidth={0.5}
          strokeDasharray="4 4"
        />
      ))}

      {/* Area fill */}
      {areaPath && (
        <path d={areaPath} className="fill-primary/10" />
      )}

      {/* Line */}
      <polyline
        points={points}
        fill="none"
        className="stroke-primary"
        strokeWidth={2}
        strokeLinejoin="round"
        strokeLinecap="round"
      />

      {/* Y-axis labels */}
      {yTicks.map((tick, i) => (
        <text
          key={`y-${i}`}
          x={padding.left - 8}
          y={tick.y + 4}
          textAnchor="end"
          className="fill-muted-foreground text-[10px] font-mono"
        >
          {tick.label}
        </text>
      ))}

      {/* X-axis labels */}
      {xTicks.map((tick, i) => (
        <text
          key={`x-${i}`}
          x={tick.x}
          y={height - 8}
          textAnchor="middle"
          className="fill-muted-foreground text-[10px]"
        >
          {tick.label}
        </text>
      ))}
    </svg>
  )
}

// ============================================================================
// Events Tab
// ============================================================================

function EventsTab() {
  const today = new Date()
  const nextWeek = new Date(today)
  nextWeek.setDate(today.getDate() + 7)

  const [fromDate, setFromDate] = useState(toInputDate(today))
  const [toDate, setToDate] = useState(toInputDate(nextWeek))
  const [impactFilter, setImpactFilter] = useState<string>('all')
  const [countryFilter, setCountryFilter] = useState('')
  const [events, setEvents] = useState<EconomicEvent[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [fetched, setFetched] = useState(false)

  const fetchEvents = useCallback(async () => {
    if (!fromDate || !toDate) {
      toast.error('Please select both from and to dates')
      return
    }
    setLoading(true)
    setError(null)
    try {
      const result = await providerCommands.getEconomicCalendar(fromDate, toDate)
      setEvents(result as EconomicEvent[])
      setFetched(true)
      if (result.length === 0) {
        toast.info('No events found for the selected date range')
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.toLowerCase().includes('api key') || msg.toLowerCase().includes('unauthorized')) {
        setError('api_key')
      } else {
        setError(msg)
      }
      toast.error('Failed to fetch economic calendar')
    } finally {
      setLoading(false)
    }
  }, [fromDate, toDate])

  const filteredEvents = useMemo(() => {
    let filtered = [...events]

    if (impactFilter !== 'all') {
      filtered = filtered.filter(
        (e) => normalizeImpact(e.impact) === impactFilter
      )
    }

    if (countryFilter.trim()) {
      const query = countryFilter.toLowerCase().trim()
      filtered = filtered.filter(
        (e) =>
          (e.country || '').toLowerCase().includes(query) ||
          (e.currency || '').toLowerCase().includes(query)
      )
    }

    filtered.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())

    return filtered
  }, [events, impactFilter, countryFilter])

  if (error === 'api_key') {
    return (
      <Card>
        <CardContent className="pt-6">
          <div className="flex flex-col items-center justify-center gap-4 py-12">
            <AlertCircle className="h-12 w-12 text-muted-foreground" />
            <p className="text-muted-foreground text-sm">
              An FMP API key is required for the economic calendar.
            </p>
            <Link to="/generic-setup">
              <Button variant="outline" className="h-10">Configure API Key</Button>
            </Link>
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-4">
      {/* Filters */}
      <Card>
        <CardContent className="pt-6">
          <div className="flex flex-wrap items-end gap-4">
            <div className="space-y-1.5">
              <Label className="text-xs font-semibold">From</Label>
              <Input
                type="date"
                value={fromDate}
                onChange={(e) => setFromDate(e.target.value)}
                className="h-10 w-[160px] font-mono text-sm"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs font-semibold">To</Label>
              <Input
                type="date"
                value={toDate}
                onChange={(e) => setToDate(e.target.value)}
                className="h-10 w-[160px] font-mono text-sm"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs font-semibold">Impact</Label>
              <Select value={impactFilter} onValueChange={setImpactFilter}>
                <SelectTrigger className="h-10 w-[140px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All</SelectItem>
                  <SelectItem value="High">High</SelectItem>
                  <SelectItem value="Medium">Medium</SelectItem>
                  <SelectItem value="Low">Low</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs font-semibold">Country</Label>
              <Input
                placeholder="e.g. US, EU, JP"
                value={countryFilter}
                onChange={(e) => setCountryFilter(e.target.value)}
                className="h-10 w-[160px] text-sm"
              />
            </div>
            <Button
              onClick={fetchEvents}
              disabled={loading}
              className="h-10"
            >
              {loading ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="h-4 w-4" />
              )}
              <span className="ml-2">Fetch</span>
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Results */}
      <Card>
        <CardContent className="pt-6">
          {loading ? (
            <div className="space-y-3">
              {Array.from({ length: 8 }).map((_, i) => (
                <Skeleton key={i} className="h-12 w-full" />
              ))}
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center gap-4 py-12">
              <AlertCircle className="h-12 w-12 text-destructive" />
              <p className="text-muted-foreground text-sm">{error}</p>
              <Button variant="outline" onClick={fetchEvents} className="h-10">
                Retry
              </Button>
            </div>
          ) : !fetched ? (
            <div className="flex flex-col items-center justify-center gap-2 py-12">
              <Calendar className="h-12 w-12 text-muted-foreground" />
              <p className="text-muted-foreground text-sm">
                Select a date range and click Fetch to load events
              </p>
            </div>
          ) : filteredEvents.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-12">
              <Calendar className="h-12 w-12 text-muted-foreground" />
              <p className="text-muted-foreground text-sm">No events found</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Time</TableHead>
                    <TableHead>Event</TableHead>
                    <TableHead>Country</TableHead>
                    <TableHead>Impact</TableHead>
                    <TableHead className="text-right font-mono">Actual</TableHead>
                    <TableHead className="text-right font-mono">Estimate</TableHead>
                    <TableHead className="text-right font-mono">Previous</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredEvents.map((evt, i) => {
                    const impact = normalizeImpact(evt.impact)
                    const highlight = hasMajorDifference(evt.actual, evt.estimate)
                    return (
                      <TableRow
                        key={`${evt.date}-${evt.event}-${i}`}
                        className={highlight ? 'bg-yellow-500/5' : ''}
                      >
                        <TableCell className="font-mono text-xs">
                          {formatDateString(evt.date)}
                        </TableCell>
                        <TableCell className="font-mono text-xs text-muted-foreground">
                          {formatTimeString(evt.date)}
                        </TableCell>
                        <TableCell className="max-w-[280px] truncate font-medium">
                          {evt.event}
                        </TableCell>
                        <TableCell className="text-xs">
                          {evt.country}
                          {evt.currency ? (
                            <span className="ml-1 text-muted-foreground">({evt.currency})</span>
                          ) : null}
                        </TableCell>
                        <TableCell>
                          <ImpactBadge impact={impact} />
                        </TableCell>
                        <TableCell
                          className={`text-right font-mono text-sm tabular-nums ${
                            highlight ? 'text-yellow-600 dark:text-yellow-400 font-semibold' : ''
                          }`}
                        >
                          {formatNumber(evt.actual)}
                        </TableCell>
                        <TableCell className="text-right font-mono text-sm tabular-nums">
                          {formatNumber(evt.estimate)}
                        </TableCell>
                        <TableCell className="text-right font-mono text-sm tabular-nums text-muted-foreground">
                          {formatNumber(evt.previous)}
                        </TableCell>
                      </TableRow>
                    )
                  })}
                </TableBody>
              </Table>
              <p className="mt-3 text-xs text-muted-foreground">
                {filteredEvents.length} event{filteredEvents.length !== 1 ? 's' : ''}
                {events.length !== filteredEvents.length
                  ? ` (filtered from ${events.length})`
                  : ''}
              </p>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

function ImpactBadge({ impact }: { impact: 'High' | 'Medium' | 'Low' }) {
  if (impact === 'High') {
    return (
      <Badge variant="destructive" className="text-[10px]">
        High
      </Badge>
    )
  }
  if (impact === 'Medium') {
    return (
      <Badge className="border-transparent bg-yellow-500/20 text-yellow-700 dark:text-yellow-400 text-[10px]">
        Medium
      </Badge>
    )
  }
  return (
    <Badge className="border-transparent bg-green-500/20 text-green-700 dark:text-green-400 text-[10px]">
      Low
    </Badge>
  )
}

// ============================================================================
// FRED Data Tab
// ============================================================================

function FredTab() {
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<FredSeriesResult[]>([])
  const [searching, setSearching] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)

  const [selectedSeries, setSelectedSeries] = useState<FredSeriesResult | null>(null)
  const [observations, setObservations] = useState<FredObservation[]>([])
  const [loadingSeries, setLoadingSeries] = useState(false)
  const [seriesError, setSeriesError] = useState<string | null>(null)

  const twoYearsAgo = new Date()
  twoYearsAgo.setFullYear(twoYearsAgo.getFullYear() - 2)
  const [obsStart, setObsStart] = useState(toInputDate(twoYearsAgo))
  const [obsEnd, setObsEnd] = useState(toInputDate(new Date()))

  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) return
    setSearching(true)
    setSearchError(null)
    try {
      const results = await providerCommands.searchFredSeries(searchQuery.trim(), 20)
      setSearchResults(results as FredSeriesResult[])
      if (results.length === 0) {
        toast.info('No FRED series found for that query')
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.toLowerCase().includes('api key') || msg.toLowerCase().includes('unauthorized')) {
        setSearchError('api_key')
      } else {
        setSearchError(msg)
      }
      toast.error('Failed to search FRED series')
    } finally {
      setSearching(false)
    }
  }, [searchQuery])

  const handleSelectSeries = useCallback(
    async (series: FredSeriesResult) => {
      setSelectedSeries(series)
      setLoadingSeries(true)
      setSeriesError(null)
      try {
        const data = await providerCommands.getFredSeries(
          series.id,
          obsStart || undefined,
          obsEnd || undefined
        )
        setObservations(data as FredObservation[])
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err)
        setSeriesError(msg)
        toast.error('Failed to load series data')
      } finally {
        setLoadingSeries(false)
      }
    },
    [obsStart, obsEnd]
  )

  const reloadSeries = useCallback(() => {
    if (selectedSeries) {
      handleSelectSeries(selectedSeries)
    }
  }, [selectedSeries, handleSelectSeries])

  // Reload data when date range changes and a series is selected
  useEffect(() => {
    if (selectedSeries) {
      handleSelectSeries(selectedSeries)
    }
    // Only reload when dates change, not when handleSelectSeries reference changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [obsStart, obsEnd])

  if (searchError === 'api_key') {
    return (
      <Card>
        <CardContent className="pt-6">
          <div className="flex flex-col items-center justify-center gap-4 py-12">
            <AlertCircle className="h-12 w-12 text-muted-foreground" />
            <p className="text-muted-foreground text-sm">
              A FRED API key is required to search economic data.
            </p>
            <Link to="/generic-setup">
              <Button variant="outline" className="h-10">Configure API Key</Button>
            </Link>
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-4">
      {/* Search */}
      <Card>
        <CardContent className="pt-6">
          <div className="flex items-end gap-4">
            <div className="flex-1 space-y-1.5">
              <Label className="text-xs font-semibold">Search FRED Series</Label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder='e.g. GDP, CPI, unemployment rate, federal funds'
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleSearch()
                  }}
                  className="h-10 pl-9 text-sm"
                />
              </div>
            </div>
            <Button onClick={handleSearch} disabled={searching || !searchQuery.trim()} className="h-10">
              {searching ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Search className="h-4 w-4" />
              )}
              <span className="ml-2">Search</span>
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-[380px_1fr] gap-4">
        {/* Search Results */}
        <Card className="max-h-[600px] flex flex-col">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-semibold">
              {searchResults.length > 0
                ? `Results (${searchResults.length})`
                : 'Search Results'}
            </CardTitle>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto pb-4">
            {searching ? (
              <div className="space-y-3">
                {Array.from({ length: 6 }).map((_, i) => (
                  <Skeleton key={i} className="h-16 w-full" />
                ))}
              </div>
            ) : searchError ? (
              <div className="flex flex-col items-center gap-2 py-8">
                <AlertCircle className="h-8 w-8 text-destructive" />
                <p className="text-xs text-muted-foreground text-center">{searchError}</p>
              </div>
            ) : searchResults.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-8">
                <Search className="h-8 w-8 text-muted-foreground" />
                <p className="text-xs text-muted-foreground text-center">
                  Search for economic data series
                </p>
              </div>
            ) : (
              <div className="space-y-1">
                {searchResults.map((series) => (
                  <button
                    key={series.id}
                    type="button"
                    onClick={() => handleSelectSeries(series)}
                    className={`w-full text-left rounded-lg border px-3 py-2.5 transition-colors cursor-pointer ${
                      selectedSeries?.id === series.id
                        ? 'border-primary bg-primary/5'
                        : 'border-transparent hover:bg-muted/50'
                    }`}
                  >
                    <p className="text-sm font-medium leading-tight truncate">
                      {series.title}
                    </p>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-[10px] font-semibold text-muted-foreground font-mono">
                        {series.id}
                      </span>
                      <span className="text-[10px] text-muted-foreground">
                        {series.frequency}
                      </span>
                      {series.last_updated && (
                        <span className="text-[10px] text-muted-foreground">
                          Updated {formatDateString(series.last_updated)}
                        </span>
                      )}
                    </div>
                  </button>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {/* Chart / Data View */}
        <Card className="flex flex-col">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-4">
              <CardTitle className="text-sm font-semibold truncate">
                {selectedSeries ? selectedSeries.title : 'Series Data'}
              </CardTitle>
              {selectedSeries && (
                <div className="flex items-center gap-2 shrink-0">
                  <Input
                    type="date"
                    value={obsStart}
                    onChange={(e) => setObsStart(e.target.value)}
                    className="h-8 w-[130px] font-mono text-xs"
                  />
                  <span className="text-xs text-muted-foreground">to</span>
                  <Input
                    type="date"
                    value={obsEnd}
                    onChange={(e) => setObsEnd(e.target.value)}
                    className="h-8 w-[130px] font-mono text-xs"
                  />
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={reloadSeries}
                    disabled={loadingSeries}
                    className="h-8 w-8 p-0"
                  >
                    {loadingSeries ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="h-3.5 w-3.5" />
                    )}
                  </Button>
                </div>
              )}
            </div>
            {selectedSeries && (
              <div className="flex items-center gap-3 mt-1">
                <span className="text-[10px] font-mono text-muted-foreground">
                  {selectedSeries.id}
                </span>
                <span className="text-[10px] text-muted-foreground">
                  {selectedSeries.units}
                </span>
                <span className="text-[10px] text-muted-foreground">
                  {selectedSeries.seasonal_adjustment}
                </span>
              </div>
            )}
          </CardHeader>
          <CardContent className="flex-1 pb-4">
            {loadingSeries ? (
              <div className="space-y-3">
                <Skeleton className="h-[300px] w-full" />
              </div>
            ) : seriesError ? (
              <div className="flex flex-col items-center gap-2 py-12">
                <AlertCircle className="h-8 w-8 text-destructive" />
                <p className="text-xs text-muted-foreground text-center">{seriesError}</p>
                <Button variant="outline" size="sm" onClick={reloadSeries} className="h-8">
                  Retry
                </Button>
              </div>
            ) : !selectedSeries ? (
              <div className="flex flex-col items-center justify-center gap-2 py-12">
                <TrendingUp className="h-12 w-12 text-muted-foreground" />
                <p className="text-muted-foreground text-sm">
                  Select a series from the search results to view data
                </p>
              </div>
            ) : observations.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-2 py-12">
                <TrendingUp className="h-12 w-12 text-muted-foreground" />
                <p className="text-muted-foreground text-sm">
                  No observations available for this period
                </p>
              </div>
            ) : (
              <div>
                <LineChart data={observations} />
                <div className="flex items-center justify-between mt-3 text-xs text-muted-foreground">
                  <span>{observations.length} observations</span>
                  <span className="font-mono tabular-nums">
                    Latest: {formatNumber(observations[observations.length - 1]?.value ?? null)}
                    {' '}
                    ({formatDateString(observations[observations.length - 1]?.date ?? '')})
                  </span>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

// ============================================================================
// Main Page
// ============================================================================

export default function CalendarPage() {
  return (
    <div className="py-6 max-w-6xl mx-auto space-y-6 px-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Calendar className="h-6 w-6" />
            Economic Calendar
          </h1>
        </div>
        <p className="text-muted-foreground text-sm">
          Track economic events and explore FRED data series
        </p>
      </div>

      {/* Tabs */}
      <Tabs defaultValue="events">
        <TabsList>
          <TabsTrigger value="events">Events</TabsTrigger>
          <TabsTrigger value="fred">FRED Data</TabsTrigger>
        </TabsList>

        <TabsContent value="events">
          <EventsTab />
        </TabsContent>

        <TabsContent value="fred">
          <FredTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
