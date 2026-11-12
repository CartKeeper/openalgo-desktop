// @vitest-environment jsdom
import '@/test/setup'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ModelSwitch } from '@/components/options/ModelSwitch'

describe('ModelSwitch', () => {
  it('renders both model buttons with the current one pressed', () => {
    render(<ModelSwitch value="black_scholes" onChange={() => {}} />)

    const blackScholes = screen.getByRole('button', { name: 'Black-Scholes' })
    const binomial = screen.getByRole('button', { name: 'Binomial (American)' })
    expect(blackScholes).toHaveAttribute('aria-pressed', 'true')
    expect(binomial).toHaveAttribute('aria-pressed', 'false')
  })

  it('calls onChange with "binomial" when Binomial (American) is clicked', async () => {
    const onChange = vi.fn()
    render(<ModelSwitch value="black_scholes" onChange={onChange} />)

    await userEvent.click(screen.getByRole('button', { name: 'Binomial (American)' }))

    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith('binomial')
  })

  it('reflects a binomial value and lets the user switch back', async () => {
    const onChange = vi.fn()
    render(<ModelSwitch value="binomial" onChange={onChange} />)

    expect(screen.getByRole('button', { name: 'Binomial (American)' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    await userEvent.click(screen.getByRole('button', { name: 'Black-Scholes' }))

    expect(onChange).toHaveBeenCalledWith('black_scholes')
  })

  it('does not fire while disabled', async () => {
    const onChange = vi.fn()
    render(<ModelSwitch value="black_scholes" onChange={onChange} disabled />)

    await userEvent.click(screen.getByRole('button', { name: 'Binomial (American)' }))

    expect(onChange).not.toHaveBeenCalled()
  })
})
