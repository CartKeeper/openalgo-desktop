/**
 * Toast utility for the pages carried from upstream OpenAlgo Desktop (Flow).
 *
 * Upstream's `showToast` filters each toast by an alert category the trader
 * can switch off in its alert settings. This app has no per-category toast
 * settings, so every toast shows through sonner; the category argument is
 * accepted so the carried pages keep upstream's call shape
 * (`showToast.success('Workflow saved', 'flow')`) and stay diffable against
 * upstream (docs/flow/PORT.md, Phase B).
 */

import { toast } from 'sonner'

type ToastType = 'success' | 'error' | 'warning' | 'info'

interface ToastOptions {
  duration?: number
  description?: string
}

/** Show a toast. `_category` is upstream's filter key, unused here. */
const show = (type: ToastType, message: string, _category?: string, options?: ToastOptions) => {
  toast[type](message, options)
}

const success = (message: string, category?: string, options?: ToastOptions) => {
  show('success', message, category, options)
}

const error = (message: string, category?: string, options?: ToastOptions) => {
  show('error', message, category, options)
}

const warning = (message: string, category?: string, options?: ToastOptions) => {
  show('warning', message, category, options)
}

const info = (message: string, category?: string, options?: ToastOptions) => {
  show('info', message, category, options)
}

/** Dismiss every toast. */
const dismissAll = () => {
  toast.dismiss()
}

/** Show a toast whose type is decided at run time. */
const dynamic = (type: ToastType, message: string, category?: string, options?: ToastOptions) => {
  show(type, message, category, options)
}

export const showToast = {
  success,
  error,
  warning,
  info,
  dismissAll,
  dynamic,
  show,
}

// The raw toast, for a caller that wants sonner directly.
export { toast }
