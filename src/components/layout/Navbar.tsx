import { BarChart3, Bell, BookOpen, LogOut, Menu, Moon, Sun, Zap } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { authApi } from '@/api/auth'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet'
import {
  filterNavItems,
  filterSidebarGroups,
  isActiveRoute,
  profileMenuItems,
  sidebarGroups,
} from '@/config/navigation'
import { cn } from '@/lib/utils'
import { useAlertStore } from '@/stores/alertStore'
import { useAuthStore } from '@/stores/authStore'
import { useThemeStore } from '@/stores/themeStore'

export function Navbar() {
  const location = useLocation()
  const navigate = useNavigate()
  const [mobileOpen, setMobileOpen] = useState(false)
  const { mode, appMode, toggleMode, toggleAppMode, isTogglingMode } = useThemeStore()
  const { user, logout } = useAuthStore()
  const { unacknowledgedCount, fetchUnacknowledgedCount } = useAlertStore()

  useEffect(() => {
    fetchUnacknowledgedCount()
    const interval = setInterval(fetchUnacknowledgedCount, 30000)
    return () => clearInterval(interval)
  }, [fetchUnacknowledgedCount])

  const handleLogout = async () => {
    try {
      await authApi.logout()
      logout()
      navigate('/login')
      toast.success('Logged out successfully')
    } catch {
      logout()
      navigate('/login')
    }
  }

  const handleModeToggle = async () => {
    const result = await toggleAppMode()
    if (result.success) {
      const newMode = useThemeStore.getState().appMode
      toast.success(`Switched to ${newMode === 'live' ? 'Live' : 'Analyze'} mode`)

      // Show warning toast when enabling analyzer mode (like old UI)
      if (newMode === 'analyzer') {
        setTimeout(() => {
          toast.warning('Analyzer (Sandbox) mode is for testing purposes only', {
            duration: 10000,
          })
        }, 2000)
      }
    } else {
      toast.error(result.message || 'Failed to toggle mode')
    }
  }

  const brokerId = user?.brokerId
  const filteredProfileItems = filterNavItems(profileMenuItems, brokerId)
  const filteredGroups = filterSidebarGroups(sidebarGroups, brokerId)

  return (
    <nav className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 flex-shrink-0">
      <div className="px-4 flex h-14 items-center">
        {/* Mobile Menu (hamburger triggers sidebar sheet) */}
        <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
          <SheetTrigger asChild className="md:hidden">
            <Button variant="ghost" size="icon" className="mr-2 min-h-[44px] min-w-[44px]">
              <Menu className="h-5 w-5" />
              <span className="sr-only">Toggle menu</span>
            </Button>
          </SheetTrigger>
          <SheetContent side="left" className="w-72 p-0">
            <div className="flex flex-col h-full">
              {/* Mobile sidebar header */}
              <div className="flex items-center gap-2 px-4 h-14 border-b flex-shrink-0">
                <img src="/logo.png" alt="OpenAlgo" className="h-8 w-8" />
                <span className="font-semibold">OpenAlgo</span>
              </div>

              {/* Mobile sidebar nav groups */}
              <nav className="flex-1 overflow-y-auto py-3 px-2">
                {filteredGroups.map((group, groupIdx) => (
                  <div key={group.label} className={cn(groupIdx > 0 && 'mt-4')}>
                    <div className="px-2 mb-1">
                      <span className="text-[10px] font-semibold uppercase tracking-[0.05em] text-muted-foreground">
                        {group.label}
                      </span>
                    </div>
                    <div className="flex flex-col gap-0.5">
                      {group.items.map((item) => {
                        const active = isActiveRoute(location.pathname, item.href)
                        return (
                          <Link
                            key={item.href}
                            to={item.href}
                            onClick={() => setMobileOpen(false)}
                            className={cn(
                              'flex items-center gap-3 rounded-lg px-3 h-10 text-sm font-medium transition-colors relative min-h-[44px] touch-manipulation',
                              active
                                ? 'bg-primary/10 text-primary'
                                : 'text-muted-foreground hover:bg-muted hover:text-foreground active:bg-muted'
                            )}
                          >
                            {active && (
                              <span className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-r-full bg-primary" />
                            )}
                            <item.icon className="h-4 w-4 flex-shrink-0" />
                            <span className="truncate">{item.label}</span>
                          </Link>
                        )
                      })}
                    </div>
                  </div>
                ))}
              </nav>
            </div>
          </SheetContent>
        </Sheet>

        {/* Logo */}
        <Link to="/dashboard" className="flex items-center gap-2 mr-4">
          <img src="/logo.png" alt="OpenAlgo" className="h-8 w-8" />
          <span className="hidden font-semibold sm:inline-block">OpenAlgo</span>
        </Link>

        {/* Right Side */}
        <div className="ml-auto flex items-center gap-1 sm:gap-2">
          {/* Broker Badge */}
          {user?.broker && (
            <Badge variant="outline" className="hidden sm:flex text-xs">
              {user.broker}
            </Badge>
          )}

          {/* Mode Badge */}
          <Badge
            variant={appMode === 'live' ? 'default' : 'secondary'}
            className={cn(
              'text-xs',
              appMode === 'analyzer' && 'bg-purple-500 hover:bg-purple-600 text-white'
            )}
          >
            <span className="hidden sm:inline">
              {appMode === 'live' ? 'Live Mode' : 'Analyze Mode'}
            </span>
            <span className="sm:hidden">{appMode === 'live' ? 'Live' : 'Analyze'}</span>
          </Badge>

          {/* Mode Toggle */}
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={handleModeToggle}
            disabled={isTogglingMode}
            title={`Switch to ${appMode === 'live' ? 'Analyze' : 'Live'} mode`}
            aria-label={`Switch to ${appMode === 'live' ? 'Analyze' : 'Live'} mode`}
          >
            {isTogglingMode ? (
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
            ) : appMode === 'live' ? (
              <Zap className="h-4 w-4" />
            ) : (
              <BarChart3 className="h-4 w-4" />
            )}
          </Button>

          {/* Theme Toggle */}
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={toggleMode}
            disabled={appMode !== 'live'}
            title={mode === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
            aria-label={mode === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
          >
            {mode === 'light' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>

          {/* Alert Bell */}
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 relative"
            onClick={() => navigate('/alerts')}
            title="Alert Center"
            aria-label="Alert Center"
          >
            <Bell className="h-4 w-4" />
            {unacknowledgedCount > 0 && (
              <span className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-red-500 px-1 text-[9px] font-bold text-white">
                {unacknowledgedCount > 99 ? '99+' : unacknowledgedCount}
              </span>
            )}
          </Button>

          {/* Profile Dropdown */}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 rounded-full bg-primary text-primary-foreground"
                aria-label="Open user menu"
              >
                <span className="text-sm font-medium">
                  {user?.username?.[0]?.toUpperCase() || 'O'}
                </span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              {filteredProfileItems.map((item) => (
                <DropdownMenuItem
                  key={item.href}
                  onSelect={() => navigate(item.href)}
                  className="cursor-pointer"
                >
                  <item.icon className="h-4 w-4 mr-2" />
                  {item.label}
                </DropdownMenuItem>
              ))}
              <DropdownMenuItem asChild>
                <a
                  href="https://docs.openalgo.in"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="flex items-center gap-2"
                >
                  <BookOpen className="h-4 w-4" />
                  Docs
                </a>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={handleLogout}
                className="text-destructive focus:text-destructive"
              >
                <LogOut className="h-4 w-4 mr-2" />
                Logout
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </nav>
  )
}
