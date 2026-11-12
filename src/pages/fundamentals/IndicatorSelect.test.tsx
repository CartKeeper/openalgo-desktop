// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import type { IndicatorSpec } from '@/api/indicators'
import { IndicatorSelect } from '@/components/fundamentals/IndicatorSelect'

class ObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return []
  }
}

beforeAll(() => {
  // Radix Select relies on pointer capture and scrollIntoView, which jsdom lacks,
  // and its popper (Floating UI) constructs observers, so the shared setup's
  // arrow-function mocks are replaced with constructible stubs here.
  Element.prototype.hasPointerCapture = () => false
  Element.prototype.setPointerCapture = () => {}
  Element.prototype.releasePointerCapture = () => {}
  Element.prototype.scrollIntoView = () => {}
  window.ResizeObserver = ObserverStub as unknown as typeof ResizeObserver
  window.IntersectionObserver = ObserverStub as unknown as typeof IntersectionObserver
})

const period = { name: 'period', default: 14, min: 2, max: 100000, kind: 'int' as const }

const specs: IndicatorSpec[] = [
  { name: 'SMA', label: 'Simple Moving Average', group: 'Overlap', inputs: ['close'], params: [{ ...period, default: 30, min: 1 }], outputs: ['value'] },
  { name: 'RSI', label: 'Relative Strength Index', group: 'Momentum', inputs: ['close'], params: [period], outputs: ['value'] },
  { name: 'ADX', label: 'Average Directional Movement Index', group: 'Momentum', inputs: ['high', 'low', 'close'], params: [period], outputs: ['value'] },
  { name: 'ATR', label: 'Average True Range', group: 'Volatility', inputs: ['high', 'low', 'close'], params: [{ ...period, min: 1 }], outputs: ['value'] },
  { name: 'OBV', label: 'On Balance Volume', group: 'Volume', inputs: ['close', 'volume'], params: [], outputs: ['value'] },
  { name: 'HT_DCPERIOD', label: 'Hilbert Transform - Dominant Cycle Period', group: 'Cycle', inputs: ['close'], params: [], outputs: ['value'] },
  { name: 'STDDEV', label: 'Standard Deviation', group: 'Statistics', inputs: ['close'], params: [{ ...period, default: 5 }], outputs: ['value'] },
  { name: 'CDLDOJI', label: 'Doji', group: 'Patterns', inputs: ['open', 'high', 'low', 'close'], params: [], outputs: ['pattern'] },
]

describe('IndicatorSelect', () => {
  it('renders the seven catalog groups in order and lists ADX under Momentum', async () => {
    const user = userEvent.setup()
    render(<IndicatorSelect value="SMA" onChange={() => {}} specs={specs} />)

    await user.click(screen.getByRole('combobox', { name: /indicator/i }))

    const groups = await screen.findAllByRole('group')
    expect(groups).toHaveLength(7)
    expect(groups.map((g) => g.querySelector('[data-slot="select-label"]')?.textContent)).toEqual([
      'Overlap',
      'Momentum',
      'Volatility',
      'Volume',
      'Cycle',
      'Statistics',
      'Patterns',
    ])

    const momentum = groups[1]
    expect(within(momentum).getByRole('option', { name: /ADX/ })).toBeInTheDocument()
    expect(within(momentum).getByRole('option', { name: /RSI/ })).toBeInTheDocument()
    expect(within(groups[0]).queryByRole('option', { name: /ADX/ })).not.toBeInTheDocument()
  })

  it('calls onChange with the indicator name when ADX is picked', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    render(<IndicatorSelect value="SMA" onChange={onChange} specs={specs} />)

    await user.click(screen.getByRole('combobox', { name: /indicator/i }))
    await user.click(await screen.findByRole('option', { name: /ADX/ }))

    expect(onChange).toHaveBeenCalledWith('ADX')
  })

  it('shows the current selection on the trigger', () => {
    render(<IndicatorSelect value="ADX" onChange={() => {}} specs={specs} />)
    expect(screen.getByRole('combobox', { name: /indicator/i })).toHaveTextContent('ADX')
  })
})
