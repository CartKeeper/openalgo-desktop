import * as DialogPrimitive from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { useCallback, useState } from 'react'
import { settingsCommands } from '@/api/tauri-client'
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

/**
 * Desktop departure from the web's Flow pages (WP8 Phase C, the
 * coordinator's live-money decision). A Flow order goes through the app's
 * API order path, which skips the per-order Live/Paper confirmation, so
 * activating a workflow while the app is in Live mode asks once first. In
 * Analyze mode activation stays one click. The mode is the app's own (the
 * setting that routes the orders), read when Activate is chosen, since an
 * API client can switch it behind the header; a mode that cannot be read is
 * treated as Live.
 */

/** CLAUDE.md's medium button: 40px high, 16px padding, at least 80px wide. */
const BUTTON_40 =
  'h-10 min-w-20 px-4 transition-colors duration-150 active:scale-[0.97] focus-visible:ring-2 focus-visible:ring-offset-2'

/** Whether the app trades live now (unreadable counts as live). */
async function tradingLive(): Promise<boolean> {
  try {
    return !(await settingsCommands.getAnalyzeMode()).analyze_mode
  } catch {
    return true
  }
}

/** The question asked before a workflow is activated in Live mode. */
export function LiveActivationDialog({
  open,
  onOpenChange,
  onConfirm,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      {/* The shared AlertDialogContent's overlay fades even under reduced
          motion, so the dialog is built from its parts here. */}
      <AlertDialogPortal>
        <AlertDialogOverlay className="data-[state=closed]:duration-150 motion-reduce:animate-none" />
        <DialogPrimitive.Content
          data-slot="alert-dialog-content"
          role="alertdialog"
          className="bg-background data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] rounded-[16px] border shadow-lg duration-200 data-[state=closed]:duration-150 motion-reduce:animate-none sm:max-w-lg"
        >
          <AlertDialogHeader className="flex-row items-center justify-between gap-4 border-b px-6 py-4 sm:text-left">
            <AlertDialogTitle className="text-base leading-[1.4] font-semibold">
              Activate in Live mode?
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
            This workflow will place real orders on the live account without asking each time.
          </AlertDialogDescription>
          <AlertDialogFooter className="border-t px-6 py-4">
            <DialogPrimitive.Close asChild>
              <Button variant="outline" className={BUTTON_40}>
                Cancel
              </Button>
            </DialogPrimitive.Close>
            <Button className={BUTTON_40} onClick={onConfirm}>
              Activate
            </Button>
          </AlertDialogFooter>
        </DialogPrimitive.Content>
      </AlertDialogPortal>
    </AlertDialog>
  )
}

/**
 * Guard `activate` with the Live-mode question: `request` reads the app's
 * mode and either asks (`dialog`, to render) or activates at once;
 * `checking` while the mode is read.
 */
export function useLiveActivation(activate: () => void) {
  const [asking, setAsking] = useState(false)
  const [checking, setChecking] = useState(false)

  const request = useCallback(async () => {
    setChecking(true)
    const live = await tradingLive()
    setChecking(false)
    if (live) {
      setAsking(true)
    } else {
      activate()
    }
  }, [activate])

  const dialog = (
    <LiveActivationDialog
      open={asking}
      onOpenChange={setAsking}
      onConfirm={() => {
        setAsking(false)
        activate()
      }}
    />
  )

  return { request, checking, dialog }
}
