import { invoke } from '@tauri-apps/api/core'
import {
  AlertTriangle,
  CheckCircle2,
  FileJson,
  FileSpreadsheet,
  HelpCircle,
  Loader2,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
  Upload,
  X,
} from 'lucide-react'
import { useRef, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { downloadGoldmanBriefPdf } from '@/components/reports/goldman/generator'
import type { GoldmanBrief } from '@/components/reports/goldman/types'
import type { ImportReport } from '@/types/clients'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err) return (err as { message: string }).message
  return String(err)
}

interface ImportDocumentsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  clientId: number
  onSuccess: (report: ImportReport) => void
  /** When true, rendered as part of a wizard — Cancel hidden, success auto-closes upstream */
  embeddedWizard?: boolean
  /**
   * Called when the user clicks a "View" action on a violation row. The parent
   * is expected to navigate to ClientDetail and highlight the offending row
   * (typically by passing `state.highlight` to react-router navigate()).
   */
  onViewViolation?: (violation: import('@/types/clients').ComplianceViolation) => void
}

interface FilePayload {
  filename: string
  content: string
  size: number
}

function detectTransactionsKind(filename: string): 'json' | 'csv' | 'unknown' {
  const lower = filename.toLowerCase()
  if (lower.endsWith('.json')) return 'json'
  if (lower.endsWith('.csv')) return 'csv'
  return 'unknown'
}

function fmtMoney(n: number): string {
  return n.toLocaleString('en-US', { style: 'currency', currency: 'USD' })
}

function fmtQty(n: number): string {
  return n.toLocaleString('en-US', { maximumFractionDigits: 4 })
}

export default function ImportCsvDialog({
  open,
  onOpenChange,
  clientId,
  onSuccess,
  embeddedWizard,
  onViewViolation,
}: ImportDocumentsDialogProps) {
  const txnRef = useRef<HTMLInputElement>(null)
  const orderRef = useRef<HTMLInputElement>(null)

  const [transactions, setTransactions] = useState<FilePayload | null>(null)
  const [orderStatus, setOrderStatus] = useState<FilePayload | null>(null)
  const [report, setReport] = useState<ImportReport | null>(null)
  const [isImporting, setIsImporting] = useState(false)
  const [isGeneratingBrief, setIsGeneratingBrief] = useState(false)
  const [showHelp, setShowHelp] = useState(false)

  const reset = () => {
    setTransactions(null)
    setOrderStatus(null)
    setReport(null)
    setIsImporting(false)
    if (txnRef.current) txnRef.current.value = ''
    if (orderRef.current) orderRef.current.value = ''
  }

  const handlePick = async (
    e: React.ChangeEvent<HTMLInputElement>,
    target: 'transactions' | 'order_status',
  ) => {
    const file = e.target.files?.[0]
    if (!file) return
    const content = await file.text()
    const payload: FilePayload = { filename: file.name, content, size: file.size }
    if (target === 'transactions') setTransactions(payload)
    else setOrderStatus(payload)
    setReport(null)
  }

  const handleImport = async () => {
    if (!transactions) return
    setIsImporting(true)
    try {
      const result = await invoke<ImportReport>('import_schwab_documents', {
        clientId,
        transactionsFilename: transactions.filename,
        transactionsContent: transactions.content,
        orderStatusFilename: orderStatus?.filename ?? null,
        orderStatusContent: orderStatus?.content ?? null,
      })
      setReport(result)
      // Soft-fail: import always succeeds. Violations are persisted as flags
      // so the user can review them on ClientDetail and mark them resolved.
      const violCount = result.summary.violation_count
      if (violCount === 0) {
        toast.success(
          `Imported: ${result.summary.total_holdings} holdings, ${result.open_orders.length} open orders. 401(k) compliant.`,
        )
      } else {
        toast.warning(
          `Imported with ${violCount} 401(k) flag${violCount === 1 ? '' : 's'} — review on the client page.`,
        )
      }
      // Always notify parent so the wizard can finalize. Generate-Brief gating
      // happens inside the report view (compliant only).
      onSuccess(result)
    } catch (err) {
      toast.error(`Import failed: ${errMsg(err)}`)
    } finally {
      setIsImporting(false)
    }
  }

  const handleResolveViolation = async (violationId: number, reason: string) => {
    try {
      await invoke('resolve_compliance_violation', { violationId, reason })
      // Optimistically update local report state
      setReport((prev) => {
        if (!prev) return prev
        const violations = prev.violations.map((v) =>
          v.id === violationId
            ? { ...v, resolved: true, resolved_reason: reason }
            : v,
        )
        const stillUnresolved = violations.filter((v) => !v.resolved).length
        return {
          ...prev,
          violations,
          summary: {
            ...prev.summary,
            violation_count: stillUnresolved,
            is_compliant: stillUnresolved === 0,
          },
        }
      })
      toast.success('Violation marked resolved.')
    } catch (err) {
      toast.error(`Failed to resolve: ${errMsg(err)}`)
    }
  }

  const handleClose = (next: boolean) => {
    if (!next) reset()
    onOpenChange(next)
  }

  const handleGenerateBrief = async () => {
    if (!report) return
    setIsGeneratingBrief(true)
    try {
      const brief = await invoke<GoldmanBrief>('generate_client_brief', {
        clientId: report.client_id,
      })
      await downloadGoldmanBriefPdf(brief)
      toast.success('Goldman Sax & Violins brief generated and downloaded.')
    } catch (err) {
      toast.error(`Brief generation failed: ${errMsg(err)}`)
    } finally {
      setIsGeneratingBrief(false)
    }
  }

  const txnKind = transactions ? detectTransactionsKind(transactions.filename) : null
  const canImport = !!transactions && !isImporting
  const violations = report?.violations ?? []
  const isBlocked = report ? !report.summary.is_compliant : false

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-3xl h-[75vh] flex flex-col p-0">
        <DialogHeader className="px-6 pt-6 pb-3 border-b shrink-0">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1.5">
              <DialogTitle>Import Schwab Documents</DialogTitle>
              <DialogDescription>
                Upload a Schwab Transactions file (required) and an optional Order Status file. Strict 401(k) rules are enforced — any violation blocks import.
              </DialogDescription>
            </div>
            <button
              type="button"
              onClick={() => setShowHelp((v) => !v)}
              className="text-muted-foreground hover:text-foreground"
              aria-label="Help"
            >
              <HelpCircle className="h-5 w-5" strokeWidth={1.5} />
            </button>
          </div>
        </DialogHeader>

        <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 space-y-4">
          {showHelp && (
            <div className="rounded-lg border bg-muted/40 p-3 text-xs space-y-1.5">
              <p className="font-semibold text-sm">How to export from Schwab</p>
              <p>
                <strong>Transactions:</strong> History → Export → choose JSON or CSV. Required.
              </p>
              <p>
                <strong>Order Status:</strong> Order Status → Export to CSV. Optional, but recommended — enables open-order detection and cross-validation against the transaction ledger.
              </p>
              <p className="text-muted-foreground pt-1">
                On any mismatch between the two files, transactions win (the holdings always reflect the transaction ledger). Mismatches are surfaced for review.
              </p>
            </div>
          )}

          {/* File pickers */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <FilePicker
              label="Transactions (JSON or CSV)"
              required
              file={transactions}
              kind={txnKind}
              onChoose={() => txnRef.current?.click()}
              onClear={() => {
                setTransactions(null)
                if (txnRef.current) txnRef.current.value = ''
                setReport(null)
              }}
              disabled={isImporting}
            />
            <FilePicker
              label="Order Status (CSV) — optional"
              file={orderStatus}
              kind={orderStatus ? 'csv' : null}
              onChoose={() => orderRef.current?.click()}
              onClear={() => {
                setOrderStatus(null)
                if (orderRef.current) orderRef.current.value = ''
                setReport(null)
              }}
              disabled={isImporting}
            />
            <input
              ref={txnRef}
              type="file"
              accept=".json,.csv"
              className="hidden"
              onChange={(e) => handlePick(e, 'transactions')}
            />
            <input
              ref={orderRef}
              type="file"
              accept=".csv"
              className="hidden"
              onChange={(e) => handlePick(e, 'order_status')}
            />
          </div>

          {/* Report */}
          {report && (
            <div className="space-y-4">
              <SummaryCard report={report} />

              {report.summary.is_compliant && (
                <div className="flex items-center justify-between rounded-lg border border-amber-300/60 bg-amber-50/40 dark:border-amber-900/60 dark:bg-amber-950/20 px-4 py-3">
                  <div className="space-y-0.5">
                    <p className="text-sm font-semibold flex items-center gap-2">
                      <Sparkles className="h-4 w-4 text-amber-600" strokeWidth={1.75} />
                      Generate Goldman Sax &amp; Violins brief
                    </p>
                    <p className="text-xs text-muted-foreground">
                      Four-movement portfolio brief authored by Claude from this client's data. Downloads as PDF.
                    </p>
                  </div>
                  <Button
                    size="sm"
                    onClick={handleGenerateBrief}
                    disabled={isGeneratingBrief}
                  >
                    {isGeneratingBrief && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                    {isGeneratingBrief ? 'Composing…' : 'Generate Brief'}
                  </Button>
                </div>
              )}

              <ComplianceTable
                violations={violations}
                onResolve={handleResolveViolation}
                onView={onViewViolation}
              />

              <HoldingsSection report={report} />

              <OpenOrdersSection report={report} />

              {report.reconciliation_mismatches.length > 0 && (
                <MismatchSection report={report} />
              )}
            </div>
          )}
        </div>

        <DialogFooter className="px-6 py-3 border-t shrink-0">
          {!embeddedWizard && (
            <Button variant="outline" onClick={() => handleClose(false)} disabled={isImporting}>
              {report ? 'Close' : 'Cancel'}
            </Button>
          )}
          {!report ? (
            <Button onClick={handleImport} disabled={!canImport}>
              {isImporting && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
              Validate &amp; Import
            </Button>
          ) : (
            <Button onClick={() => handleClose(false)}>
              {isBlocked
                ? `Continue with ${report.summary.violation_count} flag${report.summary.violation_count === 1 ? '' : 's'}`
                : 'Done'}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

// ---------------------------------------------------------------------------
// Subcomponents
// ---------------------------------------------------------------------------

function FilePicker(props: {
  label: string
  required?: boolean
  file: FilePayload | null
  kind: 'json' | 'csv' | 'unknown' | null
  onChoose: () => void
  onClear: () => void
  disabled?: boolean
}) {
  const { label, required, file, kind, onChoose, onClear, disabled } = props
  const Icon = kind === 'json' ? FileJson : FileSpreadsheet

  return (
    <div className="rounded-lg border p-3 space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {label} {required && <span className="text-destructive">*</span>}
        </span>
        {file && (
          <button
            type="button"
            onClick={onClear}
            disabled={disabled}
            className="text-muted-foreground hover:text-foreground disabled:opacity-50"
            aria-label="Remove file"
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </div>
      {file ? (
        <div className="flex items-center gap-2 text-sm">
          <Icon className="h-4 w-4 text-muted-foreground shrink-0" />
          <span className="truncate">{file.filename}</span>
          <span className="ml-auto text-xs text-muted-foreground tabular-nums">
            {(file.size / 1024).toFixed(1)} KB
          </span>
        </div>
      ) : (
        <Button
          variant="outline"
          size="sm"
          className="h-8 w-full"
          onClick={onChoose}
          disabled={disabled}
        >
          <Upload className="h-4 w-4 mr-2" />
          Choose File
        </Button>
      )}
    </div>
  )
}

function SummaryCard({ report }: { report: ImportReport }) {
  const s = report.summary
  const compliant = s.is_compliant
  return (
    <div
      className={`rounded-lg border p-4 ${compliant ? 'border-emerald-200 bg-emerald-50/40 dark:border-emerald-900 dark:bg-emerald-950/20' : 'border-destructive/30 bg-destructive/5'}`}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          {compliant ? (
            <ShieldCheck className="h-5 w-5 text-emerald-600" strokeWidth={1.75} />
          ) : (
            <ShieldAlert className="h-5 w-5 text-destructive" strokeWidth={1.75} />
          )}
          <span className="text-sm font-semibold">
            {compliant ? '401(k) Compliant' : `Blocked — ${s.violation_count} violation${s.violation_count === 1 ? '' : 's'}`}
          </span>
        </div>
      </div>
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-sm">
        <Stat label="Holdings" value={String(s.total_holdings)} />
        <Stat label="Cost basis" value={fmtMoney(s.total_cost_basis)} />
        <Stat
          label="Open orders"
          value={`${s.open_buy_orders + s.open_sell_orders}`}
          sub={`${s.open_buy_orders} buy / ${s.open_sell_orders} sell`}
        />
        <Stat
          label="Transactions"
          value={String(s.transactions_processed)}
          sub={s.order_status_processed > 0 ? `${s.order_status_processed} orders parsed` : 'no order status file'}
        />
      </div>
    </div>
  )
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div>
      <div className="text-[10px] uppercase tracking-wider text-muted-foreground font-semibold">{label}</div>
      <div className="text-base font-semibold tabular-nums">{value}</div>
      {sub && <div className="text-[10px] text-muted-foreground tabular-nums">{sub}</div>}
    </div>
  )
}

/**
 * Sortable table of 401(k) violations with View / Mark Resolved actions.
 * Used inside the import dialog and (re-exported below) on ClientDetail.
 */
export function ComplianceTable({
  violations,
  onResolve,
  onView,
}: {
  violations: ImportReport['violations']
  onResolve?: (violationId: number, reason: string) => void | Promise<void>
  onView?: (v: ImportReport['violations'][number]) => void
}) {
  const [resolveTarget, setResolveTarget] = useState<ImportReport['violations'][number] | null>(null)
  const [reason, setReason] = useState('')
  const [isResolving, setIsResolving] = useState(false)
  const [sort, setSort] = useState<{ key: 'symbol' | 'type' | 'qty' | 'status'; dir: 'asc' | 'desc' }>({
    key: 'status',
    dir: 'asc',
  })

  if (violations.length === 0) {
    return (
      <div className="rounded-lg border p-3 flex items-center gap-2 text-sm">
        <CheckCircle2 className="h-4 w-4 text-emerald-600" />
        <span>No 401(k) violations detected.</span>
      </div>
    )
  }

  const sorted = [...violations].sort((a, b) => {
    const dir = sort.dir === 'asc' ? 1 : -1
    switch (sort.key) {
      case 'symbol':
        return ((a.symbol ?? '').localeCompare(b.symbol ?? '')) * dir
      case 'type':
        return a.violation_type.localeCompare(b.violation_type) * dir
      case 'qty':
        return ((a.quantity ?? 0) - (b.quantity ?? 0)) * dir
      case 'status':
        return ((Number(a.resolved) - Number(b.resolved))) * dir
    }
  })

  const toggleSort = (key: typeof sort.key) =>
    setSort((s) => (s.key === key ? { key, dir: s.dir === 'asc' ? 'desc' : 'asc' } : { key, dir: 'asc' }))

  const sortIndicator = (key: typeof sort.key) =>
    sort.key === key ? (sort.dir === 'asc' ? ' ↑' : ' ↓') : ''

  const unresolvedCount = violations.filter((v) => !v.resolved).length

  return (
    <>
      <div className="rounded-lg border border-amber-300 dark:border-amber-900 overflow-hidden">
        <div className="px-3 py-2 bg-amber-100/60 dark:bg-amber-950/30 border-b border-amber-300 dark:border-amber-900 flex items-center gap-2">
          <ShieldAlert className="h-4 w-4 text-amber-600" />
          <span className="text-sm font-semibold">401(k) Compliance Flags</span>
          <span className="ml-auto text-xs text-muted-foreground tabular-nums">
            {unresolvedCount} unresolved · {violations.length} total
          </span>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/20">
                <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  <button onClick={() => toggleSort('symbol')} className="hover:text-foreground">
                    Symbol{sortIndicator('symbol')}
                  </button>
                </th>
                <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  <button onClick={() => toggleSort('type')} className="hover:text-foreground">
                    Type{sortIndicator('type')}
                  </button>
                </th>
                <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  <button onClick={() => toggleSort('qty')} className="hover:text-foreground">
                    Qty{sortIndicator('qty')}
                  </button>
                </th>
                <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Reason
                </th>
                <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  <button onClick={() => toggleSort('status')} className="hover:text-foreground">
                    Status{sortIndicator('status')}
                  </button>
                </th>
                <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Action
                </th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((v, i) => (
                <tr
                  key={v.id ?? i}
                  className={`border-b last:border-b-0 ${v.resolved ? 'opacity-50' : ''}`}
                >
                  <td className="h-12 px-3 font-mono font-semibold">{v.symbol ?? '—'}</td>
                  <td className="h-12 px-3 text-xs uppercase tracking-wider text-muted-foreground">
                    {v.violation_type.replace(/_/g, ' ')}
                  </td>
                  <td className="h-12 px-3 text-right tabular-nums">
                    {v.quantity != null ? v.quantity.toLocaleString('en-US', { maximumFractionDigits: 4 }) : '—'}
                  </td>
                  <td className="h-12 px-3 text-xs text-muted-foreground max-w-md truncate" title={v.message}>
                    {v.message}
                  </td>
                  <td className="h-12 px-3 text-xs">
                    {v.resolved ? (
                      <span className="inline-flex items-center gap-1 text-emerald-700 dark:text-emerald-400">
                        <CheckCircle2 className="h-3.5 w-3.5" />
                        Resolved
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-destructive">
                        <AlertTriangle className="h-3.5 w-3.5" />
                        Flagged
                      </span>
                    )}
                  </td>
                  <td className="h-12 px-3 text-right">
                    <div className="inline-flex items-center gap-1">
                      {onView && (
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 px-2 text-xs"
                          onClick={() => onView(v)}
                        >
                          View
                        </Button>
                      )}
                      {!v.resolved && onResolve && v.id != null && (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 px-2 text-xs"
                          onClick={() => {
                            setResolveTarget(v)
                            setReason('')
                          }}
                        >
                          Mark Resolved
                        </Button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Resolve confirmation dialog */}
      <Dialog open={!!resolveTarget} onOpenChange={(o) => { if (!o) setResolveTarget(null) }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Mark violation resolved</DialogTitle>
            <DialogDescription>
              Provide a short reason for the audit trail. This is stored with the violation and visible later on the client page.
            </DialogDescription>
          </DialogHeader>
          {resolveTarget && (
            <div className="space-y-3">
              <div className="rounded-md border bg-muted/30 px-3 py-2 text-xs space-y-1">
                <div>
                  <span className="font-mono font-semibold">{resolveTarget.symbol ?? '—'}</span>
                  <span className="text-muted-foreground ml-2 uppercase tracking-wider">
                    {resolveTarget.violation_type.replace(/_/g, ' ')}
                  </span>
                </div>
                <div className="text-muted-foreground">{resolveTarget.message}</div>
              </div>
              <textarea
                className="w-full min-h-[80px] rounded-md border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                placeholder='e.g. "Sold next trading day" or "Plan permits this fund per 2026 SPD update"'
                value={reason}
                onChange={(e) => setReason(e.target.value)}
              />
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setResolveTarget(null)} disabled={isResolving}>
              Cancel
            </Button>
            <Button
              onClick={async () => {
                if (!resolveTarget?.id || !onResolve) return
                if (!reason.trim()) {
                  toast.error('Please enter a reason for the audit trail.')
                  return
                }
                setIsResolving(true)
                try {
                  await onResolve(resolveTarget.id, reason.trim())
                  setResolveTarget(null)
                } finally {
                  setIsResolving(false)
                }
              }}
              disabled={isResolving || !reason.trim()}
            >
              {isResolving && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
              Confirm Resolve
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

function HoldingsSection({ report }: { report: ImportReport }) {
  if (report.holdings.length === 0) {
    return (
      <div className="rounded-lg border p-3 text-sm text-muted-foreground">
        No holdings reconstructed from transactions.
      </div>
    )
  }
  return (
    <div className="rounded-lg border overflow-hidden">
      <div className="px-3 py-2 bg-muted/50 border-b text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        Reconstructed Holdings ({report.holdings.length})
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b bg-muted/20">
              <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Symbol</th>
              <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Qty</th>
              <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Avg Cost</th>
              <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Total Cost</th>
              <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Realized P&amp;L</th>
            </tr>
          </thead>
          <tbody>
            {report.holdings.map((h) => (
              <tr key={h.symbol} className="border-b last:border-b-0">
                <td className="h-12 px-3 font-mono font-semibold">{h.symbol}</td>
                <td className="h-12 px-3 text-right tabular-nums">{fmtQty(h.quantity)}</td>
                <td className="h-12 px-3 text-right tabular-nums">{fmtMoney(h.avg_cost)}</td>
                <td className="h-12 px-3 text-right tabular-nums">{fmtMoney(h.total_cost)}</td>
                <td className={`h-12 px-3 text-right tabular-nums ${h.realized_pnl > 0 ? 'text-emerald-600' : h.realized_pnl < 0 ? 'text-destructive' : ''}`}>
                  {fmtMoney(h.realized_pnl)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function OpenOrdersSection({ report }: { report: ImportReport }) {
  if (report.open_orders.length === 0) return null
  return (
    <div className="rounded-lg border overflow-hidden">
      <div className="px-3 py-2 bg-muted/50 border-b text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        Open Orders ({report.open_orders.length})
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b bg-muted/20">
              <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Symbol</th>
              <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Action</th>
              <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Qty</th>
              <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Type</th>
              <th className="h-10 px-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Trigger</th>
              <th className="h-10 px-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">TIF</th>
            </tr>
          </thead>
          <tbody>
            {report.open_orders.map((o, i) => (
              <tr key={`${o.symbol}-${i}`} className="border-b last:border-b-0">
                <td className="h-12 px-3 font-mono font-semibold">{o.symbol}</td>
                <td className={`h-12 px-3 text-xs font-semibold uppercase ${o.action.toLowerCase() === 'buy' ? 'text-emerald-600' : 'text-destructive'}`}>{o.action}</td>
                <td className="h-12 px-3 text-right tabular-nums">{fmtQty(o.quantity)}</td>
                <td className="h-12 px-3 text-xs text-muted-foreground">{o.order_type ?? '—'}</td>
                <td className="h-12 px-3 text-right tabular-nums text-xs">
                  {o.stop_price != null
                    ? `Stop ${fmtMoney(o.stop_price)}`
                    : o.limit_price != null
                      ? `Limit ${fmtMoney(o.limit_price)}`
                      : '—'}
                </td>
                <td className="h-12 px-3 text-xs">{o.time_in_force ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function MismatchSection({ report }: { report: ImportReport }) {
  return (
    <div className="rounded-lg border border-amber-300 dark:border-amber-800 overflow-hidden">
      <div className="px-3 py-2 bg-amber-100/60 dark:bg-amber-950/30 border-b border-amber-300 dark:border-amber-800 flex items-center gap-2">
        <AlertTriangle className="h-4 w-4 text-amber-600" />
        <span className="text-sm font-semibold">Reconciliation Mismatches (transactions kept)</span>
        <span className="ml-auto text-xs text-muted-foreground tabular-nums">{report.reconciliation_mismatches.length}</span>
      </div>
      <ul className="divide-y">
        {report.reconciliation_mismatches.map((m, i) => (
          <li key={i} className="px-3 py-2 text-sm">
            <div className="font-mono text-xs">{m.symbol} • {m.action} • {m.mismatch_kind.replace(/_/g, ' ')}</div>
            <div className="text-xs text-muted-foreground">{m.note}</div>
          </li>
        ))}
      </ul>
    </div>
  )
}
