import { CheckCircle2, Circle, Loader2, XCircle } from 'lucide-react'
import type { StageRow, StageStatus } from '@/api/stagedAnalysis'
import { cn } from '@/lib/utils'

function StatusIcon({ status }: { status: StageStatus }) {
  const base = 'h-4 w-4 shrink-0'
  switch (status) {
    case 'running':
      return <Loader2 data-testid="stage-icon-running" className={cn(base, 'animate-spin text-primary')} aria-hidden="true" />
    case 'done':
      return (
        <CheckCircle2
          data-testid="stage-icon-done"
          className={cn(base, 'text-green-600 dark:text-green-400')}
          aria-hidden="true"
        />
      )
    case 'failed':
      return <XCircle data-testid="stage-icon-failed" className={cn(base, 'text-destructive')} aria-hidden="true" />
    default:
      return <Circle data-testid="stage-icon-pending" className={cn(base, 'text-muted-foreground')} aria-hidden="true" />
  }
}

interface StagedProgressProps {
  stages: StageRow[]
}

/**
 * The nine stages of a full analysis with an icon per status, compact enough
 * to sit above the Analyze dialog's input while a run is in flight.
 */
export function StagedProgress({ stages }: StagedProgressProps) {
  return (
    <ul aria-label="Full analysis progress" aria-live="polite" className="grid grid-cols-1 gap-1 sm:grid-cols-3">
      {stages.map((stage) => (
        <li
          key={stage.key}
          data-stage={stage.key}
          data-status={stage.status}
          className="flex h-6 min-w-0 items-center gap-2 text-xs"
        >
          <StatusIcon status={stage.status} />
          <span
            className={cn(
              'shrink-0 font-semibold',
              stage.status === 'pending' && 'text-muted-foreground',
              stage.status === 'failed' && 'text-destructive'
            )}
          >
            {stage.label}
          </span>
          <span className="sr-only">{stage.status}</span>
          {stage.detail && (
            <span className="truncate text-muted-foreground" title={stage.detail}>
              {stage.detail}
            </span>
          )}
        </li>
      ))}
    </ul>
  )
}
