import { ArrowUpRight, Briefcase, Loader2 } from 'lucide-react'
import { Link } from 'react-router-dom'
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
import type { EnrichedPosition } from '@/hooks/useLivePositions'
import { cn } from '@/lib/utils'
import { formatSignedUSD, formatUSD, pnlColorClass } from '@/lib/format'

interface LivePositionsCardProps {
  positions: EnrichedPosition[]
  isLoading: boolean
  isLive: boolean
}

export function LivePositionsCard({ positions, isLoading, isLive }: LivePositionsCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Briefcase className="h-4 w-4 text-primary" />
          Open Positions
          <Badge variant="secondary" className="ml-1 tabular-nums">
            {positions.length}
          </Badge>
          {isLive && (
            <span className="flex items-center gap-1.5 text-xs font-normal text-green-600 dark:text-green-400">
              <span className="relative flex h-2 w-2">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-500 opacity-75" />
                <span className="relative inline-flex h-2 w-2 rounded-full bg-green-500" />
              </span>
              Live
            </span>
          )}
        </CardTitle>
        <Link
          to="/positions"
          className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          Manage <ArrowUpRight className="h-3.5 w-3.5" />
        </Link>
      </CardHeader>
      <CardContent className="p-0">
        {isLoading && positions.length === 0 ? (
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : positions.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-center text-muted-foreground">
            <Briefcase className="mb-2 h-8 w-8 opacity-30" />
            <p className="text-sm">No open positions</p>
          </div>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Symbol</TableHead>
                <TableHead className="text-right">Qty</TableHead>
                <TableHead className="text-right">Avg</TableHead>
                <TableHead className="text-right">Last</TableHead>
                <TableHead className="text-right">Unrealized P&L</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {positions.map((p) => {
                const cost = p.average_price * Math.abs(p.quantity)
                const pnlPct = cost > 0 ? (p.liveUnrealized / cost) * 100 : 0
                return (
                  <TableRow key={`${p.exchange}:${p.symbol}`}>
                    <TableCell className="font-semibold">
                      <div className="flex items-center gap-2">
                        {p.symbol}
                        <Badge
                          variant="outline"
                          className={cn(
                            'text-[10px]',
                            p.quantity >= 0
                              ? 'border-green-500/40 text-green-600 dark:text-green-400'
                              : 'border-red-500/40 text-red-600 dark:text-red-400'
                          )}
                        >
                          {p.quantity >= 0 ? 'LONG' : 'SHORT'}
                        </Badge>
                      </div>
                    </TableCell>
                    <TableCell className="text-right tabular-nums">{p.quantity}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatUSD(p.average_price)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">{formatUSD(p.liveLtp)}</TableCell>
                    <TableCell
                      className={cn('text-right tabular-nums font-medium', pnlColorClass(p.liveUnrealized))}
                    >
                      {formatSignedUSD(p.liveUnrealized)}
                      <span className="ml-1 text-xs opacity-80">
                        ({pnlPct >= 0 ? '+' : ''}
                        {pnlPct.toFixed(2)}%)
                      </span>
                    </TableCell>
                  </TableRow>
                )
              })}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  )
}
