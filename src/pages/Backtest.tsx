import {
  AlertTriangle,
  BookMarked,
  ChevronDown,
  ChevronUp,
  Download,
  FlaskConical,
  HelpCircle,
  Loader2,
  Play,
  RefreshCw,
  Save,
} from 'lucide-react'
import { useCallback, useState } from 'react'
import { toast } from 'sonner'
import { backtestApi, type BacktestConfig, type BacktestResult, type BacktestRunRecord, type StrategySpec, type Sizing } from '@/api/backtest'
import { historifyCommands } from '@/api/tauri-client'
import { EquityChart, DrawdownChart } from '@/components/backtest/EquityChart'
import { MetricsTable } from '@/components/backtest/MetricsTable'
import { TradeTable } from '@/components/backtest/TradeTable'
import { Badge } from '@/components/ui/badge'
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
import { cn } from '@/lib/utils'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type PageState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'success'; result: BacktestResult }

/** Extract a human-readable message from a Tauri/Rust error. AppError serializes
 *  to an object { code, message }, so a bare String(e) yields "[object Object]". */
function errMsg(e: unknown): string {
  if (typeof e === 'string') return e
  if (e && typeof e === 'object' && 'message' in e && typeof (e as { message: unknown }).message === 'string') {
    return (e as { message: string }).message
  }
  return String(e)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function pct(v: number) {
  return `${(v * 100).toFixed(2)}%`
}
function fmtDollar(v: number) {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2 }).format(v)
}

// ---------------------------------------------------------------------------
// Strategy param component
// ---------------------------------------------------------------------------

type StrategyKind = StrategySpec['kind']

interface StrategyParamsProps {
  kind: StrategyKind
  params: Record<string, string>
  onChange: (key: string, value: string) => void
}

function StrategyParams({ kind, params, onChange }: StrategyParamsProps) {
  const field = (key: string, label: string, placeholder: string) => (
    <div className="flex flex-col gap-1.5">
      <Label className="text-xs">{label}</Label>
      <Input
        type="number"
        value={params[key] ?? ''}
        onChange={(e) => onChange(key, e.target.value)}
        placeholder={placeholder}
        className="h-10 text-sm"
      />
    </div>
  )

  if (kind === 'SmaCrossover' || kind === 'EmaCrossover') {
    return (
      <>
        {field('fast', 'Fast period', '20')}
        {field('slow', 'Slow period', '50')}
      </>
    )
  }
  if (kind === 'RsiThreshold') {
    return (
      <>
        {field('period', 'RSI period', '14')}
        {field('oversold', 'Oversold', '30')}
        {field('overbought', 'Overbought', '70')}
      </>
    )
  }
  if (kind === 'MacdCross') {
    return (
      <>
        {field('fast', 'Fast', '12')}
        {field('slow', 'Slow', '26')}
        {field('signal', 'Signal', '9')}
      </>
    )
  }
  if (kind === 'BollingerReversion') {
    return (
      <>
        {field('period', 'Period', '20')}
        {field('std_dev', 'Std dev', '2')}
      </>
    )
  }
  return null
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

export default function Backtest() {
  // Config form state
  const [strategyKind, setStrategyKind] = useState<StrategyKind>('SmaCrossover')
  const [strategyParams, setStrategyParams] = useState<Record<string, string>>({
    fast: '20',
    slow: '50',
    period: '14',
    oversold: '30',
    overbought: '70',
    signal: '9',
    std_dev: '2',
  })
  const [symbol, setSymbol] = useState('AAPL')
  const [exchange, setExchange] = useState('NASDAQ')
  const [barInterval, setBarInterval] = useState('D')
  const [fromDate, setFromDate] = useState('2020-01-01')
  const [toDate, setToDate] = useState(new Date().toISOString().slice(0, 10))
  const [startingCapital, setStartingCapital] = useState('100000')
  const [sizingKind, setSizingKind] = useState<Sizing['kind']>('AllIn')
  const [sizingValue, setSizingValue] = useState('0.5')
  const [slippageBps, setSlippageBps] = useState('5')
  const [regFees, setRegFees] = useState(true)
  const [stTaxPct, setStTaxPct] = useState('35')
  const [ltTaxPct, setLtTaxPct] = useState('15')
  const [fractional, setFractional] = useState(false)
  const [riskFreeRate, setRiskFreeRate] = useState('5')

  // UI state
  const [pageState, setPageState] = useState<PageState>({ status: 'idle' })
  const [savedRuns, setSavedRuns] = useState<BacktestRunRecord[]>([])
  const [showSavedRuns, setShowSavedRuns] = useState(false)
  const [showHelp, setShowHelp] = useState(false)
  const [downloading, setDownloading] = useState(false)

  const updateParam = useCallback((key: string, value: string) => {
    setStrategyParams((prev) => ({ ...prev, [key]: value }))
  }, [])

  // Build BacktestConfig from form state
  const buildConfig = (): BacktestConfig => {
    const p = strategyParams
    let strategy: StrategySpec
    if (strategyKind === 'SmaCrossover') strategy = { kind: 'SmaCrossover', fast: Number(p.fast), slow: Number(p.slow) }
    else if (strategyKind === 'EmaCrossover') strategy = { kind: 'EmaCrossover', fast: Number(p.fast), slow: Number(p.slow) }
    else if (strategyKind === 'RsiThreshold') strategy = { kind: 'RsiThreshold', period: Number(p.period), oversold: Number(p.oversold), overbought: Number(p.overbought) }
    else if (strategyKind === 'MacdCross') strategy = { kind: 'MacdCross', fast: Number(p.fast), slow: Number(p.slow), signal: Number(p.signal) }
    else strategy = { kind: 'BollingerReversion', period: Number(p.period), std_dev: Number(p.std_dev) }

    let sizing: Sizing
    if (sizingKind === 'AllIn') sizing = { kind: 'AllIn' }
    else if (sizingKind === 'FixedFraction') sizing = { kind: 'FixedFraction', value: Number(sizingValue) }
    else sizing = { kind: 'FixedShares', value: Number(sizingValue) }

    return {
      symbol: symbol.trim().toUpperCase(),
      exchange: exchange.trim().toUpperCase(),
      interval: barInterval,
      from_date: fromDate,
      to_date: toDate,
      starting_capital: Number(startingCapital),
      sizing,
      costs: {
        commission_per_trade: 0,
        slippage_bps: Number(slippageBps),
        reg_fees_enabled: regFees,
      },
      tax: {
        st_rate: Number(stTaxPct) / 100,
        lt_rate: Number(ltTaxPct) / 100,
      },
      fractional,
      risk_free_rate: Number(riskFreeRate) / 100,
      strategy,
    }
  }

  const handleRun = async () => {
    setPageState({ status: 'loading' })
    try {
      const config = buildConfig()
      const result = await backtestApi.run(config)
      setPageState({ status: 'success', result })
    } catch (e) {
      setPageState({ status: 'error', message: errMsg(e) })
    }
  }

  const handleDownloadAndRun = async () => {
    setDownloading(true)
    try {
      const resp = await historifyCommands.downloadHistoricalData({
        symbol: symbol.trim().toUpperCase(),
        exchange: exchange.trim().toUpperCase(),
        timeframe: barInterval,
        from_date: fromDate,
        to_date: toDate,
      })
      if (resp.rows_downloaded > 0) {
        toast.success(`Downloaded ${resp.rows_downloaded} bars`)
        setDownloading(false)
        await handleRun()
      } else {
        toast.warning('No bars returned — check the symbol, date range, or your Alpaca market-data entitlement.')
        setDownloading(false)
      }
    } catch (e) {
      setPageState({ status: 'error', message: errMsg(e) })
      setDownloading(false)
    }
  }

  const handleSave = async () => {
    if (pageState.status !== 'success') return
    try {
      const config = buildConfig()
      const id = await backtestApi.save(config, JSON.stringify(pageState.result.metrics))
      toast.success(`Run saved`, { description: `ID: ${id}` })
    } catch (e) {
      toast.error('Save failed', { description: errMsg(e) })
    }
  }

  const handleLoadSavedRuns = async () => {
    try {
      const runs = await backtestApi.list()
      setSavedRuns(runs)
      setShowSavedRuns(true)
    } catch (e) {
      toast.error('Could not load saved runs', { description: errMsg(e) })
    }
  }

  const handleLoadRun = async (run: BacktestRunRecord) => {
    try {
      const cfg: BacktestConfig = JSON.parse(run.config_json)
      // Populate form from config
      setSymbol(cfg.symbol)
      setExchange(cfg.exchange)
      setBarInterval(cfg.interval)
      setFromDate(cfg.from_date)
      setToDate(cfg.to_date)
      setStartingCapital(String(cfg.starting_capital))
      setSizingKind(cfg.sizing.kind)
      if (cfg.sizing.kind !== 'AllIn') setSizingValue(String(cfg.sizing.value))
      setSlippageBps(String(cfg.costs.slippage_bps))
      setRegFees(cfg.costs.reg_fees_enabled)
      setStTaxPct(String(Math.round(cfg.tax.st_rate * 100)))
      setLtTaxPct(String(Math.round(cfg.tax.lt_rate * 100)))
      setFractional(cfg.fractional)
      setRiskFreeRate(String(Math.round(cfg.risk_free_rate * 100)))
      setStrategyKind(cfg.strategy.kind)
      const sp = { ...strategyParams }
      if ('fast' in cfg.strategy) sp.fast = String(cfg.strategy.fast)
      if ('slow' in cfg.strategy) sp.slow = String(cfg.strategy.slow)
      if ('period' in cfg.strategy) sp.period = String(cfg.strategy.period)
      if ('oversold' in cfg.strategy) sp.oversold = String(cfg.strategy.oversold)
      if ('overbought' in cfg.strategy) sp.overbought = String(cfg.strategy.overbought)
      if ('signal' in cfg.strategy) sp.signal = String(cfg.strategy.signal)
      if ('std_dev' in cfg.strategy) sp.std_dev = String(cfg.strategy.std_dev)
      setStrategyParams(sp)
      setShowSavedRuns(false)
      toast.success('Config loaded — click Run to re-run')
    } catch {
      toast.error('Could not parse saved config')
    }
  }

  const result = pageState.status === 'success' ? pageState.result : null

  // Metric summary cards (top-line quick look)
  const summaryCards = result
    ? [
        { label: 'Total Return', value: pct(result.metrics.total_return), sub: `Net: ${pct(result.metrics.net_total_return)}`, good: result.metrics.total_return >= 0 },
        { label: 'CAGR', value: pct(result.metrics.cagr), sub: `B&H: ${pct(result.benchmark.cagr)}`, good: result.metrics.cagr >= 0 },
        { label: 'Sharpe', value: result.metrics.sharpe.toFixed(2), sub: `Sortino: ${result.metrics.sortino.toFixed(2)}`, good: result.metrics.sharpe >= 1 },
        { label: 'Max Drawdown', value: pct(result.metrics.max_drawdown), sub: '', good: false },
        { label: 'Win Rate', value: pct(result.metrics.win_rate), sub: `${result.metrics.num_trades} trades`, good: result.metrics.win_rate >= 0.5 },
        { label: 'Total Tax', value: fmtDollar(result.metrics.total_tax), sub: `Fees: ${fmtDollar(result.metrics.total_fees)}`, good: false },
      ]
    : null

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="container mx-auto px-6 py-6 space-y-6">

        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-violet-500/10 border border-violet-500/20 flex items-center justify-center">
              <FlaskConical className="w-5 h-5 text-violet-400" />
            </div>
            <div>
              <h1 className="text-xl font-semibold tracking-tight">Backtest</h1>
              <p className="text-xs text-muted-foreground">Deterministic single-symbol strategy backtester</p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" className="h-9 gap-1.5 text-xs" onClick={handleLoadSavedRuns}>
              <BookMarked className="w-3.5 h-3.5" />
              Saved Runs
            </Button>
            {/* Bare help icon — no filled background per project rules */}
            <button
              type="button"
              onClick={() => setShowHelp(!showHelp)}
              className="h-9 w-9 flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              aria-label="Help"
            >
              <HelpCircle className="w-5 h-5" strokeWidth={1.5} />
            </button>
          </div>
        </div>

        {/* Help panel */}
        {showHelp && (
          <Card className="rounded-xl border-border">
            <CardContent className="pt-4 pb-4 space-y-3">
              <p className="text-sm font-semibold">Backtest Help</p>
              <ul className="text-xs text-muted-foreground space-y-2 list-disc list-inside">
                <li><strong>History required:</strong> Download daily OHLCV data for your symbol via Historify before running a backtest.</li>
                <li><strong>Strategies:</strong> SMA/EMA Crossover, RSI Threshold, MACD Cross, Bollinger Reversion. Each is long-only with next-open fills.</li>
                <li><strong>Sizing:</strong> All-In uses all available cash. Fixed Fraction uses a fraction of equity. Fixed Shares buys a fixed number each entry.</li>
                <li><strong>Slippage:</strong> Entered in basis points (1 bps = 0.01%). Applied to both entry and exit fills.</li>
                <li><strong>Tax:</strong> Enter rates as percentages (e.g. 35 = 35%). Gains are bucketed into ST (&lt;365 days) and LT (≥365 days).</li>
                <li><strong>Equity curve:</strong> Mark-to-market gross equity at each daily close. Benchmark figures are buy-and-hold summary only.</li>
                <li><strong>CSV export:</strong> Exports all trades to your Downloads folder.</li>
              </ul>
            </CardContent>
          </Card>
        )}

        {/* Saved runs panel */}
        {showSavedRuns && (
          <Card className="rounded-xl border-border">
            <CardHeader className="py-3 px-4 border-b border-border/60">
              <div className="flex items-center justify-between">
                <CardTitle className="text-sm font-semibold">Saved Runs</CardTitle>
                <button
                  type="button"
                  className="text-xs text-muted-foreground hover:text-foreground transition-colors"
                  onClick={() => setShowSavedRuns(false)}
                >
                  Close
                </button>
              </div>
            </CardHeader>
            <CardContent className="p-0">
              {savedRuns.length === 0 ? (
                <p className="text-sm text-muted-foreground text-center py-6">No saved runs yet.</p>
              ) : (
                <table className="w-full text-xs">
                  <thead>
                    <tr className="border-b border-border/60 bg-muted/30" style={{ height: 36 }}>
                      <th className="text-left px-4 text-muted-foreground font-semibold uppercase tracking-wider">Date</th>
                      <th className="text-left px-4 text-muted-foreground font-semibold uppercase tracking-wider">Symbol</th>
                      <th className="text-left px-4 text-muted-foreground font-semibold uppercase tracking-wider">Strategy</th>
                      <th className="text-left px-4 text-muted-foreground font-semibold uppercase tracking-wider">Range</th>
                      <th className="px-4" />
                    </tr>
                  </thead>
                  <tbody>
                    {savedRuns.map((run) => (
                      <tr key={run.id} className="border-b border-border/40 last:border-0 hover:bg-muted/20 transition-colors" style={{ height: 44 }}>
                        <td className="px-4 font-mono tabular-nums text-foreground/70">{run.created_at.slice(0, 10)}</td>
                        <td className="px-4 font-semibold text-foreground">{run.exchange}:{run.symbol}</td>
                        <td className="px-4 text-foreground/80">{run.strategy_kind}</td>
                        <td className="px-4 text-foreground/70 font-mono tabular-nums">{run.from_date} → {run.to_date}</td>
                        <td className="px-4">
                          <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => handleLoadRun(run)}>
                            Load
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </CardContent>
          </Card>
        )}

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* ---------------------------------------------------------------- */}
          {/* Config panel                                                      */}
          {/* ---------------------------------------------------------------- */}
          <div className="lg:col-span-1 space-y-4">
            <Card className="rounded-xl border-border">
              <CardHeader className="py-3 px-4 border-b border-border/60">
                <CardTitle className="text-sm font-semibold">Strategy</CardTitle>
              </CardHeader>
              <CardContent className="p-4 space-y-4">
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs">Strategy</Label>
                  <Select value={strategyKind} onValueChange={(v) => setStrategyKind(v as StrategyKind)}>
                    <SelectTrigger className="h-10">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="SmaCrossover">SMA Crossover</SelectItem>
                      <SelectItem value="EmaCrossover">EMA Crossover</SelectItem>
                      <SelectItem value="RsiThreshold">RSI Threshold</SelectItem>
                      <SelectItem value="MacdCross">MACD Cross</SelectItem>
                      <SelectItem value="BollingerReversion">Bollinger Reversion</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                <StrategyParams kind={strategyKind} params={strategyParams} onChange={updateParam} />
              </CardContent>
            </Card>

            <Card className="rounded-xl border-border">
              <CardHeader className="py-3 px-4 border-b border-border/60">
                <CardTitle className="text-sm font-semibold">Symbol & Dates</CardTitle>
              </CardHeader>
              <CardContent className="p-4 space-y-4">
                <div className="flex gap-2">
                  <div className="flex-1 flex flex-col gap-1.5">
                    <Label className="text-xs">Symbol</Label>
                    <Input value={symbol} onChange={(e) => setSymbol(e.target.value)} className="h-10 uppercase" placeholder="AAPL" />
                  </div>
                  <div className="w-32 flex flex-col gap-1.5">
                    <Label className="text-xs">Exchange</Label>
                    <Input value={exchange} onChange={(e) => setExchange(e.target.value)} className="h-10 uppercase" placeholder="NASDAQ" />
                  </div>
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs">Interval</Label>
                  <Select value={barInterval} onValueChange={setBarInterval}>
                    <SelectTrigger className="h-10">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="D">Daily (D)</SelectItem>
                      <SelectItem value="W">Weekly (W)</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex gap-2">
                  <div className="flex-1 flex flex-col gap-1.5">
                    <Label className="text-xs">From</Label>
                    <Input type="date" value={fromDate} onChange={(e) => setFromDate(e.target.value)} className="h-10" />
                  </div>
                  <div className="flex-1 flex flex-col gap-1.5">
                    <Label className="text-xs">To</Label>
                    <Input type="date" value={toDate} onChange={(e) => setToDate(e.target.value)} className="h-10" />
                  </div>
                </div>
              </CardContent>
            </Card>

            <Card className="rounded-xl border-border">
              <CardHeader className="py-3 px-4 border-b border-border/60">
                <CardTitle className="text-sm font-semibold">Capital & Sizing</CardTitle>
              </CardHeader>
              <CardContent className="p-4 space-y-4">
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs">Starting Capital ($)</Label>
                  <Input type="number" value={startingCapital} onChange={(e) => setStartingCapital(e.target.value)} className="h-10" placeholder="100000" />
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs">Position Sizing</Label>
                  <Select value={sizingKind} onValueChange={(v) => setSizingKind(v as Sizing['kind'])}>
                    <SelectTrigger className="h-10">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="AllIn">All-In</SelectItem>
                      <SelectItem value="FixedFraction">Fixed Fraction</SelectItem>
                      <SelectItem value="FixedShares">Fixed Shares</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                {sizingKind !== 'AllIn' && (
                  <div className="flex flex-col gap-1.5">
                    <Label className="text-xs">
                      {sizingKind === 'FixedFraction' ? 'Fraction (0–1)' : 'Number of shares'}
                    </Label>
                    <Input type="number" value={sizingValue} onChange={(e) => setSizingValue(e.target.value)} className="h-10" placeholder={sizingKind === 'FixedFraction' ? '0.5' : '10'} />
                  </div>
                )}
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="fractional"
                    checked={fractional}
                    onChange={(e) => setFractional(e.target.checked)}
                    className="w-4 h-4 accent-accent cursor-pointer"
                  />
                  <Label htmlFor="fractional" className="text-xs cursor-pointer">Allow fractional shares</Label>
                </div>
              </CardContent>
            </Card>

            <Card className="rounded-xl border-border">
              <CardHeader className="py-3 px-4 border-b border-border/60">
                <CardTitle className="text-sm font-semibold">Costs & Tax</CardTitle>
              </CardHeader>
              <CardContent className="p-4 space-y-4">
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs">Slippage (bps)</Label>
                  <Input type="number" value={slippageBps} onChange={(e) => setSlippageBps(e.target.value)} className="h-10" placeholder="5" />
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="regfees"
                    checked={regFees}
                    onChange={(e) => setRegFees(e.target.checked)}
                    className="w-4 h-4 accent-accent cursor-pointer"
                  />
                  <Label htmlFor="regfees" className="text-xs cursor-pointer">SEC/FINRA reg fees on sells</Label>
                </div>
                <div className="flex gap-2">
                  <div className="flex-1 flex flex-col gap-1.5">
                    <Label className="text-xs">ST Tax (%)</Label>
                    <Input type="number" value={stTaxPct} onChange={(e) => setStTaxPct(e.target.value)} className="h-10" placeholder="35" />
                  </div>
                  <div className="flex-1 flex flex-col gap-1.5">
                    <Label className="text-xs">LT Tax (%)</Label>
                    <Input type="number" value={ltTaxPct} onChange={(e) => setLtTaxPct(e.target.value)} className="h-10" placeholder="15" />
                  </div>
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs">Risk-Free Rate (%)</Label>
                  <Input type="number" value={riskFreeRate} onChange={(e) => setRiskFreeRate(e.target.value)} className="h-10" placeholder="5" />
                </div>
              </CardContent>
            </Card>

            {/* Run + Save buttons */}
            <div className="flex gap-2">
              <Button
                className="flex-1 h-10 gap-2 font-semibold"
                onClick={handleRun}
                disabled={pageState.status === 'loading'}
              >
                {pageState.status === 'loading' ? (
                  <Loader2 className="w-4 h-4 animate-spin motion-reduce:animate-none" />
                ) : (
                  <Play className="w-4 h-4" />
                )}
                {pageState.status === 'loading' ? 'Running…' : 'Run Backtest'}
              </Button>
              {result && (
                <Button
                  variant="outline"
                  className="h-10 gap-2 font-semibold px-4"
                  onClick={handleSave}
                >
                  <Save className="w-4 h-4" />
                  Save
                </Button>
              )}
            </div>
          </div>

          {/* ---------------------------------------------------------------- */}
          {/* Results panel                                                     */}
          {/* ---------------------------------------------------------------- */}
          <div className="lg:col-span-2 space-y-4">

            {/* Idle (empty state) */}
            {pageState.status === 'idle' && (
              <div className="flex flex-col items-center justify-center h-96 rounded-xl border border-dashed border-border bg-muted/20">
                <FlaskConical className="w-12 h-12 text-muted-foreground/40 mb-4" />
                <p className="text-sm font-semibold text-foreground/60">Configure a backtest to begin</p>
                <p className="text-xs text-muted-foreground mt-1">Set your strategy, symbol, and date range — then click Run.</p>
              </div>
            )}

            {/* Loading skeleton */}
            {pageState.status === 'loading' && (
              <div className="space-y-4">
                <div className="rounded-xl border border-border p-4 space-y-3">
                  <Skeleton className="h-5 w-32" />
                  <div className="grid grid-cols-3 gap-3">
                    {[...Array(6)].map((_, i) => (
                      <Skeleton key={i} className="h-20 rounded-lg" />
                    ))}
                  </div>
                </div>
                <div className="rounded-xl border border-border p-4">
                  <Skeleton className="h-5 w-32 mb-3" />
                  <Skeleton className="h-64 w-full rounded" />
                </div>
              </div>
            )}

            {/* Error state */}
            {pageState.status === 'error' && (() => {
              const isNoData = pageState.message.toLowerCase().includes('no historical data')
              return (
                <div className="rounded-xl border border-rose-500/30 bg-rose-500/5 p-6 flex flex-col gap-3">
                  <div className="flex items-center gap-2 text-rose-500">
                    <AlertTriangle className="w-5 h-5" />
                    <span className="font-semibold text-sm">Backtest failed</span>
                  </div>
                  <p className="text-sm text-foreground/80">{pageState.message}</p>
                  {isNoData ? (
                    <Button
                      className="w-fit h-10 gap-2 font-semibold"
                      onClick={handleDownloadAndRun}
                      disabled={downloading}
                    >
                      {downloading ? (
                        <Loader2 className="w-4 h-4 animate-spin motion-reduce:animate-none" />
                      ) : (
                        <Download className="w-4 h-4" />
                      )}
                      {downloading
                        ? 'Downloading…'
                        : `Download ${symbol.trim().toUpperCase()} ${barInterval} history`}
                    </Button>
                  ) : (
                    <Button variant="outline" size="sm" className="w-fit h-9 gap-2" onClick={handleRun}>
                      <RefreshCw className="w-3.5 h-3.5" />
                      Retry
                    </Button>
                  )}
                </div>
              )
            })()}

            {/* Success: results */}
            {result && (
              <>
                {/* Warnings */}
                {result.warnings.length > 0 && (
                  <div className="flex items-start gap-2 px-4 py-3 rounded-lg border border-amber-500/30 bg-amber-500/5 text-amber-600 dark:text-amber-400">
                    <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
                    <div className="space-y-0.5">
                      {result.warnings.map((w, i) => (
                        <p key={i} className="text-xs">{w}</p>
                      ))}
                    </div>
                  </div>
                )}

                {/* Summary metric cards */}
                {summaryCards && (
                  <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
                    {summaryCards.map((card) => (
                      <div key={card.label} className="rounded-xl border border-border bg-card px-4 py-3">
                        <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1">{card.label}</p>
                        <p className={cn(
                          'text-xl font-bold font-mono tabular-nums tracking-tight',
                          card.good ? 'text-emerald-500' : 'text-foreground'
                        )}>
                          {card.value}
                        </p>
                        {card.sub && <p className="text-[10px] text-muted-foreground mt-0.5">{card.sub}</p>}
                      </div>
                    ))}
                  </div>
                )}

                {/* Equity chart */}
                <Card className="rounded-xl border-border">
                  <CardHeader className="py-3 px-4 border-b border-border/60">
                    <div className="flex items-center justify-between">
                      <CardTitle className="text-sm font-semibold">Equity Curve</CardTitle>
                      <div className="flex items-center gap-2">
                        <Badge variant="outline" className="text-[10px] text-emerald-500 border-emerald-500/30">Strategy</Badge>
                        <span className="text-xs text-muted-foreground">B&H CAGR: {pct(result.benchmark.cagr)}</span>
                      </div>
                    </div>
                  </CardHeader>
                  <CardContent className="p-4">
                    <EquityChart equityCurve={result.equity_curve} />
                  </CardContent>
                </Card>

                {/* Drawdown chart */}
                <Card className="rounded-xl border-border">
                  <CardHeader className="py-3 px-4 border-b border-border/60">
                    <CardTitle className="text-sm font-semibold">Drawdown</CardTitle>
                  </CardHeader>
                  <CardContent className="p-4">
                    <DrawdownChart equityCurve={result.equity_curve} />
                  </CardContent>
                </Card>

                {/* Metrics table */}
                <Card className="rounded-xl border-border">
                  <CardHeader className="py-3 px-4 border-b border-border/60">
                    <CardTitle className="text-sm font-semibold">Full Metrics</CardTitle>
                  </CardHeader>
                  <CardContent className="p-4">
                    <MetricsTable metrics={result.metrics} benchmark={result.benchmark} />
                  </CardContent>
                </Card>

                {/* Trade list */}
                <Card className="rounded-xl border-border">
                  <CardHeader className="py-3 px-4 border-b border-border/60">
                    <div className="flex items-center justify-between">
                      <CardTitle className="text-sm font-semibold">
                        Trades
                        <span className="ml-2 text-xs text-muted-foreground font-normal">({result.trades.length})</span>
                      </CardTitle>
                    </div>
                  </CardHeader>
                  <CardContent className="p-4">
                    <TradeTable trades={result.trades} />
                  </CardContent>
                </Card>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
