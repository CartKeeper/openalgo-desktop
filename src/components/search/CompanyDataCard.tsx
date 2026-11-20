import { Download, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  type MetadataStatus,
  type SymbolMetadataRow,
  symbolMetadataApi,
} from '@/api/symbolMetadata'
import type { HelpItem } from '@/components/HelpButton'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

/** Help-panel entries for the page that hosts this card. */
export const COMPANY_DATA_HELP_ITEMS: HelpItem[] = [
  {
    title: 'Download symbol metadata',
    description:
      'Downloads sector, industry, country and identifiers (ISIN, CUSIP, FIGI) for every US-listed equity and ETF from FinanceDatabase (MIT), pinned to one commit so results are reproducible. The data is stored locally and survives restarts; download again only when you want a newer snapshot.',
  },
  {
    title: 'Include OTC (pink sheets)',
    description:
      'Adds the 14.5 MB pink-sheet file to the download. Check it when a client holds OTC names; otherwise leave it off.',
  },
  {
    title: 'Company data',
    description:
      'As you type a symbol or company name, matching companies appear: exact and prefix symbol matches first, then name matches. A symbol listed on two exchanges shows one row per exchange. Delisted symbols are hidden.',
  },
]

const SEARCH_DEBOUNCE_MS = 300

const DATA_DATE_FORMAT = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  day: 'numeric',
  year: 'numeric',
  timeZone: 'UTC',
})

const REFRESH_TIME_FORMAT = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  day: 'numeric',
  year: 'numeric',
  hour: 'numeric',
  minute: '2-digit',
  hour12: true,
  timeZone: 'America/New_York',
  timeZoneName: 'short',
})

/** "2026-11-08" → "Nov 8, 2026" (a calendar date, so no time-zone shift). */
function formatDataDate(isoDate: string): string {
  const date = new Date(`${isoDate}T00:00:00Z`)
  return Number.isNaN(date.getTime()) ? isoDate : DATA_DATE_FORMAT.format(date)
}

/** UTC ISO timestamp → Eastern time, 12-hour, e.g. "Nov 12, 2026, 3:30 PM EST". */
function formatRefreshTime(isoTimestamp: string): string {
  const date = new Date(isoTimestamp)
  return Number.isNaN(date.getTime()) ? isoTimestamp : REFRESH_TIME_FORMAT.format(date)
}

function statusLine(status: MetadataStatus): string {
  if (!status.last_refresh) return 'Not downloaded yet'
  return `${status.rows.toLocaleString('en-US')} symbols · FinanceDatabase ${status.source_commit} (data as of ${formatDataDate(status.source_as_of)}) · downloaded ${formatRefreshTime(status.last_refresh)}`
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

interface CompanyDataCardProps {
  /** Live search text; the results table shows once it has one character. */
  query: string
}

export function CompanyDataCard({ query }: CompanyDataCardProps) {
  const [status, setStatus] = useState<MetadataStatus | null>(null)
  const [statusLoading, setStatusLoading] = useState(true)
  const [statusError, setStatusError] = useState<string | null>(null)
  const [includeOtc, setIncludeOtc] = useState(false)
  const [downloading, setDownloading] = useState(false)

  const [results, setResults] = useState<SymbolMetadataRow[]>([])
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)
  const latestSearch = useRef(0)

  const trimmedQuery = query.trim()

  const loadStatus = useCallback(async () => {
    setStatusLoading(true)
    setStatusError(null)
    try {
      setStatus(await symbolMetadataApi.status())
    } catch (error) {
      setStatusError(errorMessage(error))
    } finally {
      setStatusLoading(false)
    }
  }, [])

  useEffect(() => {
    void loadStatus()
  }, [loadStatus])

  // The newest request wins: a slow earlier response never overwrites a newer one.
  const performSearch = useCallback(async (text: string) => {
    const requestId = ++latestSearch.current
    setSearchLoading(true)
    setSearchError(null)
    try {
      const rows = await symbolMetadataApi.search(text)
      if (requestId !== latestSearch.current) return
      setResults(rows)
    } catch (error) {
      if (requestId !== latestSearch.current) return
      setSearchError(errorMessage(error))
    } finally {
      if (requestId === latestSearch.current) setSearchLoading(false)
    }
  }, [])

  // Search as the user types, debounced.
  useEffect(() => {
    if (trimmedQuery.length === 0) {
      latestSearch.current += 1
      setResults([])
      setSearchError(null)
      setSearchLoading(false)
      return
    }
    setSearchLoading(true)
    setSearchError(null)
    const timer = setTimeout(() => void performSearch(trimmedQuery), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [trimmedQuery, performSearch])

  const download = async () => {
    setDownloading(true)
    try {
      const next = await symbolMetadataApi.refresh(includeOtc)
      setStatus(next)
      setStatusError(null)
      toast.success(`Symbol metadata downloaded: ${next.rows.toLocaleString('en-US')} symbols`)
      if (trimmedQuery.length > 0) void performSearch(trimmedQuery)
    } catch (error) {
      toast.error(`Symbol metadata download failed: ${errorMessage(error)}`)
    } finally {
      setDownloading(false)
    }
  }

  const retrySearch = () => void performSearch(trimmedQuery)

  return (
    <Card data-testid="company-data-card">
      <CardHeader>
        <CardTitle>Company data</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {statusLoading ? (
          <Skeleton className="h-5 w-3/4" data-testid="company-data-status-skeleton" />
        ) : statusError ? (
          <div
            role="alert"
            className="flex flex-wrap items-center justify-between gap-4 rounded-lg border border-destructive p-4 text-sm"
          >
            <span>Something went wrong loading the symbol metadata status.</span>
            <Button variant="outline" size="sm" onClick={() => void loadStatus()}>
              Retry
            </Button>
          </div>
        ) : status ? (
          <p data-testid="company-data-status" className="text-sm text-muted-foreground">
            {statusLine(status)}
          </p>
        ) : null}

        <div className="flex flex-wrap items-center gap-4">
          <Button
            variant="secondary"
            className="h-10 px-4 has-[>svg]:px-4"
            onClick={download}
            disabled={downloading}
            aria-busy={downloading}
          >
            {downloading ? (
              <Loader2 className="h-4 w-4 animate-spin" data-testid="download-spinner" />
            ) : (
              <Download className="h-4 w-4" />
            )}
            Download symbol metadata
          </Button>
          <div className="flex items-center gap-2">
            <Checkbox
              id="company-data-include-otc"
              checked={includeOtc}
              onCheckedChange={(checked) => setIncludeOtc(checked === true)}
              disabled={downloading}
            />
            <Label htmlFor="company-data-include-otc" className="text-sm font-normal">
              Include OTC (pink sheets)
            </Label>
          </div>
        </div>

        {trimmedQuery.length >= 1 &&
          (searchError ? (
            <div
              role="alert"
              className="flex flex-wrap items-center justify-between gap-4 rounded-lg border border-destructive p-4 text-sm"
            >
              <span>Something went wrong searching company data.</span>
              <Button variant="outline" size="sm" onClick={retrySearch}>
                Retry
              </Button>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Symbol</TableHead>
                    <TableHead>Name</TableHead>
                    <TableHead>Exchange</TableHead>
                    <TableHead>Sector</TableHead>
                    <TableHead>Industry</TableHead>
                    <TableHead>Country</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {searchLoading ? (
                    [0, 1, 2].map((i) => (
                      <TableRow key={i} data-testid="company-data-skeleton-row">
                        {[0, 1, 2, 3, 4, 5].map((j) => (
                          <TableCell key={j}>
                            <Skeleton className="h-4 w-full" />
                          </TableCell>
                        ))}
                      </TableRow>
                    ))
                  ) : results.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={6} className="py-8 text-center text-muted-foreground">
                        No company data matches
                      </TableCell>
                    </TableRow>
                  ) : (
                    results.map((row) => (
                      <TableRow
                        key={`${row.symbol}-${row.exchange}`}
                        data-testid={`company-data-row-${row.symbol}-${row.exchange}`}
                      >
                        <TableCell className="font-semibold">{row.symbol}</TableCell>
                        <TableCell className="max-w-[280px] truncate text-muted-foreground">
                          {row.name}
                        </TableCell>
                        <TableCell>
                          <Badge variant="outline">{row.exchange}</Badge>
                        </TableCell>
                        <TableCell className="max-w-[200px] truncate">{row.sector ?? '—'}</TableCell>
                        <TableCell className="max-w-[200px] truncate">{row.industry ?? '—'}</TableCell>
                        <TableCell className="max-w-[160px] truncate">{row.country ?? '—'}</TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          ))}

        <p className="text-xs text-muted-foreground">Data: FinanceDatabase (MIT)</p>
      </CardContent>
    </Card>
  )
}
