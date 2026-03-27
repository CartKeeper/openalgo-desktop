import { invoke } from '@tauri-apps/api/core'
import { ArrowLeft, Download, Loader2, Plus, Trash2, Upload } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
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

interface PortfolioPosition {
  id: number | null
  symbol: string
  exchange: string
  quantity: number
  avg_price: number
  current_price: number | null
  pnl: number | null
  pnl_percent: number | null
  position_type: string
  asset_class: string | null
  notes: string | null
  added_at: string | null
  updated_at: string | null
}

export default function PortfolioManager() {
  const [positions, setPositions] = useState<PortfolioPosition[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [showAddDialog, setShowAddDialog] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [form, setForm] = useState({
    symbol: '',
    exchange: 'GENERIC',
    quantity: '',
    avg_price: '',
    position_type: 'long',
    asset_class: '',
    notes: '',
  })

  const loadPositions = useCallback(async () => {
    try {
      const data = await invoke<PortfolioPosition[]>('get_portfolio_positions')
      setPositions(data)
    } catch (err) {
      console.error('Failed to load positions:', err)
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    loadPositions()
  }, [loadPositions])

  const handleAddPosition = async () => {
    if (!form.symbol || !form.quantity || !form.avg_price) {
      toast.error('Symbol, quantity, and average price are required')
      return
    }

    setIsSubmitting(true)
    try {
      await invoke('add_portfolio_position', {
        symbol: form.symbol.toUpperCase(),
        exchange: form.exchange,
        quantity: parseFloat(form.quantity),
        avg_price: parseFloat(form.avg_price),
        position_type: form.position_type,
        asset_class: form.asset_class || null,
        notes: form.notes || null,
      })
      toast.success(`Added ${form.symbol.toUpperCase()}`)
      setShowAddDialog(false)
      setForm({
        symbol: '',
        exchange: 'GENERIC',
        quantity: '',
        avg_price: '',
        position_type: 'long',
        asset_class: '',
        notes: '',
      })
      await loadPositions()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Failed to add position: ${msg}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleDeletePosition = async (id: number, symbol: string) => {
    try {
      await invoke('delete_portfolio_position', { id })
      toast.success(`Removed ${symbol}`)
      await loadPositions()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Failed to delete: ${msg}`)
    }
  }

  const handleImportCsv = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    try {
      const text = await file.text()
      const count = await invoke<number>('import_portfolio_csv', { csv_content: text })
      toast.success(`Imported ${count} positions`)
      await loadPositions()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Import failed: ${msg}`)
    }

    // Reset file input
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
    }
  }

  const handleExportCsv = async () => {
    try {
      const csv = await invoke<string>('export_portfolio_csv')
      const blob = new Blob([csv], { type: 'text/csv' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = 'portfolio.csv'
      a.click()
      URL.revokeObjectURL(url)
      toast.success('Portfolio exported')
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Export failed: ${msg}`)
    }
  }

  const totalValue = positions.reduce((sum, p) => sum + p.quantity * p.avg_price, 0)
  const totalPnl = positions.reduce((sum, p) => sum + (p.pnl || 0), 0)

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 className="h-8 w-8 animate-spin" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
              <ArrowLeft className="h-4 w-4" />
            </Link>
            <h1 className="text-2xl font-bold">Portfolio Manager</h1>
          </div>
          <p className="text-sm text-muted-foreground">
            Manage your manual portfolio positions
          </p>
        </div>
        <div className="flex items-center gap-2">
          <input
            ref={fileInputRef}
            type="file"
            accept=".csv"
            onChange={handleImportCsv}
            className="hidden"
          />
          <Button
            variant="outline"
            size="sm"
            onClick={() => fileInputRef.current?.click()}
          >
            <Upload className="h-4 w-4 mr-1" />
            Import CSV
          </Button>
          {positions.length > 0 && (
            <Button variant="outline" size="sm" onClick={handleExportCsv}>
              <Download className="h-4 w-4 mr-1" />
              Export CSV
            </Button>
          )}
          <Button size="sm" onClick={() => setShowAddDialog(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Add Position
          </Button>
        </div>
      </div>

      {/* Summary cards */}
      {positions.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <Card>
            <CardContent className="pt-6">
              <p className="text-sm text-muted-foreground">Positions</p>
              <p className="text-2xl font-bold tabular-nums">{positions.length}</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-6">
              <p className="text-sm text-muted-foreground">Cost Basis</p>
              <p className="text-2xl font-bold tabular-nums">
                ${totalValue.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-6">
              <p className="text-sm text-muted-foreground">Unrealized P&L</p>
              <p className={`text-2xl font-bold tabular-nums ${totalPnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                {totalPnl >= 0 ? '+' : ''}{totalPnl.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
              </p>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Positions table */}
      {positions.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-16">
            <p className="text-lg font-medium mb-2">No positions yet</p>
            <p className="text-sm text-muted-foreground mb-4">
              Add positions manually or import from a CSV file.
            </p>
            <Button onClick={() => setShowAddDialog(true)}>
              <Plus className="h-4 w-4 mr-1" />
              Add Position
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b">
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase text-muted-foreground">Symbol</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase text-muted-foreground">Exchange</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase text-muted-foreground">Qty</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase text-muted-foreground">Avg Price</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase text-muted-foreground">Current</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase text-muted-foreground">P&L</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase text-muted-foreground">Type</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase text-muted-foreground">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {positions.map((pos) => (
                    <tr key={pos.id} className="border-b last:border-0 hover:bg-muted/50 transition-colors h-12">
                      <td className="px-4 font-semibold text-sm">{pos.symbol}</td>
                      <td className="px-4 text-sm text-muted-foreground">{pos.exchange}</td>
                      <td className="px-4 text-right text-sm tabular-nums">{pos.quantity}</td>
                      <td className="px-4 text-right text-sm tabular-nums">{pos.avg_price.toFixed(2)}</td>
                      <td className="px-4 text-right text-sm tabular-nums">
                        {pos.current_price != null ? pos.current_price.toFixed(2) : '—'}
                      </td>
                      <td className={`px-4 text-right text-sm tabular-nums ${
                        pos.pnl != null ? (pos.pnl >= 0 ? 'text-green-500' : 'text-red-500') : ''
                      }`}>
                        {pos.pnl != null
                          ? `${pos.pnl >= 0 ? '+' : ''}${pos.pnl.toFixed(2)} (${pos.pnl_percent?.toFixed(1)}%)`
                          : '—'}
                      </td>
                      <td className="px-4 text-sm capitalize">{pos.position_type}</td>
                      <td className="px-4 text-right">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-8 w-8 p-0 text-destructive hover:text-destructive"
                          onClick={() => pos.id != null && handleDeletePosition(pos.id, pos.symbol)}
                        >
                          <Trash2 className="h-3 w-3" />
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Add Position Dialog */}
      <Dialog open={showAddDialog} onOpenChange={setShowAddDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Add Position</DialogTitle>
            <DialogDescription>
              Manually add a portfolio position for tracking.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="symbol">Symbol *</Label>
                <Input
                  id="symbol"
                  placeholder="AAPL"
                  value={form.symbol}
                  onChange={(e) => setForm({ ...form, symbol: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="exchange">Exchange</Label>
                <Input
                  id="exchange"
                  placeholder="GENERIC"
                  value={form.exchange}
                  onChange={(e) => setForm({ ...form, exchange: e.target.value })}
                />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="quantity">Quantity *</Label>
                <Input
                  id="quantity"
                  type="number"
                  step="any"
                  placeholder="100"
                  value={form.quantity}
                  onChange={(e) => setForm({ ...form, quantity: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="avg_price">Avg Price *</Label>
                <Input
                  id="avg_price"
                  type="number"
                  step="any"
                  placeholder="150.00"
                  value={form.avg_price}
                  onChange={(e) => setForm({ ...form, avg_price: e.target.value })}
                />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="position_type">Position Type</Label>
                <Select
                  value={form.position_type}
                  onValueChange={(v) => setForm({ ...form, position_type: v })}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="long">Long</SelectItem>
                    <SelectItem value="short">Short</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="asset_class">Asset Class</Label>
                <Input
                  id="asset_class"
                  placeholder="Equity"
                  value={form.asset_class}
                  onChange={(e) => setForm({ ...form, asset_class: e.target.value })}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="notes">Notes</Label>
              <Input
                id="notes"
                placeholder="Optional notes"
                value={form.notes}
                onChange={(e) => setForm({ ...form, notes: e.target.value })}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowAddDialog(false)}>
              Cancel
            </Button>
            <Button onClick={handleAddPosition} disabled={isSubmitting}>
              {isSubmitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Adding...
                </>
              ) : (
                'Add Position'
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
