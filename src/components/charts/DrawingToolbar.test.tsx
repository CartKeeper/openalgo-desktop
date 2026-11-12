// @vitest-environment jsdom
import '@/test/setup'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { OVERLAY_SPECS } from 'lightweight-charts-drawing'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import {
  DrawingToolbar,
  type DrawingToolbarProps,
  HOST_ONLY_TOOLS,
  MORE_TOOL_GROUPS,
  PRIMARY_TOOLS,
  toolLabel,
} from './DrawingToolbar'

// Radix popper (floating-ui autoUpdate) constructs these with `new`; the shared
// setup mocks them with arrow functions, which vitest 4 cannot construct.
class ObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
beforeAll(() => {
  window.ResizeObserver = ObserverStub as unknown as typeof ResizeObserver
  window.IntersectionObserver = ObserverStub as unknown as typeof IntersectionObserver
})

function props(over: Partial<DrawingToolbarProps> = {}): DrawingToolbarProps {
  return {
    activeTool: null,
    onSelectTool: vi.fn(),
    allHidden: false,
    onToggleHidden: vi.fn(),
    onClearAll: vi.fn(),
    onDeleteSelection: vi.fn(),
    hasSelection: false,
    ...over,
  }
}

describe('DrawingToolbar', () => {
  it('renders the pointer, the six primary tools and a More tools button', () => {
    render(<DrawingToolbar {...props()} />)
    for (const name of [
      'Pointer',
      'Trend line',
      'Horizontal line',
      'Ray',
      'Rectangle',
      'Fibonacci retracement',
      'Text',
      'More tools',
    ]) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument()
    }
  })

  it('clicking Trend line arms the trend-line tool and Pointer disarms', () => {
    const onSelectTool = vi.fn()
    render(<DrawingToolbar {...props({ onSelectTool })} />)
    fireEvent.click(screen.getByRole('button', { name: 'Trend line' }))
    expect(onSelectTool).toHaveBeenLastCalledWith('trend-line')
    fireEvent.click(screen.getByRole('button', { name: 'Pointer' }))
    expect(onSelectTool).toHaveBeenLastCalledWith(null)
  })

  it('marks only the active tool with aria-pressed', () => {
    render(<DrawingToolbar {...props({ activeTool: 'ray' })} />)
    expect(screen.getByRole('button', { name: 'Ray' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Trend line' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(screen.getByRole('button', { name: 'Pointer' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
  })

  it('Pointer is pressed when no tool is armed', () => {
    render(<DrawingToolbar {...props()} />)
    expect(screen.getByRole('button', { name: 'Pointer' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('Delete selection is disabled without a selection', () => {
    const onDeleteSelection = vi.fn()
    const { rerender } = render(<DrawingToolbar {...props({ onDeleteSelection })} />)
    const del = screen.getByRole('button', { name: 'Delete selection' })
    expect(del).toBeDisabled()
    rerender(<DrawingToolbar {...props({ onDeleteSelection, hasSelection: true })} />)
    expect(screen.getByRole('button', { name: 'Delete selection' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Delete selection' }))
    expect(onDeleteSelection).toHaveBeenCalledTimes(1)
  })

  it('Hide all and Clear all call their handlers; hidden state flips the label', () => {
    const onToggleHidden = vi.fn()
    const onClearAll = vi.fn()
    const { rerender } = render(<DrawingToolbar {...props({ onToggleHidden, onClearAll })} />)
    fireEvent.click(screen.getByRole('button', { name: 'Hide all drawings' }))
    expect(onToggleHidden).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: 'Clear all drawings' }))
    expect(onClearAll).toHaveBeenCalledTimes(1)
    rerender(<DrawingToolbar {...props({ onToggleHidden, onClearAll, allHidden: true })} />)
    expect(screen.getByRole('button', { name: 'Show all drawings' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
  })

  it('More tools opens a grouped menu and selecting an entry arms it', async () => {
    const user = userEvent.setup()
    const onSelectTool = vi.fn()
    render(<DrawingToolbar {...props({ onSelectTool })} />)
    await user.click(screen.getByRole('button', { name: 'More tools' }))
    const item = await screen.findByRole('menuitem', { name: 'Parallel channel' })
    await user.click(item)
    expect(onSelectTool).toHaveBeenCalledWith('parallel-channel')
  })

  it('primary + More tools + host-only tools cover every library tool exactly once', () => {
    const all = Object.keys(OVERLAY_SPECS).sort()
    const listed = [
      ...PRIMARY_TOOLS.map((t) => t.kind),
      ...MORE_TOOL_GROUPS.flatMap((g) => g.tools),
      ...HOST_ONLY_TOOLS,
    ]
    expect([...listed].sort()).toEqual(all)
    expect(new Set(listed).size).toBe(listed.length)
    expect(MORE_TOOL_GROUPS.every((g) => g.label && g.tools.length > 0)).toBe(true)
  })

  it('tool labels are human readable', () => {
    expect(toolLabel('parallel-channel')).toBe('Parallel channel')
    expect(toolLabel('trend-based-fib-extension')).toBe('Trend-based Fib extension')
    expect(toolLabel('anchored-vwap')).toBe('Anchored VWAP')
    expect(toolLabel('xabcd-pattern')).toBe('XABCD pattern')
    expect(toolLabel('elliott-impulse')).toBe('Elliott impulse')
  })
})
