// @vitest-environment jsdom
import '@/test/setup'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import type { ActiveIndicator } from '@/lib/chartTools/activeIndicators'
import { IndicatorChips } from './IndicatorChips'

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

const rsi: ActiveIndicator = { key: 'k1', id: 'rsi', inputs: { length: 14, src: 'close' } }

describe('IndicatorChips', () => {
  it('shows the catalog name and the remove button calls onRemove(key)', () => {
    const onRemove = vi.fn()
    render(<IndicatorChips active={[rsi]} onChangeInputs={vi.fn()} onRemove={onRemove} />)
    expect(screen.getByText('Relative strength index')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Remove Relative strength index' }))
    expect(onRemove).toHaveBeenCalledWith('k1')
  })

  it('the gear opens the numeric inputs and Enter applies them', async () => {
    const user = userEvent.setup()
    const onChangeInputs = vi.fn()
    render(<IndicatorChips active={[rsi]} onChangeInputs={onChangeInputs} onRemove={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: 'Relative strength index settings' }))
    const length = await screen.findByLabelText('RSI Length')
    expect(length).toHaveValue(14)
    // Only int/float inputs are editable here.
    expect(screen.queryByLabelText('Source')).toBeNull()
    await user.clear(length)
    await user.type(length, '21{Enter}')
    expect(onChangeInputs).toHaveBeenCalledWith(
      'k1',
      expect.objectContaining({ length: 21, src: 'close' })
    )
  })

  it('blur applies a changed value too', async () => {
    const user = userEvent.setup()
    const onChangeInputs = vi.fn()
    render(<IndicatorChips active={[rsi]} onChangeInputs={onChangeInputs} onRemove={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: 'Relative strength index settings' }))
    const length = await screen.findByLabelText('RSI Length')
    await user.clear(length)
    await user.type(length, '9')
    await user.tab()
    expect(onChangeInputs).toHaveBeenCalledWith('k1', expect.objectContaining({ length: 9 }))
  })

  it('an id the catalog does not know falls back to the raw id', () => {
    render(
      <IndicatorChips
        active={[{ key: 'k2', id: 'mystery-indicator', inputs: {} }]}
        onChangeInputs={vi.fn()}
        onRemove={vi.fn()}
      />
    )
    expect(screen.getByText('mystery-indicator')).toBeInTheDocument()
  })
})
