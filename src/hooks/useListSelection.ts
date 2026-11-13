import type { KeyboardEvent } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

/**
 * The selection system of CLAUDE.md's TABLE INTERACTION DESIGN SPEC for a
 * list of items in display order (`ids`), following the app's first full
 * implementation, src/components/backtest/TradeTable.tsx (WP8 Phase D: the
 * Flow run history and the Flow workflow cards share it).
 *
 * - A click selects only that item (a click on the sole selected item
 *   clears it); Shift+click selects the range from the anchor, replacing
 *   the previous Shift range; Cmd/Ctrl+click toggles one item and moves the
 *   anchor. An item's checkbox adds to or takes from the selection, and
 *   with Shift selects a range.
 * - Select all: from none or some (indeterminate) it selects all, from all
 *   it clears (the spec's §3.3; TradeTable cleared from indeterminate).
 * - Keys on the list: Cmd/Ctrl+A, Escape (only when something is selected,
 *   so a dialog may close on it otherwise), arrows move the focus,
 *   Shift+arrows, Shift+Home/End and Shift+Page Up/Down extend from the
 *   anchor, Space toggles the focused item, Enter opens it.
 * - A right-click on an unselected item selects only it; on a selected one
 *   it keeps the selection (the spec's §6).
 * - Items that leave the list leave the selection; the selection lives in
 *   memory, so a refetch keeps it.
 */

export interface PointerModifiers {
  shiftKey: boolean
  metaKey: boolean
  ctrlKey: boolean
}

export interface ListSelectionOptions {
  /** The item's word for the live announcement: one, several. */
  noun: [string, string]
  /** How many items Shift+Page Up/Down moves (default 10). */
  pageSize?: number
  /** Enter on the focused item. */
  onOpen?: (id: string) => void
}

export interface ListSelection {
  selected: ReadonlySet<string>
  /** The selected ids in display order. */
  selectedIds: string[]
  count: number
  isSelected: (id: string) => boolean
  allSelected: boolean
  /** Some but not all selected: the master checkbox's indeterminate state. */
  someSelected: boolean
  focusedId: string | null
  /** For the list's aria-live region. */
  announcement: string
  /** A click on the item. */
  select: (id: string, mods: PointerModifiers) => void
  /** A click on the item's checkbox. */
  check: (id: string, mods: PointerModifiers) => void
  /** The master checkbox. */
  toggleAll: () => void
  selectAll: () => void
  clear: () => void
  /** A right-click on the item, before its menu opens. */
  contextSelect: (id: string) => void
  focus: (id: string) => void
  /** Keys on the list (only those that belong to it: `keyIsForTheList`). */
  onKeyDown: (e: KeyboardEvent) => void
}

/**
 * Whether a key belongs to the list: typed on the list itself, or on a
 * part of an item that is not a control of its own (a card's body, a
 * row's cell). A key typed in an item's checkbox or button is that
 * control's, and one from a dialog an item opens (a React child rendered
 * elsewhere in the page, whose events still bubble to the list) is the
 * dialog's.
 */
export function keyIsForTheList(e: KeyboardEvent): boolean {
  const target = e.target as Element | null
  const list = e.currentTarget as Element | null
  if (!target || !list || target === list) return true
  if (typeof list.contains !== 'function' || !list.contains(target)) return false
  return target.closest('input, button, select, textarea, a, [role="menuitem"]') === null
}

/** The ids from `a` to `b` in `ids`, inclusive, either direction. */
function span(ids: string[], a: number, b: number): string[] {
  const [lo, hi] = a < b ? [a, b] : [b, a]
  return ids.slice(lo, hi + 1)
}

export function useListSelection(ids: string[], options: ListSelectionOptions): ListSelection {
  const { noun, pageSize = 10, onOpen } = options
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const [focusedId, setFocusedId] = useState<string | null>(null)
  const anchor = useRef<string | null>(null)
  /** What the last Shift range added, so the next one replaces it. */
  const lastRange = useRef<Set<string>>(new Set())
  const [cleared, setCleared] = useState(false)

  // Items that left the list leave the selection, the anchor and the focus.
  const present = useMemo(() => new Set(ids), [ids])
  useEffect(() => {
    setSelected((prev) => {
      const kept = [...prev].filter((id) => present.has(id))
      return kept.length === prev.size ? prev : new Set(kept)
    })
    if (anchor.current !== null && !present.has(anchor.current)) anchor.current = null
    setFocusedId((f) => (f !== null && present.has(f) ? f : null))
  }, [present])

  const change = useCallback((next: Set<string>) => {
    setCleared(next.size === 0)
    setSelected(next)
  }, [])

  /** Replace the last Shift range with the range from the anchor to `toIndex`. */
  const extendTo = useCallback(
    (toIndex: number, from: Set<string>) => {
      const anchorIndex = anchor.current === null ? -1 : ids.indexOf(anchor.current)
      if (anchorIndex < 0) return from
      const next = new Set(from)
      for (const id of lastRange.current) next.delete(id)
      const range = span(ids, anchorIndex, toIndex)
      for (const id of range) next.add(id)
      lastRange.current = new Set(range)
      return next
    },
    [ids]
  )

  const select = useCallback(
    (id: string, mods: PointerModifiers) => {
      const index = ids.indexOf(id)
      if (index < 0) return
      setFocusedId(id)
      if (mods.shiftKey && anchor.current !== null && ids.includes(anchor.current)) {
        change(extendTo(index, selected))
        return
      }
      lastRange.current = new Set()
      if (mods.metaKey || mods.ctrlKey) {
        const next = new Set(selected)
        if (next.has(id)) next.delete(id)
        else next.add(id)
        anchor.current = id
        change(next)
        return
      }
      if (selected.size === 1 && selected.has(id)) {
        anchor.current = null
        change(new Set())
        return
      }
      anchor.current = id
      change(new Set([id]))
    },
    [ids, selected, change, extendTo]
  )

  const check = useCallback(
    (id: string, mods: PointerModifiers) => {
      select(id, { shiftKey: mods.shiftKey, metaKey: !mods.shiftKey, ctrlKey: false })
    },
    [select]
  )

  const selectAll = useCallback(() => {
    lastRange.current = new Set()
    change(new Set(ids))
  }, [ids, change])

  const clear = useCallback(() => {
    lastRange.current = new Set()
    anchor.current = null
    change(new Set())
  }, [change])

  const count = selected.size
  const allSelected = ids.length > 0 && count === ids.length
  const someSelected = count > 0 && !allSelected

  const toggleAll = useCallback(() => {
    if (allSelected) clear()
    else selectAll()
  }, [allSelected, clear, selectAll])

  const contextSelect = useCallback(
    (id: string) => {
      setFocusedId(id)
      if (selected.has(id)) return
      lastRange.current = new Set()
      anchor.current = id
      change(new Set([id]))
    },
    [selected, change]
  )

  const focus = useCallback((id: string) => setFocusedId(id), [])

  const onKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!keyIsForTheList(e)) return
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault()
        selectAll()
        return
      }
      if (e.key === 'Escape') {
        if (count > 0) {
          e.preventDefault()
          clear()
        }
        return
      }
      if (ids.length === 0) return
      const current = focusedId === null ? -1 : ids.indexOf(focusedId)
      let target: number | null = null
      if (e.key === 'ArrowDown') target = Math.min(ids.length - 1, current + 1)
      else if (e.key === 'ArrowUp') target = Math.max(0, current - 1)
      else if (e.key === 'Home') target = 0
      else if (e.key === 'End') target = ids.length - 1
      else if (e.key === 'PageDown')
        target = Math.min(ids.length - 1, Math.max(current, 0) + pageSize)
      else if (e.key === 'PageUp') target = Math.max(0, current - pageSize)
      if (target !== null) {
        e.preventDefault()
        if (e.shiftKey) {
          let base = selected
          if (anchor.current === null || !ids.includes(anchor.current)) {
            // Nothing to extend from: start at the focused item.
            const start = current >= 0 ? ids[current] : ids[target]
            anchor.current = start
            lastRange.current = new Set()
            base = new Set([...selected, start])
          }
          change(extendTo(target, base))
        }
        setFocusedId(ids[target])
        return
      }
      if (e.key === ' ' && current >= 0) {
        e.preventDefault()
        const id = ids[current]
        lastRange.current = new Set()
        const next = new Set(selected)
        if (next.has(id)) next.delete(id)
        else {
          next.add(id)
          anchor.current = id
        }
        change(next)
        return
      }
      if (e.key === 'Enter' && current >= 0 && onOpen) {
        e.preventDefault()
        onOpen(ids[current])
      }
    },
    [ids, focusedId, selected, count, pageSize, onOpen, selectAll, clear, change, extendTo]
  )

  const selectedIds = useMemo(() => ids.filter((id) => selected.has(id)), [ids, selected])
  const isSelected = useCallback((id: string) => selected.has(id), [selected])
  const [one, many] = noun
  const announcement =
    count > 0
      ? `${count} ${count === 1 ? one : many} selected`
      : cleared
        ? `No ${many} selected`
        : ''

  return {
    selected,
    selectedIds,
    count,
    isSelected,
    allSelected,
    someSelected,
    focusedId,
    announcement,
    select,
    check,
    toggleAll,
    selectAll,
    clear,
    contextSelect,
    focus,
    onKeyDown,
  }
}
