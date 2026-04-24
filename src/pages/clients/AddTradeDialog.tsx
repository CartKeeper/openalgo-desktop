import { Loader2 } from 'lucide-react'
import { useState } from 'react'
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

interface AddTradeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  shortSellingAllowed?: boolean
  onSubmit: (trade: {
    symbol: string
    exchange: string
    side: string
    quantity: number
    price: number
  }) => Promise<void>
}

export default function AddTradeDialog({
  open,
  onOpenChange,
  shortSellingAllowed = true,
  onSubmit,
}: AddTradeDialogProps) {
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [form, setForm] = useState({
    symbol: '',
    exchange: 'GENERIC',
    side: 'buy',
    quantity: '',
    price: '',
  })

  const handleSubmit = async () => {
    if (!form.symbol.trim() || !form.quantity || !form.price) return
    setIsSubmitting(true)
    try {
      await onSubmit({
        symbol: form.symbol.toUpperCase().trim(),
        exchange: form.exchange,
        side: form.side,
        quantity: parseFloat(form.quantity),
        price: parseFloat(form.price),
      })
      setForm({
        symbol: '',
        exchange: 'GENERIC',
        side: 'buy',
        quantity: '',
        price: '',
      })
      onOpenChange(false)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Apply Trade</DialogTitle>
          <DialogDescription>
            Simulate a trade on this scenario. Existing positions will be updated
            with weighted-average cost basis.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="trade-symbol">Symbol *</Label>
              <Input
                id="trade-symbol"
                value={form.symbol}
                onChange={(e) =>
                  setForm({ ...form, symbol: e.target.value })
                }
                placeholder="AAPL"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="trade-exchange">Exchange</Label>
              <Input
                id="trade-exchange"
                value={form.exchange}
                onChange={(e) =>
                  setForm({ ...form, exchange: e.target.value })
                }
              />
            </div>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="trade-side">Side *</Label>
            <Select
              value={form.side}
              onValueChange={(v) => setForm({ ...form, side: v })}
            >
              <SelectTrigger id="trade-side">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="buy">Buy (Long)</SelectItem>
                <SelectItem value="sell" disabled={!shortSellingAllowed}>
                  Sell (Short){!shortSellingAllowed ? ' — restricted' : ''}
                </SelectItem>
              </SelectContent>
            </Select>
            {!shortSellingAllowed && (
              <p className="text-[10px] text-muted-foreground">
                Short selling is restricted for this account type.
              </p>
            )}
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="trade-qty">Quantity *</Label>
              <Input
                id="trade-qty"
                type="number"
                step="any"
                min="0"
                value={form.quantity}
                onChange={(e) =>
                  setForm({ ...form, quantity: e.target.value })
                }
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="trade-price">Price *</Label>
              <Input
                id="trade-price"
                type="number"
                step="any"
                min="0"
                value={form.price}
                onChange={(e) =>
                  setForm({ ...form, price: e.target.value })
                }
              />
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleSubmit}
            disabled={
              isSubmitting ||
              !form.symbol.trim() ||
              !form.quantity ||
              !form.price
            }
          >
            {isSubmitting && (
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            )}
            Apply Trade
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
