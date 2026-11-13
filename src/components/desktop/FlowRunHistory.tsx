import * as DialogPrimitive from '@radix-ui/react-dialog'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  Clock,
  History,
  Loader2,
  Play,
  ScrollText,
  X,
  XCircle,
} from 'lucide-react'
import { type MouseEvent, useCallback, useMemo, useRef, useState } from 'react'
import {
  deleteWorkflowExecutions,
  type ExecutionLog,
  flowQueryKeys,
  getWorkflowExecutions,
  type WorkflowExecution,
} from '@/api/flow'
import {
  BUTTON_40,
  counted,
  DeleteManyDialog,
  type MenuEntry,
  SelectionActionBar,
  SelectionMenu,
} from '@/components/desktop/FlowSelectionParts'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useListSelection } from '@/hooks/useListSelection'
import { logTime, runDuration, runTime } from '@/lib/flow/listDate'
import { cn } from '@/lib/utils'
import { showToast } from '@/utils/toast'

/**
 * A workflow's run history (WP8 Phase D, coordinator decision 6a): every
 * run the store keeps (the newest 200 are read; the store keeps the newest
 * 500 and 30 days), when it started in Eastern time, its status, how long
 * it took and its error; opening one (double-click, Enter or its Open
 * button) shows its log lines. The list is read again every 10 seconds and
 * when the window gets focus while it is open (the backend sends no event
 * when a run ends). It carries CLAUDE.md's selection system: Copy puts the
 * chosen runs on the clipboard as text, Move is shown disabled, Delete asks
 * first and deletes them in one call.
 *
 * A dialog of fixed size (75% of the window high, up to 896px wide) over
 * the Flow page or the editor, from the workflow's ... menu.
 */

const NOUN: [string, string] = ['run', 'runs']
const MOVE_REASON = 'Runs stay with their workflow'
const DUPLICATE_REASON = "Runs can't be duplicated"
/** The most runs one history read returns (`executions::QUERY_MAX`). */
const READ = 200

const STATUS: Record<string, { word: string; className: string; icon: React.ReactNode }> = {
  completed: {
    word: 'Completed',
    className: 'bg-buy/10 text-buy',
    icon: <CheckCircle2 className="size-3.5" aria-hidden="true" />,
  },
  failed: {
    word: 'Failed',
    className: 'bg-destructive/10 text-destructive',
    icon: <XCircle className="size-3.5" aria-hidden="true" />,
  },
  running: {
    word: 'Running',
    className: 'bg-primary/10 text-primary',
    icon: (
      <Loader2 className="size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
    ),
  },
  pending: {
    word: 'Pending',
    className: 'bg-muted text-muted-foreground',
    icon: <Clock className="size-3.5" aria-hidden="true" />,
  },
}

function statusWord(status: string): string {
  return STATUS[status]?.word ?? (status ? status[0].toUpperCase() + status.slice(1) : 'Unknown')
}

function StatusBadge({ status }: { status: string }) {
  const s = STATUS[status]
  return (
    <span
      className={cn(
        'inline-flex h-6 items-center gap-1 rounded-[6px] px-2 text-xs font-semibold',
        s?.className ?? 'bg-muted text-muted-foreground'
      )}
    >
      {s?.icon ?? <AlertCircle className="size-3.5" aria-hidden="true" />}
      {statusWord(status)}
    </span>
  )
}

/** One run as the clipboard gets it. */
function runText(run: WorkflowExecution): string {
  const lines = [
    `${runTime(run.started_at)} · ${statusWord(run.status)} · ${runDuration(run.started_at, run.completed_at)}`,
  ]
  if (run.error) lines.push(`Error: ${run.error}`)
  for (const log of run.logs ?? []) {
    lines.push(`${logTime(log.time)}  ${log.level}  ${log.message}`)
  }
  return lines.join('\n')
}

export function FlowRunHistory({
  workflowId,
  workflowName,
  open,
  onOpenChange,
  onRunNow,
}: {
  workflowId: number
  workflowName: string
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The empty state's Run Now (the host's, which asks first in Live mode). */
  onRunNow: () => void
}) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/50 duration-200 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0 data-[state=closed]:duration-150 motion-reduce:animate-none" />
        <RunHistoryWindow workflowId={workflowId} workflowName={workflowName} onRunNow={onRunNow} />
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}

/** The window's content: mounted only while open, so its state starts afresh each time. */
function RunHistoryWindow({
  workflowId,
  workflowName,
  onRunNow,
}: {
  workflowId: number
  workflowName: string
  onRunNow: () => void
}) {
  const queryClient = useQueryClient()
  const windowRef = useRef<HTMLDivElement>(null)
  const [openRunId, setOpenRunId] = useState<number | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const [asking, setAsking] = useState(false)

  const runs = useQuery({
    queryKey: flowQueryKeys.executions(workflowId),
    queryFn: () => getWorkflowExecutions(workflowId, READ),
    refetchInterval: 10_000,
    refetchOnWindowFocus: true,
  })
  const list = runs.data ?? []
  const ids = useMemo(() => list.map((r) => String(r.id)), [list])
  const openRun = useCallback((id: string) => setOpenRunId(Number(id)), [])
  const selection = useListSelection(ids, { noun: NOUN, onOpen: openRun })
  const chosen = list.filter((r) => selection.isSelected(String(r.id)))

  const remove = useMutation({
    mutationFn: (runIds: number[]) => deleteWorkflowExecutions(workflowId, runIds),
    onSuccess: (_reply, runIds) => {
      setAsking(false)
      selection.clear()
      showToast.success(`${counted(runIds.length, NOUN)} deleted`, 'flow')
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.executions(workflowId) })
      // The cards' last run (the list only: its key prefixes every workflow's).
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows(), exact: true })
    },
    onError: (error: Error) => {
      setAsking(false)
      showToast.error(error.message, 'flow')
    },
  })

  const copy = async () => {
    const text = chosen.map(runText).join('\n\n')
    try {
      await navigator.clipboard.writeText(text)
      showToast.success(`${counted(chosen.length, NOUN)} copied`, 'clipboard')
    } catch {
      showToast.error('The runs could not be copied to the clipboard', 'clipboard')
    }
  }

  const shown = openRunId === null ? undefined : list.find((r) => r.id === openRunId)
  const viewingRun = shown !== undefined

  const onRowClick = (id: string, e: MouseEvent) => {
    // The second click of a double-click is the open, not a toggle.
    if (e.detail > 1) return
    selection.select(id, e)
  }

  const onRowMenu = (id: string, e: MouseEvent) => {
    e.preventDefault()
    selection.contextSelect(id)
    const frame = windowRef.current?.getBoundingClientRect()
    setMenu({ x: e.clientX - (frame?.left ?? 0), y: e.clientY - (frame?.top ?? 0) })
  }

  const single = selection.count === 1 ? selection.selectedIds[0] : null
  const menuEntries: MenuEntry[] = [
    { label: 'Copy', onSelect: copy },
    { label: 'Move to…', disabledReason: MOVE_REASON },
    { label: 'Delete', onSelect: () => setAsking(true), danger: true },
    { label: 'Select All', onSelect: selection.selectAll, separatorBefore: true },
    { label: 'Deselect All', onSelect: selection.clear },
    single === null
      ? { label: 'Open', disabledReason: 'Choose one run to open', separatorBefore: true }
      : { label: 'Open', onSelect: () => openRun(single), separatorBefore: true },
    { label: 'Duplicate', disabledReason: DUPLICATE_REASON },
  ]

  return (
    <DialogPrimitive.Content
      ref={windowRef}
      aria-describedby={undefined}
      onEscapeKeyDown={(e) => {
        // Escape closes the right-click menu, then clears a selection
        // (the table spec), and only then closes the window.
        if (menu) {
          e.preventDefault()
          setMenu(null)
        } else if (!viewingRun && selection.count > 0) {
          e.preventDefault()
          selection.clear()
        }
      }}
      className="fixed top-1/2 left-1/2 z-50 flex h-[75vh] w-full max-w-[calc(100%-2rem)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[16px] border bg-background shadow-lg duration-200 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0 data-[state=open]:zoom-in-95 data-[state=closed]:zoom-out-95 data-[state=closed]:duration-150 motion-reduce:animate-none sm:max-w-4xl"
    >
      <div className="flex h-16 shrink-0 items-center justify-between gap-4 border-b px-6">
        <div className="min-w-0">
          <DialogPrimitive.Title className="text-base leading-[1.4] font-semibold">
            Run history
          </DialogPrimitive.Title>
          <p className="truncate text-sm text-muted-foreground">{workflowName}</p>
        </div>
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
      </div>

      <span aria-live="polite" aria-atomic="true" className="sr-only">
        {selection.announcement}
      </span>

      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        {viewingRun ? (
          <RunDetail run={shown} onBack={() => setOpenRunId(null)} />
        ) : runs.isPending ? (
          <RunsSkeleton />
        ) : runs.isError ? (
          <div className="flex items-start gap-3 rounded-[12px] border border-destructive p-4">
            <AlertTriangle className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden="true" />
            <div className="min-w-0 flex-1 space-y-3">
              <div>
                <p className="text-sm font-semibold">Could not load the run history</p>
                <p className="text-sm text-muted-foreground">
                  {runs.error instanceof Error
                    ? runs.error.message
                    : 'Something went wrong while reading the runs.'}
                </p>
              </div>
              <Button variant="outline" className={BUTTON_40} onClick={() => runs.refetch()}>
                Retry
              </Button>
            </div>
          </div>
        ) : list.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <History className="size-12 text-muted-foreground" aria-hidden="true" />
            <p className="text-base font-semibold">No runs yet</p>
            <p className="max-w-sm text-sm text-muted-foreground">
              A run appears here when the workflow runs: Run Now, its schedule or alert, or its
              webhook.
            </p>
            <Button className={BUTTON_40} onClick={onRunNow}>
              <Play className="size-4" aria-hidden="true" />
              Run Now
            </Button>
          </div>
        ) : (
          <div className="space-y-3">
            {selection.count > 0 && (
              <SelectionActionBar
                className="sticky top-0 z-10"
                count={selection.count}
                noun={NOUN}
                onClear={selection.clear}
                onCopy={copy}
                moveDisabledReason={MOVE_REASON}
                onDelete={() => setAsking(true)}
              />
            )}
            <div className="overflow-hidden rounded-[12px] border">
              {/* biome-ignore lint/a11y/useSemanticElements: an interactive data grid (CLAUDE.md's table spec), the WAI-ARIA grid pattern on a table element */}
              <table
                // biome-ignore lint/a11y/noNoninteractiveElementToInteractiveRole: selectable rows with aria-selected and keys on the table (the WAI-ARIA grid pattern)
                role="grid"
                tabIndex={0}
                aria-label={`Runs of ${workflowName}`}
                aria-multiselectable="true"
                aria-rowcount={list.length}
                aria-activedescendant={
                  selection.focusedId === null ? undefined : `flow-run-${selection.focusedId}`
                }
                onKeyDown={selection.onKeyDown}
                className="w-full table-fixed border-collapse text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
              >
                <colgroup>
                  <col className="w-10" />
                  <col className="w-[200px]" />
                  <col className="w-32" />
                  <col className="w-24" />
                  <col />
                  <col className="w-14" />
                </colgroup>
                <thead>
                  <tr className="h-10 border-b bg-muted/40 text-left text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                    <th className="text-center">
                      <input
                        type="checkbox"
                        data-select-all="true"
                        aria-label="Select all runs"
                        className="size-4 cursor-pointer align-middle accent-primary"
                        checked={selection.allSelected}
                        ref={(el) => {
                          if (el) el.indeterminate = selection.someSelected
                        }}
                        onChange={selection.toggleAll}
                      />
                    </th>
                    <th className="px-4 font-semibold">Started</th>
                    <th className="px-4 font-semibold">Status</th>
                    <th className="px-4 text-right font-semibold">Duration</th>
                    <th className="px-4 font-semibold">Error</th>
                    <th>
                      <span className="sr-only">Open</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {list.map((run, index) => {
                    const id = String(run.id)
                    const selected = selection.isSelected(id)
                    const started = runTime(run.started_at)
                    return (
                      <tr
                        key={id}
                        id={`flow-run-${id}`}
                        aria-rowindex={index + 1}
                        aria-selected={selected}
                        data-selectable="true"
                        data-selected={selected}
                        data-item-id={id}
                        onClick={(e) => onRowClick(id, e)}
                        onDoubleClick={() => openRun(id)}
                        onContextMenu={(e) => onRowMenu(id, e)}
                        className={cn(
                          'h-12 cursor-pointer border-b transition-colors duration-150 last:border-b-0',
                          selected
                            ? 'bg-primary/8 shadow-[inset_3px_0_0_var(--color-primary)]'
                            : 'hover:bg-primary/4',
                          selection.focusedId === id && 'outline-2 -outline-offset-2 outline-ring'
                        )}
                      >
                        <td className="text-center">
                          <input
                            type="checkbox"
                            aria-label={`Select the run of ${started}`}
                            className="size-4 cursor-pointer align-middle accent-primary"
                            checked={selected}
                            onChange={() => {}}
                            onClick={(e) => {
                              e.stopPropagation()
                              selection.check(id, e)
                            }}
                          />
                        </td>
                        <td className="truncate px-4 font-mono font-semibold tabular-nums">
                          {started}
                        </td>
                        <td className="px-4">
                          <StatusBadge status={run.status} />
                        </td>
                        <td className="px-4 text-right font-mono text-muted-foreground tabular-nums">
                          {runDuration(run.started_at, run.completed_at)}
                        </td>
                        <td
                          className="truncate px-4 text-muted-foreground"
                          title={run.error ?? undefined}
                        >
                          {run.error ?? ''}
                        </td>
                        <td className="pr-3 text-right">
                          <Button
                            variant="ghost"
                            size="icon"
                            className="size-8 transition-colors duration-150 active:scale-[0.97] focus-visible:ring-2 focus-visible:ring-offset-2"
                            aria-label={`Open the run of ${started}`}
                            onClick={(e) => {
                              e.stopPropagation()
                              openRun(id)
                            }}
                            onDoubleClick={(e) => e.stopPropagation()}
                          >
                            <ScrollText className="size-4" aria-hidden="true" />
                          </Button>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>

      {menu && (
        <SelectionMenu
          at={menu}
          within
          label="Run actions"
          entries={menuEntries}
          onClose={() => setMenu(null)}
        />
      )}
      <DeleteManyDialog
        open={asking}
        onOpenChange={setAsking}
        count={selection.count}
        noun={NOUN}
        busy={remove.isPending}
        onConfirm={() => remove.mutate(chosen.map((r) => r.id))}
      />
    </DialogPrimitive.Content>
  )
}

/** The rows' shape while the runs load. */
function RunsSkeleton() {
  return (
    <output
      aria-busy="true"
      aria-label="Loading runs"
      className="block overflow-hidden rounded-[12px] border"
    >
      <div className="h-10 border-b bg-muted/40" />
      {[0, 1, 2, 3, 4].map((i) => (
        <div
          key={i}
          className="grid h-12 grid-cols-[40px_200px_128px_96px_minmax(0,1fr)_56px] items-center border-b last:border-b-0"
        >
          <Skeleton className="mx-auto size-4 rounded-[4px]" />
          <Skeleton className="mx-4 h-4" />
          <Skeleton className="mx-4 h-6 w-20" />
          <Skeleton className="mx-4 h-4" />
          <Skeleton className="mx-4 h-4" />
          <Skeleton className="mx-auto size-8" />
        </div>
      ))}
    </output>
  )
}

/** One run: what it was, and every line of its log, times in Eastern time. */
function RunDetail({ run, onBack }: { run: WorkflowExecution; onBack: () => void }) {
  const level = (l: ExecutionLog) => (l.level === 'warn' ? 'warning' : l.level)
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <Button
          variant="ghost"
          size="sm"
          className="h-8 gap-1.5 px-3 text-xs transition-colors duration-150 active:scale-[0.97]"
          onClick={onBack}
        >
          <ArrowLeft className="size-3.5" aria-hidden="true" />
          Back to runs
        </Button>
        <span className="font-mono text-sm font-semibold tabular-nums">
          {runTime(run.started_at)}
        </span>
        <StatusBadge status={run.status} />
        <span className="font-mono text-sm text-muted-foreground tabular-nums">
          {runDuration(run.started_at, run.completed_at)}
        </span>
      </div>
      {run.error && (
        <p className="rounded-[8px] border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive">
          {run.error}
        </p>
      )}
      {(run.logs ?? []).length === 0 ? (
        <p className="text-sm text-muted-foreground">This run has no log lines.</p>
      ) : (
        <ol className="space-y-2" aria-label="Run log">
          {run.logs.map((log, i) => (
            <li
              // A run's log is fixed once written: its order is its identity.
              key={i}
              className={cn(
                'flex items-start gap-3 rounded-[8px] border p-3 text-sm',
                level(log) === 'error' && 'border-destructive/30 bg-destructive/5',
                level(log) === 'warning' && 'border-node-condition/30 bg-node-condition/5'
              )}
            >
              <span className="w-28 shrink-0 font-mono text-xs text-muted-foreground tabular-nums">
                {logTime(log.time)}
              </span>
              <span
                className={cn(
                  'min-w-0 flex-1 break-words',
                  level(log) === 'error' && 'text-destructive'
                )}
              >
                {log.message}
              </span>
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}
