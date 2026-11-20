import { Button } from '@/components/ui/button'
import type { CardMetrics, PricesStatus } from './useScenarioCardMetrics'

const money = (n: number) =>
  new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
  }).format(n)

const pct = (n: number) => `${n >= 0 ? '+' : ''}${n.toFixed(2)}%`

const EPS = 0.005

function signClass(n: number) {
  if (n > EPS) return 'text-green-500'
  if (n < -EPS) return 'text-red-500'
  return 'text-muted-foreground'
}

function arrow(n: number) {
  if (n > EPS) return '▲'
  if (n < -EPS) return '▼'
  return '→'
}

interface Delta {
  /** e.g. "▲ $1,210.00 vs baseline" */
  text: string
  className: string
  /** Hover text: both values, so the change explains itself */
  title: string
}

function moneyDelta(now: number, base: number, baselineName: string): Delta {
  const d = now - base
  return {
    text: Math.abs(d) <= EPS ? '→ no change vs baseline' : `${arrow(d)} ${money(Math.abs(d))} vs baseline`,
    className: signClass(d),
    title: `This scenario: ${money(now)}\n${baselineName}: ${money(base)}`,
  }
}

function pointsDelta(now: number, base: number, baselineName: string): Delta {
  const d = now - base
  return {
    text: Math.abs(d) <= EPS ? '→ no change vs baseline' : `${arrow(d)} ${Math.abs(d).toFixed(2)} pts vs baseline`,
    className: signClass(d),
    title: `This scenario: ${pct(now)}\n${baselineName}: ${pct(base)}`,
  }
}

function Metric({
  label,
  value,
  valueClassName = '',
  valueTitle,
  delta,
  reserveDelta,
  loading,
}: {
  label: string
  value: string
  valueClassName?: string
  valueTitle: string
  delta: Delta | null
  /** Keep the delta line's height so every card's metrics line up */
  reserveDelta: boolean
  loading: boolean
}) {
  return (
    <div className="min-w-0">
      <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground truncate">
        {label}
      </p>
      {loading ? (
        <>
          <div className="h-6 w-24 mt-1 rounded-md bg-muted animate-pulse" />
          {reserveDelta && <div className="h-4 w-28 mt-1 rounded-md bg-muted animate-pulse" />}
        </>
      ) : (
        <>
          <p
            className={`text-base font-bold font-mono tabular-nums mt-1 truncate ${valueClassName}`}
            title={valueTitle}
          >
            {value}
          </p>
          {delta ? (
            <p
              className={`text-xs font-semibold font-mono tabular-nums mt-1 truncate ${delta.className}`}
              title={delta.title}
            >
              {delta.text}
            </p>
          ) : (
            reserveDelta && <p className="text-xs mt-1 invisible" aria-hidden="true">–</p>
          )}
        </>
      )}
    </div>
  )
}

/**
 * Market Value, Cash, Unrealized P&L and Return for one scenario card. A clone
 * also shows how each number differs from its baseline.
 */
export function ScenarioCardMetrics({
  cash,
  metrics,
  baseline,
  baselineName,
  status,
  onRetry,
  reserveDeltas,
}: {
  /** The scenario's recorded cash — shown even when prices fail */
  cash: number | null
  metrics: CardMetrics | undefined
  /** The baseline's numbers; absent on a baseline card */
  baseline: CardMetrics | undefined
  baselineName: string | undefined
  status: PricesStatus
  onRetry: () => void
  reserveDeltas: boolean
}) {
  const loading = status === 'loading'
  const failed = status === 'error' || (status === 'ready' && !metrics)
  const compare = !failed && metrics && baseline && baselineName ? { metrics, baseline, baselineName } : null
  const unpriced = metrics ? metrics.positionCount - metrics.pricedCount : 0

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-4">
        <Metric
          label="Market Value"
          value={failed || !metrics ? '—' : money(metrics.marketValue)}
          valueTitle={
            metrics
              ? `Live value of ${metrics.pricedCount} of ${metrics.positionCount} positions${
                  unpriced > 0 ? ` — ${unpriced} have no live price and aren't counted` : ''
                }`
              : 'Live prices unavailable'
          }
          delta={compare ? moneyDelta(compare.metrics.marketValue, compare.baseline.marketValue, compare.baselineName) : null}
          reserveDelta={reserveDeltas}
          loading={loading}
        />
        <Metric
          label="Cash"
          value={cash == null ? 'Not recorded' : money(cash)}
          valueTitle={
            cash == null
              ? "This scenario's cash isn't recorded."
              : 'Uninvested cash. Applied sells add to it and buys spend it.'
          }
          delta={
            compare && compare.metrics.cash != null && compare.baseline.cash != null
              ? moneyDelta(compare.metrics.cash, compare.baseline.cash, compare.baselineName)
              : null
          }
          reserveDelta={reserveDeltas}
          loading={false}
        />
        <Metric
          label="Unrealized P&L"
          value={failed || !metrics ? '—' : money(metrics.pnl)}
          valueClassName={metrics && !failed ? (metrics.pnl >= 0 ? 'text-green-500' : 'text-red-500') : ''}
          valueTitle="Live market value minus cost basis, across priced positions"
          delta={compare ? moneyDelta(compare.metrics.pnl, compare.baseline.pnl, compare.baselineName) : null}
          reserveDelta={reserveDeltas}
          loading={loading}
        />
        <Metric
          label="Return"
          value={failed || !metrics ? '—' : pct(metrics.returnPct)}
          valueClassName={metrics && !failed ? (metrics.returnPct >= 0 ? 'text-green-500' : 'text-red-500') : ''}
          valueTitle="Unrealized P&L as a percent of total cost basis"
          delta={compare ? pointsDelta(compare.metrics.returnPct, compare.baseline.returnPct, compare.baselineName) : null}
          reserveDelta={reserveDeltas}
          loading={loading}
        />
      </div>
      {failed && (
        <div
          className="flex items-center justify-between gap-2 rounded-md border border-red-500/40 bg-red-500/5 px-3 h-10"
          onClick={(e) => e.stopPropagation()}
        >
          <span className="text-xs font-semibold text-red-500 truncate">Live prices unavailable</span>
          <Button
            variant="outline"
            size="sm"
            className="h-8 shrink-0"
            onClick={(e) => {
              e.stopPropagation()
              onRetry()
            }}
          >
            Retry
          </Button>
        </div>
      )}
    </div>
  )
}
