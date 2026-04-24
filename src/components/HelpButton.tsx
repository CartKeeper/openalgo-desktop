import { LifeBuoy, X } from 'lucide-react'
import { useState } from 'react'
import { cn } from '@/lib/utils'

export interface HelpItem {
  title: string
  description: string
}

interface HelpButtonProps {
  /** Page-specific help items */
  items: HelpItem[]
  /** Optional page title shown at top of panel */
  pageTitle?: string
}

/**
 * Floating help button (bottom-right corner).
 * Opens a panel with context-specific guidance for the current page.
 */
export function HelpButton({ items, pageTitle }: HelpButtonProps) {
  const [open, setOpen] = useState(false)

  return (
    <>
      {/* Floating trigger */}
      <button
        onClick={() => setOpen(true)}
        className={cn(
          'fixed bottom-6 right-6 z-50',
          'h-12 w-12 rounded-full',
          'bg-primary text-primary-foreground',
          'shadow-lg hover:shadow-xl',
          'flex items-center justify-center',
          'transition-colors duration-150',
          'hover:bg-primary/90 active:scale-[0.97]',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2',
          open && 'hidden'
        )}
        aria-label="Help"
      >
        <LifeBuoy className="h-5 w-5" />
      </button>

      {/* Backdrop */}
      {open && (
        <div
          className="fixed inset-0 z-50 bg-black/50"
          onClick={() => setOpen(false)}
        />
      )}

      {/* Panel */}
      <div
        className={cn(
          'fixed bottom-6 right-6 z-50',
          'w-80 max-h-[70vh] rounded-xl',
          'bg-popover text-popover-foreground border shadow-xl',
          'flex flex-col',
          'transition-all duration-200 ease-out',
          open
            ? 'opacity-100 scale-100 translate-y-0'
            : 'opacity-0 scale-95 translate-y-2 pointer-events-none'
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b shrink-0">
          <div className="flex items-center gap-2">
            <LifeBuoy className="h-4 w-4 text-primary" />
            <span className="text-sm font-semibold">
              {pageTitle || 'Help'}
            </span>
          </div>
          <button
            onClick={() => setOpen(false)}
            className="h-6 w-6 rounded-md flex items-center justify-center hover:bg-muted transition-colors"
            aria-label="Close help"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3 space-y-3">
          {items.map((item, i) => (
            <div key={i} className="space-y-1">
              <p className="text-xs font-semibold">{item.title}</p>
              <p className="text-xs text-muted-foreground leading-relaxed">
                {item.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </>
  )
}
