import {
  BarChart3,
  Bell,
  BookOpen,
  Bot,
  BriefcaseBusiness,
  Calendar,
  ClipboardList,
  Clock,
  Code2,
  Database,
  FileBarChart,
  FileText,
  Filter,
  FlaskConical,
  Globe,
  Key,
  Landmark,
  Layers,
  LayoutDashboard,
  LineChart,
  type LucideIcon,
  MessageSquare,
  Newspaper,
  Search,
  Settings,
  Sigma,
  TrendingUp,
  User,
  Users,
} from 'lucide-react'

export interface NavItem {
  href: string
  label: string
  icon: LucideIcon
  /** If set, only show for these broker regions. Omit to show for all. */
  region?: ('india' | 'us' | 'generic')[]
}

export interface SidebarGroup {
  label: string
  items: NavItem[]
}

// US broker IDs
const US_BROKER_IDS = new Set(['alpaca', 'tradier', 'schwab', 'ibkr'])

/** Get the region for a given brokerId */
export function getBrokerRegion(brokerId: string | null | undefined): 'india' | 'us' | 'generic' {
  if (!brokerId) return 'india' // default
  if (brokerId === 'generic') return 'generic'
  if (US_BROKER_IDS.has(brokerId)) return 'us'
  return 'india'
}

/** Filter nav items by broker region */
export function filterNavItems(items: NavItem[], brokerId: string | null | undefined): NavItem[] {
  const region = getBrokerRegion(brokerId)
  return items.filter((item) => !item.region || item.region.includes(region))
}

/** Filter sidebar groups by broker region, removing empty groups */
export function filterSidebarGroups(
  groups: SidebarGroup[],
  brokerId: string | null | undefined
): SidebarGroup[] {
  const region = getBrokerRegion(brokerId)
  return groups
    .map((group) => ({
      ...group,
      items: group.items.filter((item) => !item.region || item.region.includes(region)),
    }))
    .filter((group) => group.items.length > 0)
}

// Sidebar navigation groups — consolidates all nav items into logical sections
export const sidebarGroups: SidebarGroup[] = [
  {
    label: 'Core',
    items: [
      { href: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
      { href: '/briefing', label: 'Briefing', icon: Clock, region: ['us', 'generic'] },
    ],
  },
  {
    label: 'Trading',
    items: [
      { href: '/orderbook', label: 'Orderbook', icon: ClipboardList, region: ['india', 'us'] },
      { href: '/tradebook', label: 'Tradebook', icon: FileText, region: ['india', 'us'] },
      { href: '/positions', label: 'Positions', icon: TrendingUp, region: ['india', 'us'] },
      { href: '/holdings', label: 'Holdings', icon: ClipboardList, region: ['india', 'us'] },
      { href: '/action-center', label: 'Action Center', icon: Bell, region: ['india'] },
    ],
  },
  {
    label: 'Research',
    items: [
      { href: '/copilot', label: 'AI Research', icon: Bot },
      { href: '/fundamentals', label: 'Fundamentals', icon: BriefcaseBusiness },
      { href: '/analyst', label: 'Analyst', icon: LineChart },
      { href: '/screener', label: 'Screener', icon: Filter },
      { href: '/news', label: 'News', icon: Newspaper },
      { href: '/reports', label: 'Reports', icon: BookOpen },
    ],
  },
  {
    label: 'Market Data',
    items: [
      { href: '/congress', label: 'Congress', icon: Landmark, region: ['us', 'generic'] },
      { href: '/calendar', label: 'Calendar', icon: Calendar },
      { href: '/options', label: 'Options', icon: Sigma },
      { href: '/quant', label: 'Quant', icon: BarChart3 },
      { href: '/historify', label: 'Historify', icon: Database },
    ],
  },
  {
    label: 'Strategy',
    items: [
      { href: '/strategy', label: 'Strategies', icon: Code2, region: ['india'] },
      { href: '/python', label: 'Python', icon: Code2, region: ['india'] },
      { href: '/platforms', label: 'Platforms', icon: Layers, region: ['india'] },
    ],
  },
  {
    label: 'System',
    items: [
      { href: '/logs', label: 'Logs', icon: FileBarChart },
      { href: '/portfolio', label: 'Portfolio', icon: TrendingUp, region: ['generic'] },
      { href: '/pnl-tracker', label: 'PnL Tracker', icon: BarChart3, region: ['india'] },
      { href: '/sandbox', label: 'Sandbox', icon: FlaskConical, region: ['india'] },
      { href: '/clients', label: 'Clients', icon: Users },
      { href: '/search/token', label: 'Search', icon: Search },
      { href: '/admin', label: 'Admin', icon: Settings },
    ],
  },
]

// Flat list of all sidebar nav items (for backwards compat with mobile nav)
export const navItems: NavItem[] = sidebarGroups.flatMap((g) => g.items)

// Items shown in mobile bottom navigation
export const bottomNavItems: NavItem[] = [
  { href: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { href: '/orderbook', label: 'Orderbook', icon: ClipboardList },
  { href: '/tradebook', label: 'Tradebook', icon: FileText },
  { href: '/positions', label: 'Positions', icon: TrendingUp },
  { href: '/copilot', label: 'Research', icon: Bot },
]

// Paths in bottom nav (for filtering mobile sheet items)
const bottomNavPaths = bottomNavItems.map((item) => item.href)

// Secondary items for mobile sheet (items not in bottom nav)
export const mobileSheetItems = navItems.filter((item) => !bottomNavPaths.includes(item.href))

// Profile dropdown menu items (user settings only — main nav moved to sidebar)
export const profileMenuItems: NavItem[] = [
  { href: '/profile', label: 'Profile', icon: User },
  { href: '/apikey', label: 'API Key', icon: Key },
  { href: '/providers', label: 'Data Providers', icon: Globe },
  { href: '/telegram', label: 'Telegram Bot', icon: MessageSquare, region: ['india'] },
]

// External links
export const externalLinks = {
  docs: { href: 'https://docs.openalgo.in', label: 'Docs', icon: BookOpen },
}

// Shared utility to check if a route is active
// Uses startsWith for routes with nested pages (like /strategy/*)
export function isActiveRoute(pathname: string, href: string): boolean {
  if (href === '/strategy') {
    return pathname.startsWith('/strategy')
  }
  if (href === '/python') {
    return pathname.startsWith('/python')
  }
  if (href === '/logs') {
    return pathname.startsWith('/logs')
  }
  if (href === '/admin') {
    return pathname.startsWith('/admin')
  }
  if (href === '/telegram') {
    return pathname.startsWith('/telegram')
  }
  if (href === '/clients') {
    return pathname.startsWith('/clients')
  }
  return pathname === href
}
