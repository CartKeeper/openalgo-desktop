import { invoke } from '@tauri-apps/api/core'
import { ArrowLeft, BarChart3, Loader2, TrendingDown } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SymbolMetrics {
  symbol: string
  annualized_return: number
  annualized_volatility: number
  sharpe_ratio: number
  sortino_ratio: number
  max_drawdown: number
  max_drawdown_peak_date: string | null
  max_drawdown_trough_date: string | null
  beta: number | null
  alpha: number | null
  var_95: number
  cvar_95: number
  calmar_ratio: number
}

interface CorrelationEntry {
  symbol_a: string
  symbol_b: string
  correlation: number
}

type DrawdownPoint = [string, number]

type AnalysisState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | {
      status: 'success'
      metrics: SymbolMetrics[]
      correlations: CorrelationEntry[]
      drawdown: DrawdownPoint[]
    }

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BENCHMARKS = [
  { value: 'SPY', label: 'SPY' },
  { value: 'QQQ', label: 'QQQ' },
  { value: 'IWM', label: 'IWM' },
  { value: 'DIA', label: 'DIA' },
]

const PERIODS = [
  { value: '6m', label: '6 Months' },
  { value: '1y', label: '1 Year' },
  { value: '2y', label: '2 Years' },
  { value: '5y', label: '5 Years' },
]

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatPct(value: number): string {
  return `${(value * 100).toFixed(2)}%`
}

function formatRatio(value: number): string {
  return value.toFixed(2)
}

/** Map a correlation value in [-1, 1] to a CSS background colour. */
function correlationColor(v: number): string {
  // Clamp to [-1, 1]
  const c = Math.max(-1, Math.min(1, v))
  if (c >= 0) {
    // 0 => white, 1 => dark green
    const g = Math.round(180 - c * 120) // green channel 180..60
    const r = Math.round(255 - c * 200) // red channel 255..55
    const b = Math.round(255 - c * 200)
    return `rgb(${r}, ${g}, ${b})`
  }
  // -1 => dark red, 0 => white
  const abs = Math.abs(c)
  const r = Math.round(180 + abs * 75) // red channel 180..255
  const g = Math.round(255 - abs * 200)
  const b = Math.round(255 - abs * 200)
  return `rgb(${r}, ${g}, ${b})`
}

/** Determine whether text should be white or dark on a given correlation cell. */
function correlationTextClass(v: number): string {
  return Math.abs(v) > 0.6 ? 'text-white' : 'text-gray-900'
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function MetricRow({ label, value, className }: { label: string; value: string; className?: string }) {
  return (
    <div>
      <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
        {label}
      </span>
      <p className={`text-sm font-semibold tabular-nums ${className ?? ''}`}>{value}</p>
    </div>
  )
}

function MetricCard({ m }: { m: SymbolMetrics }) {
  const returnColor = m.annualized_return >= 0 ? 'text-emerald-500' : 'text-red-500'
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-lg">{m.symbol}</CardTitle>
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-x-4 gap-y-3">
        <MetricRow label="Ann. Return" value={formatPct(m.annualized_return)} className={returnColor} />
        <MetricRow label="Volatility" value={formatPct(m.annualized_volatility)} />
        <MetricRow label="Sharpe" value={formatRatio(m.sharpe_ratio)} />
        <MetricRow label="Sortino" value={formatRatio(m.sortino_ratio)} />
        <MetricRow label="Max Drawdown" value={formatPct(m.max_drawdown)} className="text-red-500" />
        {m.beta !== null && <MetricRow label="Beta" value={formatRatio(m.beta)} />}
        <MetricRow label="VaR 95%" value={formatPct(m.var_95)} />
        <MetricRow label="CVaR 95%" value={formatPct(m.cvar_95)} />
        <MetricRow label="Calmar" value={formatRatio(m.calmar_ratio)} />
        {m.alpha !== null && <MetricRow label="Alpha" value={formatRatio(m.alpha)} />}
      </CardContent>
    </Card>
  )
}

function SkeletonCard() {
  return (
    <Card>
      <CardHeader className="pb-2">
        <Skeleton className="h-5 w-20" />
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-x-4 gap-y-3">
        {Array.from({ length: 8 }).map((_, i) => (
          <div key={i}>
            <Skeleton className="h-3 w-16 mb-1" />
            <Skeleton className="h-4 w-12" />
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Drawdown Chart (SVG)
// ---------------------------------------------------------------------------

function DrawdownChart({ data, symbol }: { data: DrawdownPoint[]; symbol: string }) {
  if (data.length === 0) return null

  const chartW = 720
  const chartH = 260
  const padL = 60
  const padR = 16
  const padT = 16
  const padB = 40
  const plotW = chartW - padL - padR
  const plotH = chartH - padT - padB

  // Drawdown values are negative (or zero). Find the minimum (worst).
  const values = data.map((d) => d[1])
  const minVal = Math.min(...values)
  // Y-axis from 0 (top) to minVal (bottom)
  const yDomain = Math.abs(minVal) || 0.01 // avoid zero range

  const toX = (i: number) => padL + (i / (data.length - 1)) * plotW
  const toY = (v: number) => padT + (Math.abs(v) / yDomain) * plotH

  // Build SVG path
  let areaPath = `M ${toX(0)} ${toY(0)}`
  let linePath = `M ${toX(0)} ${toY(values[0])}`
  for (let i = 0; i < data.length; i++) {
    linePath += ` L ${toX(i)} ${toY(values[i])}`
  }
  // Close area path going across the bottom then back
  areaPath = `M ${toX(0)} ${toY(values[0])}`
  for (let i = 1; i < data.length; i++) {
    areaPath += ` L ${toX(i)} ${toY(values[i])}`
  }
  areaPath += ` L ${toX(data.length - 1)} ${toY(0)} L ${toX(0)} ${toY(0)} Z`

  // Max drawdown point
  const minIdx = values.indexOf(minVal)
  const minDate = data[minIdx][0]

  // X-axis tick labels (show ~6 labels)
  const tickCount = Math.min(6, data.length)
  const xTicks: { x: number; label: string }[] = []
  for (let t = 0; t < tickCount; t++) {
    const idx = Math.round((t / (tickCount - 1)) * (data.length - 1))
    const dateStr = data[idx][0]
    // Show as MMM YY
    const d = new Date(dateStr)
    const label = d.toLocaleDateString('en-US', { month: 'short', year: '2-digit' })
    xTicks.push({ x: toX(idx), label })
  }

  // Y-axis tick labels (show 5)
  const yTickCount = 5
  const yTicks: { y: number; label: string }[] = []
  for (let t = 0; t <= yTickCount; t++) {
    const val = -(yDomain * t) / yTickCount
    yTicks.push({ y: toY(val), label: `${(val * 100).toFixed(1)}%` })
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <TrendingDown className="h-4 w-4 text-red-500" />
          Drawdown — {symbol}
        </CardTitle>
      </CardHeader>
      <CardContent className="overflow-x-auto">
        <svg
          viewBox={`0 0 ${chartW} ${chartH}`}
          className="w-full max-w-[720px]"
          preserveAspectRatio="xMidYMid meet"
        >
          {/* Grid lines */}
          {yTicks.map((t, i) => (
            <line
              key={i}
              x1={padL}
              y1={t.y}
              x2={chartW - padR}
              y2={t.y}
              className="stroke-muted-foreground/15"
              strokeWidth={1}
            />
          ))}

          {/* Area fill */}
          <path d={areaPath} fill="rgba(239, 68, 68, 0.15)" />
          {/* Line */}
          <path d={linePath} fill="none" stroke="rgb(239, 68, 68)" strokeWidth={1.5} />

          {/* Max drawdown marker */}
          <circle
            cx={toX(minIdx)}
            cy={toY(minVal)}
            r={4}
            fill="rgb(239, 68, 68)"
            stroke="white"
            strokeWidth={1.5}
          />
          <text
            x={toX(minIdx)}
            y={toY(minVal) - 10}
            textAnchor="middle"
            className="fill-red-500 text-[10px] font-semibold"
            style={{ fontVariantNumeric: 'tabular-nums' }}
          >
            {(minVal * 100).toFixed(2)}%
          </text>
          <text
            x={toX(minIdx)}
            y={toY(minVal) - 22}
            textAnchor="middle"
            className="fill-muted-foreground text-[9px]"
          >
            {minDate}
          </text>

          {/* Y-axis labels */}
          {yTicks.map((t, i) => (
            <text
              key={i}
              x={padL - 6}
              y={t.y + 3}
              textAnchor="end"
              className="fill-muted-foreground text-[10px]"
              style={{ fontVariantNumeric: 'tabular-nums' }}
            >
              {t.label}
            </text>
          ))}

          {/* X-axis labels */}
          {xTicks.map((t, i) => (
            <text
              key={i}
              x={t.x}
              y={chartH - 6}
              textAnchor="middle"
              className="fill-muted-foreground text-[10px]"
            >
              {t.label}
            </text>
          ))}
        </svg>
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Correlation Matrix
// ---------------------------------------------------------------------------

function CorrelationMatrix({
  symbols,
  entries,
}: {
  symbols: string[]
  entries: CorrelationEntry[]
}) {
  // Build a lookup map
  const map = useMemo(() => {
    const m = new Map<string, number>()
    for (const e of entries) {
      m.set(`${e.symbol_a}:${e.symbol_b}`, e.correlation)
      m.set(`${e.symbol_b}:${e.symbol_a}`, e.correlation)
    }
    // Diagonal = 1
    for (const s of symbols) {
      m.set(`${s}:${s}`, 1)
    }
    return m
  }, [symbols, entries])

  if (symbols.length < 2) return null

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Correlation Matrix</CardTitle>
      </CardHeader>
      <CardContent className="overflow-x-auto">
        <table className="border-collapse">
          <thead>
            <tr>
              <th className="px-3 py-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider" />
              {symbols.map((s) => (
                <th
                  key={s}
                  className="px-3 py-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider text-center"
                >
                  {s}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {symbols.map((rowSym) => (
              <tr key={rowSym}>
                <td className="px-3 py-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap">
                  {rowSym}
                </td>
                {symbols.map((colSym) => {
                  const val = map.get(`${rowSym}:${colSym}`) ?? 0
                  return (
                    <td
                      key={colSym}
                      className={`px-3 py-2 text-center text-xs font-semibold tabular-nums min-w-[56px] ${correlationTextClass(val)}`}
                      style={{ backgroundColor: correlationColor(val) }}
                    >
                      {val.toFixed(2)}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Main Page
// ---------------------------------------------------------------------------

export default function QuantPage() {
  const [symbolsInput, setSymbolsInput] = useState('')
  const [benchmark, setBenchmark] = useState('SPY')
  const [period, setPeriod] = useState('1y')
  const [state, setState] = useState<AnalysisState>({ status: 'idle' })

  const parsedSymbols = useMemo(() => {
    return symbolsInput
      .split(',')
      .map((s) => s.trim().toUpperCase())
      .filter(Boolean)
  }, [symbolsInput])

  const handleAnalyze = useCallback(async () => {
    if (parsedSymbols.length === 0) {
      toast.error('Enter at least one symbol to analyze.')
      return
    }

    setState({ status: 'loading' })

    try {
      // Fetch metrics for each symbol in parallel
      const metricsPromises = parsedSymbols.map((symbol) =>
        invoke<SymbolMetrics>('get_symbol_metrics', { symbol, benchmark, period })
      )

      // Fetch drawdown chart for the first symbol
      const drawdownPromise = invoke<DrawdownPoint[]>('get_drawdown_chart', {
        symbol: parsedSymbols[0],
        period,
      })

      // Fetch correlation matrix if 2+ symbols
      const correlationPromise =
        parsedSymbols.length >= 2
          ? invoke<CorrelationEntry[]>('get_correlation_matrix', {
              symbols: parsedSymbols,
              period,
            })
          : Promise.resolve([] as CorrelationEntry[])

      const [metricsResults, drawdown, correlations] = await Promise.all([
        Promise.all(metricsPromises),
        drawdownPromise,
        correlationPromise,
      ])

      if (metricsResults.length === 0) {
        setState({ status: 'error', message: 'No data returned for the given symbols.' })
        return
      }

      setState({
        status: 'success',
        metrics: metricsResults,
        correlations: correlations,
        drawdown: drawdown,
      })
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err)
      setState({ status: 'error', message })
      toast.error('Analysis failed', { description: message })
    }
  }, [parsedSymbols, benchmark, period])

  return (
    <div className="py-6 max-w-6xl mx-auto space-y-6 px-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <BarChart3 className="h-6 w-6" />
            Quant Analytics
          </h1>
        </div>
        <p className="text-muted-foreground">
          Analyze risk metrics, drawdowns, and correlations for your portfolio symbols.
        </p>
      </div>

      {/* Input Section */}
      <Card>
        <CardContent className="pt-6">
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 items-end">
            {/* Symbols */}
            <div className="sm:col-span-2 lg:col-span-1 space-y-1.5">
              <Label htmlFor="symbols-input">Symbols</Label>
              <Input
                id="symbols-input"
                placeholder="AAPL, MSFT, GOOGL, AMZN"
                value={symbolsInput}
                onChange={(e) => setSymbolsInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleAnalyze()
                }}
              />
            </div>

            {/* Benchmark */}
            <div className="space-y-1.5">
              <Label>Benchmark</Label>
              <Select value={benchmark} onValueChange={setBenchmark}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {BENCHMARKS.map((b) => (
                    <SelectItem key={b.value} value={b.value}>
                      {b.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Period */}
            <div className="space-y-1.5">
              <Label>Period</Label>
              <Select value={period} onValueChange={setPeriod}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PERIODS.map((p) => (
                    <SelectItem key={p.value} value={p.value}>
                      {p.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Analyze Button */}
            <div>
              <Button
                onClick={handleAnalyze}
                disabled={state.status === 'loading' || parsedSymbols.length === 0}
                className="w-full"
              >
                {state.status === 'loading' ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Analyzing...
                  </>
                ) : (
                  'Analyze'
                )}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Idle State */}
      {state.status === 'idle' && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <BarChart3 className="h-12 w-12 text-muted-foreground/40 mb-4" />
          <p className="text-muted-foreground text-sm">
            Enter symbols to analyze risk metrics
          </p>
        </div>
      )}

      {/* Loading State */}
      {state.status === 'loading' && (
        <div className="space-y-6">
          <div className="flex items-center gap-2 text-muted-foreground text-sm">
            <Loader2 className="h-4 w-4 animate-spin" />
            Analyzing...
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
            {Array.from({ length: Math.max(parsedSymbols.length, 1) }).map((_, i) => (
              <SkeletonCard key={i} />
            ))}
          </div>
        </div>
      )}

      {/* Error State */}
      {state.status === 'error' && (
        <Card className="border-destructive">
          <CardContent className="pt-6 text-center">
            <p className="text-destructive font-semibold mb-1">Analysis Failed</p>
            <p className="text-sm text-muted-foreground">{state.message}</p>
          </CardContent>
        </Card>
      )}

      {/* Success State */}
      {state.status === 'success' && (
        <div className="space-y-6">
          {/* Metrics Cards Grid */}
          {state.metrics.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
              {state.metrics.map((m) => (
                <MetricCard key={m.symbol} m={m} />
              ))}
            </div>
          ) : (
            <Card>
              <CardContent className="pt-6 text-center">
                <p className="text-muted-foreground text-sm">No metric data returned for the given symbols.</p>
              </CardContent>
            </Card>
          )}

          {/* Drawdown Chart */}
          {state.drawdown.length > 0 && (
            <DrawdownChart data={state.drawdown} symbol={parsedSymbols[0]} />
          )}

          {/* Correlation Matrix */}
          {parsedSymbols.length >= 2 && state.correlations.length > 0 && (
            <CorrelationMatrix symbols={parsedSymbols} entries={state.correlations} />
          )}
        </div>
      )}
    </div>
  )
}
