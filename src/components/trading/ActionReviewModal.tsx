import { Check, Loader2, ShoppingCart, X } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { emit } from '@tauri-apps/api/event'
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
  const { items, isReviewOpen, isSubmitting, lastResults, updateItem, removeItem, close } =
    useActionQueueStore()
  const setSubmitting = useActionQueueStore((s) => s.setSubmitting)
  const setResults = useActionQueueStore((s) => s.setResults)
  const [showResults, setShowResults] = useState(false)
  const [cloneName, setCloneName] = useState('')

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
        }))
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
    close()
  }

  const defaultApplyLabel = onApply
    ? cloneNameRequired
      ? 'Create Clone & Apply'
      : 'Apply to Scenario'
    : `Place ${items.length} Order${items.length !== 1 ? 's' : ''}`
  const resolvedApplyLabel = applyButtonLabel ?? defaultApplyLabel

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
                  (cloneNameRequired && cloneName.trim().length === 0)
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
