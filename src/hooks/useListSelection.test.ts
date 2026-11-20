// @vitest-environment jsdom
/**
 * The selection system of CLAUDE.md's TABLE INTERACTION DESIGN SPEC, shared
 * by the Flow run history and the Flow workflow cards (WP8 Phase D,
 * decisions 6a and 6b), following src/components/backtest/TradeTable.tsx.
 */

import { act, renderHook } from '@testing-library/react'
import type { KeyboardEvent } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { useListSelection } from './useListSelection'

const IDS = ['a', 'b', 'c', 'd', 'e', 'f']
const PLAIN = { shiftKey: false, metaKey: false, ctrlKey: false }
const SHIFT = { ...PLAIN, shiftKey: true }
const META = { ...PLAIN, metaKey: true }
const CTRL = { ...PLAIN, ctrlKey: true }

function setup(ids: string[] = IDS, onOpen = vi.fn()) {
  return renderHook(
    ({ list }) => useListSelection(list, { noun: ['run', 'runs'], pageSize: 3, onOpen }),
    { initialProps: { list: ids } }
  )
}

/** A key pressed on the list, with what the handler did to it. */
function key(k: string, mods: Partial<typeof PLAIN> = {}) {
  const event = { key: k, ...PLAIN, ...mods, preventDefault: vi.fn() }
  return event as unknown as KeyboardEvent & { preventDefault: ReturnType<typeof vi.fn> }
}

const selected = (r: ReturnType<typeof setup>) => [...r.result.current.selected].sort()

describe('clicking items', () => {
  it('selects only the clicked item, and a click on the sole selected one clears it', () => {
    const r = setup()
    act(() => r.result.current.select('b', PLAIN))
    act(() => r.result.current.select('d', PLAIN))
    expect(selected(r)).toEqual(['d'])
    act(() => r.result.current.select('d', PLAIN))
    expect(selected(r)).toEqual([])
  })

  it('Shift+click selects the range from the anchor, inclusive, in either direction', () => {
    const r = setup()
    act(() => r.result.current.select('b', PLAIN))
    act(() => r.result.current.select('e', SHIFT))
    expect(selected(r)).toEqual(['b', 'c', 'd', 'e'])
    // Another Shift+click replaces the range, from the same anchor.
    act(() => r.result.current.select('a', SHIFT))
    expect(selected(r)).toEqual(['a', 'b'])
  })

  it('Cmd/Ctrl+click toggles one item, keeps the rest, and moves the anchor', () => {
    const r = setup()
    act(() => r.result.current.select('a', PLAIN))
    act(() => r.result.current.select('c', META))
    expect(selected(r)).toEqual(['a', 'c'])
    act(() => r.result.current.select('a', CTRL))
    expect(selected(r)).toEqual(['c'])
    // The anchor is now a (the last Cmd/Ctrl+click): a range from there.
    act(() => r.result.current.select('e', CTRL))
    act(() => r.result.current.select('f', SHIFT))
    expect(selected(r)).toEqual(['c', 'e', 'f'])
  })

  it("an item's checkbox adds to the selection, and with Shift selects a range", () => {
    const r = setup()
    act(() => r.result.current.check('b', PLAIN))
    act(() => r.result.current.check('d', PLAIN))
    expect(selected(r)).toEqual(['b', 'd'])
    act(() => r.result.current.check('b', PLAIN))
    expect(selected(r)).toEqual(['d'])
    // The anchor is the last item clicked without Shift (the spec's §4.1):
    // b, even though that click took it off.
    act(() => r.result.current.check('f', SHIFT))
    expect(selected(r)).toEqual(['b', 'c', 'd', 'e', 'f'])
  })
})

describe('select all', () => {
  it('selects all from none and from some (indeterminate), and clears from all', () => {
    const r = setup()
    expect(r.result.current.allSelected).toBe(false)
    expect(r.result.current.someSelected).toBe(false)
    act(() => r.result.current.select('b', PLAIN))
    expect(r.result.current.someSelected).toBe(true)
    act(() => r.result.current.toggleAll())
    expect(selected(r)).toEqual(IDS)
    expect(r.result.current.allSelected).toBe(true)
    expect(r.result.current.someSelected).toBe(false)
    act(() => r.result.current.toggleAll())
    expect(selected(r)).toEqual([])
  })

  it('an empty list is never all selected', () => {
    const r = setup([])
    expect(r.result.current.allSelected).toBe(false)
  })
})

describe('the keyboard', () => {
  it('Cmd+A and Ctrl+A select all; Escape clears', () => {
    const r = setup()
    const cmdA = key('a', { metaKey: true })
    act(() => r.result.current.onKeyDown(cmdA))
    expect(selected(r)).toEqual(IDS)
    expect(cmdA.preventDefault).toHaveBeenCalled()
    const esc = key('Escape')
    act(() => r.result.current.onKeyDown(esc))
    expect(selected(r)).toEqual([])
    expect(esc.preventDefault).toHaveBeenCalled()
    act(() => r.result.current.onKeyDown(key('A', { ctrlKey: true })))
    expect(selected(r)).toEqual(IDS)
  })

  it('Escape with nothing selected leaves the key alone (a dialog may close on it)', () => {
    const r = setup()
    const esc = key('Escape')
    act(() => r.result.current.onKeyDown(esc))
    expect(esc.preventDefault).not.toHaveBeenCalled()
  })

  it('arrows move the focus; Shift+arrows extend the selection from the anchor', () => {
    const r = setup()
    act(() => r.result.current.select('b', PLAIN))
    act(() => r.result.current.onKeyDown(key('ArrowDown')))
    expect(r.result.current.focusedId).toBe('c')
    expect(selected(r)).toEqual(['b'])
    act(() => r.result.current.onKeyDown(key('ArrowDown', { shiftKey: true })))
    expect(r.result.current.focusedId).toBe('d')
    expect(selected(r)).toEqual(['b', 'c', 'd'])
    act(() => r.result.current.onKeyDown(key('ArrowUp', { shiftKey: true })))
    expect(selected(r)).toEqual(['b', 'c'])
  })

  it('Shift+Home and Shift+End extend to the first and the last item', () => {
    const r = setup()
    act(() => r.result.current.select('c', PLAIN))
    act(() => r.result.current.onKeyDown(key('End', { shiftKey: true })))
    expect(selected(r)).toEqual(['c', 'd', 'e', 'f'])
    act(() => r.result.current.onKeyDown(key('Home', { shiftKey: true })))
    expect(selected(r)).toEqual(['a', 'b', 'c'])
  })

  it('Shift+Page Down and Shift+Page Up extend by a page', () => {
    const r = setup()
    act(() => r.result.current.select('a', PLAIN))
    act(() => r.result.current.onKeyDown(key('PageDown', { shiftKey: true })))
    expect(selected(r)).toEqual(['a', 'b', 'c', 'd'])
    act(() => r.result.current.onKeyDown(key('PageUp', { shiftKey: true })))
    expect(selected(r)).toEqual(['a'])
  })

  it('Shift+Arrow with nothing selected starts from the focused item', () => {
    const r = setup()
    act(() => r.result.current.focus('c'))
    act(() => r.result.current.onKeyDown(key('ArrowDown', { shiftKey: true })))
    expect(selected(r)).toEqual(['c', 'd'])
  })

  it('Space toggles the focused item; Enter opens it', () => {
    const onOpen = vi.fn()
    const r = setup(IDS, onOpen)
    act(() => r.result.current.focus('e'))
    act(() => r.result.current.onKeyDown(key(' ')))
    expect(selected(r)).toEqual(['e'])
    act(() => r.result.current.onKeyDown(key(' ')))
    expect(selected(r)).toEqual([])
    act(() => r.result.current.onKeyDown(key('Enter')))
    expect(onOpen).toHaveBeenCalledWith('e')
  })
})

describe('the right-click menu', () => {
  it('a right-click on an unselected item selects only it; on a selected one keeps all', () => {
    const r = setup()
    act(() => r.result.current.select('a', PLAIN))
    act(() => r.result.current.select('b', META))
    act(() => r.result.current.contextSelect('b'))
    expect(selected(r)).toEqual(['a', 'b'])
    act(() => r.result.current.contextSelect('d'))
    expect(selected(r)).toEqual(['d'])
  })
})

describe('a list that changes', () => {
  it('drops the items that left the list from the selection, in display order', () => {
    const r = setup()
    act(() => r.result.current.toggleAll())
    r.rerender({ list: ['f', 'b', 'x'] })
    expect(r.result.current.selectedIds).toEqual(['f', 'b'])
    expect(r.result.current.count).toBe(2)
    expect(r.result.current.someSelected).toBe(true)
  })
})

describe('the announcement', () => {
  it('says how many are selected, and when the selection is cleared', () => {
    const r = setup()
    expect(r.result.current.announcement).toBe('')
    act(() => r.result.current.select('a', PLAIN))
    expect(r.result.current.announcement).toBe('1 run selected')
    act(() => r.result.current.select('c', SHIFT))
    expect(r.result.current.announcement).toBe('3 runs selected')
    act(() => r.result.current.clear())
    expect(r.result.current.announcement).toBe('No runs selected')
  })
})
