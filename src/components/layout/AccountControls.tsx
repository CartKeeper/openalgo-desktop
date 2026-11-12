import { BarChart3, BookOpen, LogOut, Moon, Sun, Zap } from 'lucide-react'
// 'react-router', as the carried Flow tree imports it: under vitest a
// 'react-router-dom' re-export and a direct 'react-router' import load as two
// module instances, and a hook from one cannot see a Router from the other.
// In the Vite bundle both names resolve to one module.
import { useNavigate } from 'react-router'
import { AccountSwitcher } from '@/components/AccountSwitcher'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { filterNavItems, profileMenuItems } from '@/config/navigation'
import { cn } from '@/lib/utils'
import { useAuthStore } from '@/stores/authStore'
import { useThemeStore } from '@/stores/themeStore'
import { showToast } from '@/utils/toast'

/**
 * The account end of the app's header, for a page that draws its own header
 * instead of the Navbar: the Flow editor, carried from upstream OpenAlgo
 * Desktop, whose FlowEditorAccountControls imports this path.
 *
 * The pieces are the Navbar's own (src/components/layout/Navbar.tsx): the
 * broker badge, the mode badge, the Live/Analyze switch with its sandbox
 * warning, the theme switch and the profile menu, preceded by the
 * AccountSwitcher, so the editor shows at a glance whether orders go to the
 * sandbox and which account is selected. The markup and handlers are the
 * Navbar's, copied: WP8 Phase B could not edit Navbar.tsx to share one
 * component, so a change to the Navbar's account controls is made here too
 * (docs/flow/PORT.md).
 *
 * `onNavigate`: where a profile menu entry goes through, for a page that must
 * ask before leaving (the Flow editor with unsaved changes).
 */
export function AccountControls({ onNavigate }: { onNavigate?: (href: string) => void }) {
  const navigate = useNavigate()
  const { mode, appMode, toggleMode, toggleAppMode, isTogglingMode } = useThemeStore()
  const { user, logout } = useAuthStore()
  const go = onNavigate ?? navigate

  // One logout request: the store's logout() sends it and clears the session
  // whether or not it succeeds. (The Navbar sends it twice, once itself and
  // once through the store; not changed here.)
  const handleLogout = async () => {
    await logout()
    navigate('/login')
    showToast.success('Logged out successfully')
  }

  const handleModeToggle = async () => {
    const result = await toggleAppMode()
    if (result.success) {
      const newMode = useThemeStore.getState().appMode
      showToast.success(`Switched to ${newMode === 'live' ? 'Live' : 'Analyze'} mode`)

      // Show warning toast when enabling analyzer mode (like old UI)
      if (newMode === 'analyzer') {
        setTimeout(() => {
          showToast.warning('Analyzer (Sandbox) mode is for testing purposes only', undefined, {
            duration: 10000,
          })
        }, 2000)
      }
    } else {
      showToast.error(result.message || 'Failed to toggle mode')
    }
  }

  const filteredProfileItems = filterNavItems(profileMenuItems, user?.brokerId)

  return (
    <>
      <AccountSwitcher />

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
              onSelect={() => go(item.href)}
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
    </>
  )
}
