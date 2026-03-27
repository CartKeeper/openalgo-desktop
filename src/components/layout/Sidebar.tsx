import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { filterSidebarGroups, isActiveRoute, type SidebarGroup, sidebarGroups } from '@/config/navigation'
import { cn } from '@/lib/utils'
import { useAuthStore } from '@/stores/authStore'

const SIDEBAR_STORAGE_KEY = 'openalgo-sidebar-collapsed'
const SIDEBAR_EXPANDED_WIDTH = 240
const SIDEBAR_COLLAPSED_WIDTH = 60

function getInitialCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_STORAGE_KEY) === 'true'
  } catch {
    return false
  }
}

interface SidebarProps {
  className?: string
}

export function Sidebar({ className }: SidebarProps) {
  const location = useLocation()
  const { user } = useAuthStore()
  const [collapsed, setCollapsed] = useState(getInitialCollapsed)

  const filteredGroups = filterSidebarGroups(sidebarGroups, user?.brokerId)

  useEffect(() => {
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, String(collapsed))
    } catch {
      // ignore
    }
  }, [collapsed])

  return (
    <aside
      className={cn(
        'hidden md:flex flex-col border-r bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 transition-[width] duration-200 ease-out flex-shrink-0 relative',
        className
      )}
      style={{ width: collapsed ? SIDEBAR_COLLAPSED_WIDTH : SIDEBAR_EXPANDED_WIDTH }}
    >
      {/* Scrollable nav area */}
      <ScrollArea className="flex-1 min-h-0 overflow-hidden">
        <nav className="py-3 px-3">
          {filteredGroups.map((group, groupIdx) => (
            <SidebarGroupSection
              key={group.label}
              group={group}
              collapsed={collapsed}
              pathname={location.pathname}
              isFirst={groupIdx === 0}
            />
          ))}
        </nav>
      </ScrollArea>

      {/* Collapse toggle */}
      <div className="border-t p-3 flex-shrink-0">
        <Button
          variant="ghost"
          size="icon"
          className={cn('h-8 w-8', !collapsed && 'ml-auto')}
          onClick={() => setCollapsed((prev) => !prev)}
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
        </Button>
      </div>
    </aside>
  )
}

function SidebarGroupSection({
  group,
  collapsed,
  pathname,
  isFirst,
}: {
  group: SidebarGroup
  collapsed: boolean
  pathname: string
  isFirst: boolean
}) {
  return (
    <div className={cn(!isFirst && 'mt-4')}>
      {/* Group label */}
      {!collapsed && (
        <div className="px-2 mb-1">
          <span className="text-[10px] font-semibold uppercase tracking-[0.05em] text-muted-foreground">
            {group.label}
          </span>
        </div>
      )}
      {collapsed && !isFirst && <div className="mx-2 mb-2 border-t" />}

      {/* Items */}
      <div className="flex flex-col gap-0.5">
        {group.items.map((item) => {
          const active = isActiveRoute(pathname, item.href)

          const linkContent = (
            <Link
              to={item.href}
              className={cn(
                'flex items-center gap-3 rounded-lg text-sm font-medium transition-colors relative',
                collapsed ? 'justify-center h-10 w-10 mx-auto' : 'px-3 h-10',
                active
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground hover:bg-muted hover:text-foreground'
              )}
            >
              {/* Active indicator */}
              {active && (
                <span className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-r-full bg-primary" />
              )}
              <item.icon className="h-4 w-4 flex-shrink-0" />
              {!collapsed && (
                <span className="truncate">{item.label}</span>
              )}
            </Link>
          )

          if (collapsed) {
            return (
              <Tooltip key={item.href}>
                <TooltipTrigger asChild>{linkContent}</TooltipTrigger>
                <TooltipContent side="right" sideOffset={8}>
                  {item.label}
                </TooltipContent>
              </Tooltip>
            )
          }

          return <div key={item.href}>{linkContent}</div>
        })}
      </div>
    </div>
  )
}

export { SIDEBAR_EXPANDED_WIDTH, SIDEBAR_COLLAPSED_WIDTH }
