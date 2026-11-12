// @vitest-environment jsdom
import '@/test/setup'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import { IndicatorPicker } from './IndicatorPicker'

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
  // cmdk scrolls the selected option into view; jsdom has no layout.
  Element.prototype.scrollIntoView = vi.fn()
})

describe('IndicatorPicker', () => {
  it('typing macd lists MACD and clicking it calls onAdd', async () => {
    const user = userEvent.setup()
    const onAdd = vi.fn()
    render(<IndicatorPicker onAdd={onAdd} />)

    await user.click(screen.getByRole('button', { name: 'Indicators' }))
    const input = await screen.findByPlaceholderText('Search indicators...')
    await user.type(input, 'macd')

    const option = await screen.findByRole('option', {
      name: /moving average convergence divergence/i,
    })
    await user.click(option)

    expect(onAdd).toHaveBeenCalledTimes(1)
    expect(onAdd.mock.calls[0][0].id).toMatch(/macd/i)
  })

  it('lists at most 50 results, grouped', async () => {
    const user = userEvent.setup()
    render(<IndicatorPicker onAdd={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: 'Indicators' }))
    await screen.findByPlaceholderText('Search indicators...')
    const options = await screen.findAllByRole('option')
    expect(options.length).toBeGreaterThan(0)
    expect(options.length).toBeLessThanOrEqual(50)
    expect(screen.getAllByRole('group').length).toBeGreaterThan(0)
  })

  it('shows an empty state for a query nothing matches', async () => {
    const user = userEvent.setup()
    render(<IndicatorPicker onAdd={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: 'Indicators' }))
    const input = await screen.findByPlaceholderText('Search indicators...')
    await user.type(input, 'zzzz-no-such-indicator')
    expect(await screen.findByText('No indicators found')).toBeInTheDocument()
  })
})
