import * as DialogPrimitive from '@radix-ui/react-dialog'
import { Clipboard, FolderInput, Trash2, X } from 'lucide-react'
import { useEffect, useId, useRef } from 'react'
import {
  AlertDialog,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

/**
 * The parts of CLAUDE.md's TABLE INTERACTION DESIGN SPEC that the Flow run
 * history and the Flow workflow cards share (WP8 Phase D, decisions 6a and
 * 6b): the selection action bar (§5), the right-click menu (§6) and the
 * question before a permanent delete (§5.3). The selection itself is
 * src/hooks/useListSelection.ts.
 */

/** CLAUDE.md's small button: 32px high, 12px padding and type, at least 64px wide. */
export const BUTTON_32 =
  'h-8 min-w-16 gap-1.5 rounded-md px-3 text-xs transition-colors duration-150 active:scale-[0.97] focus-visible:ring-2 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50'

/** CLAUDE.md's medium button: 40px high, 16px padding, at least 80px wide. */
export const BUTTON_40 =
  'h-10 min-w-20 px-4 transition-colors duration-150 active:scale-[0.97] focus-visible:ring-2 focus-visible:ring-offset-2'

/** "3 runs", "1 workflow". */
export function counted(count: number, noun: [string, string]): string {
  return `${count} ${count === 1 ? noun[0] : noun[1]}`
}

/**
 * A button shown disabled with the reason it is: in its tooltip (on the
 * wrapper, since a disabled button takes no pointer) and read out with it.
 */
function DisabledWithReason({
  label,
  reason,
  icon,
}: {
  label: string
  reason: string
  icon: React.ReactNode
}) {
  const id = useId()
  return (
    <span title={reason} className="inline-flex cursor-not-allowed">
      <Button variant="outline" size="sm" className={BUTTON_32} disabled aria-describedby={id}>
        {icon}
        {label}
      </Button>
      <span id={id} className="sr-only">
        {reason}
      </span>
    </span>
  )
}

/**
 * The bar shown while anything is selected: the count and Clear selection
 * on the left; Copy, Move and Delete on the right, Delete rightmost in the
 * danger colour. Move is shown disabled with its reason.
 */
export function SelectionActionBar({
  count,
  noun,
  onClear,
  onCopy,
  moveDisabledReason,
  onDelete,
  className,
}: {
  count: number
  noun: [string, string]
  onClear: () => void
  onCopy: () => void
  moveDisabledReason: string
  onDelete: () => void
  className?: string
}) {
  return (
    <div
      data-action-bar="true"
      role="toolbar"
      aria-label={`Actions for the selected ${noun[1]}`}
      className={cn(
        'flex items-center justify-between gap-3 rounded-[8px] border border-primary/30 bg-primary/5 px-3 py-2',
        className
      )}
    >
      <div className="flex min-w-0 items-center gap-2">
        <span className="truncate text-sm font-semibold tabular-nums">
          {counted(count, noun)} selected
        </span>
        <Button variant="ghost" size="sm" className={BUTTON_32} onClick={onClear}>
          <X className="size-3.5" aria-hidden="true" />
          Clear selection
        </Button>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <Button variant="outline" size="sm" className={BUTTON_32} onClick={onCopy}>
          <Clipboard className="size-3.5" aria-hidden="true" />
          Copy
        </Button>
        <DisabledWithReason
          label="Move"
          reason={moveDisabledReason}
          icon={<FolderInput className="size-3.5" aria-hidden="true" />}
        />
        <Button variant="destructive" size="sm" className={BUTTON_32} onClick={onDelete}>
          <Trash2 className="size-3.5" aria-hidden="true" />
          Delete
        </Button>
      </div>
    </div>
  )
}

/** One entry of the right-click menu. */
export interface MenuEntry {
  label: string
  onSelect?: () => void
  /** Shown disabled, with this reason under its label. */
  disabledReason?: string
  danger?: boolean
  /** A line above this entry. */
  separatorBefore?: boolean
}

/**
 * The right-click menu at the pointer. It closes on a click outside, on
 * Escape and on any scroll, and after an entry is chosen, so it is never
 * left behind. `at` is the pointer in the viewport, or, with `within`, in
 * that element (a dialog: its transform makes it the menu's frame).
 */
export function SelectionMenu({
  at,
  within,
  label,
  entries,
  onClose,
}: {
  at: { x: number; y: number }
  within?: boolean
  label: string
  entries: MenuEntry[]
  onClose: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const close = () => onClose()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('mousedown', close)
    document.addEventListener('scroll', close, true)
    document.addEventListener('keydown', onKey)
    ref.current?.querySelector<HTMLButtonElement>('button:not([disabled])')?.focus()
    return () => {
      document.removeEventListener('mousedown', close)
      document.removeEventListener('scroll', close, true)
      document.removeEventListener('keydown', onKey)
    }
  }, [onClose])
  return (
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      className={cn(
        'z-[60] min-w-52 rounded-[8px] border bg-popover py-1 text-sm text-popover-foreground shadow-lg',
        within ? 'absolute' : 'fixed'
      )}
      style={{ top: at.y, left: at.x }}
      onMouseDown={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
    >
      {entries.map((entry) => (
        <div key={entry.label}>
          {entry.separatorBefore && <hr className="my-1 border-border" />}
          <button
            type="button"
            role="menuitem"
            disabled={entry.disabledReason !== undefined}
            aria-disabled={entry.disabledReason !== undefined}
            className={cn(
              'flex w-full flex-col items-start px-3 py-1.5 text-left transition-colors duration-150 hover:bg-muted focus-visible:bg-muted focus-visible:outline-none disabled:cursor-not-allowed',
              entry.danger && 'text-destructive'
            )}
            onClick={() => {
              onClose()
              entry.onSelect?.()
            }}
          >
            <span className={cn(entry.disabledReason !== undefined && 'opacity-50')}>
              {entry.label}
            </span>
            {entry.disabledReason !== undefined && (
              <span className="text-xs text-muted-foreground">{entry.disabledReason}</span>
            )}
          </button>
        </div>
      ))}
    </div>
  )
}

/**
 * The question before a permanent delete, whatever the count: "Delete 3
 * runs?" / "This can't be undone." with Cancel and Delete.
 */
export function DeleteManyDialog({
  open,
  onOpenChange,
  count,
  noun,
  detail,
  onConfirm,
  busy,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  count: number
  noun: [string, string]
  /** A sentence after "This can't be undone." */
  detail?: string
  onConfirm: () => void
  busy?: boolean
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogPortal>
        <AlertDialogOverlay className="data-[state=closed]:duration-150 motion-reduce:animate-none" />
        <DialogPrimitive.Content
          data-slot="alert-dialog-content"
          role="alertdialog"
          className="bg-background data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] rounded-[16px] border shadow-lg duration-200 data-[state=closed]:duration-150 motion-reduce:animate-none sm:max-w-lg"
        >
          <AlertDialogHeader className="flex-row items-center justify-between gap-4 border-b px-6 py-4 sm:text-left">
            <AlertDialogTitle className="text-base leading-[1.4] font-semibold">
              Delete {counted(count, noun)}?
            </AlertDialogTitle>
            <DialogPrimitive.Close asChild>
              <Button
                variant="ghost"
                size="icon"
                className="size-8 shrink-0 transition-colors duration-150 active:scale-[0.97] focus-visible:ring-2 focus-visible:ring-offset-2"
                aria-label="Close"
              >
                <X className="h-4 w-4" aria-hidden="true" />
              </Button>
            </DialogPrimitive.Close>
          </AlertDialogHeader>
          <AlertDialogDescription className="px-6 py-4">
            This can't be undone.{detail ? ` ${detail}` : ''}
          </AlertDialogDescription>
          <AlertDialogFooter className="border-t px-6 py-4">
            <DialogPrimitive.Close asChild>
              <Button variant="outline" className={BUTTON_40}>
                Cancel
              </Button>
            </DialogPrimitive.Close>
            <Button variant="destructive" className={BUTTON_40} onClick={onConfirm} disabled={busy}>
              Delete
            </Button>
          </AlertDialogFooter>
        </DialogPrimitive.Content>
      </AlertDialogPortal>
    </AlertDialog>
  )
}
