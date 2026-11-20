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
 * activating a workflow while the app is in Live mode asks once first, and
 * so does Run Now (WP8 Phase D, decision 6c), which places the workflow's
 * orders at once. In Analyze mode both stay one click. The mode is the
 * app's own (the setting that routes the orders), read when the button is
 * chosen, since an API client can switch it behind the header; a mode that
 * cannot be read is treated as Live.
 */

/** What the Live-mode question says for one action. */
interface LiveQuestion {
  title: string
  description: string
  confirm: string
}

const ACTIVATION: LiveQuestion = {
  title: 'Activate in Live mode?',
  description: 'This workflow will place real orders on the live account without asking each time.',
  confirm: 'Activate',
}

const RUN: LiveQuestion = {
  title: 'Run in Live mode?',
  description: 'This run will place real orders on the live account without asking for each order.',
  confirm: 'Run',
}

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
export function LiveActivationDialog(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
}) {
  return <LiveModeDialog {...props} question={ACTIVATION} />
}

/** The Live-mode question for one action (activation, or a run). */
function LiveModeDialog({
  open,
  onOpenChange,
  onConfirm,
  question,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
  question: LiveQuestion
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
              {question.title}
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
            {question.description}
          </AlertDialogDescription>
          <AlertDialogFooter className="border-t px-6 py-4">
            <DialogPrimitive.Close asChild>
              <Button variant="outline" className={BUTTON_40}>
                Cancel
              </Button>
            </DialogPrimitive.Close>
            <Button className={BUTTON_40} onClick={onConfirm}>
              {question.confirm}
            </Button>
          </AlertDialogFooter>
        </DialogPrimitive.Content>
      </AlertDialogPortal>
    </AlertDialog>
  )
}

/**
 * Guard `action` with the Live-mode `question`: `request` reads the app's
 * mode and either asks (`dialog`, to render) or acts at once; `checking`
 * while the mode is read.
 */
function useLiveQuestion(action: () => void, question: LiveQuestion) {
  const [asking, setAsking] = useState(false)
  const [checking, setChecking] = useState(false)

  const request = useCallback(async () => {
    setChecking(true)
    const live = await tradingLive()
    setChecking(false)
    if (live) {
      setAsking(true)
    } else {
      action()
    }
  }, [action])

  const dialog = (
    <LiveModeDialog
      open={asking}
      onOpenChange={setAsking}
      question={question}
      onConfirm={() => {
        setAsking(false)
        action()
      }}
    />
  )

  return { request, checking, dialog }
}

/**
 * Guard `activate` with the Live-mode question: `request` reads the app's
 * mode and either asks (`dialog`, to render) or activates at once;
 * `checking` while the mode is read.
 */
export function useLiveActivation(activate: () => void) {
  return useLiveQuestion(activate, ACTIVATION)
}

/**
 * Guard Run Now with the Live-mode question ("Run in Live mode?"): the
 * same as {@link useLiveActivation}, for a run.
 */
export function useLiveRun(run: () => void) {
  return useLiveQuestion(run, RUN)
}
