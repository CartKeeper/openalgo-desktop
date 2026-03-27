import { open } from '@tauri-apps/plugin-shell'
import {
  AlertCircle,
  ArrowLeft,
  ArrowUpDown,
  ExternalLink,
  Landmark,
  RefreshCw,
  Search,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { type CongressionalTrade, providerCommands } from '@/api/tauri-client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'

// ============================================================================
// Types
// ============================================================================

type Chamber = 'senate' | 'house'
type LoadingState = 'idle' | 'loading' | 'success' | 'error'
type SortField = 'date' | 'name' | 'amount' | 'ticker'
type SortDir = 'asc' | 'desc'

// ============================================================================
// Helpers
// ============================================================================

/**
 * Cleans up the amount string from FMP.
 * FMP returns ranges like "$15,001 - $50,000" but sometimes has encoding issues
 * or non-standard characters. Ensures clean USD display.
 */
function formatAmount(amount: string | null): string {
  if (!amount) return '-'
  // Strip any non-printable / weird unicode characters
  let clean = amount.replace(/[^\x20-\x7E]/g, '').trim()
  if (!clean) return '-'
  // If it doesn't start with $ and contains numbers, prefix with $
  if (!clean.startsWith('$') && /\d/.test(clean)) {
    clean = '$' + clean
  }
  return clean
}

function formatDate(dateStr: string | null): string {
  if (!dateStr) return '-'
  const d = new Date(dateStr)
  if (isNaN(d.getTime())) return dateStr
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

function getTransactionColor(type: string | null): string {
  if (!type) return 'text-muted-foreground'
  const lower = type.toLowerCase()
  if (lower.includes('purchase') || lower.includes('buy')) return 'text-green-500'
  if (lower.includes('sale') || lower.includes('sell')) return 'text-red-500'
  if (lower.includes('exchange')) return 'text-yellow-500'
  return 'text-muted-foreground'
}

function getTransactionBadgeVariant(type: string | null): 'default' | 'secondary' | 'destructive' | 'outline' {
  if (!type) return 'secondary'
  const lower = type.toLowerCase()
  if (lower.includes('purchase') || lower.includes('buy')) return 'default'
  if (lower.includes('sale') || lower.includes('sell')) return 'destructive'
  return 'secondary'
}

function sortTrades(trades: CongressionalTrade[], field: SortField, dir: SortDir): CongressionalTrade[] {
  return [...trades].sort((a, b) => {
    let cmp = 0
    switch (field) {
      case 'date':
        cmp = (a.transaction_date || a.date_received || '').localeCompare(b.transaction_date || b.date_received || '')
        break
      case 'name':
        cmp = `${a.last_name || ''} ${a.first_name || ''}`.localeCompare(`${b.last_name || ''} ${b.first_name || ''}`)
        break
      case 'amount':
        cmp = (a.amount || '').localeCompare(b.amount || '')
        break
      case 'ticker':
        cmp = (a.ticker || '').localeCompare(b.ticker || '')
        break
    }
    return dir === 'asc' ? cmp : -cmp
  })
}

// ============================================================================
// Main Component
// ============================================================================

export default function CongressPage() {
  const [chamber, setChamber] = useState<Chamber>('senate')
  const [trades, setTrades] = useState<CongressionalTrade[]>([])
  const [loadingState, setLoadingState] = useState<LoadingState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [nameFilter, setNameFilter] = useState('')
  const [tickerFilter, setTickerFilter] = useState('')
  const [sortField, setSortField] = useState<SortField>('date')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const fetchTrades = useCallback(async (ch: Chamber, name?: string) => {
    setLoadingState('loading')
    setErrorMsg('')
    try {
      let result: CongressionalTrade[]
      if (name && name.trim().length > 0) {
        result = ch === 'senate'
          ? await providerCommands.getSenateTradesByName(name.trim())
          : await providerCommands.getHouseTradesByName(name.trim())
      } else {
        result = ch === 'senate'
          ? await providerCommands.getSenateTrades(0, 25)
          : await providerCommands.getHouseTrades(0, 25)
      }
      setTrades(result)
      setLoadingState('success')
    } catch (err) {
      const msg = err instanceof Error
        ? err.message
        : typeof err === 'object' && err !== null && 'message' in err
          ? String((err as { message: string }).message)
          : String(err)
      setErrorMsg(msg)
      setLoadingState('error')
      toast.error(`Failed to fetch ${ch} trades: ${msg}`)
    }
  }, [])

  useEffect(() => {
    fetchTrades(chamber)
  }, [chamber, fetchTrades])

  const handleSearch = () => {
    fetchTrades(chamber, nameFilter)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleSearch()
  }

  const toggleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortField(field)
      setSortDir('desc')
    }
  }

  // Apply ticker filter and sorting
  const filteredTrades = trades.filter((t) => {
    if (!tickerFilter) return true
    return (t.ticker || '').toLowerCase().includes(tickerFilter.toLowerCase())
  })
  const sortedTrades = sortTrades(filteredTrades, sortField, sortDir)

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold">Congressional Trading</h1>
        </div>
        <p className="text-sm text-muted-foreground">
          Track stock trades disclosed by U.S. Senators and House Representatives.
        </p>
      </div>

      {/* Controls */}
      <div className="flex flex-col sm:flex-row gap-3">
        <Select value={chamber} onValueChange={(v) => setChamber(v as Chamber)}>
          <SelectTrigger className="w-[160px] h-10">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="senate">Senate</SelectItem>
            <SelectItem value="house">House</SelectItem>
          </SelectContent>
        </Select>

        <div className="flex gap-2 flex-1">
          <Input
            placeholder="Search by name (e.g., Pelosi)"
            value={nameFilter}
            onChange={(e) => setNameFilter(e.target.value)}
            onKeyDown={handleKeyDown}
            className="h-10 max-w-xs"
          />
          <Button size="sm" className="h-10" onClick={handleSearch}>
            <Search className="h-4 w-4 mr-1" />
            Search
          </Button>
        </div>

        <Input
          placeholder="Filter by ticker"
          value={tickerFilter}
          onChange={(e) => setTickerFilter(e.target.value)}
          className="h-10 w-[140px]"
        />

        <Button
          variant="outline"
          size="sm"
          className="h-10"
          onClick={() => {
            setNameFilter('')
            setTickerFilter('')
            fetchTrades(chamber)
          }}
        >
          <RefreshCw className="h-4 w-4" />
        </Button>
      </div>

      {/* Results */}
      {loadingState === 'loading' && (
        <div className="space-y-3">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-16 w-full rounded-lg" />
          ))}
        </div>
      )}

      {loadingState === 'error' && (
        <Card className="p-8 text-center">
          <AlertCircle className="h-12 w-12 mx-auto text-muted-foreground mb-3" />
          <p className="text-sm font-medium">Failed to load trades</p>
          <p className="text-xs text-muted-foreground mt-1">{errorMsg}</p>
          <Button variant="outline" size="sm" className="mt-4" onClick={() => fetchTrades(chamber)}>
            Retry
          </Button>
        </Card>
      )}

      {loadingState === 'success' && sortedTrades.length === 0 && (
        <Card className="p-8 text-center">
          <Landmark className="h-12 w-12 mx-auto text-muted-foreground mb-3" />
          <p className="text-sm font-medium">No trades found</p>
          <p className="text-xs text-muted-foreground mt-1">
            {nameFilter ? `No results for "${nameFilter}"` : 'No recent disclosures available.'}
          </p>
        </Card>
      )}

      {loadingState === 'success' && sortedTrades.length > 0 && (
        <>
          <div className="text-xs text-muted-foreground">
            {sortedTrades.length} trade{sortedTrades.length !== 1 ? 's' : ''}
            {nameFilter ? ` matching "${nameFilter}"` : ''}
            {tickerFilter ? ` filtered by "${tickerFilter}"` : ''}
          </div>

          {/* Table */}
          <div className="rounded-lg border overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="h-10 border-b bg-muted/50 text-xs text-muted-foreground uppercase tracking-wider">
                    <th className="px-4 text-left font-semibold cursor-pointer" onClick={() => toggleSort('date')}>
                      <span className="flex items-center gap-1">
                        Date <ArrowUpDown className="h-3 w-3" />
                      </span>
                    </th>
                    <th className="px-4 text-left font-semibold cursor-pointer" onClick={() => toggleSort('name')}>
                      <span className="flex items-center gap-1">
                        Member <ArrowUpDown className="h-3 w-3" />
                      </span>
                    </th>
                    <th className="px-4 text-left font-semibold cursor-pointer" onClick={() => toggleSort('ticker')}>
                      <span className="flex items-center gap-1">
                        Ticker <ArrowUpDown className="h-3 w-3" />
                      </span>
                    </th>
                    <th className="px-4 text-left font-semibold">Asset</th>
                    <th className="px-4 text-left font-semibold">Type</th>
                    <th className="px-4 text-left font-semibold cursor-pointer" onClick={() => toggleSort('amount')}>
                      <span className="flex items-center gap-1">
                        Amount <ArrowUpDown className="h-3 w-3" />
                      </span>
                    </th>
                    <th className="px-4 text-left font-semibold">Owner</th>
                    <th className="px-4 text-right font-semibold">Link</th>
                  </tr>
                </thead>
                <tbody>
                  {sortedTrades.map((trade, idx) => (
                    <tr
                      key={idx}
                      className="h-12 border-b last:border-b-0 hover:bg-muted/30 transition-colors duration-150"
                    >
                      <td className="px-4 whitespace-nowrap font-medium tabular-nums">
                        {formatDate(trade.transaction_date || trade.date_received)}
                      </td>
                      <td className="px-4 whitespace-nowrap font-semibold">
                        {[trade.first_name, trade.last_name].filter(Boolean).join(' ') || '-'}
                        {trade.office && (
                          <span className="text-xs text-muted-foreground ml-1">({trade.office})</span>
                        )}
                      </td>
                      <td className="px-4 whitespace-nowrap">
                        {trade.ticker ? (
                          <Badge variant="outline" className="font-mono text-xs">
                            {trade.ticker}
                          </Badge>
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </td>
                      <td className="px-4 max-w-[200px] truncate" title={trade.asset_description || undefined}>
                        {trade.asset_description || '-'}
                      </td>
                      <td className="px-4 whitespace-nowrap">
                        {trade.transaction_type ? (
                          <Badge variant={getTransactionBadgeVariant(trade.transaction_type)} className="text-xs">
                            <span className={getTransactionColor(trade.transaction_type)}>
                              {trade.transaction_type}
                            </span>
                          </Badge>
                        ) : (
                          '-'
                        )}
                      </td>
                      <td className="px-4 whitespace-nowrap tabular-nums">
                        {formatAmount(trade.amount)}
                      </td>
                      <td className="px-4 whitespace-nowrap text-muted-foreground">
                        {trade.owner || '-'}
                      </td>
                      <td className="px-4 text-right">
                        {trade.link && (
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-8 w-8 p-0"
                            onClick={() => open(trade.link!)}
                          >
                            <ExternalLink className="h-3.5 w-3.5" />
                          </Button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

        </>
      )}
    </div>
  )
}
