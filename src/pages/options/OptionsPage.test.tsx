// @vitest-environment jsdom
import '@/test/setup'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('sonner', () => ({ toast: mocks.toast }))

import OptionsPage from './OptionsPage'

type Model = 'black_scholes' | 'binomial'

function row(strike: number, optionType: 'call' | 'put', model: Model, premium: number | null) {
  return {
    strike,
    expiry: '30d',
    option_type: optionType,
    theoretical_price: optionType === 'call' ? 12.34 : 5.67,
    implied_volatility: null,
    delta: optionType === 'call' ? 0.55 : -0.45,
    gamma: 0.02,
    theta: -0.03,
    vega: 0.11,
    rho: 4.2,
    model,
    early_exercise_premium: premium,
  }
}

/** Strikes 90..110 step 5, as the page generates them from the form below. */
function chain(model: Model) {
  return [90, 95, 100, 105, 110].flatMap((strike) => [
    row(strike, 'call', model, model === 'binomial' ? 0 : null),
    row(strike, 'put', model, model === 'binomial' ? (strike > 100 ? 1.25 : 0.4) : null),
  ])
}

function single(model: Model, premium: number | null) {
  return row(100, 'call', model, premium)
}

function renderPage() {
  return render(
    <MemoryRouter>
      <OptionsPage />
    </MemoryRouter>
  )
}

async function fillChainParameters(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText('Spot Price'), '100')
  await user.type(screen.getByLabelText('Days to Expiry'), '30')
  await user.type(screen.getByLabelText('Strike From'), '90')
  await user.type(screen.getByLabelText('Strike To'), '110')
}

const earlyExerciseHeaders = () =>
  screen.queryAllByRole('columnheader', { name: 'Early exercise' })

describe('OptionsPage pricing model', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
    mocks.toast.error.mockReset()
  })

  it('sends camelCase arguments with the model, and shows early-exercise columns only for binomial rows', async () => {
    const user = userEvent.setup()
    renderPage()
    await fillChainParameters(user)

    mocks.invoke.mockResolvedValueOnce(chain('black_scholes'))
    await user.click(screen.getByRole('button', { name: 'Calculate Chain' }))

    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith('compute_greeks_batch', {
        spotPrice: 100,
        strikes: [90, 95, 100, 105, 110],
        expiryDays: 30,
        riskFreeRate: 0.045,
        volatility: 0.3,
        model: 'black_scholes',
      })
    )
    await screen.findAllByText('12.34')
    expect(earlyExerciseHeaders()).toHaveLength(0)

    // Switching re-prices the loaded chain with the binomial model.
    mocks.invoke.mockResolvedValueOnce(chain('binomial'))
    await user.click(screen.getByRole('button', { name: 'Binomial (American)' }))

    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenLastCalledWith(
        'compute_greeks_batch',
        expect.objectContaining({ model: 'binomial' })
      )
    )
    await waitFor(() => expect(earlyExerciseHeaders()).toHaveLength(2))
    expect(screen.getAllByText('1.25').length).toBeGreaterThan(0)

    // A switch whose recalculation is refused by input validation must leave the
    // binomial rows labelled as binomial: the columns follow the data on screen.
    await user.clear(screen.getByLabelText('Spot Price'))
    await user.click(screen.getByRole('button', { name: 'Black-Scholes' }))

    expect(mocks.toast.error).toHaveBeenCalledWith('Spot price must be a positive number')
    expect(mocks.invoke).toHaveBeenCalledTimes(2)
    expect(earlyExerciseHeaders()).toHaveLength(2)
    expect(screen.getAllByText('1.25').length).toBeGreaterThan(0)
  })

  it('single calculator sends the model and shows the early-exercise tile for binomial results', async () => {
    const user = userEvent.setup()
    renderPage()
    await fillChainParameters(user)
    await user.type(screen.getByLabelText('Strike Price'), '100')

    mocks.invoke.mockResolvedValueOnce(single('black_scholes', null))
    await user.click(screen.getByRole('button', { name: 'Calculate Greeks' }))

    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith('compute_greeks', {
        spotPrice: 100,
        strike: 100,
        expiryDays: 30,
        riskFreeRate: 0.045,
        volatility: 0.3,
        isCall: true,
        marketPrice: null,
        model: 'black_scholes',
      })
    )
    await screen.findByText('12.34')
    expect(screen.queryByText('Early exercise')).toBeNull()

    mocks.invoke.mockResolvedValueOnce(single('binomial', 0.5))
    await user.click(screen.getByRole('button', { name: 'Binomial (American)' }))

    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenLastCalledWith(
        'compute_greeks',
        expect.objectContaining({ model: 'binomial' })
      )
    )
    await screen.findByText('Early exercise')
    expect(screen.getByText('0.50')).toBeInTheDocument()
  })
})
