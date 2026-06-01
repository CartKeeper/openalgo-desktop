import { Clipboard, Download, X } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { saveTextFile } from '@/lib/exportFile'
import type { Trade } from '@/api/backtest'
import { cn } from '@/lib/utils'

interface TradeTableProps {
  trades: Trade[]
}

function fmtDollar(v: number) {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
  }).format(v)
}

function fmtNum(v: number, digits = 4) {
  return v.toFixed(digits)
}

export function TradeTable({ trades }: TradeTableProps) {
  // Selection state — Set of trade index strings (trades have no unique id, so use index)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const anchorRef = useRef<string | null>(null)

  const tradeIds = trades.map((_, i) => String(i))

  const allSelected = selected.size === tradeIds.length && tradeIds.length > 0
  const someSelected = selected.size > 0 && !allSelected

  // Keyboard handler: Escape clears selection
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setSelected(new Set())
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  // Ctrl+A selects all when table has focus
  const handleTableKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'a') {
      e.preventDefault()
      setSelected(new Set(tradeIds))
    }
  }

  const toggleAll = () => {
    if (allSelected || someSelected) {
      setSelected(new Set())
      anchorRef.current = null
    } else {
      setSelected(new Set(tradeIds))
      anchorRef.current = null
    }
  }

  const handleRowClick = useCallback(
    (id: string, e: React.MouseEvent) => {
      e.preventDefault()

      setSelected((prev) => {
        const next = new Set(prev)

        if (e.shiftKey && anchorRef.current !== null) {
          // Shift+click: range select from anchor to id
          const anchorIdx = tradeIds.indexOf(anchorRef.current)
          const targetIdx = tradeIds.indexOf(id)
          const [lo, hi] = anchorIdx < targetIdx ? [anchorIdx, targetIdx] : [targetIdx, anchorIdx]
          for (let i = lo; i <= hi; i++) {
            next.add(tradeIds[i])
          }
          return next
        }

        if (e.ctrlKey || e.metaKey) {
          // Ctrl+click: additive toggle
          if (next.has(id)) {
            next.delete(id)
          } else {
            next.add(id)
          }
          anchorRef.current = id
          return next
        }

        // Plain click: select only this row
        if (next.size === 1 && next.has(id)) {
          next.clear()
          anchorRef.current = null
        } else {
          next.clear()
          next.add(id)
          anchorRef.current = id
        }
        return next
      })

      if (!e.shiftKey && !e.ctrlKey && !e.metaKey) {
        anchorRef.current = id
      }
    },
    [tradeIds]
  )

  const copySelected = async () => {
    const rows = Array.from(selected)
      .map(Number)
      .sort((a, b) => a - b)
      .map((i) => {
        const t = trades[i]
        return [
          t.entry_time,
          t.exit_time,
          fmtNum(t.entry_price),
          fmtNum(t.exit_price),
          fmtNum(t.shares),
          fmtNum(t.pnl_after_fees),
          fmtNum(t.fees),
          t.holding_days,
          t.long_term ? 'LT' : 'ST',
        ].join('\t')
      })
    const header = 'Entry\tExit\tEntry Price\tExit Price\tShares\tP&L\tFees\tDays\tTerm'
    await navigator.clipboard.writeText(header + '\n' + rows.join('\n'))
    toast.success(`${rows.length} trade${rows.length !== 1 ? 's' : ''} copied to clipboard`)
  }

  const exportCsv = async () => {
    const header = 'entry_time,exit_time,entry_price,exit_price,shares,pnl_after_fees,fees,holding_days,long_term'
    const rows = trades.map((t) =>
      [
        t.entry_time,
        t.exit_time,
        t.entry_price,
        t.exit_price,
        t.shares,
        t.pnl_after_fees,
        t.fees,
        t.holding_days,
        t.long_term,
      ].join(',')
    )
    await saveTextFile('backtest-trades.csv', [header, ...rows].join('\n'))
  }

  if (trades.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center">
        <p className="text-sm font-semibold text-foreground/70">No trades</p>
        <p className="text-xs text-muted-foreground mt-1">The strategy produced no round-trip trades in this date range.</p>
      </div>
    )
  }

  return (
    <div>
      {/* Action bar — appears when ≥1 selected */}
      {selected.size > 0 && (
        <div
          data-action-bar="true"
          className="flex items-center justify-between px-4 py-2 mb-2 rounded-lg bg-accent/10 border border-accent/30"
        >
          <div className="flex items-center gap-3">
            <span className="text-sm font-medium text-accent-foreground">
              {selected.size} selected
            </span>
            <button
              type="button"
              className="text-xs text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1"
              onClick={() => { setSelected(new Set()); anchorRef.current = null }}
            >
              <X className="w-3 h-3" /> Clear
            </button>
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              className="h-8 gap-1.5 text-xs"
              onClick={copySelected}
            >
              <Clipboard className="w-3.5 h-3.5" />
              Copy
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-8 gap-1.5 text-xs"
              onClick={exportCsv}
            >
              <Download className="w-3.5 h-3.5" />
              Export CSV
            </Button>
          </div>
        </div>
      )}

      <div
        className="overflow-x-auto rounded-lg border border-border"
        onKeyDown={handleTableKeyDown}
        tabIndex={0}
      >
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-muted/40" style={{ height: 40 }}>
              {/* Master checkbox */}
              <th className="w-10 px-3">
                <input
                  type="checkbox"
                  data-select-all="true"
                  checked={allSelected}
                  ref={(el) => {
                    if (el) el.indeterminate = someSelected
                  }}
                  onChange={toggleAll}
                  className="w-3.5 h-3.5 cursor-pointer accent-accent"
                  aria-label="Select all trades"
                />
              </th>
              <th className="text-left px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground whitespace-nowrap">Entry</th>
              <th className="text-left px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground whitespace-nowrap">Exit</th>
              <th className="text-right px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Entry $</th>
              <th className="text-right px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Exit $</th>
              <th className="text-right px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Shares</th>
              <th className="text-right px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">P&L</th>
              <th className="text-right px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Fees</th>
              <th className="text-right px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Days</th>
              <th className="text-center px-4 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Term</th>
            </tr>
          </thead>
          <tbody>
            {trades.map((trade, i) => {
              const id = String(i)
              const isSelected = selected.has(id)
              const isWin = trade.pnl_after_fees >= 0

              return (
                <tr
                  key={id}
                  data-selectable="true"
                  data-selected={isSelected}
                  data-item-id={id}
                  aria-selected={isSelected}
                  onClick={(e) => handleRowClick(id, e)}
                  className={cn(
                    'border-b border-border/60 last:border-0 cursor-pointer transition-colors',
                    isSelected
                      ? 'bg-accent/8 border-l-[3px] border-l-accent'
                      : 'hover:bg-muted/30'
                  )}
                  style={{ height: 48 }}
                >
                  <td className="px-3" onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      checked={isSelected}
                      onChange={(e) => {
                        e.stopPropagation()
                        handleRowClick(id, e as unknown as React.MouseEvent)
                      }}
                      className="w-3.5 h-3.5 cursor-pointer accent-accent"
                      aria-label={`Select trade ${i + 1}`}
                    />
                  </td>
                  <td className="px-4 text-foreground font-medium whitespace-nowrap">{trade.entry_time.slice(0, 10)}</td>
                  <td className="px-4 text-foreground/80 whitespace-nowrap">{trade.exit_time.slice(0, 10)}</td>
                  <td className="px-4 text-right font-mono tabular-nums text-foreground/80">{fmtNum(trade.entry_price)}</td>
                  <td className="px-4 text-right font-mono tabular-nums text-foreground/80">{fmtNum(trade.exit_price)}</td>
                  <td className="px-4 text-right font-mono tabular-nums text-foreground/70">{fmtNum(trade.shares, 2)}</td>
                  <td className={cn('px-4 text-right font-mono tabular-nums font-semibold', isWin ? 'text-emerald-500' : 'text-rose-500')}>
                    {fmtDollar(trade.pnl_after_fees)}
                  </td>
                  <td className="px-4 text-right font-mono tabular-nums text-muted-foreground">{fmtDollar(trade.fees)}</td>
                  <td className="px-4 text-right font-mono tabular-nums text-foreground/70">{trade.holding_days}</td>
                  <td className="px-4 text-center">
                    <span className={cn(
                      'inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wider',
                      trade.long_term
                        ? 'bg-blue-500/10 text-blue-500'
                        : 'bg-amber-500/10 text-amber-500'
                    )}>
                      {trade.long_term ? 'LT' : 'ST'}
                    </span>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      {/* Footer controls: export CSV always visible */}
      <div className="flex items-center justify-between mt-3">
        <span className="text-xs text-muted-foreground">
          {trades.length} total trade{trades.length !== 1 ? 's' : ''}
          {selected.size > 0 ? ` · ${selected.size} selected` : ''}
        </span>
        {selected.size === 0 && (
          <Button
            size="sm"
            variant="outline"
            className="h-8 gap-1.5 text-xs"
            onClick={exportCsv}
          >
            <Download className="w-3.5 h-3.5" />
            Export CSV
          </Button>
        )}
      </div>
    </div>
  )
}
