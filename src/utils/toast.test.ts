import { describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
    dismiss: vi.fn(),
  }),
}))
vi.mock('sonner', () => ({ toast: mocks.toast }))

import { showToast, toast } from './toast'

/**
 * The Flow pages are carried from upstream, where `showToast` filters by an
 * alert category the trader can switch off. This app has no per-category
 * toast settings, so the utility shows every toast through sonner and only
 * keeps upstream's call shape (message, category, options).
 */
describe('showToast', () => {
  it('shows each level through sonner with the message and the options', () => {
    showToast.success('Workflow saved', 'flow')
    expect(mocks.toast.success).toHaveBeenCalledWith('Workflow saved', undefined)
    showToast.error('Could not save', 'flow', { duration: 10000 })
    expect(mocks.toast.error).toHaveBeenCalledWith('Could not save', { duration: 10000 })
    showToast.warning('Trigger changed', 'flow', { description: 'Reactivate the workflow' })
    expect(mocks.toast.warning).toHaveBeenCalledWith('Trigger changed', {
      description: 'Reactivate the workflow',
    })
    showToast.info('Workflow started')
    expect(mocks.toast.info).toHaveBeenCalledWith('Workflow started', undefined)
  })

  it('shows a toast whatever category it is given', () => {
    mocks.toast.success.mockClear()
    showToast.success('Copied', 'clipboard')
    showToast.success('Saved', 'orders')
    showToast.success('Plain')
    expect(mocks.toast.success).toHaveBeenCalledTimes(3)
  })

  it('routes a dynamic type, dismisses all, and re-exports sonner', () => {
    showToast.dynamic('error', 'Execution failed', 'flow')
    expect(mocks.toast.error).toHaveBeenCalledWith('Execution failed', undefined)
    showToast.show('info', 'hello')
    expect(mocks.toast.info).toHaveBeenCalledWith('hello', undefined)
    showToast.dismissAll()
    expect(mocks.toast.dismiss).toHaveBeenCalledTimes(1)
    expect(toast).toBe(mocks.toast)
  })
})
