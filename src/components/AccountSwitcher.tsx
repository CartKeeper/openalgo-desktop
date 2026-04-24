import { ChevronDown, Layers, Wallet } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { ACCOUNT_TYPES } from '@/types/clients'
import { useAccountStore } from '@/stores/accountStore'
import { cn } from '@/lib/utils'

/** Resolve account_type value to a display label */
function getAccountLabel(accountType: string): string {
  if (accountType === 'all') return 'All Accounts'
  if (accountType === 'manual') return 'Manual Trades'
  if (accountType === 'unspecified') return 'Unspecified'
  const found = ACCOUNT_TYPES.find((t) => t.value === accountType)
  return found?.label ?? accountType
}

export function AccountSwitcher() {
  const { selectedAccount, accounts, setSelectedAccount } = useAccountStore()

  // Don't render if there are fewer than 2 distinct accounts (no switching needed)
  if (accounts.length < 2) return null

  const currentLabel = getAccountLabel(selectedAccount)

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="h-8 gap-1.5 text-xs font-medium px-3 max-w-[200px]"
        >
          <Wallet className="h-3.5 w-3.5 flex-shrink-0" />
          <span className="truncate">{currentLabel}</span>
          <ChevronDown className="h-3 w-3 flex-shrink-0 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-56">
        {/* All Accounts option */}
        <DropdownMenuItem
          onSelect={() => setSelectedAccount('all')}
          className={cn('cursor-pointer gap-2', selectedAccount === 'all' && 'bg-primary/10 text-primary')}
        >
          <Layers className="h-4 w-4" />
          <span className="flex-1">All Accounts</span>
          <span className="text-[10px] font-medium text-muted-foreground tabular-nums">
            {accounts.reduce((sum, a) => sum + a.trade_count, 0)}
          </span>
        </DropdownMenuItem>

        <DropdownMenuSeparator />

        {/* Individual account options */}
        {accounts.map((account) => (
          <DropdownMenuItem
            key={account.account_type}
            onSelect={() => setSelectedAccount(account.account_type)}
            className={cn(
              'cursor-pointer gap-2',
              selectedAccount === account.account_type && 'bg-primary/10 text-primary'
            )}
          >
            <Wallet className="h-4 w-4" />
            <span className="flex-1 truncate">{getAccountLabel(account.account_type)}</span>
            <span className="text-[10px] font-medium text-muted-foreground tabular-nums">
              {account.trade_count}
            </span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
