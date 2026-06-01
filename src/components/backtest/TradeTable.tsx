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

interface ContextMenu {
  x: number
  y: number
  rowId: string
}

export function TradeTable({ trades }: TradeTableProps) {
  // Selection state — Set of trade index strings (trades have no unique id, so use index)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const anchorRef = useRef<string | null>(null)
  // Track the set of IDs added by the last shift-range so we can replace it
  const lastShiftRangeRef = useRef<Set<string>>(new Set())
  // Focused row for keyboard navigation
  const [focusedIdx, setFocusedIdx] = useState<number | null>(null)
  // aria-live announcement
  const [announcement, setAnnouncement] = useState('')
  // Context menu state
  const [contextMenu, setContextMenu] = useState<ContextMenu | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  const tradeIds = trades.map((_, i) => String(i))

  const allSelected = selected.size === tradeIds.length && tradeIds.length > 0
  const someSelected = selected.size > 0 && !allSelected

  // Announce selection count changes
  useEffect(() => {
    if (selected.size > 0) {
      setAnnouncement(`${selected.size} trade${selected.size !== 1 ? 's' : ''} selected`)
    } else {
      setAnnouncement('')
    }
  }, [selected.size])

  // Close context menu on outside click, Escape, or scroll
  useEffect(() => {
    if (!contextMenu) return
    const handleClick = () => setContextMenu(null)
    const handleScroll = () => setContextMenu(null)
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setContextMenu(null)
    }
    document.addEventListener('mousedown', handleClick)
    document.addEventListener('scroll', handleScroll, true)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('mousedown', handleClick)
      document.removeEventListener('scroll', handleScroll, true)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [contextMenu])

  // Global Escape clears selection; handled in table keydown below for table focus,
  // but keep global handler for safety when focus is elsewhere
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setSelected(new Set())
        lastShiftRangeRef.current = new Set()
        anchorRef.current = null
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  const toggleAll = () => {
    if (allSelected || someSelected) {
      setSelected(new Set())
      lastShiftRangeRef.current = new Set()
      anchorRef.current = null
    } else {
      setSelected(new Set(tradeIds))
      lastShiftRangeRef.current = new Set()
      anchorRef.current = null
    }
  }

  const handleRowClick = useCallback(
    (id: string, e: React.MouseEvent) => {
      e.preventDefault()

      setSelected((prev) => {
        const next = new Set(prev)

        if (e.shiftKey && anchorRef.current !== null) {
          // Shift+click: REPLACE last shift-range with new anchor→target range
          // Remove the previously shift-selected IDs first
          for (const prevId of lastShiftRangeRef.current) {
            next.delete(prevId)
          }

          const anchorIdx = tradeIds.indexOf(anchorRef.current)
          const targetIdx = tradeIds.indexOf(id)
          const [lo, hi] = anchorIdx < targetIdx ? [anchorIdx, targetIdx] : [targetIdx, anchorIdx]
          const newRange = new Set<string>()
          for (let i = lo; i <= hi; i++) {
            newRange.add(tradeIds[i])
          }
          // Add new range
          for (const rangeId of newRange) {
            next.add(rangeId)
          }
          lastShiftRangeRef.current = newRange
          return next
        }

        if (e.ctrlKey || e.metaKey) {
          // Ctrl+click: additive toggle — clears shift range tracking
          lastShiftRangeRef.current = new Set()
          if (next.has(id)) {
            next.delete(id)
          } else {
            next.add(id)
          }
          anchorRef.current = id
          return next
        }

        // Plain click: select only this row
        lastShiftRangeRef.current = new Set()
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

      if (!e.shiftKey) {
        if (!e.ctrlKey && !e.metaKey) {
          anchorRef.current = id
        }
        setFocusedIdx(tradeIds.indexOf(id))
      }
    },
    [tradeIds]
  )

  // Keyboard navigation handler on table container
  const handleTableKeyDown = (e: React.KeyboardEvent) => {
    // Ctrl/Cmd+A: select all
    if ((e.ctrlKey || e.metaKey) && e.key === 'a') {
      e.preventDefault()
      setSelected(new Set(tradeIds))
      lastShiftRangeRef.current = new Set()
      return
    }

    // Escape: clear selection
    if (e.key === 'Escape') {
      e.preventDefault()
      setSelected(new Set())
      lastShiftRangeRef.current = new Set()
      anchorRef.current = null
      return
    }

    const currentIdx = focusedIdx ?? -1

    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const delta = e.key === 'ArrowDown' ? 1 : -1
      const nextIdx = Math.max(0, Math.min(tradeIds.length - 1, currentIdx + delta))

      if (e.shiftKey && anchorRef.current !== null) {
        // Shift+Arrow: extend selection from anchor to nextIdx (replace last shift range)
        setSelected((prev) => {
          const next = new Set(prev)
          for (const prevId of lastShiftRangeRef.current) {
            next.delete(prevId)
          }
          const anchorIdx = tradeIds.indexOf(anchorRef.current!)
          const [lo, hi] = anchorIdx < nextIdx ? [anchorIdx, nextIdx] : [nextIdx, anchorIdx]
          const newRange = new Set<string>()
          for (let i = lo; i <= hi; i++) {
            newRange.add(tradeIds[i])
          }
          for (const rangeId of newRange) {
            next.add(rangeId)
          }
          lastShiftRangeRef.current = newRange
          return next
        })
      }

      setFocusedIdx(nextIdx)
      return
    }

    if ((e.key === 'Home' || e.key === 'End') && e.shiftKey && anchorRef.current !== null) {
      e.preventDefault()
      const targetIdx = e.key === 'Home' ? 0 : tradeIds.length - 1
      setSelected((prev) => {
        const next = new Set(prev)
        for (const prevId of lastShiftRangeRef.current) {
          next.delete(prevId)
        }
        const anchorIdx = tradeIds.indexOf(anchorRef.current!)
        const [lo, hi] = anchorIdx < targetIdx ? [anchorIdx, targetIdx] : [targetIdx, anchorIdx]
        const newRange = new Set<string>()
        for (let i = lo; i <= hi; i++) {
          newRange.add(tradeIds[i])
        }
        for (const rangeId of newRange) {
          next.add(rangeId)
        }
        lastShiftRangeRef.current = newRange
        return next
      })
      setFocusedIdx(targetIdx)
      return
    }

    // Space: toggle focused row
    if (e.key === ' ' && currentIdx >= 0) {
      e.preventDefault()
      const id = tradeIds[currentIdx]
      lastShiftRangeRef.current = new Set()
      setSelected((prev) => {
        const next = new Set(prev)
        if (next.has(id)) {
          next.delete(id)
        } else {
          next.add(id)
          anchorRef.current = id
        }
        return next
      })
    }
  }

  const copySelected = async (idsToUse?: Set<string>) => {
    const ids = idsToUse ?? selected
    const rows = Array.from(ids)
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

  const handleContextMenu = (e: React.MouseEvent, id: string) => {
    e.preventDefault()
    // OS behavior: right-clicking an unselected row selects only it
    if (!selected.has(id)) {
      setSelected(new Set([id]))
      lastShiftRangeRef.current = new Set()
      anchorRef.current = id
    }
    setContextMenu({ x: e.clientX, y: e.clientY, rowId: id })
  }

  const handleContextCopy = async () => {
    setContextMenu(null)
    await copySelected()
  }

  const handleContextExport = async () => {
    setContextMenu(null)
    await exportCsv()
  }

  const handleContextSelectAll = () => {
    setContextMenu(null)
    setSelected(new Set(tradeIds))
    lastShiftRangeRef.current = new Set()
  }

  const handleContextClearSelection = () => {
    setContextMenu(null)
    setSelected(new Set())
    lastShiftRangeRef.current = new Set()
    anchorRef.current = null
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
      {/* aria-live region for screen reader announcements */}
      <span
        aria-live="polite"
        aria-atomic="true"
        className="sr-only"
      >
        {announcement}
      </span>

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
              onClick={() => { setSelected(new Set()); lastShiftRangeRef.current = new Set(); anchorRef.current = null }}
            >
              <X className="w-3 h-3" /> Clear
            </button>
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              className="h-8 gap-1.5 text-xs"
              onClick={() => copySelected()}
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
        ref={containerRef}
        className="overflow-x-auto rounded-lg border border-border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        onKeyDown={handleTableKeyDown}
        tabIndex={0}
        role="grid"
        aria-label="Trade results"
        aria-multiselectable="true"
        aria-rowcount={trades.length}
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
              const isFocused = focusedIdx === i
              const isWin = trade.pnl_after_fees >= 0

              return (
                <tr
                  key={id}
                  data-selectable="true"
                  data-selected={isSelected}
                  data-item-id={id}
                  aria-selected={isSelected}
                  aria-rowindex={i + 1}
                  onClick={(e) => handleRowClick(id, e)}
                  onContextMenu={(e) => handleContextMenu(e, id)}
                  className={cn(
                    'border-b border-border/60 last:border-0 cursor-pointer transition-colors',
                    isSelected
                      ? 'bg-accent/8 border-l-[3px] border-l-accent'
                      : 'hover:bg-muted/30',
                    isFocused && 'ring-2 ring-inset ring-accent'
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

      {/* Right-click context menu */}
      {contextMenu && (
        <div
          role="menu"
          aria-label="Trade actions"
          className="fixed z-50 min-w-40 rounded-lg border border-border bg-popover shadow-lg py-1 text-sm"
          style={{ top: contextMenu.y, left: contextMenu.x }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          <button
            role="menuitem"
            type="button"
            className="flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-muted/60 transition-colors"
            onClick={handleContextCopy}
          >
            <Clipboard className="w-3.5 h-3.5 text-muted-foreground" />
            Copy
          </button>
          <button
            role="menuitem"
            type="button"
            className="flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-muted/60 transition-colors"
            onClick={handleContextExport}
          >
            <Download className="w-3.5 h-3.5 text-muted-foreground" />
            Export CSV
          </button>
          <div className="my-1 border-t border-border/60" />
          <button
            role="menuitem"
            type="button"
            className="flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-muted/60 transition-colors"
            onClick={handleContextSelectAll}
          >
            Select All
          </button>
          <button
            role="menuitem"
            type="button"
            className="flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-muted/60 transition-colors text-muted-foreground"
            onClick={handleContextClearSelection}
          >
            Clear Selection
          </button>
        </div>
      )}
    </div>
  )
}
