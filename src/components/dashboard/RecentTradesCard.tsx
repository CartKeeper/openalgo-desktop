import { ArrowUpRight, History, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { type Trade, tradingApi } from '@/api/trading'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { cn } from '@/lib/utils'
import { formatUSD } from '@/lib/format'

const POLL_MS = 15000
const MAX_ROWS = 8

function formatTime(ts: string | undefined): string {
  if (!ts) return '--'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ts
  return d.toLocaleTimeString('en-US', {
    timeZone: 'America/New_York',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function RecentTradesCard() {
  const [trades, setTrades] = useState<Trade[]>([])
  const [isLoading, setIsLoading] = useState(true)

  const fetchTrades = useCallback(async () => {
    try {
      const res = await tradingApi.getTrades('')
      if (res.status === 'success' && res.data) {
        const sorted = [...res.data].sort((a, b) =>
          (b.timestamp || '').localeCompare(a.timestamp || '')
        )
        setTrades(sorted.slice(0, MAX_ROWS))
      }
    } catch {
      // transient; keep last known
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchTrades()
    const id = setInterval(fetchTrades, POLL_MS)
    return () => clearInterval(id)
  }, [fetchTrades])

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <History className="h-4 w-4 text-primary" />
          Recent Trades
          <Badge variant="secondary" className="ml-1 tabular-nums">
            {trades.length}
          </Badge>
        </CardTitle>
        <Link
          to="/tradebook"
          className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          Trade Book <ArrowUpRight className="h-3.5 w-3.5" />
        </Link>
      </CardHeader>
      <CardContent className="p-0">
        {isLoading && trades.length === 0 ? (
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : trades.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-center text-muted-foreground">
            <History className="mb-2 h-8 w-8 opacity-30" />
            <p className="text-sm">No fills yet today</p>
          </div>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Time</TableHead>
                <TableHead>Symbol</TableHead>
                <TableHead>Side</TableHead>
                <TableHead className="text-right">Qty</TableHead>
                <TableHead className="text-right">Price</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {trades.map((t, i) => (
                <TableRow key={`${t.orderid}-${i}`}>
                  <TableCell className="tabular-nums text-muted-foreground">
                    {formatTime(t.timestamp)}
                  </TableCell>
                  <TableCell className="font-semibold">{t.symbol}</TableCell>
                  <TableCell>
                    <Badge
                      variant="outline"
                      className={cn(
                        'text-[10px]',
                        t.action === 'BUY'
                          ? 'border-green-500/40 text-green-600 dark:text-green-400'
                          : 'border-red-500/40 text-red-600 dark:text-red-400'
                      )}
                    >
                      {t.action}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{t.quantity}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatUSD(t.average_price)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  )
}
