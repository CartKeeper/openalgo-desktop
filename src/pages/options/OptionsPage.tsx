import { invoke } from '@tauri-apps/api/core'
import {
  ArrowLeft,
  Calculator,
  HelpCircle,
  Info,
  Loader2,
} from 'lucide-react'
import { useCallback, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface OptionGreeksResult {
  strike: number
  expiry: string
  option_type: string
  theoretical_price: number
  implied_volatility: number | null
  delta: number
  gamma: number
  theta: number
  vega: number
  rho: number
}

interface ChainParams {
  spotPrice: string
  expiryDays: string
  riskFreeRate: string
  volatility: string
  strikeFrom: string
  strikeTo: string
  strikeStep: string
}

interface SingleCalcParams {
  strike: string
  optionType: 'call' | 'put'
  marketPrice: string
}

type ChainState = 'idle' | 'loading' | 'loaded' | 'error'
type SingleState = 'idle' | 'loading' | 'loaded' | 'error'

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

function fmtPrice(v: number): string {
  return v.toFixed(2)
}

function fmtGreek(v: number): string {
  return v.toFixed(4)
}

function fmtIV(v: number | null): string {
  if (v === null || v === undefined) return '--'
  return `${(v * 100).toFixed(2)}%`
}

// ---------------------------------------------------------------------------
// Greek tooltip descriptions
// ---------------------------------------------------------------------------

const GREEK_TOOLTIPS: Record<string, string> = {
  delta: 'Rate of change of option price per $1 move in the underlying.',
  gamma: 'Rate of change of delta per $1 move in the underlying.',
  theta: 'Daily time decay — how much value the option loses per day.',
  vega: 'Sensitivity of option price to a 1% change in implied volatility.',
  rho: 'Sensitivity of option price to a 1% change in interest rates.',
  iv: 'The market-implied volatility derived from the option\'s market price.',
  theoretical_price: 'Theoretical fair value computed via Black-Scholes.',
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function GreekLabel({ label, tooltipKey }: { label: string; tooltipKey: string }) {
  return (
    <div className="flex items-center gap-1">
      <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        {label}
      </span>
      <Tooltip>
        <TooltipTrigger asChild>
          <Info className="h-3 w-3 text-muted-foreground cursor-help" />
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-[240px]">
          {GREEK_TOOLTIPS[tooltipKey] ?? ''}
        </TooltipContent>
      </Tooltip>
    </div>
  )
}

function ChainSkeletonRows({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: count }).map((_, i) => (
        <TableRow key={i}>
          {Array.from({ length: 11 }).map((_, j) => (
            <TableCell key={j}>
              <Skeleton className="h-4 w-14" />
            </TableCell>
          ))}
        </TableRow>
      ))}
    </>
  )
}

function EmptyChainMessage() {
  return (
    <div className="flex flex-col items-center justify-center py-12 gap-3 text-center">
      <Calculator className="h-12 w-12 text-muted-foreground/40" />
      <p className="text-sm text-muted-foreground">
        Enter parameters above to calculate the options chain
      </p>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

function greekColor(value: number, alwaysNegative = false): string {
  if (alwaysNegative) return 'text-red-500'
  if (value > 0) return 'text-emerald-500'
  if (value < 0) return 'text-red-500'
  return 'text-muted-foreground'
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function OptionsPage() {
  // Chain params
  const [chainParams, setChainParams] = useState<ChainParams>({
    spotPrice: '',
    expiryDays: '',
    riskFreeRate: '4.5',
    volatility: '30',
    strikeFrom: '',
    strikeTo: '',
    strikeStep: '5',
  })

  // Chain data
  const [chainState, setChainState] = useState<ChainState>('idle')
  const [chainError, setChainError] = useState('')
  const [callData, setCallData] = useState<OptionGreeksResult[]>([])
  const [putData, setPutData] = useState<OptionGreeksResult[]>([])
  const [strikes, setStrikes] = useState<number[]>([])

  // Single calc params
  const [singleParams, setSingleParams] = useState<SingleCalcParams>({
    strike: '',
    optionType: 'call',
    marketPrice: '',
  })
  const [singleState, setSingleState] = useState<SingleState>('idle')
  const [singleError, setSingleError] = useState('')
  const [singleResult, setSingleResult] = useState<OptionGreeksResult | null>(null)

  // ---------------------------
  // Chain calculation
  // ---------------------------

  const generateStrikes = useCallback((): number[] => {
    const from = parseFloat(chainParams.strikeFrom)
    const to = parseFloat(chainParams.strikeTo)
    const step = parseFloat(chainParams.strikeStep)

    if (isNaN(from) || isNaN(to) || isNaN(step) || step <= 0 || from > to) {
      return []
    }

    const result: number[] = []
    for (let s = from; s <= to; s += step) {
      result.push(parseFloat(s.toFixed(4)))
    }

    // Safety cap
    if (result.length > 200) {
      return result.slice(0, 200)
    }

    return result
  }, [chainParams.strikeFrom, chainParams.strikeTo, chainParams.strikeStep])

  const calculateChain = useCallback(async () => {
    const spotPrice = parseFloat(chainParams.spotPrice)
    const expiryDays = parseFloat(chainParams.expiryDays)
    const riskFreeRate = parseFloat(chainParams.riskFreeRate) / 100
    const volatility = parseFloat(chainParams.volatility) / 100

    if (isNaN(spotPrice) || spotPrice <= 0) {
      toast.error('Spot price must be a positive number')
      return
    }
    if (isNaN(expiryDays) || expiryDays <= 0) {
      toast.error('Days to expiry must be a positive number')
      return
    }
    if (isNaN(riskFreeRate)) {
      toast.error('Risk-free rate must be a valid number')
      return
    }
    if (isNaN(volatility) || volatility <= 0) {
      toast.error('Implied volatility must be a positive number')
      return
    }

    const generatedStrikes = generateStrikes()
    if (generatedStrikes.length === 0) {
      toast.error('Invalid strike range. Ensure From <= To and Step > 0')
      return
    }

    setChainState('loading')
    setChainError('')

    try {
      const results = await invoke<OptionGreeksResult[]>('compute_greeks_batch', {
        spot_price: spotPrice,
        strikes: generatedStrikes,
        expiry_days: expiryDays,
        risk_free_rate: riskFreeRate,
        volatility: volatility,
      })

      // Split results into calls and puts
      const calls: OptionGreeksResult[] = []
      const puts: OptionGreeksResult[] = []

      for (const r of results) {
        if (r.option_type === 'call') {
          calls.push(r)
        } else {
          puts.push(r)
        }
      }

      // Sort both by strike ascending
      calls.sort((a, b) => a.strike - b.strike)
      puts.sort((a, b) => a.strike - b.strike)

      setCallData(calls)
      setPutData(puts)
      setStrikes(generatedStrikes)
      setChainState('loaded')
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      setChainError(msg)
      setChainState('error')
      toast.error('Failed to compute options chain')
    }
  }, [chainParams, generateStrikes])

  // ---------------------------
  // Single option calculation
  // ---------------------------

  const calculateSingle = useCallback(async () => {
    const spotPrice = parseFloat(chainParams.spotPrice)
    const expiryDays = parseFloat(chainParams.expiryDays)
    const riskFreeRate = parseFloat(chainParams.riskFreeRate) / 100
    const volatility = parseFloat(chainParams.volatility) / 100
    const strike = parseFloat(singleParams.strike)
    const marketPrice = singleParams.marketPrice ? parseFloat(singleParams.marketPrice) : null

    if (isNaN(spotPrice) || spotPrice <= 0) {
      toast.error('Set a valid spot price in the chain parameters above')
      return
    }
    if (isNaN(expiryDays) || expiryDays <= 0) {
      toast.error('Set valid days to expiry in the chain parameters above')
      return
    }
    if (isNaN(strike) || strike <= 0) {
      toast.error('Strike price must be a positive number')
      return
    }
    if (marketPrice !== null && (isNaN(marketPrice) || marketPrice <= 0)) {
      toast.error('Market price must be a positive number or left empty')
      return
    }

    setSingleState('loading')
    setSingleError('')

    try {
      const result = await invoke<OptionGreeksResult>('compute_greeks', {
        spot_price: spotPrice,
        strike: strike,
        expiry_days: expiryDays,
        risk_free_rate: riskFreeRate,
        volatility: volatility,
        is_call: singleParams.optionType === 'call',
        market_price: marketPrice,
      })

      setSingleResult(result)
      setSingleState('loaded')
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      setSingleError(msg)
      setSingleState('error')
      toast.error('Failed to compute Greeks')
    }
  }, [chainParams, singleParams])

  // ---------------------------
  // Helpers for chain rendering
  // ---------------------------

  const spotPrice = parseFloat(chainParams.spotPrice) || 0

  function findATMStrike(): number | null {
    if (strikes.length === 0 || spotPrice <= 0) return null
    let closest = strikes[0]
    let minDiff = Math.abs(strikes[0] - spotPrice)
    for (const s of strikes) {
      const diff = Math.abs(s - spotPrice)
      if (diff < minDiff) {
        minDiff = diff
        closest = s
      }
    }
    return closest
  }

  const atmStrike = findATMStrike()

  function getCallForStrike(strike: number): OptionGreeksResult | undefined {
    return callData.find((c) => c.strike === strike)
  }

  function getPutForStrike(strike: number): OptionGreeksResult | undefined {
    return putData.find((p) => p.strike === strike)
  }

  function isCallITM(strike: number): boolean {
    return spotPrice > 0 && strike < spotPrice
  }

  function isPutITM(strike: number): boolean {
    return spotPrice > 0 && strike > spotPrice
  }

  // ---------------------------
  // Render
  // ---------------------------

  return (
    <div className="py-6 max-w-7xl mx-auto space-y-6 px-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Calculator className="h-6 w-6" />
            Options Calculator
          </h1>
        </div>
        <p className="text-muted-foreground">
          Black-Scholes options chain and single-option Greeks calculator
        </p>
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Input Section                                                      */}
      {/* ----------------------------------------------------------------- */}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Chain Parameters
            <Tooltip>
              <TooltipTrigger asChild>
                <HelpCircle className="h-4 w-4 text-muted-foreground cursor-help" />
              </TooltipTrigger>
              <TooltipContent side="right" className="max-w-[280px]">
                Configure the underlying price, expiry, and strike range to generate
                a full options chain with Greeks.
              </TooltipContent>
            </Tooltip>
          </CardTitle>
          <CardDescription>
            Set underlying parameters and strike range to compute the options chain
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {/* Spot Price */}
            <div className="space-y-1.5">
              <Label htmlFor="spot-price">Spot Price</Label>
              <Input
                id="spot-price"
                type="number"
                placeholder="e.g. 175.00"
                min={0}
                step="0.01"
                value={chainParams.spotPrice}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, spotPrice: e.target.value }))
                }
              />
            </div>

            {/* Days to Expiry */}
            <div className="space-y-1.5">
              <Label htmlFor="expiry-days">Days to Expiry</Label>
              <Input
                id="expiry-days"
                type="number"
                placeholder="e.g. 30"
                min={1}
                step="1"
                value={chainParams.expiryDays}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, expiryDays: e.target.value }))
                }
              />
            </div>

            {/* Risk-Free Rate */}
            <div className="space-y-1.5">
              <Label htmlFor="risk-free-rate">Risk-Free Rate (%)</Label>
              <Input
                id="risk-free-rate"
                type="number"
                placeholder="4.5"
                step="0.1"
                value={chainParams.riskFreeRate}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, riskFreeRate: e.target.value }))
                }
              />
            </div>

            {/* Implied Volatility */}
            <div className="space-y-1.5">
              <Label htmlFor="volatility">Implied Volatility (%)</Label>
              <Input
                id="volatility"
                type="number"
                placeholder="30"
                step="0.5"
                value={chainParams.volatility}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, volatility: e.target.value }))
                }
              />
            </div>
          </div>

          {/* Strike Range */}
          <div className="mt-4 grid grid-cols-1 sm:grid-cols-4 gap-4 items-end">
            <div className="space-y-1.5">
              <Label htmlFor="strike-from">Strike From</Label>
              <Input
                id="strike-from"
                type="number"
                placeholder="150"
                min={0}
                step="1"
                value={chainParams.strikeFrom}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, strikeFrom: e.target.value }))
                }
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="strike-to">Strike To</Label>
              <Input
                id="strike-to"
                type="number"
                placeholder="200"
                min={0}
                step="1"
                value={chainParams.strikeTo}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, strikeTo: e.target.value }))
                }
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="strike-step">Step</Label>
              <Input
                id="strike-step"
                type="number"
                placeholder="5"
                min={0.01}
                step="0.5"
                value={chainParams.strikeStep}
                onChange={(e) =>
                  setChainParams((p) => ({ ...p, strikeStep: e.target.value }))
                }
              />
            </div>
            <Button
              onClick={calculateChain}
              disabled={chainState === 'loading'}
              className="h-9"
            >
              {chainState === 'loading' ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Calculating...
                </>
              ) : (
                'Calculate Chain'
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* ----------------------------------------------------------------- */}
      {/* Options Chain Table                                                */}
      {/* ----------------------------------------------------------------- */}

      <Card>
        <CardHeader>
          <CardTitle>Options Chain</CardTitle>
          <CardDescription>
            Calls on the left, puts on the right. ITM options are highlighted.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {chainState === 'idle' && <EmptyChainMessage />}

          {chainState === 'error' && (
            <div className="flex flex-col items-center justify-center py-12 gap-3 text-center">
              <div className="text-red-500 text-sm font-medium">
                {chainError || 'An unexpected error occurred'}
              </div>
              <Button variant="outline" size="sm" onClick={calculateChain}>
                Retry
              </Button>
            </div>
          )}

          {(chainState === 'loading' || chainState === 'loaded') && (
            <div className="overflow-x-auto -mx-6">
              <div className="min-w-[900px] px-6">
                <Table>
                  <TableHeader>
                    <TableRow>
                      {/* Call headers */}
                      <TableHead className="text-right text-xs uppercase tracking-wider">Price</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Delta</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Gamma</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Theta</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Vega</TableHead>

                      {/* Strike center */}
                      <TableHead className="text-center text-xs uppercase tracking-wider font-bold border-x border-border">
                        Strike
                      </TableHead>

                      {/* Put headers */}
                      <TableHead className="text-right text-xs uppercase tracking-wider">Price</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Delta</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Gamma</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Theta</TableHead>
                      <TableHead className="text-right text-xs uppercase tracking-wider">Vega</TableHead>
                    </TableRow>

                    {/* Section labels row */}
                    <TableRow>
                      <TableHead colSpan={5} className="text-center text-xs font-semibold text-emerald-500 py-1">
                        CALLS
                      </TableHead>
                      <TableHead className="border-x border-border" />
                      <TableHead colSpan={5} className="text-center text-xs font-semibold text-red-500 py-1">
                        PUTS
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {chainState === 'loading' && <ChainSkeletonRows count={8} />}

                    {chainState === 'loaded' &&
                      strikes.map((strike) => {
                        const call = getCallForStrike(strike)
                        const put = getPutForStrike(strike)
                        const callITM = isCallITM(strike)
                        const putITM = isPutITM(strike)
                        const isATM = strike === atmStrike

                        return (
                          <TableRow
                            key={strike}
                            className={
                              isATM
                                ? 'border-y-2 border-primary/40 bg-primary/5'
                                : ''
                            }
                          >
                            {/* Call side */}
                            <TableCell
                              className={`text-right tabular-nums ${callITM ? 'bg-emerald-500/8' : ''}`}
                            >
                              {call ? fmtPrice(call.theoretical_price) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${callITM ? 'bg-emerald-500/8' : ''} ${call ? greekColor(call.delta) : ''}`}
                            >
                              {call ? fmtGreek(call.delta) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${callITM ? 'bg-emerald-500/8' : ''}`}
                            >
                              {call ? fmtGreek(call.gamma) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${callITM ? 'bg-emerald-500/8' : ''} ${call ? greekColor(call.theta, true) : ''}`}
                            >
                              {call ? fmtGreek(call.theta) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${callITM ? 'bg-emerald-500/8' : ''}`}
                            >
                              {call ? fmtGreek(call.vega) : '--'}
                            </TableCell>

                            {/* Strike center */}
                            <TableCell
                              className={`text-center font-semibold tabular-nums border-x border-border ${
                                isATM ? 'text-primary font-bold' : ''
                              }`}
                            >
                              {fmtPrice(strike)}
                              {isATM && (
                                <span className="ml-1 text-[10px] font-semibold text-primary/60 uppercase">
                                  ATM
                                </span>
                              )}
                            </TableCell>

                            {/* Put side */}
                            <TableCell
                              className={`text-right tabular-nums ${putITM ? 'bg-red-500/8' : ''}`}
                            >
                              {put ? fmtPrice(put.theoretical_price) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${putITM ? 'bg-red-500/8' : ''} ${put ? greekColor(put.delta) : ''}`}
                            >
                              {put ? fmtGreek(put.delta) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${putITM ? 'bg-red-500/8' : ''}`}
                            >
                              {put ? fmtGreek(put.gamma) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${putITM ? 'bg-red-500/8' : ''} ${put ? greekColor(put.theta, true) : ''}`}
                            >
                              {put ? fmtGreek(put.theta) : '--'}
                            </TableCell>
                            <TableCell
                              className={`text-right tabular-nums ${putITM ? 'bg-red-500/8' : ''}`}
                            >
                              {put ? fmtGreek(put.vega) : '--'}
                            </TableCell>
                          </TableRow>
                        )
                      })}
                  </TableBody>
                </Table>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* ----------------------------------------------------------------- */}
      {/* Single Option Calculator                                           */}
      {/* ----------------------------------------------------------------- */}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Single Option Calculator
            <Tooltip>
              <TooltipTrigger asChild>
                <HelpCircle className="h-4 w-4 text-muted-foreground cursor-help" />
              </TooltipTrigger>
              <TooltipContent side="right" className="max-w-[280px]">
                Compute Greeks for a single option. Provide a market price to
                calculate implied volatility.
              </TooltipContent>
            </Tooltip>
          </CardTitle>
          <CardDescription>
            Compute individual option Greeks with optional IV calculation
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 items-end">
            {/* Strike */}
            <div className="space-y-1.5">
              <Label htmlFor="single-strike">Strike Price</Label>
              <Input
                id="single-strike"
                type="number"
                placeholder="e.g. 175"
                min={0}
                step="0.01"
                value={singleParams.strike}
                onChange={(e) =>
                  setSingleParams((p) => ({ ...p, strike: e.target.value }))
                }
              />
            </div>

            {/* Option Type Toggle */}
            <div className="space-y-1.5">
              <Label>Option Type</Label>
              <div className="flex h-9 rounded-md border border-input overflow-hidden">
                <button
                  type="button"
                  className={`flex-1 text-sm font-medium transition-colors ${
                    singleParams.optionType === 'call'
                      ? 'bg-emerald-500 text-white'
                      : 'bg-transparent text-muted-foreground hover:text-foreground'
                  }`}
                  onClick={() =>
                    setSingleParams((p) => ({ ...p, optionType: 'call' }))
                  }
                >
                  Call
                </button>
                <button
                  type="button"
                  className={`flex-1 text-sm font-medium transition-colors border-l border-input ${
                    singleParams.optionType === 'put'
                      ? 'bg-red-500 text-white'
                      : 'bg-transparent text-muted-foreground hover:text-foreground'
                  }`}
                  onClick={() =>
                    setSingleParams((p) => ({ ...p, optionType: 'put' }))
                  }
                >
                  Put
                </button>
              </div>
            </div>

            {/* Market Price (optional) */}
            <div className="space-y-1.5">
              <Label htmlFor="market-price">
                Market Price
                <span className="text-muted-foreground font-normal ml-1">(optional)</span>
              </Label>
              <Input
                id="market-price"
                type="number"
                placeholder="For IV calc"
                min={0}
                step="0.01"
                value={singleParams.marketPrice}
                onChange={(e) =>
                  setSingleParams((p) => ({ ...p, marketPrice: e.target.value }))
                }
              />
            </div>

            {/* Calculate */}
            <Button
              onClick={calculateSingle}
              disabled={singleState === 'loading'}
              className="h-9"
            >
              {singleState === 'loading' ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Calculating...
                </>
              ) : (
                'Calculate Greeks'
              )}
            </Button>
          </div>

          {/* Single calc results */}
          {singleState === 'error' && (
            <div className="mt-4 text-red-500 text-sm font-medium">
              {singleError || 'An unexpected error occurred'}
            </div>
          )}

          {singleState === 'loading' && (
            <div className="mt-6 grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-7 gap-4">
              {Array.from({ length: 7 }).map((_, i) => (
                <div key={i} className="space-y-2">
                  <Skeleton className="h-3 w-16" />
                  <Skeleton className="h-6 w-20" />
                </div>
              ))}
            </div>
          )}

          {singleState === 'loaded' && singleResult && (
            <div className="mt-6 grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-7 gap-4">
              {/* Theoretical Price */}
              <div className="space-y-1">
                <GreekLabel label="Price" tooltipKey="theoretical_price" />
                <div className="text-lg font-bold tabular-nums">
                  {fmtPrice(singleResult.theoretical_price)}
                </div>
              </div>

              {/* IV */}
              <div className="space-y-1">
                <GreekLabel label="IV" tooltipKey="iv" />
                <div className="text-lg font-bold tabular-nums">
                  {fmtIV(singleResult.implied_volatility)}
                </div>
              </div>

              {/* Delta */}
              <div className="space-y-1">
                <GreekLabel label="Delta" tooltipKey="delta" />
                <div
                  className={`text-lg font-bold tabular-nums ${greekColor(singleResult.delta)}`}
                >
                  {fmtGreek(singleResult.delta)}
                </div>
              </div>

              {/* Gamma */}
              <div className="space-y-1">
                <GreekLabel label="Gamma" tooltipKey="gamma" />
                <div className="text-lg font-bold tabular-nums">
                  {fmtGreek(singleResult.gamma)}
                </div>
              </div>

              {/* Theta */}
              <div className="space-y-1">
                <GreekLabel label="Theta" tooltipKey="theta" />
                <div
                  className={`text-lg font-bold tabular-nums ${greekColor(singleResult.theta, true)}`}
                >
                  {fmtGreek(singleResult.theta)}
                </div>
              </div>

              {/* Vega */}
              <div className="space-y-1">
                <GreekLabel label="Vega" tooltipKey="vega" />
                <div className="text-lg font-bold tabular-nums">
                  {fmtGreek(singleResult.vega)}
                </div>
              </div>

              {/* Rho */}
              <div className="space-y-1">
                <GreekLabel label="Rho" tooltipKey="rho" />
                <div
                  className={`text-lg font-bold tabular-nums ${greekColor(singleResult.rho)}`}
                >
                  {fmtGreek(singleResult.rho)}
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
