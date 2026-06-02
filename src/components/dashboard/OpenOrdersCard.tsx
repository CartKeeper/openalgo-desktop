import { ArrowUpRight, ClipboardList, Loader2, Pencil, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import type { Order } from '@/api/tauri-client'
import { tradingApi } from '@/api/trading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
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

const POLL_MS = 7000

export function OpenOrdersCard() {
  const navigate = useNavigate()
  const [orders, setOrders] = useState<Order[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [cancelling, setCancelling] = useState<string | null>(null)

  const fetchOrders = useCallback(async () => {
    try {
      const res = await tradingApi.getOrders('')
      if (res.status === 'success' && res.data) {
        setOrders(
          res.data.orders.filter(
            (o) => o.order_status === 'open' || o.order_status === 'trigger pending'
          )
        )
      }
    } catch {
      // transient; keep last known orders
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchOrders()
    const id = setInterval(fetchOrders, POLL_MS)
    return () => clearInterval(id)
  }, [fetchOrders])

  const handleCancel = async (orderid: string) => {
    setCancelling(orderid)
    try {
      const res = await tradingApi.cancelOrder(orderid)
      if (res.status === 'success') {
        toast.success(`Order cancelled: ${orderid}`)
        setTimeout(fetchOrders, 800)
      } else {
        toast.error(res.message || 'Failed to cancel order')
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to cancel order')
    } finally {
      setCancelling(null)
    }
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <ClipboardList className="h-4 w-4 text-primary" />
          Open Orders
          <Badge variant="secondary" className="ml-1 tabular-nums">
            {orders.length}
          </Badge>
        </CardTitle>
        <Link
          to="/orderbook"
          className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          Order Book <ArrowUpRight className="h-3.5 w-3.5" />
        </Link>
      </CardHeader>
      <CardContent className="p-0">
        {isLoading && orders.length === 0 ? (
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : orders.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-center text-muted-foreground">
            <ClipboardList className="mb-2 h-8 w-8 opacity-30" />
            <p className="text-sm">No working orders</p>
          </div>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Symbol</TableHead>
                <TableHead>Side</TableHead>
                <TableHead className="text-right">Qty</TableHead>
                <TableHead className="text-right">Price</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {orders.map((o) => (
                <TableRow key={o.orderid}>
                  <TableCell className="font-semibold">{o.symbol}</TableCell>
                  <TableCell>
                    <Badge
                      variant="outline"
                      className={cn(
                        'text-[10px]',
                        o.action === 'BUY'
                          ? 'border-green-500/40 text-green-600 dark:text-green-400'
                          : 'border-red-500/40 text-red-600 dark:text-red-400'
                      )}
                    >
                      {o.action} {o.pricetype}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{o.quantity}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {o.price > 0 ? formatUSD(o.price) : 'MKT'}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        title="Modify in Order Book"
                        onClick={() => navigate('/orderbook')}
                      >
                        <Pencil className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 text-destructive hover:text-destructive"
                        title="Cancel order"
                        disabled={cancelling === o.orderid}
                        onClick={() => handleCancel(o.orderid)}
                      >
                        {cancelling === o.orderid ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <X className="h-3.5 w-3.5" />
                        )}
                      </Button>
                    </div>
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
