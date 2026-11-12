/**
 * Test-only. The Flow frontend is carried from upstream OpenAlgo Desktop,
 * whose vitest.config.ts runs every test in jsdom with its src/test/setup.ts.
 * This repository has no vitest config: each jsdom test file declares
 * `// @vitest-environment jsdom` and imports `@/test/setup` itself, so the
 * carried tests do the same and then import this module, which holds what
 * upstream's setup provides and this repository's does not:
 *
 * - `ResizeObserver` and `IntersectionObserver` as real classes. A
 *   `vi.fn().mockImplementation()` cannot be called with `new`, and
 *   @floating-ui constructs a ResizeObserver directly, so a test that opened a
 *   Radix dropdown, select or popover died on "is not a constructor".
 * - Pointer capture and `scrollIntoView`, which Radix's Select, Dropdown and
 *   Popover call and jsdom does not implement.
 *
 * Nothing in the app imports this file (docs/flow/PORT.md, Phase B).
 */
import { vi } from 'vitest'

class MockResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
window.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver

class MockIntersectionObserver {
  readonly root = null
  readonly rootMargin = ''
  readonly thresholds: number[] = []
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords(): IntersectionObserverEntry[] {
    return []
  }
}
window.IntersectionObserver = MockIntersectionObserver as unknown as typeof IntersectionObserver

window.HTMLElement.prototype.hasPointerCapture = vi.fn(() => false)
window.HTMLElement.prototype.setPointerCapture = vi.fn()
window.HTMLElement.prototype.releasePointerCapture = vi.fn()
window.HTMLElement.prototype.scrollIntoView = vi.fn()
