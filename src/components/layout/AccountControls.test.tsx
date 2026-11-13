// @vitest-environment jsdom
import '@/test/setup'
import '@/components/flow/testSetup'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useAccountStore } from '@/stores/accountStore'
import { useAuthStore } from '@/stores/authStore'
import { useThemeStore } from '@/stores/themeStore'
import { AccountControls } from './AccountControls'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(async () => null),
  toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn(), info: vi.fn() },
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('sonner', () => ({ toast: mocks.toast }))

function renderControls(onNavigate?: (href: string) => void) {
  return render(
    <MemoryRouter>
      <AccountControls onNavigate={onNavigate} />
    </MemoryRouter>
  )
}

beforeEach(() => {
  useThemeStore.setState({ mode: 'light', appMode: 'live', isTogglingMode: false })
  useAuthStore.setState({
    user: {
      username: 'jason',
      broker: 'alpaca',
      brokerId: 'alpaca',
      isLoggedIn: true,
      loginTime: null,
    },
    isAuthenticated: true,
  })
  useAccountStore.setState({ accounts: [], selectedAccount: 'all' })
})

/**
 * The account end of the app's header, for the Flow editor (which has no
 * navbar): the same pieces as the Navbar, so the editor shows at a glance
 * whether orders go to the sandbox, and a profile entry goes through the
 * editor's guarded navigate.
 */
describe('AccountControls', () => {
  it('shows the broker and the mode, and switches the mode through the theme store', async () => {
    const toggleAppMode = vi.fn(async () => ({ success: true }))
    useThemeStore.setState({ toggleAppMode })
    renderControls()
    expect(screen.getByText('alpaca')).toBeInTheDocument()
    expect(screen.getByText('Live Mode')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Switch to Analyze mode' }))
    expect(toggleAppMode).toHaveBeenCalledTimes(1)
  })

  it('switches between light and dark', async () => {
    const toggleMode = vi.fn()
    useThemeStore.setState({ toggleMode })
    renderControls()
    await userEvent.click(screen.getByRole('button', { name: 'Switch to dark mode' }))
    expect(toggleMode).toHaveBeenCalledTimes(1)
  })

  it('routes a profile menu entry through onNavigate', async () => {
    const onNavigate = vi.fn()
    renderControls(onNavigate)
    await userEvent.click(screen.getByRole('button', { name: 'Open user menu' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: /Profile/ }))
    expect(onNavigate).toHaveBeenCalledWith('/profile')
  })

  it('shows the account switcher when there is more than one account', () => {
    useAccountStore.setState({
      accounts: [
        { account_type: 'individual', trade_count: 3 },
        { account_type: '401k', trade_count: 1 },
      ] as never,
    })
    renderControls()
    expect(screen.getByRole('button', { name: /All Accounts/ })).toBeInTheDocument()
  })
})
