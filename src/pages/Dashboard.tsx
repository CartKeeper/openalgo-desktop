import { invoke } from '@tauri-apps/api/core'
import {
  BarChart3,
  BookOpen,
  FileText,
  HelpCircle,
  MessageCircle,
  Search,
  Zap,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { LivePositionsCard } from '@/components/dashboard/LivePositionsCard'
import { OpenOrdersCard } from '@/components/dashboard/OpenOrdersCard'
import { RecentTradesCard } from '@/components/dashboard/RecentTradesCard'
import { WatchlistCard } from '@/components/dashboard/WatchlistCard'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useLivePositions } from '@/hooks/useLivePositions'
import { formatSignedUSD, formatUSD, pnlColorClass } from '@/lib/format'
import { cn } from '@/lib/utils'
import { onModeChange, useThemeStore } from '@/stores/themeStore'

interface FundsData {
  available_cash: number
  used_margin: number
  total_margin: number
  opening_balance: number
  payin: number
  payout: number
  span: number
  exposure: number
  collateral: number
}

interface MarginData {
  availablecash: string
  collateral: string
  m2munrealized: string
  m2mrealized: string
  utiliseddebits: string
}

interface MasterContractStatus {
  status: 'pending' | 'downloading' | 'success' | 'error'
  message?: string
  total_symbols?: number
}

interface SandboxFunds {
  initial_capital: number
  available_capital: number
  used_margin: number
  realized_pnl: number
  unrealized_pnl: number
  total_pnl: number
}

export default function Dashboard() {
  const [marginData, setMarginData] = useState<MarginData | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [masterContract, setMasterContract] = useState<MasterContractStatus>({
    status: 'pending',
  })
  const [isAuthenticated, setIsAuthenticated] = useState(true) // Assume authenticated initially
  const { appMode } = useThemeStore()

  // Live positions (broker poll + WebSocket price overlay) drive the portfolio
  // metrics and the positions table below.
  const { positions, totals, isLoading: positionsLoading, isLive } = useLivePositions()

  // Fetch dashboard funds data using Tauri invoke
  const fetchFundsData = useCallback(async () => {
    try {
      setIsLoading(true)

      // In analyzer mode, fetch sandbox funds instead of live funds
      if (appMode === 'analyzer' || appMode === 'sandbox') {
        const sandboxFunds = await invoke<SandboxFunds>('get_sandbox_funds')
        // Convert SandboxFunds to MarginData format for compatibility
        setMarginData({
          availablecash: sandboxFunds.available_capital.toString(),
          collateral: '0', // Sandbox doesn't track collateral
          m2munrealized: sandboxFunds.unrealized_pnl.toString(),
          m2mrealized: sandboxFunds.realized_pnl.toString(),
          utiliseddebits: sandboxFunds.used_margin.toString(),
        })
        setError(null)
      } else {
        // Live mode - fetch from broker
        const funds = await invoke<FundsData>('get_funds')
        // Convert FundsData to MarginData format for compatibility
        setMarginData({
          availablecash: funds.available_cash.toString(),
          collateral: funds.collateral.toString(),
          m2munrealized: '0', // Not available from funds API
          m2mrealized: '0', // Not available from funds API
          utiliseddebits: funds.used_margin.toString(),
        })
        setError(null)
      }
    } catch (err) {
      console.error('Error fetching funds:', err)
      // Check if it's an auth error
      const errorMsg = err instanceof Error ? err.message : String(err)
      if (errorMsg.includes('not authenticated') || errorMsg.includes('No broker session')) {
        setIsAuthenticated(false)
      } else {
        setError('Failed to fetch margin data')
      }
    } finally {
      setIsLoading(false)
    }
  }, [appMode])

  useEffect(() => {
    fetchFundsData()
    // Refresh every 30 seconds
    const interval = setInterval(fetchFundsData, 30000)
    return () => clearInterval(interval)
  }, [fetchFundsData])

  // Listen for mode changes and refresh data
  useEffect(() => {
    const unsubscribe = onModeChange(() => {
      // Refresh funds data when mode changes
      fetchFundsData()
    })
    return () => unsubscribe()
  }, [fetchFundsData])

  // Check master contract status using Tauri invoke
  const checkMasterContractStatus = useCallback(async () => {
    try {
      // Get symbol count from the symbol cache
      const count = await invoke<number>('get_symbol_count')
      if (count > 0) {
        setMasterContract({ status: 'success', total_symbols: count })
      } else {
        setMasterContract({ status: 'pending', message: 'No symbols loaded' })
      }
    } catch (_err) {
      setMasterContract({ status: 'error', message: 'Failed to check status' })
    }
  }, [])

  useEffect(() => {
    checkMasterContractStatus()

    // Poll every 5 seconds until successful
    const interval = setInterval(() => {
      setMasterContract((prev) => {
        if (prev.status === 'success') {
          return prev // Don't check again if already successful
        }
        checkMasterContractStatus()
        return prev
      })
    }, 5000)

    return () => clearInterval(interval)
  }, [checkMasterContractStatus])

  // Master Contract LED color
  const getMasterContractLedColor = () => {
    switch (masterContract.status) {
      case 'success':
        return 'bg-green-500'
      case 'downloading':
        return 'bg-yellow-500 animate-pulse'
      case 'error':
        return 'bg-red-500'
      default:
        return 'bg-gray-400 animate-pulse'
    }
  }

  const getMasterContractStatusText = () => {
    switch (masterContract.status) {
      case 'success':
        return masterContract.total_symbols
          ? `Ready (${masterContract.total_symbols} symbols)`
          : 'Ready'
      case 'downloading':
        return 'Downloading...'
      case 'error':
        return 'Error'
      default:
        return 'Checking...'
    }
  }

  const getMasterContractTextColor = () => {
    switch (masterContract.status) {
      case 'success':
        return 'text-green-600 dark:text-green-400'
      case 'downloading':
        return 'text-yellow-600 dark:text-yellow-400'
      case 'error':
        return 'text-red-600 dark:text-red-400'
      default:
        return 'text-muted-foreground'
    }
  }

  const quickAccessCards = [
    {
      href: '/search',
      label: 'OpenAlgo Symbols',
      description: 'Universal symbology across brokers',
      icon: Search,
      gradient: 'from-primary/10 to-primary/5 hover:from-primary/20 hover:to-primary/10',
      iconBg: 'bg-primary/20',
      iconColor: 'text-primary',
      borderColor: 'border-primary/20 hover:border-primary/40',
    },
    {
      href: '/logs',
      label: 'Live Logs',
      description: 'Real-time trading activity logs',
      icon: FileText,
      gradient:
        'from-violet-500/10 to-violet-500/5 hover:from-violet-500/20 hover:to-violet-500/10',
      iconBg: 'bg-violet-500/20',
      iconColor: 'text-violet-500',
      borderColor: 'border-violet-500/20 hover:border-violet-500/40',
    },
    {
      href: 'https://docs.openalgo.in',
      label: 'Documentation',
      description: 'Tutorials, API docs & features',
      icon: BookOpen,
      gradient: 'from-cyan-500/10 to-cyan-500/5 hover:from-cyan-500/20 hover:to-cyan-500/10',
      iconBg: 'bg-cyan-500/20',
      iconColor: 'text-cyan-500',
      borderColor: 'border-cyan-500/20 hover:border-cyan-500/40',
      external: true,
    },
    {
      href: '/pnl-tracker',
      label: 'P&L Tracker',
      description: 'Live intraday MTM tracker',
      icon: BarChart3,
      gradient: 'from-green-500/10 to-green-500/5 hover:from-green-500/20 hover:to-green-500/10',
      iconBg: 'bg-green-500/20',
      iconColor: 'text-green-500',
      borderColor: 'border-green-500/20 hover:border-green-500/40',
    },
    {
      href: '/telegram',
      label: 'Telegram Alerts',
      description: 'Configure telegram notifications',
      icon: MessageCircle,
      gradient: 'from-blue-500/10 to-blue-500/5 hover:from-blue-500/20 hover:to-blue-500/10',
      iconBg: 'bg-blue-500/20',
      iconColor: 'text-blue-500',
      borderColor: 'border-blue-500/20 hover:border-blue-500/40',
    },
    {
      href: '/latency',
      label: 'Latency Monitor',
      description: 'Monitor order & API latency',
      icon: Zap,
      gradient:
        'from-orange-500/10 to-orange-500/5 hover:from-orange-500/20 hover:to-orange-500/10',
      iconBg: 'bg-orange-500/20',
      iconColor: 'text-orange-500',
      borderColor: 'border-orange-500/20 hover:border-orange-500/40',
    },
  ]

  // If not authenticated, show login prompt
  if (!isAuthenticated) {
    return (
      <div className="flex flex-col items-center justify-center min-h-[50vh] space-y-4">
        <h1 className="text-2xl font-bold">Session Expired</h1>
        <p className="text-muted-foreground">Please log in to access the dashboard.</p>
        <Link to="/login" className="text-primary hover:underline">
          Go to Login
        </Link>
      </div>
    )
  }

  return (
    <div className="space-y-6 md:space-y-12">
      {/* Dashboard Header */}
      <div className="flex flex-col lg:flex-row lg:items-start gap-4">
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h1 className="text-2xl md:text-3xl font-bold">Trading Dashboard</h1>
            <Popover>
              <PopoverTrigger
                aria-label="Dashboard help"
                className="text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-full"
              >
                <HelpCircle className="h-5 w-5" strokeWidth={1.5} />
              </PopoverTrigger>
              <PopoverContent align="start" className="w-80 text-sm">
                <p className="font-semibold mb-1">Dashboard</p>
                <ul className="space-y-1.5 text-muted-foreground">
                  <li>
                    <strong className="text-foreground">Metrics</strong> show cash and margin from
                    your broker; P&amp;L and market value are computed live from your open positions.
                  </li>
                  <li>
                    <strong className="text-foreground">Open Positions</strong> update tick-by-tick
                    when the live price feed is connected (green “Live” dot).
                  </li>
                  <li>
                    <strong className="text-foreground">Open Orders</strong> lists working orders —
                    cancel inline, or open the Order Book to modify.
                  </li>
                  <li>
                    <strong className="text-foreground">Recent Trades</strong> shows today’s fills.
                  </li>
                </ul>
              </PopoverContent>
            </Popover>
          </div>
          <p className="text-muted-foreground mt-1 md:mt-2 text-sm md:text-base">
            Overview of your trading account and market positions
          </p>
        </div>
        {/* Master Contract Status Indicator */}
        <div className="flex items-center gap-2 md:gap-3 bg-muted rounded-lg px-3 md:px-4 py-2 md:py-3 w-fit lg:ml-auto lg:self-start">
          <span className="text-xs md:text-sm font-medium whitespace-nowrap">Master Contract:</span>
          <div className="flex items-center gap-2">
            <div
              className={cn('w-2.5 h-2.5 md:w-3 md:h-3 rounded-full', getMasterContractLedColor())}
            />
            <span
              className={cn('text-xs md:text-sm', getMasterContractTextColor())}
              title={masterContract.message}
            >
              {getMasterContractStatusText()}
            </span>
          </div>
        </div>
      </div>

      {/* Portfolio Metrics (USD). P&L / market value come from live positions;
          cash / collateral / margin come from the broker funds endpoint. */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-4 md:gap-6">
        {[
          {
            label: 'Available Cash',
            value: marginData ? formatUSD(marginData.availablecash) : '--',
            badge: 'Cash Balance',
            color: 'text-primary',
          },
          {
            label: 'Unrealized P&L',
            value: formatSignedUSD(totals.unrealized),
            badge: 'Mark to Market',
            color: pnlColorClass(totals.unrealized),
          },
          {
            label: 'Realized P&L',
            value: formatSignedUSD(totals.realized),
            badge: 'Booked P&L',
            color: pnlColorClass(totals.realized),
          },
          {
            label: 'Market Value',
            value: formatUSD(totals.marketValue),
            badge: 'Positions',
            color: 'text-foreground',
          },
          {
            label: 'Open Positions',
            value: String(totals.openCount),
            badge: 'Holdings',
            color: 'text-foreground',
          },
          {
            label: 'Collateral',
            value: marginData ? formatUSD(marginData.collateral) : '--',
            badge: 'Total Collateral',
            color: 'text-violet-500 dark:text-violet-400',
          },
          {
            label: 'Utilised Margin',
            value: marginData ? formatUSD(marginData.utiliseddebits) : '--',
            badge: 'Used Margin',
            color: 'text-cyan-500 dark:text-cyan-400',
          },
        ].map((m) => (
          <Card key={m.label}>
            <CardContent className="pt-6">
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">{m.label}</p>
                <p className={cn('text-2xl font-bold tabular-nums', m.color)}>
                  {isLoading && !marginData ? '...' : m.value}
                </p>
                <Badge variant="secondary" className="mt-2">
                  {m.badge}
                </Badge>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Live trading data */}
      <div className="grid grid-cols-1 xl:grid-cols-2 gap-4 md:gap-6">
        <LivePositionsCard positions={positions} isLoading={positionsLoading} isLive={isLive} />
        <OpenOrdersCard />
      </div>
      <RecentTradesCard />

      {/* Watchlist */}
      <WatchlistCard />

      {/* Error Alert */}
      {error && (
        <Card className="border-destructive bg-destructive/5">
          <CardContent className="pt-6">
            <p className="text-destructive text-sm">{error}</p>
          </CardContent>
        </Card>
      )}

      {/* Quick Access Tools */}
      <div>
        <h2 className="text-xl md:text-2xl font-semibold mb-4 md:mb-6">Quick Access</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 md:gap-5">
          {quickAccessCards.map((card) => {
            const cardClasses = cn(
              'block rounded-lg border transition-all duration-300 hover:shadow-lg',
              `bg-gradient-to-br ${card.gradient}`,
              card.borderColor
            )

            const cardContent = (
              <div className="p-4 md:p-5">
                <div className="flex items-start gap-3 md:gap-4">
                  <div className={cn('p-2.5 md:p-3 rounded-lg flex-shrink-0', card.iconBg)}>
                    <card.icon className={cn('h-5 w-5 md:h-6 md:w-6', card.iconColor)} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="font-semibold mb-1 text-base md:text-lg">{card.label}</h3>
                    <p className="text-sm text-muted-foreground">{card.description}</p>
                  </div>
                </div>
              </div>
            )

            if (card.external) {
              return (
                <a
                  key={card.href}
                  href={card.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={cardClasses}
                >
                  {cardContent}
                </a>
              )
            }

            return (
              <Link key={card.href} to={card.href} className={cardClasses}>
                {cardContent}
              </Link>
            )
          })}
        </div>
      </div>
    </div>
  )
}
