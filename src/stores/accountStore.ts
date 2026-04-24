/**
 * Account Store — tracks which account is selected in the account switcher.
 *
 * "all" = show all accounts combined (default).
 * Any other string = an account_type value from import_batches (e.g. "individual", "401k", "traditional_ira").
 *
 * This store is used by:
 * - The AccountSwitcher dropdown in the Navbar (main app)
 * - The ClientDetail page (client sandbox)
 */

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { ClientAccount } from '@/types/clients'

interface AccountStoreState {
  /** Currently selected account_type, or "all" for combined view */
  selectedAccount: string
  /** Available accounts for the current context (client or main app) */
  accounts: ClientAccount[]
  /** Whether accounts are loading */
  isLoading: boolean

  // Actions
  setSelectedAccount: (accountType: string) => void
  fetchClientAccounts: (clientId: number) => Promise<void>
  reset: () => void
}

export const useAccountStore = create<AccountStoreState>()((set) => ({
  selectedAccount: 'all',
  accounts: [],
  isLoading: false,

  setSelectedAccount: (accountType: string) => {
    set({ selectedAccount: accountType })
  },

  fetchClientAccounts: async (clientId: number) => {
    set({ isLoading: true })
    try {
      const accounts = await invoke<ClientAccount[]>('get_client_accounts', { clientId })
      set({ accounts, isLoading: false })
    } catch {
      set({ accounts: [], isLoading: false })
    }
  },

  reset: () => {
    set({ selectedAccount: 'all', accounts: [], isLoading: false })
  },
}))
