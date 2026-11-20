// @vitest-environment jsdom
import '@/test/setup'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { StrategyLeg } from '@/api/tauri-client'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import type { OrderRecommendation } from '@/types/actionQueue'
import { ActionReviewModal } from './ActionReviewModal'

const mocks = vi.hoisted(() => ({
  getAnalyzeMode: vi.fn(),
  getFunds: vi.fn(),
  getMarketClock: vi.fn(),
  placeStrategyBasket: vi.fn(),
  placeBasketOrderCommand: vi.fn(),
  getQuote: vi.fn(),
  getGenericQuote: vi.fn(),
  recordProtectionDismissal: vi.fn(),
  tradingPlaceBasketOrder: vi.fn(),
  reconcile: vi.fn(),
  emit: vi.fn(),
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}))

vi.mock('@/api/tauri-client', () => ({
  settingsCommands: { getAnalyzeMode: mocks.getAnalyzeMode },
  fundsCommands: { getFunds: mocks.getFunds },
  marketStatusCommands: { getMarketClock: mocks.getMarketClock },
  orderCommands: {
    placeStrategyBasket: mocks.placeStrategyBasket,
    placeBasketOrder: mocks.placeBasketOrderCommand,
  },
  quoteCommands: { getQuote: mocks.getQuote },
  providerCommands: { getGenericQuote: mocks.getGenericQuote },
  rideCommands: { recordProtectionDismissal: mocks.recordProtectionDismissal },
}))
vi.mock('@/api/trading', () => ({
  tradingApi: { placeBasketOrder: mocks.tradingPlaceBasketOrder },
}))
vi.mock('@/lib/affordability', () => ({
  reconcileRecommendationsWithAccount: mocks.reconcile,
}))
vi.mock('@tauri-apps/api/event', () => ({ emit: mocks.emit }))
vi.mock('sonner', () => ({ toast: mocks.toast }))

const MARKET_CLOSED_LINE = 'Market opens at 9:30 AM ET. Place this then.'
const CLOSED = {
  timestamp: '2026-11-02T08:00:00-05:00',
  is_open: false,
  next_open: '2026-11-02T09:30:00-05:00',
  next_close: '2026-11-02T16:00:00-05:00',
}
const OPEN = { ...CLOSED, timestamp: '2026-11-02T10:00:00-05:00', is_open: true }

const base = { exchange: 'NYSE', product: 'CNC' as const, source: 'briefing' as const }

const rotation: OrderRecommendation[] = [
  {
    ...base,
    id: 'c1',
    symbol: 'PFE',
    side: 'SELL',
    quantity: 0,
    orderType: 'MARKET',
    price: 0,
    role: 'cancel',
    cancelOrderId: 'ord-stop-1',
    group: 'g1',
    rationale: 'Release the shares held by the open PFE stop.',
  },
  {
    ...base,
    id: 'f1',
    symbol: 'PFE',
    side: 'SELL',
    quantity: 10,
    orderType: 'MARKET',
    price: 0,
    role: 'fund',
    group: 'g1',
    rationale: 'Sell PFE to fund the rotation.',
  },
  {
    ...base,
    id: 'b1',
    symbol: 'BAC',
    side: 'BUY',
    quantity: 5,
    orderType: 'MARKET',
    price: 0,
    role: 'buy',
    group: 'g1',
    rationale: 'Rotate into BAC.',
  },
  {
    ...base,
    id: 'p1',
    symbol: 'BAC',
    side: 'SELL',
    quantity: 5,
    orderType: 'TRAILING_STOP',
    price: 0,
    trailPercent: 8,
    validity: 'GTC',
    role: 'protect',
    group: 'g1',
    rationale: 'Trail the new BAC position.',
  },
]

function seed(items: OrderRecommendation[]) {
  useActionQueueStore.setState({
    items,
    isReviewOpen: true,
    isSubmitting: false,
    lastResults: null,
    fundsNotice: null,
  })
}

const placeButton = () => screen.getByRole('button', { name: /^Place \d+ Orders?$/ })

/** Waits for the open-time reads to land, then ticks the Gate B checkbox. */
async function acknowledge(user: ReturnType<typeof userEvent.setup>) {
  await screen.findByText(/Available to spend \(settled\)/)
  await user.click(
    screen.getByRole('checkbox', { name: /I understand these are real, live orders/ })
  )
}

beforeEach(() => {
  vi.resetAllMocks()
  mocks.getAnalyzeMode.mockResolvedValue({ analyze_mode: false })
  mocks.getFunds.mockResolvedValue({ available_cash: 1000 })
  mocks.getMarketClock.mockResolvedValue(OPEN)
  mocks.getQuote.mockResolvedValue([])
  mocks.recordProtectionDismissal.mockResolvedValue(undefined)
  mocks.reconcile.mockImplementation(async (items: OrderRecommendation[]) => ({
    items,
    availableCash: 1000,
    notice: null,
  }))
  mocks.placeStrategyBasket.mockImplementation(async (legs: StrategyLeg[]) =>
    legs.map((leg) => ({
      group: leg.group,
      role: leg.role,
      symbol: leg.symbol,
      success: true,
      order_id: `ord-${leg.role}`,
      filled_quantity: leg.order?.quantity ?? 0,
      message: '',
    }))
  )
  seed(rotation)
})

afterEach(() => {
  vi.useRealTimers()
})

describe('ActionReviewModal', () => {
  it('renders a rotation group header and a cancel card without inputs', async () => {
    render(<ActionReviewModal />)
    await screen.findByText(/Available to spend \(settled\)/)

    const header = screen.getByText('Rotation: sell PFE → buy BAC')
    expect(header).toHaveClass(
      'text-xs',
      'font-semibold',
      'uppercase',
      'tracking-wider',
      'leading-[1.4]',
      'text-muted-foreground'
    )

    const cancelBadge = screen.getByText('Cancel order')
    const cancelCard = screen.getByTestId('cancel-card')
    expect(cancelCard).toContainElement(cancelBadge)
    expect(within(cancelCard).getByText('PFE')).toBeInTheDocument()
    expect(
      within(cancelCard).getByText('Release the shares held by the open PFE stop.')
    ).toBeInTheDocument()
    expect(within(cancelCard).queryByRole('spinbutton')).toBeNull()
    expect(within(cancelCard).queryByText('Qty')).toBeNull()
    expect(within(cancelCard).queryByText('Type')).toBeNull()
    expect(within(cancelCard).queryByText('SELL')).toBeNull()
    expect(within(cancelCard).getByRole('button')).toBeInTheDocument()

    // The other legs keep their inputs; the protect leg carries the GTC badge.
    expect(screen.getAllByRole('spinbutton').length).toBeGreaterThanOrEqual(3)
    expect(screen.getByText('Trailing Stop · GTC')).toBeInTheDocument()

    // A cancel leg is not a market-priced order: 3 legs (fund, buy, protect), not 4.
    expect(screen.getByText(/plus 3 market-priced orders/)).toBeInTheDocument()

    // The header precedes the group's first card in document order.
    expect(header.compareDocumentPosition(cancelBadge) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    )
  })

  it('disables Place and shows the market-closed line when the clock says closed', async () => {
    mocks.getMarketClock.mockResolvedValue(CLOSED)
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    const line = await screen.findByText(MARKET_CLOSED_LINE)
    expect(line).toHaveClass('text-xs', 'font-semibold', 'leading-[1.4]', 'text-muted-foreground')

    await acknowledge(user)
    expect(placeButton()).toBeDisabled()
    expect(mocks.placeStrategyBasket).not.toHaveBeenCalled()
  })

  it('re-reads the clock on Place and enables when the market has opened', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    mocks.getMarketClock
      .mockResolvedValueOnce(CLOSED) // read on open
      .mockResolvedValueOnce(OPEN) // 60 s interval read
      .mockResolvedValueOnce(OPEN) // read at the top of Place
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    render(<ActionReviewModal />)

    await screen.findByText(MARKET_CLOSED_LINE)
    await acknowledge(user)
    expect(placeButton()).toBeDisabled()
    expect(mocks.getMarketClock).toHaveBeenCalledTimes(1)

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    await waitFor(() => expect(placeButton()).toBeEnabled())
    expect(screen.queryByText(MARKET_CLOSED_LINE)).toBeNull()
    expect(mocks.getMarketClock).toHaveBeenCalledTimes(2)

    await user.click(placeButton())
    await waitFor(() => expect(mocks.placeStrategyBasket).toHaveBeenCalledTimes(1))
    expect(mocks.getMarketClock).toHaveBeenCalledTimes(3)
    await screen.findByText('BAC · protect')
    // The strategy path counts legs, not orders.
    expect(mocks.toast.success).toHaveBeenCalledWith('All 4 legs completed')
  })

  it.each([
    ['closed', () => mocks.getMarketClock.mockResolvedValueOnce(CLOSED), MARKET_CLOSED_LINE],
    [
      'unreadable',
      () => mocks.getMarketClock.mockRejectedValueOnce('clock timeout'),
      `${MARKET_CLOSED_LINE} (could not read the market clock)`,
    ],
  ])('refuses at submit time when the on-Place clock read is %s', async (_label, onPlaceRead, line) => {
    mocks.getMarketClock.mockResolvedValueOnce(OPEN) // read on open
    onPlaceRead() // read at the top of Place
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    await acknowledge(user)
    await waitFor(() => expect(placeButton()).toBeEnabled())
    expect(screen.queryByText(MARKET_CLOSED_LINE, { exact: false })).toBeNull()

    await user.click(placeButton())

    expect(await screen.findByText(line)).toBeInTheDocument()
    expect(mocks.getMarketClock).toHaveBeenCalledTimes(2)
    expect(mocks.placeStrategyBasket).not.toHaveBeenCalled()
    expect(mocks.reconcile).not.toHaveBeenCalled()
    expect(mocks.toast.error).not.toHaveBeenCalled()
    expect(useActionQueueStore.getState().isSubmitting).toBe(false)
    expect(placeButton()).toBeDisabled()
  })

  it('keeps the on-Place refusal when an older interval read lands afterwards', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    let resolveIntervalRead: (clock: typeof OPEN) => void = () => undefined
    mocks.getMarketClock
      .mockResolvedValueOnce(OPEN) // read on open
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveIntervalRead = resolve
          })
      ) // 60 s interval read, still in flight when Place is clicked
      .mockResolvedValueOnce(CLOSED) // read at the top of Place
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    render(<ActionReviewModal />)

    await acknowledge(user)
    await waitFor(() => expect(placeButton()).toBeEnabled())
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(mocks.getMarketClock).toHaveBeenCalledTimes(2)

    await user.click(placeButton())
    await screen.findByText(MARKET_CLOSED_LINE)
    expect(mocks.placeStrategyBasket).not.toHaveBeenCalled()

    // The older interval read now resolves "open"; it must not undo the refusal.
    await act(async () => {
      resolveIntervalRead(OPEN)
    })
    expect(screen.getByText(MARKET_CLOSED_LINE)).toBeInTheDocument()
    expect(placeButton()).toBeDisabled()
  })

  it('does not gate a batch that is only GTC trailing stops', async () => {
    const protectOnly: OrderRecommendation = {
      ...base,
      id: 'p9',
      symbol: 'NVDA',
      exchange: 'NASDAQ',
      side: 'SELL',
      quantity: 3,
      orderType: 'TRAILING_STOP',
      price: 0,
      trailPercent: 10,
      validity: 'GTC',
      role: 'protect',
      group: 'g2',
      rationale: 'Protect NVDA.',
    }
    seed([protectOnly])
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    expect(screen.getByText('Protect NVDA')).toBeInTheDocument()
    expect(screen.getByText('Trailing Stop · GTC')).toBeInTheDocument()
    await acknowledge(user)
    expect(placeButton()).toBeEnabled()
    expect(screen.queryByText(MARKET_CLOSED_LINE)).toBeNull()

    await user.click(placeButton())
    await waitFor(() => expect(mocks.placeStrategyBasket).toHaveBeenCalledTimes(1))
    expect(mocks.placeStrategyBasket).toHaveBeenCalledWith(
      [
        {
          group: 'g2',
          role: 'protect',
          symbol: 'NVDA',
          order: {
            symbol: 'NVDA',
            exchange: 'NASDAQ',
            side: 'SELL',
            quantity: 3,
            price: 0,
            order_type: 'TRAILING_STOP',
            product: 'CNC',
            validity: 'GTC',
            amo: false,
            trail_percent: 10,
          },
        },
      ],
      false
    )
    expect(mocks.getMarketClock).not.toHaveBeenCalled()
    expect(mocks.tradingPlaceBasketOrder).not.toHaveBeenCalled()
    await screen.findByText('NVDA · protect')
  })

  it('passes validity GTC through to the strategy basket legs', async () => {
    mocks.placeStrategyBasket.mockResolvedValue([
      {
        group: 'g1',
        role: 'cancel',
        symbol: 'PFE',
        success: true,
        order_id: null,
        filled_quantity: 0,
        message: 'Cancelled ord-stop-1',
      },
      {
        group: 'g1',
        role: 'fund',
        symbol: 'PFE',
        success: true,
        order_id: 'ord-f',
        filled_quantity: 10,
        message: 'Filled 10, remainder confirmed cancelled',
      },
      {
        group: 'g1',
        role: 'buy',
        symbol: 'BAC',
        success: true,
        order_id: 'ord-b',
        filled_quantity: 5,
        message: '',
      },
      {
        group: 'g1',
        role: 'protect',
        symbol: 'BAC',
        success: false,
        order_id: null,
        filled_quantity: 0,
        message: 'Trailing stop rejected',
      },
    ])
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    await acknowledge(user)
    await waitFor(() => expect(placeButton()).toBeEnabled())
    await user.click(placeButton())

    await waitFor(() => expect(mocks.placeStrategyBasket).toHaveBeenCalledTimes(1))
    const marketOrder = { exchange: 'NYSE', price: 0, order_type: 'MARKET', product: 'CNC' }
    expect(mocks.placeStrategyBasket).toHaveBeenCalledWith(
      [
        { group: 'g1', role: 'cancel', symbol: 'PFE', cancel_order_id: 'ord-stop-1' },
        {
          group: 'g1',
          role: 'fund',
          symbol: 'PFE',
          order: {
            ...marketOrder,
            symbol: 'PFE',
            side: 'SELL',
            quantity: 10,
            validity: 'DAY',
            amo: false,
          },
        },
        {
          group: 'g1',
          role: 'buy',
          symbol: 'BAC',
          order: {
            ...marketOrder,
            symbol: 'BAC',
            side: 'BUY',
            quantity: 5,
            validity: 'DAY',
            amo: false,
          },
        },
        {
          group: 'g1',
          role: 'protect',
          symbol: 'BAC',
          order: {
            symbol: 'BAC',
            exchange: 'NYSE',
            side: 'SELL',
            quantity: 5,
            price: 0,
            order_type: 'TRAILING_STOP',
            product: 'CNC',
            validity: 'GTC',
            amo: false,
            trail_percent: 8,
          },
        },
      ],
      false
    )
    expect(mocks.tradingPlaceBasketOrder).not.toHaveBeenCalled()

    // Results list each leg as SYMBOL · role with its message (successes too).
    expect(await screen.findByText('PFE · fund')).toBeInTheDocument()
    // A successful strategy leg shows its message instead of the order ID.
    expect(screen.getByText('Filled 10, remainder confirmed cancelled')).toBeInTheDocument()
    expect(screen.queryByText('Order ID: ord-f')).toBeNull()
    expect(screen.getByText('PFE · cancel')).toBeInTheDocument()
    expect(screen.getByText('Cancelled ord-stop-1')).toBeInTheDocument()
    expect(screen.getByText('BAC · buy')).toBeInTheDocument()
    expect(screen.getByText('Order ID: ord-b')).toBeInTheDocument()
    expect(screen.getByText('BAC · protect')).toBeInTheDocument()
    expect(screen.getByText('Trailing stop rejected')).toBeInTheDocument()
    expect(mocks.toast.warning).toHaveBeenCalledWith('3 completed, 1 failed')
    expect(mocks.emit).toHaveBeenCalledWith('basket_order_event', { count: 3 })
  })

  it('cannot be closed while the strategy basket is running, then shows results in the same modal', async () => {
    let finishPlacement: (results: unknown[]) => void = () => undefined
    mocks.placeStrategyBasket.mockImplementation(
      () =>
        new Promise((resolve) => {
          finishPlacement = resolve
        })
    )
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    await acknowledge(user)
    await waitFor(() => expect(placeButton()).toBeEnabled())
    await user.click(placeButton())
    await waitFor(() => expect(mocks.placeStrategyBasket).toHaveBeenCalledTimes(1))
    expect(useActionQueueStore.getState().isSubmitting).toBe(true)

    const cancelButton = screen.getByRole('button', { name: 'Cancel' })
    expect(cancelButton).toBeDisabled()

    await user.keyboard('{Escape}')
    expect(useActionQueueStore.getState().isReviewOpen).toBe(true)

    await user.click(cancelButton)
    expect(useActionQueueStore.getState().isReviewOpen).toBe(true)

    // The dialog's own X routes through onOpenChange(false), which is ignored mid-run.
    await user.click(screen.getByRole('button', { name: 'Close' }))
    expect(useActionQueueStore.getState().isReviewOpen).toBe(true)
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled()

    await act(async () => {
      finishPlacement([
        {
          group: 'g1',
          role: 'protect',
          symbol: 'BAC',
          success: true,
          order_id: 'ord-p',
          filled_quantity: 5,
          message: 'BAC trailing stop placed (GTC)',
        },
      ])
    })
    expect(await screen.findByText('BAC · protect')).toBeInTheDocument()
    expect(screen.getByText('BAC trailing stop placed (GTC)')).toBeInTheDocument()
    expect(useActionQueueStore.getState().isReviewOpen).toBe(true)
  })

  it('stops for re-review when the submit-time funds re-check changes a quantity', async () => {
    const revalidated = rotation.map((item) => (item.id === 'b1' ? { ...item, quantity: 2 } : item))
    mocks.reconcile.mockResolvedValueOnce({
      items: revalidated,
      availableCash: 40,
      notice: 'Checked against your account: reduced BAC 5→2 to fit your available cash ($40.00).',
    })
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    await acknowledge(user)
    await waitFor(() => expect(placeButton()).toBeEnabled())
    await user.click(placeButton())

    await waitFor(() =>
      expect(mocks.toast.warning).toHaveBeenCalledWith(
        'Your spendable funds changed — quantities were adjusted to what the account can afford. Review and place again.'
      )
    )
    expect(mocks.reconcile).toHaveBeenCalledTimes(1)
    expect(mocks.placeStrategyBasket).not.toHaveBeenCalled()
    expect(mocks.tradingPlaceBasketOrder).not.toHaveBeenCalled()
    const state = useActionQueueStore.getState()
    expect(state.items).toEqual(revalidated)
    expect(state.fundsNotice).toBe(
      'Checked against your account: reduced BAC 5→2 to fit your available cash ($40.00).'
    )
    expect(state.isSubmitting).toBe(false)
    expect(state.isReviewOpen).toBe(true)
    expect(await screen.findByText(/reduced BAC 5→2/)).toBeInTheDocument()
  })

  it('shows the clock-read failure on the market-closed line and keeps Place disabled', async () => {
    mocks.getMarketClock.mockRejectedValue('broker unreachable')
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    await screen.findByText(`${MARKET_CLOSED_LINE} (could not read the market clock)`)
    await acknowledge(user)
    expect(placeButton()).toBeDisabled()
    expect(mocks.placeStrategyBasket).not.toHaveBeenCalled()
  })

  it('surfaces the backend rejection text when the strategy basket is refused', async () => {
    mocks.placeStrategyBasket.mockRejectedValue(
      'Execution mode changed since you confirmed. Review and confirm again.'
    )
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    await acknowledge(user)
    await waitFor(() => expect(placeButton()).toBeEnabled())
    await user.click(placeButton())

    await waitFor(() =>
      expect(mocks.toast.error).toHaveBeenCalledWith(
        'Execution mode changed since you confirmed. Review and confirm again.'
      )
    )
    expect(placeButton()).toBeEnabled()
    expect(useActionQueueStore.getState().isSubmitting).toBe(false)
  })

  it('passes validity through the ungrouped basket path without gating', async () => {
    const ungrouped: OrderRecommendation[] = [
      {
        ...base,
        id: 'u1',
        symbol: 'AAPL',
        exchange: 'NASDAQ',
        side: 'SELL',
        quantity: 4,
        orderType: 'TRAILING_STOP',
        price: 0,
        trailPrice: 5,
        validity: 'GTC',
        rationale: 'Trail AAPL.',
      },
      {
        ...base,
        id: 'u2',
        symbol: 'KO',
        side: 'BUY',
        quantity: 2,
        orderType: 'LIMIT',
        price: 60,
        rationale: 'Add KO.',
      },
    ]
    seed(ungrouped)
    mocks.tradingPlaceBasketOrder.mockResolvedValue({
      status: 'success',
      data: [
        // The real backend always sends a success message (order_service.rs).
        {
          symbol: 'AAPL',
          exchange: 'NASDAQ',
          orderId: 'o-1',
          success: true,
          message: 'Order placed successfully',
        },
        {
          symbol: 'KO',
          exchange: 'NYSE',
          orderId: 'o-2',
          success: true,
          message: 'Order placed successfully',
        },
      ],
    })
    const user = userEvent.setup()
    render(<ActionReviewModal />)

    expect(screen.queryByText(/^Rotation:/)).toBeNull()
    await acknowledge(user)
    await user.click(placeButton())

    await waitFor(() => expect(mocks.tradingPlaceBasketOrder).toHaveBeenCalledTimes(1))
    expect(mocks.tradingPlaceBasketOrder).toHaveBeenCalledWith(
      [
        {
          symbol: 'AAPL',
          exchange: 'NASDAQ',
          action: 'SELL',
          product: 'CNC',
          pricetype: 'TRAILING_STOP',
          price: 0,
          quantity: 4,
          validity: 'GTC',
          amo: false,
          trail_price: 5,
        },
        {
          symbol: 'KO',
          exchange: 'NYSE',
          action: 'BUY',
          product: 'CNC',
          pricetype: 'LIMIT',
          price: 60,
          quantity: 2,
          validity: 'DAY',
          amo: false,
        },
      ],
      false
    )
    expect(mocks.placeStrategyBasket).not.toHaveBeenCalled()
    expect(mocks.getMarketClock).not.toHaveBeenCalled()
    // Ordinary basket results keep showing the order ID, not the stock message.
    expect(await screen.findByText('Order ID: o-2')).toBeInTheDocument()
    expect(screen.getByText('Order ID: o-1')).toBeInTheDocument()
    expect(screen.queryByText('Order placed successfully')).toBeNull()
    expect(mocks.toast.success).toHaveBeenCalledWith('All 2 orders placed successfully')
  })

  it('adds sell proceeds to the Gate B cost line and funds the buys with them', async () => {
    mocks.getQuote.mockResolvedValue([{ symbol: 'PFE', exchange: 'NYSE', ltp: 25 }])
    seed([
      rotation[1],
      { ...rotation[2], orderType: 'LIMIT', price: 40, quantity: 30 }, // $1,200 > $1,000 cash
    ])
    render(<ActionReviewModal />)

    const proceeds = await screen.findByText(/Sell proceeds about/)
    const paragraph = proceeds.closest('p') as HTMLElement
    expect(paragraph.textContent?.replace(/\s+/g, ' ')).toContain(
      'live price). Sell proceeds about $250. A realistic drawdown'
    )
    expect(mocks.getQuote).toHaveBeenCalledWith([{ exchange: 'NYSE', symbol: 'PFE' }])
    // $1,200 of buys against $1,000 cash + $250 proceeds: no over-budget warning.
    expect(screen.queryByText(/exceed it and will be reduced/)).toBeNull()
  })

  it('shows the over-budget warning for the same buy when the sell cannot be priced', async () => {
    mocks.getQuote.mockRejectedValue('quote feed down')
    seed([
      rotation[1],
      { ...rotation[2], orderType: 'LIMIT', price: 40, quantity: 30 }, // $1,200 > $1,000 cash
    ])
    render(<ActionReviewModal />)

    expect(await screen.findByText(/exceed it and will be reduced/)).toBeInTheDocument()
    await waitFor(() => expect(mocks.getQuote).toHaveBeenCalledTimes(1))
    expect(screen.queryByText(/Sell proceeds about/)).toBeNull()
  })

  describe('scenario mode', () => {
    const scenario = { exchange: 'GENERIC', product: 'CNC' as const, source: 'copilot' as const }
    const buyVoo: OrderRecommendation = {
      ...scenario,
      id: 's1',
      symbol: 'VOO',
      side: 'BUY',
      quantity: 9,
      orderType: 'MARKET',
      price: 0,
      rationale: 'Redeploy into the index core.',
    }
    const sellBngi: OrderRecommendation = {
      ...buyVoo,
      id: 's2',
      symbol: 'BNGI',
      side: 'SELL',
      quantity: 100000,
      rationale: 'Total loss.',
    }

    it('shows the live price for a $0 trade and applies at it', async () => {
      const user = userEvent.setup()
      mocks.getGenericQuote.mockResolvedValue([{ symbol: 'VOO', price: 696.94 }])
      const onApply = vi.fn().mockResolvedValue(undefined)
      seed([buyVoo])
      render(<ActionReviewModal onApply={onApply} />)

      expect(await screen.findByText('$696.94')).toBeInTheDocument()
      expect(screen.getByTestId('scenario-price').textContent).toBe('Applies at live price $696.94')
      expect(mocks.getGenericQuote).toHaveBeenCalledWith(['VOO'])
      await user.click(screen.getByRole('button', { name: 'Apply to Scenario' }))
      expect(onApply).toHaveBeenCalledWith([{ ...buyVoo, price: 696.94 }], undefined)
    })

    it('blocks a $0 trade with no live price and applies the rest', async () => {
      const user = userEvent.setup()
      mocks.getGenericQuote.mockResolvedValue([{ symbol: 'VOO', price: 696.94 }])
      const onApply = vi.fn().mockResolvedValue(undefined)
      seed([buyVoo, sellBngi])
      render(<ActionReviewModal onApply={onApply} />)

      expect(await screen.findByText("No live price — this trade won't be applied.")).toBeInTheDocument()
      await user.click(screen.getByRole('button', { name: 'Apply to Scenario' }))
      expect(onApply).toHaveBeenCalledWith([{ ...buyVoo, price: 696.94 }], undefined)
      expect(mocks.toast.warning).toHaveBeenCalledWith('Not applied (no live price): BNGI', {
        duration: 8000,
      })
    })

    it('keeps Apply disabled until the quotes land, and applies nothing when none price', async () => {
      const user = userEvent.setup()
      let resolve: (q: unknown[]) => void = () => {}
      mocks.getGenericQuote.mockReturnValue(new Promise((r) => (resolve = r)))
      const onApply = vi.fn()
      seed([buyVoo])
      render(<ActionReviewModal onApply={onApply} />)

      expect(await screen.findByText('Getting live price…')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Apply to Scenario' })).toBeDisabled()
      await act(async () => resolve([]))
      await user.click(screen.getByRole('button', { name: 'Apply to Scenario' }))
      expect(onApply).not.toHaveBeenCalled()
      expect(mocks.toast.error).toHaveBeenCalledWith('No trade has a live price, so nothing was applied.')
    })

    it('never quotes a trade that already has a price', async () => {
      seed([{ ...buyVoo, orderType: 'LIMIT', price: 690 }])
      render(<ActionReviewModal onApply={vi.fn()} />)
      await waitFor(() =>
        expect(screen.getByRole('button', { name: 'Apply to Scenario' })).toBeEnabled()
      )
      expect(mocks.getGenericQuote).not.toHaveBeenCalled()
      expect(screen.queryByTestId('scenario-price')).toBeNull()
    })
  })
})
