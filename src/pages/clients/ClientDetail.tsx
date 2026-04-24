import { invoke } from '@tauri-apps/api/core'
import {
  ArrowLeft,
  Download,
  Edit2,
  GitBranch,
  Loader2,
  Plus,
  Trash2,
  Upload,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import type {
  Client,
  ClientPosition,
  ClientTrade,
  ImportBatch,
} from '@/types/clients'
import { ACCOUNT_TYPES } from '@/types/clients'
import { useAccountStore } from '@/stores/accountStore'
import { AccountSwitcher } from '@/components/AccountSwitcher'
import ImportCsvDialog from './ImportCsvDialog'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err) return (err as { message: string }).message
  return String(err)
}

export default function ClientDetail() {
  const { id } = useParams<{ id: string }>()
  const clientId = Number(id)

  const [client, setClient] = useState<Client | null>(null)
  const [positions, setPositions] = useState<ClientPosition[]>([])
  const [trades, setTrades] = useState<ClientTrade[]>([])
  const [batches, setBatches] = useState<ImportBatch[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [activeTab, setActiveTab] = useState('positions')

  // Account switcher
  const { selectedAccount, accounts, fetchClientAccounts, reset: resetAccountStore } = useAccountStore()
  const showAccountColumn = selectedAccount === 'all' && accounts.length >= 2

  // Portfolio summary stats computed from current positions
  const portfolioStats = useMemo(() => {
    if (positions.length === 0) return null
    const totalPositions = positions.filter((p) => Math.abs(p.net_quantity) > 0.001).length
    const totalTrades = positions.reduce((sum, p) => sum + p.trade_count, 0)
    const totalFees = positions.reduce((sum, p) => sum + p.total_fees, 0)
    const totalRealizedPnl = positions.reduce((sum, p) => sum + p.realized_pnl, 0)
    const totalCostBasis = positions
      .filter((p) => p.net_quantity > 0)
      .reduce((sum, p) => sum + p.avg_price * p.net_quantity, 0)
    return { totalPositions, totalTrades, totalFees, totalRealizedPnl, totalCostBasis }
  }, [positions])

  // Edit client dialog
  const [showEditDialog, setShowEditDialog] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [editForm, setEditForm] = useState({
    name: '',
    email: '',
    phone: '',
    broker: '',
    account_id: '',
    account_type: '',
    notes: '',
  })

  // Add trade dialog
  const [showAddTrade, setShowAddTrade] = useState(false)
  const [tradeForm, setTradeForm] = useState({
    symbol: '',
    exchange: 'GENERIC',
    trade_date: new Date().toISOString().slice(0, 10),
    trade_type: 'buy',
    quantity: '',
    price: '',
    fees: '',
    order_id: '',
    notes: '',
  })

  // Import CSV dialog
  const [showImportDialog, setShowImportDialog] = useState(false)

  // Delete batch confirm
  const [deleteBatchTarget, setDeleteBatchTarget] = useState<ImportBatch | null>(null)

  const loadClient = useCallback(async () => {
    try {
      const data = await invoke<Client>('get_client', { id: clientId })
      setClient(data)
      setEditForm({
        name: data.name,
        email: data.email || '',
        phone: data.phone || '',
        broker: data.broker || '',
        account_id: data.account_id || '',
        account_type: data.account_type || '',
        notes: data.notes || '',
      })
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to load client: ${msg}`)
    }
  }, [clientId])

  const loadPositions = useCallback(async () => {
    try {
      const acct = useAccountStore.getState().selectedAccount
      const accounts = useAccountStore.getState().accounts
      let data: ClientPosition[]
      if (acct === 'all' && accounts.length >= 2) {
        // Multiple accounts — show positions split by account so each row shows which account it belongs to
        data = await invoke<ClientPosition[]>('get_client_positions_by_each_account', { clientId })
      } else if (acct === 'all') {
        // Single or no accounts — aggregate as before
        data = await invoke<ClientPosition[]>('get_client_positions', { clientId })
      } else {
        data = await invoke<ClientPosition[]>('get_client_positions_by_account', { clientId, accountType: acct })
      }
      setPositions(data)
    } catch (err) {
      console.error('Failed to load positions:', err)
    }
  }, [clientId])

  const loadTrades = useCallback(async () => {
    try {
      const acct = useAccountStore.getState().selectedAccount
      const data = acct === 'all'
        ? await invoke<ClientTrade[]>('get_client_trades', { clientId })
        : await invoke<ClientTrade[]>('get_client_trades_by_account', { clientId, accountType: acct })
      setTrades(data)
    } catch (err) {
      console.error('Failed to load trades:', err)
    }
  }, [clientId])

  const loadBatches = useCallback(async () => {
    try {
      const data = await invoke<ImportBatch[]>('get_import_batches', { clientId })
      setBatches(data)
    } catch (err) {
      console.error('Failed to load import batches:', err)
    }
  }, [clientId])

  const loadAll = useCallback(async () => {
    setIsLoading(true)
    await Promise.all([loadClient(), loadPositions(), loadTrades(), loadBatches(), fetchClientAccounts(clientId)])
    setIsLoading(false)
  }, [loadClient, loadPositions, loadTrades, loadBatches, fetchClientAccounts, clientId])

  useEffect(() => {
    loadAll()
    return () => { resetAccountStore() }
  }, [loadAll, resetAccountStore])

  // Re-fetch positions and trades when the selected account changes
  useEffect(() => {
    if (!isLoading) {
      loadPositions()
      loadTrades()
    }
  }, [selectedAccount, loadPositions, loadTrades, isLoading])

  // Edit client
  const handleEditClient = async () => {
    if (!editForm.name.trim()) {
      toast.error('Client name is required')
      return
    }
    setIsSubmitting(true)
    try {
      await invoke('update_client', {
        id: clientId,
        name: editForm.name.trim(),
        email: editForm.email.trim() || null,
        phone: editForm.phone.trim() || null,
        broker: editForm.broker.trim() || null,
        accountId: editForm.account_id.trim() || null,
        accountType: editForm.account_type || null,
        notes: editForm.notes.trim() || null,
      })
      toast.success('Client updated')
      setShowEditDialog(false)
      await loadClient()
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to update client: ${msg}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  // Add manual trade
  const handleAddTrade = async () => {
    if (!tradeForm.symbol || !tradeForm.quantity || !tradeForm.price) {
      toast.error('Symbol, quantity, and price are required')
      return
    }
    setIsSubmitting(true)
    try {
      await invoke('add_client_trade', {
        clientId,
        symbol: tradeForm.symbol.toUpperCase().trim(),
        exchange: tradeForm.exchange,
        tradeDate: tradeForm.trade_date,
        tradeType: tradeForm.trade_type,
        quantity: parseFloat(tradeForm.quantity),
        price: parseFloat(tradeForm.price),
        fees: tradeForm.fees ? parseFloat(tradeForm.fees) : null,
        orderId: tradeForm.order_id.trim() || null,
        notes: tradeForm.notes.trim() || null,
      })
      toast.success(`Added ${tradeForm.trade_type} trade: ${tradeForm.symbol.toUpperCase()}`)
      setShowAddTrade(false)
      setTradeForm({
        symbol: '',
        exchange: 'GENERIC',
        trade_date: new Date().toISOString().slice(0, 10),
        trade_type: 'buy',
        quantity: '',
        price: '',
        fees: '',
        order_id: '',
        notes: '',
      })
      await Promise.all([loadTrades(), loadPositions()])
      // Auto-sync baselines after manual trade
      invoke('sync_baseline_scenario', { clientId }).catch(() => {})
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to add trade: ${msg}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  // Delete a single trade
  const handleDeleteTrade = async (tradeId: number) => {
    try {
      await invoke('delete_client_trade', { id: tradeId })
      toast.success('Trade deleted')
      await Promise.all([loadTrades(), loadPositions()])
      // Auto-sync baselines after trade deletion
      invoke('sync_baseline_scenario', { clientId }).catch(() => {})
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to delete trade: ${msg}`)
    }
  }

  // Delete an import batch
  const handleDeleteBatch = async () => {
    if (!deleteBatchTarget?.id) return
    try {
      const count = await invoke<number>('delete_import_batch', { batchId: deleteBatchTarget.id })
      toast.success(`Undid import: removed ${count} trades`)
      setDeleteBatchTarget(null)
      await Promise.all([loadTrades(), loadPositions(), loadBatches(), fetchClientAccounts(clientId)])
      // Auto-sync baselines after batch deletion
      invoke('sync_baseline_scenario', { clientId }).catch(() => {})
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to undo import: ${msg}`)
    }
  }

  // Export trades
  const handleExport = async () => {
    try {
      const csv = await invoke<string>('export_client_trades_csv', { clientId })
      const blob = new Blob([csv], { type: 'text/csv' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${client?.name.replace(/\s+/g, '_') || 'client'}_trades.csv`
      a.click()
      URL.revokeObjectURL(url)
      toast.success('Trades exported')
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to export: ${msg}`)
    }
  }

  // After CSV import success
  const handleImportSuccess = async () => {
    setShowImportDialog(false)
    await Promise.all([loadTrades(), loadPositions(), loadBatches(), fetchClientAccounts(clientId)])
    // Auto-sync baseline scenarios so per-account baselines are created/updated
    invoke('sync_baseline_scenario', { clientId }).catch(() => {})
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (!client) {
    return (
      <div className="space-y-4">
        <Link to="/clients" className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
          <ArrowLeft className="h-4 w-4" /> Back to Clients
        </Link>
        <p className="text-muted-foreground">Client not found.</p>
      </div>
    )
  }

  const fmt = (n: number) =>
    new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2 }).format(n)

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="space-y-1">
          <Link to="/clients" className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" /> Back to Clients
          </Link>
          <h1 className="text-2xl font-bold tracking-tight">{client.name}</h1>
          <div className="flex items-center gap-3 text-sm text-muted-foreground">
            {client.broker && <span>Broker: {client.broker}</span>}
            {client.account_id && <span className="font-mono">Acct: {client.account_id}</span>}
            {client.account_type && (
              <Badge variant="outline" className="text-xs">
                {ACCOUNT_TYPES.find((t) => t.value === client.account_type)?.label || client.account_type}
              </Badge>
            )}
            {client.email && <span>{client.email}</span>}
            <AccountSwitcher />
          </div>
        </div>
        <div className="flex gap-2">
          <Link to={`/clients/${clientId}/scenarios`}>
            <Button variant="outline" size="sm">
              <GitBranch className="h-4 w-4 mr-2" />
              Scenarios
            </Button>
          </Link>
          <Button variant="outline" size="sm" onClick={() => setShowEditDialog(true)}>
            <Edit2 className="h-4 w-4 mr-2" />
            Edit
          </Button>
        </div>
      </div>

      {/* Portfolio Summary Cards */}
      {portfolioStats && (
        <div className="grid gap-4 md:grid-cols-4">
          <Card>
            <CardHeader className="pb-2">
              <CardDescription>Open Positions</CardDescription>
              <CardTitle className="text-2xl text-primary font-mono tabular-nums">
                {portfolioStats.totalPositions}
              </CardTitle>
            </CardHeader>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardDescription>Cost Basis</CardDescription>
              <CardTitle className="text-2xl font-mono tabular-nums">
                {fmt(portfolioStats.totalCostBasis)}
              </CardTitle>
            </CardHeader>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardDescription>Realized P&L</CardDescription>
              <CardTitle className={`text-2xl font-mono tabular-nums ${portfolioStats.totalRealizedPnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                {fmt(portfolioStats.totalRealizedPnl)}
              </CardTitle>
            </CardHeader>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardDescription>Total Fees</CardDescription>
              <CardTitle className="text-2xl font-mono tabular-nums text-muted-foreground">
                {fmt(portfolioStats.totalFees)}
              </CardTitle>
            </CardHeader>
          </Card>
        </div>
      )}

      {/* Tabs */}
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="positions">
            Positions ({positions.length})
          </TabsTrigger>
          <TabsTrigger value="trades">
            Trades ({trades.length})
          </TabsTrigger>
          <TabsTrigger value="imports">
            Imports ({batches.length})
          </TabsTrigger>
        </TabsList>

        {/* Positions Tab */}
        <TabsContent value="positions">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Computed Positions</CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              {positions.length === 0 ? (
                <p className="px-4 pb-4 text-sm text-muted-foreground">No positions yet. Import or add trades to see computed positions.</p>
              ) : (
                <div className="overflow-x-auto">
                  <table className="w-full">
                    <thead>
                      <tr className="border-b bg-muted/50">
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Symbol</th>
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Exchange</th>
                        {showAccountColumn && (
                          <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Account</th>
                        )}
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Net Qty</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Avg Price</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Fees</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Trades</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Realized P&L</th>
                      </tr>
                    </thead>
                    <tbody>
                      {positions.map((pos) => (
                        <tr key={`${pos.symbol}-${pos.exchange}-${pos.account_type ?? ''}`} className="border-b">
                          <td className="h-12 px-4 font-medium">{pos.symbol}</td>
                          <td className="h-12 px-4 text-muted-foreground">{pos.exchange}</td>
                          {showAccountColumn && (
                            <td className="h-12 px-4">
                              <Badge variant="outline" className="text-xs">
                                {ACCOUNT_TYPES.find((t) => t.value === pos.account_type)?.label || pos.account_type || 'Unspecified'}
                              </Badge>
                            </td>
                          )}
                          <td className="h-12 px-4 text-right font-mono tabular-nums">
                            {pos.net_quantity.toFixed(2)}
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">
                            {pos.avg_price > 0 ? fmt(pos.avg_price) : '—'}
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums text-muted-foreground">
                            {fmt(pos.total_fees)}
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums text-muted-foreground">
                            {pos.trade_count}
                          </td>
                          <td className={`h-12 px-4 text-right font-mono tabular-nums ${pos.realized_pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                            {fmt(pos.realized_pnl)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        {/* Trades Tab */}
        <TabsContent value="trades">
          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base">Trade History</CardTitle>
              <div className="flex gap-2">
                <Button variant="outline" size="sm" onClick={handleExport} disabled={trades.length === 0}>
                  <Download className="h-4 w-4 mr-2" />
                  Export
                </Button>
                <Button variant="outline" size="sm" onClick={() => setShowImportDialog(true)}>
                  <Upload className="h-4 w-4 mr-2" />
                  Import CSV
                </Button>
                <Button size="sm" onClick={() => setShowAddTrade(true)}>
                  <Plus className="h-4 w-4 mr-2" />
                  Add Trade
                </Button>
              </div>
            </CardHeader>
            <CardContent className="p-0">
              {trades.length === 0 ? (
                <p className="px-4 pb-4 text-sm text-muted-foreground">No trades yet. Import a CSV or add trades manually.</p>
              ) : (
                <div className="overflow-x-auto">
                  <table className="w-full">
                    <thead>
                      <tr className="border-b bg-muted/50">
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Date</th>
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Symbol</th>
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Type</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Qty</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Price</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Fees</th>
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Order ID</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {trades.map((trade) => (
                        <tr key={trade.id} className="border-b">
                          <td className="h-12 px-4 text-sm">{trade.trade_date}</td>
                          <td className="h-12 px-4 font-medium">{trade.symbol}</td>
                          <td className="h-12 px-4">
                            <Badge variant={trade.trade_type === 'buy' ? 'default' : 'secondary'}>
                              {trade.trade_type.toUpperCase()}
                            </Badge>
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">{trade.quantity}</td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">{fmt(trade.price)}</td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums text-muted-foreground">{fmt(trade.fees)}</td>
                          <td className="h-12 px-4 text-sm text-muted-foreground font-mono">{trade.order_id || '—'}</td>
                          <td className="h-12 px-4 text-right">
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-destructive hover:text-destructive"
                              onClick={() => trade.id && handleDeleteTrade(trade.id)}
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        {/* Imports Tab */}
        <TabsContent value="imports">
          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base">Import History</CardTitle>
              <Button variant="outline" size="sm" onClick={() => setShowImportDialog(true)}>
                <Upload className="h-4 w-4 mr-2" />
                Import CSV
              </Button>
            </CardHeader>
            <CardContent className="p-0">
              {batches.length === 0 ? (
                <p className="px-4 pb-4 text-sm text-muted-foreground">No imports yet.</p>
              ) : (
                <div className="overflow-x-auto">
                  <table className="w-full">
                    <thead>
                      <tr className="border-b bg-muted/50">
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Filename</th>
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Account Type</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Rows</th>
                        <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Imported</th>
                        <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {batches.map((batch) => (
                        <tr key={batch.id} className="border-b">
                          <td className="h-12 px-4 font-medium truncate max-w-60">{batch.filename}</td>
                          <td className="h-12 px-4 text-sm">
                            <Select
                              value={batch.account_type || 'none'}
                              onValueChange={async (v) => {
                                const newType = v === 'none' ? null : v
                                try {
                                  await invoke('update_import_batch_account_type', {
                                    batchId: batch.id,
                                    accountType: newType,
                                  })
                                  toast.success(`Account type updated for ${batch.filename}`)
                                  await Promise.all([loadBatches(), loadPositions(), fetchClientAccounts(clientId)])
                                  // Auto-sync baselines after account type change
                                  invoke('sync_baseline_scenario', { clientId }).catch(() => {})
                                } catch (err) {
                                  toast.error(`Failed to update: ${errMsg(err)}`)
                                }
                              }}
                            >
                              <SelectTrigger className="h-8 w-48 text-xs">
                                <SelectValue placeholder="Set account type" />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="none">Not specified</SelectItem>
                                {ACCOUNT_TYPES.map((at) => (
                                  <SelectItem key={at.value} value={at.value}>
                                    {at.label}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                          </td>
                          <td className="h-12 px-4 text-right font-mono tabular-nums">{batch.row_count}</td>
                          <td className="h-12 px-4 text-sm text-muted-foreground">
                            {batch.imported_at ? new Date(batch.imported_at + 'Z').toLocaleString() : '—'}
                          </td>
                          <td className="h-12 px-4 text-right">
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-destructive hover:text-destructive"
                              onClick={() => setDeleteBatchTarget(batch)}
                            >
                              <Trash2 className="h-4 w-4 mr-1" />
                              Undo
                            </Button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {/* Edit Client Dialog */}
      <Dialog open={showEditDialog} onOpenChange={setShowEditDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Edit Client</DialogTitle>
            <DialogDescription>Update client information</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="edit-name">Name *</Label>
              <Input
                id="edit-name"
                value={editForm.name}
                onChange={(e) => setEditForm({ ...editForm, name: e.target.value })}
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="edit-broker">Broker</Label>
                <Input
                  id="edit-broker"
                  value={editForm.broker}
                  onChange={(e) => setEditForm({ ...editForm, broker: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="edit-account">Account ID</Label>
                <Input
                  id="edit-account"
                  value={editForm.account_id}
                  onChange={(e) => setEditForm({ ...editForm, account_id: e.target.value })}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="edit-account-type">Account Type</Label>
              <Select
                value={editForm.account_type || 'none'}
                onValueChange={(v) => setEditForm({ ...editForm, account_type: v === 'none' ? '' : v })}
              >
                <SelectTrigger id="edit-account-type">
                  <SelectValue placeholder="Select account type" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">Not specified</SelectItem>
                  {ACCOUNT_TYPES.map((at) => (
                    <SelectItem key={at.value} value={at.value}>
                      {at.label}
                      {!at.shortSellingAllowed && (
                        <span className="text-muted-foreground ml-1">(no short selling)</span>
                      )}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="edit-email">Email</Label>
                <Input
                  id="edit-email"
                  type="email"
                  value={editForm.email}
                  onChange={(e) => setEditForm({ ...editForm, email: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="edit-phone">Phone</Label>
                <Input
                  id="edit-phone"
                  value={editForm.phone}
                  onChange={(e) => setEditForm({ ...editForm, phone: e.target.value })}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="edit-notes">Notes</Label>
              <Input
                id="edit-notes"
                value={editForm.notes}
                onChange={(e) => setEditForm({ ...editForm, notes: e.target.value })}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowEditDialog(false)}>Cancel</Button>
            <Button onClick={handleEditClient} disabled={isSubmitting}>
              {isSubmitting && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Add Trade Dialog */}
      <Dialog open={showAddTrade} onOpenChange={setShowAddTrade}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Add Trade</DialogTitle>
            <DialogDescription>Manually add a trade record</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="trade-symbol">Symbol *</Label>
                <Input
                  id="trade-symbol"
                  value={tradeForm.symbol}
                  onChange={(e) => setTradeForm({ ...tradeForm, symbol: e.target.value })}
                  placeholder="AAPL"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="trade-exchange">Exchange</Label>
                <Input
                  id="trade-exchange"
                  value={tradeForm.exchange}
                  onChange={(e) => setTradeForm({ ...tradeForm, exchange: e.target.value })}
                />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="trade-date">Date *</Label>
                <Input
                  id="trade-date"
                  type="date"
                  value={tradeForm.trade_date}
                  onChange={(e) => setTradeForm({ ...tradeForm, trade_date: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="trade-type">Type *</Label>
                <Select
                  value={tradeForm.trade_type}
                  onValueChange={(v) => setTradeForm({ ...tradeForm, trade_type: v })}
                >
                  <SelectTrigger id="trade-type">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="buy">Buy</SelectItem>
                    <SelectItem value="sell">Sell</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="grid grid-cols-3 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="trade-qty">Quantity *</Label>
                <Input
                  id="trade-qty"
                  type="number"
                  step="any"
                  value={tradeForm.quantity}
                  onChange={(e) => setTradeForm({ ...tradeForm, quantity: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="trade-price">Price *</Label>
                <Input
                  id="trade-price"
                  type="number"
                  step="any"
                  value={tradeForm.price}
                  onChange={(e) => setTradeForm({ ...tradeForm, price: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="trade-fees">Fees</Label>
                <Input
                  id="trade-fees"
                  type="number"
                  step="any"
                  value={tradeForm.fees}
                  onChange={(e) => setTradeForm({ ...tradeForm, fees: e.target.value })}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="trade-order-id">Order ID</Label>
              <Input
                id="trade-order-id"
                value={tradeForm.order_id}
                onChange={(e) => setTradeForm({ ...tradeForm, order_id: e.target.value })}
                placeholder="Optional"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowAddTrade(false)}>Cancel</Button>
            <Button onClick={handleAddTrade} disabled={isSubmitting}>
              {isSubmitting && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
              Add Trade
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Import CSV Dialog */}
      <ImportCsvDialog
        open={showImportDialog}
        onOpenChange={setShowImportDialog}
        clientId={clientId}
        onSuccess={handleImportSuccess}
      />

      {/* Delete Import Batch Confirmation */}
      <Dialog open={!!deleteBatchTarget} onOpenChange={() => setDeleteBatchTarget(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Undo Import</DialogTitle>
            <DialogDescription>
              This will delete all {deleteBatchTarget?.row_count} trades from the import of <strong>{deleteBatchTarget?.filename}</strong>. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteBatchTarget(null)}>Cancel</Button>
            <Button variant="destructive" onClick={handleDeleteBatch}>
              Delete Trades
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
