import { describe, expect, it } from 'vitest'
import {
  bottomNavItems,
  filterNavItems,
  filterSidebarGroups,
  isActiveRoute,
  mobileSheetItems,
  navItems,
  profileMenuItems,
  sidebarGroups,
} from './navigation'

describe('Navigation Config', () => {
  describe('navItems', () => {
    it('contains the expected main navigation items', () => {
      expect(navItems).toHaveLength(8)

      const labels = navItems.map((item) => item.label)
      expect(labels).toContain('Dashboard')
      expect(labels).toContain('Orderbook')
      expect(labels).toContain('Positions')
      expect(labels).toContain('Strategy')
    })

    it('all items have required properties', () => {
      navItems.forEach((item) => {
        expect(item).toHaveProperty('href')
        expect(item).toHaveProperty('label')
        expect(item).toHaveProperty('icon')
        expect(item.href).toMatch(/^\//)
        expect(item.label.length).toBeGreaterThan(0)
      })
    })
  })

  describe('bottomNavItems', () => {
    it('contains exactly 5 items for mobile bottom nav', () => {
      expect(bottomNavItems).toHaveLength(5)
    })

    it('has the correct order: Dashboard, Orderbook, Tradebook, Positions, Strategy', () => {
      const labels = bottomNavItems.map((item) => item.label)
      expect(labels).toEqual(['Dashboard', 'Orderbook', 'Tradebook', 'Positions', 'Strategy'])
    })
  })

  describe('mobileSheetItems', () => {
    it('excludes items already in bottomNavItems', () => {
      const bottomPaths = bottomNavItems.map((item) => item.href)
      const sheetPaths = mobileSheetItems.map((item) => item.href)

      sheetPaths.forEach((path) => {
        expect(bottomPaths).not.toContain(path)
      })
    })

    it('contains remaining nav items', () => {
      const sheetLabels = mobileSheetItems.map((item) => item.label)
      expect(sheetLabels).toContain('Action Center')
      expect(sheetLabels).toContain('Platforms')
      expect(sheetLabels).toContain('Logs')
    })
  })

  describe('profileMenuItems', () => {
    it('contains profile-related menu items', () => {
      const labels = profileMenuItems.map((item) => item.label)
      expect(labels).toContain('Profile')
      expect(labels).toContain('API Key')
      expect(labels).toContain('Holdings')
    })
  })

  describe('isActiveRoute', () => {
    it('returns true for exact matches', () => {
      expect(isActiveRoute('/dashboard', '/dashboard')).toBe(true)
      expect(isActiveRoute('/orderbook', '/orderbook')).toBe(true)
      expect(isActiveRoute('/positions', '/positions')).toBe(true)
    })

    it('returns false for non-matching routes', () => {
      expect(isActiveRoute('/dashboard', '/orderbook')).toBe(false)
      expect(isActiveRoute('/positions', '/holdings')).toBe(false)
    })

    it('handles /strategy route with prefix matching', () => {
      // Strategy route should match nested pages
      expect(isActiveRoute('/strategy', '/strategy')).toBe(true)
      expect(isActiveRoute('/strategy/new', '/strategy')).toBe(true)
      expect(isActiveRoute('/strategy/123', '/strategy')).toBe(true)
      expect(isActiveRoute('/strategy/123/configure', '/strategy')).toBe(true)
    })

    it('does not prefix match non-strategy routes', () => {
      // Other routes should not prefix match
      expect(isActiveRoute('/dashboard/sub', '/dashboard')).toBe(false)
      expect(isActiveRoute('/orderbookextra', '/orderbook')).toBe(false)
    })
  })
})

// WP8 Phase B: Flow (the visual workflow builder) in the sidebar.
describe('Flow in the sidebar (WP8)', () => {
  function strategyItems(brokerId: string | null) {
    const group = filterSidebarGroups(sidebarGroups, brokerId).find((g) => g.label === 'Strategy')
    return group?.items.map((item) => [item.label, item.href]) ?? []
  }

  it('is in the Strategy group for a US broker', () => {
    expect(strategyItems('alpaca')).toContainEqual(['Flow', '/flow'])
  })

  it('is in the Strategy group in generic mode', () => {
    expect(strategyItems('generic')).toContainEqual(['Flow', '/flow'])
  })

  it('is in the Strategy group for an Indian broker too', () => {
    expect(strategyItems('zerodha')).toContainEqual(['Flow', '/flow'])
  })

  it('stays highlighted on the editor and the shortcuts page, like Strategies on its pages', () => {
    expect(isActiveRoute('/flow', '/flow')).toBe(true)
    expect(isActiveRoute('/flow/editor/7', '/flow')).toBe(true)
    expect(isActiveRoute('/flow/shortcuts', '/flow')).toBe(true)
    expect(isActiveRoute('/flows', '/flow')).toBe(false)
  })
})

// WP8 Phase C: the Telegram page sets up Flow's Telegram alerts, and a
// Telegram Alert node's error points US traders there.
describe('Telegram Bot in the profile menu (WP8)', () => {
  it('is in the profile menu for a US broker, in generic mode and for an Indian broker', () => {
    for (const brokerId of ['alpaca', 'generic', 'zerodha']) {
      const items = filterNavItems(profileMenuItems, brokerId).map((item) => [item.label, item.href])
      expect(items, brokerId).toContainEqual(['Telegram Bot', '/telegram'])
    }
  })
})
