import { Check, Copy, Loader2, ShoppingCart, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { emit } from '@tauri-apps/api/event'
import { settingsCommands } from '@/api/tauri-client'
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
import { formatRecommendationsAsCsv } from '@/lib/csvRecommendations'
import { cn } from '@/lib/utils'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import type { OrderRecommendation } from '@/types/actionQueue'

const ORDER_TYPE_LABELS: Record<string, string> = {
  MARKET: 'Market',
  LIMIT: 'Limit',
  SL: 'Stop Loss Limit',
  'SL-M': 'Stop Loss Market',
  TRAILING_STOP: 'Trailing Stop',
}

function RecommendationCard({
  item,
  onUpdate,
  onRemove,
}: {
  item: OrderRecommendation
  onUpdate: (id: string, patch: Partial<OrderRecommendation>) => void
  onRemove: (id: string) => void
}) {
  return (
    <div className="rounded-lg border p-3 space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold">{item.symbol}</span>
          <Badge
            className={cn(
              'text-[10px] font-semibold',
              item.side === 'BUY'
                ? 'bg-green-500/15 text-green-500 border-green-500/30'
                : 'bg-red-500/15 text-red-500 border-red-500/30'
            )}
          >
            {item.side}
          </Badge>
          <Badge variant="outline" className="text-[10px]">
            {ORDER_TYPE_LABELS[item.orderType] || item.orderType}
          </Badge>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 text-muted-foreground hover:text-destructive"
          onClick={() => onRemove(item.id)}
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>

      {item.rationale && (
        <p className="text-xs text-muted-foreground leading-relaxed">{item.rationale}</p>
      )}

      <div className="grid grid-cols-3 gap-2">
        <div className="space-y-1">
          <Label className="text-[10px] text-muted-foreground">Qty</Label>
          <Input
            type="number"
            min={1}
            step={1}
            value={item.quantity}
            onChange={(e) => onUpdate(item.id, { quantity: parseInt(e.target.value) || 1 })}
            className="h-8 text-xs tabular-nums"
          />
        </div>
        {(item.orderType === 'LIMIT' || item.orderType === 'SL') && (
          <div className="space-y-1">
            <Label className="text-[10px] text-muted-foreground">Price</Label>
            <Input
              type="number"
              min={0}
              step={0.01}
              value={item.price}
              onChange={(e) => onUpdate(item.id, { price: parseFloat(e.target.value) || 0 })}
              className="h-8 text-xs tabular-nums"
            />
          </div>
        )}
        {(item.orderType === 'SL' || item.orderType === 'SL-M') && (
          <div className="space-y-1">
            <Label className="text-[10px] text-muted-foreground">Trigger</Label>
            <Input
              type="number"
              min={0}
              step={0.01}
              value={item.triggerPrice || 0}
              onChange={(e) =>
                onUpdate(item.id, { triggerPrice: parseFloat(e.target.value) || 0 })
              }
              className="h-8 text-xs tabular-nums"
            />
          </div>
        )}
        {item.orderType === 'TRAILING_STOP' && (
          <div className="space-y-1">
            <Label className="text-[10px] text-muted-foreground">
              {item.trailPercent ? 'Trail %' : 'Trail $'}
            </Label>
            <Input
              type="number"
              min={0}
              step={item.trailPercent ? 0.1 : 0.01}
              value={item.trailPercent || item.trailPrice || 0}
              onChange={(e) => {
                const val = parseFloat(e.target.value) || 0
                if (item.trailPercent) {
                  onUpdate(item.id, { trailPercent: val })
                } else {
                  onUpdate(item.id, { trailPrice: val })
                }
              }}
              className="h-8 text-xs tabular-nums"
            />
          </div>
        )}
        <div className="space-y-1">
          <Label className="text-[10px] text-muted-foreground">Type</Label>
          <Select
            value={item.orderType}
            onValueChange={(val) =>
              onUpdate(item.id, { orderType: val as OrderRecommendation['orderType'] })
            }
          >
            <SelectTrigger className="h-8 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="MARKET">Market</SelectItem>
              <SelectItem value="LIMIT">Limit</SelectItem>
              <SelectItem value="SL">Stop Loss</SelectItem>
              <SelectItem value="SL-M">SL Market</SelectItem>
              <SelectItem value="TRAILING_STOP">Trailing</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </div>
  )
}

export interface ActionReviewModalProps {
  /**
   * Optional override for the Apply action. When provided, the modal calls this
   * instead of submitting a live basket order. Receives the current items array
   * and (if cloneNameRequired is true) the user-typed clone name.
   */
  onApply?: (items: OrderRecommendation[], cloneName?: string) => Promise<void>
  /** When true, renders a required "Name this strategy clone" input above the list. */
  cloneNameRequired?: boolean
  /** Overrides the Apply button label. Defaults to "Place N Order(s)". */
  applyButtonLabel?: string
}

export function ActionReviewModal({
  onApply,
  cloneNameRequired = false,
  applyButtonLabel,
}: ActionReviewModalProps = {}) {
  const { items, isReviewOpen, isSubmitting, lastResults, fundsNotice, updateItem, removeItem, close } =
    useActionQueueStore()
  const setSubmitting = useActionQueueStore((s) => s.setSubmitting)
  const setResults = useActionQueueStore((s) => s.setResults)
  const [showResults, setShowResults] = useState(false)
  const [cloneName, setCloneName] = useState('')
  // Gate B (pre-execution acknowledgment) — required before any LIVE order.
  const [acknowledged, setAcknowledged] = useState(false)
  // Execution mode read from the SAME authority the order routes on
  // (get_analyze_mode). null = not yet read → treated as LIVE (fail safe).
  const [liveMode, setLiveMode] = useState<boolean | null>(null)

  // Read the authoritative mode when the modal opens on the live path. We do NOT
  // trust any cached/themeStore value — only get_analyze_mode, the same flag the
  // order router checks at place time. Failure to read => assume LIVE.
  useEffect(() => {
    if (!isReviewOpen || onApply) return
    let cancelled = false
    settingsCommands
      .getAnalyzeMode()
      .then((s) => {
        if (!cancelled) setLiveMode(!s.analyze_mode)
      })
      .catch(() => {
        if (!cancelled) setLiveMode(true)
      })
    return () => {
      cancelled = true
    }
  }, [isReviewOpen, onApply])

  // Any change in the authoritative mode invalidates a prior acknowledgment.
  useEffect(() => {
    setAcknowledged(false)
  }, [liveMode])

  const handlePlaceAll = async () => {
    if (items.length === 0) return

    // Scenario-mode path: caller-provided onApply handler.
    if (onApply) {
      setSubmitting(true)
      try {
        await onApply(items, cloneNameRequired ? cloneName.trim() : undefined)
        // onApply owns post-apply UX (toasts + navigation). Close the modal.
        setCloneName('')
        close()
      } catch (err) {
        toast.error(err instanceof Error ? err.message : 'Failed to apply recommendations')
        setSubmitting(false)
      }
      return
    }

    // Default path: live basket order.
    setSubmitting(true)

    // Gate B single-source-of-truth check: re-read the SAME authority the order
    // routes on (get_analyze_mode), at submit time. If it disagrees with what the
    // user saw and acknowledged, abort and force re-confirmation — so a LIVE order
    // can never execute under a "paper" acknowledgment, and vice versa.
    let freshLive = true
    try {
      const status = await settingsCommands.getAnalyzeMode()
      freshLive = !status.analyze_mode
    } catch {
      freshLive = true // cannot confirm => assume LIVE (fail safe)
    }
    if (freshLive !== liveMode) {
      setLiveMode(freshLive) // resets the acknowledgment via effect
      setSubmitting(false)
      toast.warning(
        freshLive
          ? 'Mode changed to LIVE (real money) — review and re-confirm before placing.'
          : 'Mode changed to paper (sandbox) — review and re-confirm.'
      )
      return
    }
    if (freshLive && !acknowledged) {
      setSubmitting(false)
      return
    }

    try {
      const response = await tradingApi.placeBasketOrder(
        items.map((item) => ({
          symbol: item.symbol,
          exchange: item.exchange,
          action: item.side,
          product: item.product,
          pricetype: item.orderType,
          price: item.orderType === 'MARKET' || item.orderType === 'TRAILING_STOP' ? 0 : item.price,
          quantity: item.quantity,
          trigger_price: item.triggerPrice,
          validity: 'DAY',
          amo: false,
          trail_price: item.orderType === 'TRAILING_STOP' ? item.trailPrice : undefined,
          trail_percent: item.orderType === 'TRAILING_STOP' ? item.trailPercent : undefined,
        })),
        // Pass the acknowledged analyze-mode to the backend. It re-reads the same
        // authority at routing time and rejects if it no longer matches — so no
        // background mode flip between this confirmation and execution can route a
        // LIVE order under a paper acknowledgment (or vice versa). freshLive is the
        // mode just confirmed, so the acknowledged analyze_mode is its inverse.
        !freshLive
      )

      if (response.status === 'success' && response.data) {
        const successCount = response.data.filter((r) => r.success).length
        const failCount = response.data.length - successCount

        setResults(response.data)
        setShowResults(true)

        // Emit basket event for auto-refresh (distinct from order_event to avoid single-order toast)
        emit('basket_order_event', { count: successCount })

        if (failCount === 0) {
          toast.success(`All ${successCount} orders placed successfully`)
        } else {
          toast.warning(`${successCount} placed, ${failCount} failed`)
        }
      } else {
        toast.error(response.message || 'Failed to place basket order')
        setSubmitting(false)
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to place orders')
      setSubmitting(false)
    }
  }

  const handleClose = () => {
    setShowResults(false)
    setCloneName('')
    setAcknowledged(false)
    close()
  }

  const handleCopyCsv = async () => {
    try {
      const csv = formatRecommendationsAsCsv(items)
      await navigator.clipboard.writeText(csv)
      toast.success(`Copied ${items.length} trade${items.length !== 1 ? 's' : ''} to clipboard`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to copy')
    }
  }

  const defaultApplyLabel = onApply
    ? cloneNameRequired
      ? 'Create Clone & Apply'
      : 'Apply to Scenario'
    : `Place ${items.length} Order${items.length !== 1 ? 's' : ''}`
  const resolvedApplyLabel = applyButtonLabel ?? defaultApplyLabel

  // Gate B figures (live path only). Real-money downside restated in the
  // user's own dollars per the trading-advice spec (C5).
  const isLivePath = !onApply
  const buyCost = items
    .filter((i) => i.side === 'BUY' && i.price > 0)
    .reduce((sum, i) => sum + i.quantity * i.price, 0)
  const marketLegs = items.filter((i) => i.price === 0).length
  const realisticDownside = buyCost * 0.25
  const usd = (n: number) =>
    n.toLocaleString('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 })
  const showGateB = isLivePath && !showResults && items.length > 0
  // null (unread) or true => treat as LIVE; only an authoritative `false` is paper.
  const isLive = liveMode !== false

  return (
    <Dialog open={isReviewOpen} onOpenChange={(open) => !open && handleClose()}>
      <DialogContent className="sm:max-w-2xl h-[75vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ShoppingCart className="h-5 w-5" />
            Review Recommendations
            {items.length > 0 && (
              <Badge variant="secondary" className="ml-1 text-xs">
                {items.length} order{items.length !== 1 ? 's' : ''}
              </Badge>
            )}
            {!showResults && items.length > 0 && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleCopyCsv}
                title="Copy as CSV"
                aria-label="Copy as CSV"
                className="ml-auto h-8 w-8 p-0 text-muted-foreground hover:text-foreground"
              >
                <Copy className="h-4 w-4" />
              </Button>
            )}
          </DialogTitle>
          <DialogDescription>
            Review AI-recommended trades before placing. Edit quantities, prices, or remove any
            you don't want.
          </DialogDescription>
        </DialogHeader>

        {!showResults && cloneNameRequired && items.length > 0 && (
          <div className="px-1 pb-2 space-y-1.5 shrink-0">
            <Label htmlFor="scenario-clone-name" className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Name this strategy clone
            </Label>
            <Input
              id="scenario-clone-name"
              value={cloneName}
              onChange={(e) => setCloneName(e.target.value)}
              placeholder="e.g. Aggressive Growth, Dividend Tilt"
              className="h-10"
              autoFocus
              disabled={isSubmitting}
            />
            <p className="text-[11px] text-muted-foreground">
              Baseline scenarios are never modified. A new clone will be created with this name.
            </p>
          </div>
        )}

        <div className="flex-1 overflow-y-auto space-y-3 py-2">
          {showResults && lastResults ? (
            // Results summary view
            lastResults.map((result, i) => (
              <div
                key={i}
                className={cn(
                  'rounded-lg border p-3 flex items-center justify-between',
                  result.success ? 'border-green-500/30 bg-green-500/5' : 'border-red-500/30 bg-red-500/5'
                )}
              >
                <div className="flex items-center gap-2">
                  {result.success ? (
                    <Check className="h-4 w-4 text-green-500" />
                  ) : (
                    <X className="h-4 w-4 text-red-500" />
                  )}
                  <span className="text-sm font-semibold">{result.symbol}</span>
                </div>
                <span className="text-xs text-muted-foreground">
                  {result.success
                    ? `Order ID: ${result.orderId || 'N/A'}`
                    : result.message || 'Failed'}
                </span>
              </div>
            ))
          ) : items.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
              <ShoppingCart className="h-12 w-12 mb-3 opacity-30" />
              <p className="text-sm">No recommendations to review</p>
            </div>
          ) : (
            items.map((item) => (
              <RecommendationCard
                key={item.id}
                item={item}
                onUpdate={updateItem}
                onRemove={removeItem}
              />
            ))
          )}
        </div>

        {!showResults && fundsNotice && items.length > 0 && (
          <div className="shrink-0 rounded-lg border border-amber-500/40 bg-amber-500/5 p-3">
            <p className="text-xs leading-relaxed text-amber-700 dark:text-amber-400">
              {fundsNotice}
            </p>
          </div>
        )}

        {showGateB && isLive && (
          <div className="shrink-0 rounded-lg border border-red-500/40 bg-red-500/5 p-3 space-y-2">
            <p className="text-sm font-semibold text-red-600 dark:text-red-400">
              LIVE — real money. Confirm before placing.
            </p>
            <p className="text-xs text-muted-foreground leading-relaxed">
              These place <strong>real orders</strong> on your live account
              {buyCost > 0 && (
                <>
                  {' '}totaling about <strong>{usd(buyCost)}</strong>
                </>
              )}
              {marketLegs > 0 && (
                <>
                  {' '}(plus {marketLegs} market-priced order{marketLegs !== 1 ? 's' : ''} filled at the
                  live price)
                </>
              )}
              .{' '}
              {buyCost > 0 && (
                <>
                  A realistic drawdown of ~25% would be about <strong>{usd(realisticDownside)}</strong> of
                  real money — and losses can be larger.{' '}
                </>
              )}
              AI recommendations can be confidently wrong and don't know your full finances.
            </p>
            <label className="flex items-start gap-2 text-xs cursor-pointer select-none">
              <input
                type="checkbox"
                checked={acknowledged}
                onChange={(e) => setAcknowledged(e.target.checked)}
                className="mt-0.5 h-4 w-4"
              />
              <span>I understand these are real, live orders and I accept the risk.</span>
            </label>
          </div>
        )}
        {showGateB && !isLive && (
          <div className="shrink-0 rounded-lg border border-border bg-muted/40 p-3">
            <p className="text-xs text-muted-foreground leading-relaxed">
              <strong className="text-foreground">Paper (sandbox) mode.</strong> These are simulated orders
              with virtual money — no real funds are used. Switch to Live mode to trade for real.
            </p>
          </div>
        )}

        <DialogFooter className="gap-2">
          {showResults ? (
            <Button onClick={handleClose}>Done</Button>
          ) : (
            <>
              <Button variant="outline" onClick={handleClose}>
                Cancel
              </Button>
              <Button
                onClick={handlePlaceAll}
                disabled={
                  isSubmitting ||
                  items.length === 0 ||
                  (cloneNameRequired && cloneName.trim().length === 0) ||
                  (showGateB && isLive && !acknowledged)
                }
                className="bg-primary"
              >
                {isSubmitting ? (
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                ) : null}
                {resolvedApplyLabel}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
