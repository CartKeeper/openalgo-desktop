import type { BacktestMetrics, BenchmarkResult } from '@/api/backtest'

interface MetricsTableProps {
  metrics: BacktestMetrics
  benchmark: BenchmarkResult
}

function pct(v: number, digits = 2) {
  return `${(v * 100).toFixed(digits)}%`
}
function num(v: number, digits = 2) {
  return v === Infinity ? '∞' : v.toFixed(digits)
}
function dollar(v: number) {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2 }).format(v)
}

interface Row {
  label: string
  gross: string
  net: string
  benchmark?: string
  highlight?: boolean
}

export function MetricsTable({ metrics, benchmark }: MetricsTableProps) {
  const rows: Row[] = [
    {
      label: 'Total Return',
      gross: pct(metrics.total_return),
      net: pct(metrics.net_total_return),
      benchmark: pct(benchmark.total_return),
      highlight: true,
    },
    {
      label: 'CAGR',
      gross: pct(metrics.cagr),
      net: pct(metrics.net_cagr),
      benchmark: pct(benchmark.cagr),
      highlight: true,
    },
    { label: 'Volatility (ann.)', gross: pct(metrics.volatility), net: '—', benchmark: '—' },
    { label: 'Sharpe', gross: num(metrics.sharpe), net: '—', benchmark: '—' },
    { label: 'Sortino', gross: num(metrics.sortino), net: '—', benchmark: '—' },
    { label: 'Calmar', gross: num(metrics.calmar), net: '—', benchmark: '—' },
    {
      label: 'Max Drawdown',
      gross: pct(metrics.max_drawdown),
      net: '—',
      benchmark: pct(benchmark.max_drawdown),
    },
    { label: 'DD Peak', gross: metrics.max_dd_peak || '—', net: '—' },
    { label: 'DD Trough', gross: metrics.max_dd_trough || '—', net: '—' },
    { label: 'Trades', gross: metrics.num_trades.toString(), net: '—' },
    { label: 'Win Rate', gross: pct(metrics.win_rate), net: '—' },
    { label: 'Avg Win', gross: dollar(metrics.avg_win), net: '—' },
    { label: 'Avg Loss', gross: dollar(metrics.avg_loss), net: '—' },
    { label: 'Profit Factor', gross: num(metrics.profit_factor), net: '—' },
    { label: 'Max Consec. Losses', gross: metrics.max_consecutive_losses.toString(), net: '—' },
    { label: 'Avg Holding (days)', gross: num(metrics.avg_holding_days, 1), net: '—' },
    { label: 'Time in Market', gross: pct(metrics.time_in_market), net: '—' },
    { label: 'Total Fees', gross: dollar(metrics.total_fees), net: '—' },
    { label: 'ST Tax', gross: dollar(metrics.st_tax), net: '—' },
    { label: 'LT Tax', gross: dollar(metrics.lt_tax), net: '—' },
    { label: 'Total Tax', gross: dollar(metrics.total_tax), net: dollar(metrics.total_tax), highlight: true },
  ]

  return (
    <div className="overflow-x-auto rounded-lg border border-border">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border bg-muted/40">
            <th className="text-left px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground" style={{ height: 40 }}>
              Metric
            </th>
            <th className="text-right px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Gross
            </th>
            <th className="text-right px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Net (after tax)
            </th>
            <th className="text-right px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              B&H Benchmark
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr
              key={row.label}
              className={`border-b border-border/60 last:border-0 transition-colors hover:bg-muted/30 ${row.highlight ? 'font-semibold' : ''}`}
              style={{ height: 48 }}
            >
              <td className="px-4 text-foreground/80">{row.label}</td>
              <td className="px-4 text-right font-mono tabular-nums text-foreground/90">{row.gross}</td>
              <td className="px-4 text-right font-mono tabular-nums text-foreground/70">{row.net}</td>
              <td className="px-4 text-right font-mono tabular-nums text-muted-foreground">
                {row.benchmark ?? '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
