import { invoke } from '@tauri-apps/api/core'
import {
  Activity,
  AlertTriangle,
  ArrowDownRight,
  ArrowRight,
  ArrowUpRight,
  Bot,
  Loader2,
  RefreshCw,
  ShoppingCart,
  TrendingDown,
  TrendingUp,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { providerCommands } from '@/api/tauri-client'
import { PlaceOrderDialog } from '@/components/trading/PlaceOrderDialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface IndicatorPoint {
  timestamp: string
  value: number
  secondaryValue: number | null
  tertiaryValue: number | null
}

interface IndicatorResult {
  name: string
  values: IndicatorPoint[]
}

interface TechnicalSignals {
  sma20: number | null
  sma50: number | null
  sma200: number | null
  rsi: number | null
  macdLine: number | null
  macdSignal: number | null
  macdHistogram: number | null
  currentPrice: number
  // Derived signals
  sma20Signal: 'bullish' | 'bearish' | 'neutral'
  sma50Signal: 'bullish' | 'bearish' | 'neutral'
  sma200Signal: 'bullish' | 'bearish' | 'neutral'
  goldenCross: boolean
  deathCross: boolean
  rsiSignal: 'oversold' | 'overbought' | 'neutral'
  macdSignalType: 'bullish' | 'bearish' | 'neutral'
  overallSentiment: 'strong_buy' | 'buy' | 'neutral' | 'sell' | 'strong_sell'
}

interface TrendData {
  dailyRate: number
  projections: { label: string; days: number; price: number; changePct: number }[]
  r2: number
}

interface CopilotResponse {
  responseText: string
  toolCallsMade: { toolName: string; summary: string }[]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatPrice(val: number | null | undefined): string {
  if (val == null) return '--'
  return val.toLocaleString('en-US', { style: 'currency', currency: 'USD' })
}

function lastValue(result: IndicatorResult | undefined): number | null {
  if (!result || result.values.length === 0) return null
  return result.values[result.values.length - 1].value
}

function lastSecondary(result: IndicatorResult | undefined): number | null {
  if (!result || result.values.length === 0) return null
  return result.values[result.values.length - 1].secondaryValue ?? null
}

function lastTertiary(result: IndicatorResult | undefined): number | null {
  if (!result || result.values.length === 0) return null
  return result.values[result.values.length - 1].tertiaryValue ?? null
}

function computeSignals(
  indicators: IndicatorResult[],
  currentPrice: number
): TechnicalSignals {
  const sma20Result = indicators.find((i) => i.name === 'SMA_20')
  const sma50Result = indicators.find((i) => i.name === 'SMA_50')
  const sma200Result = indicators.find((i) => i.name === 'SMA_200')
  const rsiResult = indicators.find((i) => i.name === 'RSI_14')
  const macdResult = indicators.find((i) => i.name === 'MACD_12_26')

  const sma20 = lastValue(sma20Result)
  const sma50 = lastValue(sma50Result)
  const sma200 = lastValue(sma200Result)
  const rsi = lastValue(rsiResult)
  const macdLine = lastValue(macdResult)
  const macdSignal = lastSecondary(macdResult)
  const macdHistogram = lastTertiary(macdResult)

  // Price vs SMA signals
  const sma20Signal = sma20 != null ? (currentPrice > sma20 ? 'bullish' : 'bearish') : 'neutral'
  const sma50Signal = sma50 != null ? (currentPrice > sma50 ? 'bullish' : 'bearish') : 'neutral'
  const sma200Signal = sma200 != null ? (currentPrice > sma200 ? 'bullish' : 'bearish') : 'neutral'

  // Golden/Death cross (50 vs 200)
  const goldenCross = sma50 != null && sma200 != null && sma50 > sma200
  const deathCross = sma50 != null && sma200 != null && sma50 < sma200

  // RSI signal
  const rsiSignal = rsi != null ? (rsi < 30 ? 'oversold' : rsi > 70 ? 'overbought' : 'neutral') : 'neutral'

  // MACD signal
  const macdSignalType =
    macdLine != null && macdSignal != null
      ? macdLine > macdSignal ? 'bullish' : 'bearish'
      : 'neutral'

  // Overall sentiment scoring
  let score = 0
  if (sma20Signal === 'bullish') score++; else if (sma20Signal === 'bearish') score--
  if (sma50Signal === 'bullish') score++; else if (sma50Signal === 'bearish') score--
  if (sma200Signal === 'bullish') score++; else if (sma200Signal === 'bearish') score--
  if (goldenCross) score++; if (deathCross) score--
  if (rsiSignal === 'oversold') score++; else if (rsiSignal === 'overbought') score--
  if (macdSignalType === 'bullish') score++; else if (macdSignalType === 'bearish') score--

  let overallSentiment: TechnicalSignals['overallSentiment']
  if (score >= 4) overallSentiment = 'strong_buy'
  else if (score >= 2) overallSentiment = 'buy'
  else if (score <= -4) overallSentiment = 'strong_sell'
  else if (score <= -2) overallSentiment = 'sell'
  else overallSentiment = 'neutral'

  return {
    sma20, sma50, sma200, rsi,
    macdLine, macdSignal, macdHistogram,
    currentPrice,
    sma20Signal, sma50Signal, sma200Signal,
    goldenCross, deathCross,
    rsiSignal, macdSignalType,
    overallSentiment,
  }
}

function computeTrend(
  historicalData: [string, number, number, number, number, number][],
  currentPrice: number
): TrendData | null {
  if (historicalData.length < 20) return null
  const closes = historicalData.map((d) => d[4])
  const n = closes.length

  let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0, sumY2 = 0
  for (let i = 0; i < n; i++) {
    sumX += i; sumY += closes[i]; sumXY += i * closes[i]; sumX2 += i * i; sumY2 += closes[i] * closes[i]
  }
  const slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX)
  const meanY = sumY / n
  let ssTot = 0, ssRes = 0
  const intercept = (sumY - slope * sumX) / n
  for (let i = 0; i < n; i++) {
    ssTot += (closes[i] - meanY) ** 2
    ssRes += (closes[i] - (slope * i + intercept)) ** 2
  }
  const r2 = ssTot > 0 ? 1 - ssRes / ssTot : 0

  const horizons = [
    { label: '1 Month', days: 21 },
    { label: '3 Months', days: 63 },
    { label: '6 Months', days: 126 },
  ]
  const projections = horizons.map((h) => {
    const projected = currentPrice + slope * h.days
    const changePct = currentPrice > 0 ? ((projected - currentPrice) / currentPrice) * 100 : 0
    return { ...h, price: Math.max(0, projected), changePct }
  })

  return { dailyRate: slope, projections, r2 }
}

const sentimentConfig = {
  strong_buy: { label: 'Strong Buy', color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/30', icon: ArrowUpRight },
  buy: { label: 'Buy', color: 'text-green-500', bg: 'bg-green-500/10 border-green-500/20', icon: TrendingUp },
  neutral: { label: 'Neutral', color: 'text-yellow-500', bg: 'bg-yellow-500/10 border-yellow-500/20', icon: ArrowRight },
  sell: { label: 'Sell', color: 'text-red-500', bg: 'bg-red-500/10 border-red-500/20', icon: TrendingDown },
  strong_sell: { label: 'Strong Sell', color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/30', icon: ArrowDownRight },
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function SignalRow({
  label,
  value,
  signal,
}: {
  label: string
  value: string
  signal: 'bullish' | 'bearish' | 'neutral'
}) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium tabular-nums">{value}</span>
        <Badge
          variant="outline"
          className={cn(
            'text-[10px] px-1.5 py-0',
            signal === 'bullish' && 'text-green-500 border-green-500/30',
            signal === 'bearish' && 'text-red-500 border-red-500/30',
            signal === 'neutral' && 'text-yellow-500 border-yellow-500/30'
          )}
        >
          {signal === 'bullish' ? 'Bullish' : signal === 'bearish' ? 'Bearish' : 'Neutral'}
        </Badge>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

type TrendRange = '6mo' | '1y' | '5y'

const rangeLabels: Record<TrendRange, string> = {
  '6mo': '6 Months',
  '1y': '1 Year',
  '5y': '5 Years',
}

export function AnalysisTab({ symbol }: { symbol: string }) {
  const [signals, setSignals] = useState<TechnicalSignals | null>(null)
  const [trend, setTrend] = useState<TrendData | null>(null)
  const [trendRange, setTrendRange] = useState<TrendRange>('6mo')
  const [aiAnalysis, setAiAnalysis] = useState<string | null>(null)
  const [isLoadingSignals, setIsLoadingSignals] = useState(false)
  const [isLoadingTrend, setIsLoadingTrend] = useState(false)
  const [isLoadingAi, setIsLoadingAi] = useState(false)
  const [aiError, setAiError] = useState<string | null>(null)

  const fetchTechnicals = useCallback(async () => {
    setIsLoadingSignals(true)
    try {
      // Fetch indicators and historical data in parallel
      const [indicatorResults, historicalData] = await Promise.all([
        invoke<IndicatorResult[]>('compute_indicators', {
          symbol,
          exchange: 'US',
          timeframe: '1d',
          indicators: [
            { name: 'SMA', params: { period: 20 } },
            { name: 'SMA', params: { period: 50 } },
            { name: 'SMA', params: { period: 200 } },
            { name: 'RSI', params: { period: 14 } },
            { name: 'MACD', params: { fast: 12, slow: 26, signal: 9 } },
          ],
        }),
        providerCommands.getYahooHistorical(symbol, '1d', '6mo'),
      ])

      // Get current price from historical data
      const latestClose = historicalData.length > 0 ? historicalData[historicalData.length - 1][4] : 0

      const computed = computeSignals(indicatorResults, latestClose)
      setSignals(computed)

      // Also compute initial trend from the same 6mo data
      const trendData = computeTrend(historicalData, latestClose)
      setTrend(trendData)
    } catch (err) {
      console.error('Failed to fetch technicals:', err)
    } finally {
      setIsLoadingSignals(false)
    }
  }, [symbol])

  const fetchTrend = useCallback(async (range: TrendRange) => {
    setIsLoadingTrend(true)
    try {
      const historicalData = await providerCommands.getYahooHistorical(symbol, '1d', range)
      const latestClose = historicalData.length > 0 ? historicalData[historicalData.length - 1][4] : 0
      const trendData = computeTrend(historicalData, latestClose)
      setTrend(trendData)
    } catch (err) {
      console.error('Failed to fetch trend:', err)
    } finally {
      setIsLoadingTrend(false)
    }
  }, [symbol])

  const fetchAiAnalysis = useCallback(async () => {
    if (!signals) return

    setIsLoadingAi(true)
    setAiError(null)
    try {
      // Build a structured prompt with all the technical data
      const prompt = buildAnalysisPrompt(symbol, signals, trend)

      const response = await invoke<CopilotResponse>('copilot_send_message', {
        message: prompt,
        conversationHistoryJson: '[]',
      })

      setAiAnalysis(response.responseText)
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.includes('not configured') || msg.includes('API key')) {
        setAiError('Anthropic API key not configured. Add it in Data Providers settings.')
      } else {
        setAiError('Failed to generate AI analysis. Try again.')
      }
      console.error('AI analysis failed:', err)
    } finally {
      setIsLoadingAi(false)
    }
  }, [symbol, signals, trend])

  // Fetch technicals when symbol changes
  useEffect(() => {
    setTrendRange('6mo') // Reset range on symbol change
    fetchTechnicals()
  }, [fetchTechnicals])

  // Refetch trend when range changes (skip initial '6mo' since fetchTechnicals handles it)
  const handleRangeChange = useCallback((range: TrendRange) => {
    setTrendRange(range)
    fetchTrend(range)
  }, [fetchTrend])

  // AI analysis is triggered manually via the Analyze button

  if (isLoadingSignals) {
    return (
      <div className="space-y-4 mt-4">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <Card><CardContent className="p-4 space-y-3">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-3/4" />
          </CardContent></Card>
          <Card><CardContent className="p-4 space-y-3">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-8 w-24" />
            <Skeleton className="h-3 w-full" />
          </CardContent></Card>
        </div>
        <Card><CardContent className="p-4 space-y-3">
          <Skeleton className="h-4 w-48" />
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-2/3" />
        </CardContent></Card>
      </div>
    )
  }

  if (!signals) {
    return (
      <div className="text-center py-12 text-muted-foreground mt-4">
        <AlertTriangle className="h-8 w-8 mx-auto mb-2 opacity-40" />
        <p className="text-sm">Unable to load technical data for {symbol}</p>
        <Button variant="ghost" size="sm" className="mt-2" onClick={fetchTechnicals}>
          <RefreshCw className="h-3 w-3 mr-1" /> Retry
        </Button>
      </div>
    )
  }

  const sentiment = sentimentConfig[signals.overallSentiment]
  const SentimentIcon = sentiment.icon

  return (
    <div className="space-y-4 mt-4">
      {/* Top row: Sentiment + Trade action */}
      <div className="flex items-center justify-between">
        <div className={cn('flex items-center gap-2 rounded-lg border px-4 py-2', sentiment.bg)}>
          <SentimentIcon className={cn('h-5 w-5', sentiment.color)} />
          <div>
            <div className={cn('text-sm font-semibold', sentiment.color)}>{sentiment.label}</div>
            <div className="text-[10px] text-muted-foreground">Technical Sentiment</div>
          </div>
        </div>
        <PlaceOrderDialog
          defaultSymbol={symbol}
          trigger={
            <Button size="sm" className="bg-green-500 hover:bg-green-600 text-white">
              <ShoppingCart className="h-4 w-4 mr-2" />
              Trade {symbol}
            </Button>
          }
        />
      </div>

      {/* Technical signals + Trend projection */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* Technical Signals */}
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-1.5 mb-3">
              <Activity className="h-4 w-4 text-muted-foreground" />
              <h4 className="text-sm font-semibold">Technical Signals</h4>
            </div>
            <div className="divide-y divide-border">
              <SignalRow
                label="Price vs SMA 20"
                value={formatPrice(signals.sma20)}
                signal={signals.sma20Signal}
              />
              <SignalRow
                label="Price vs SMA 50"
                value={formatPrice(signals.sma50)}
                signal={signals.sma50Signal}
              />
              <SignalRow
                label="Price vs SMA 200"
                value={formatPrice(signals.sma200)}
                signal={signals.sma200Signal}
              />
              <SignalRow
                label="RSI (14)"
                value={signals.rsi != null ? signals.rsi.toFixed(1) : '--'}
                signal={signals.rsiSignal === 'oversold' ? 'bullish' : signals.rsiSignal === 'overbought' ? 'bearish' : 'neutral'}
              />
              <SignalRow
                label="MACD"
                value={signals.macdLine != null ? signals.macdLine.toFixed(2) : '--'}
                signal={signals.macdSignalType}
              />
              {(signals.goldenCross || signals.deathCross) && (
                <div className="flex items-center justify-between py-1.5">
                  <span className="text-xs text-muted-foreground">SMA Crossover</span>
                  <Badge
                    variant="outline"
                    className={cn(
                      'text-[10px] px-1.5 py-0',
                      signals.goldenCross ? 'text-green-500 border-green-500/30' : 'text-red-500 border-red-500/30'
                    )}
                  >
                    {signals.goldenCross ? 'Golden Cross' : 'Death Cross'}
                  </Badge>
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        {/* Trend Projection */}
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-1.5">
                {trend && trend.dailyRate >= 0 ? (
                  <TrendingUp className="h-4 w-4 text-green-500" />
                ) : (
                  <TrendingDown className="h-4 w-4 text-red-500" />
                )}
                <h4 className="text-sm font-semibold">Trend Projection</h4>
                {isLoadingTrend && <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />}
              </div>
              <div className="flex items-center gap-2">
                {/* Range toggle */}
                <div className="flex rounded-md border overflow-hidden">
                  {(['6mo', '1y', '5y'] as TrendRange[]).map((range) => (
                    <button
                      key={range}
                      onClick={() => handleRangeChange(range)}
                      disabled={isLoadingTrend}
                      className={cn(
                        'px-2 py-0.5 text-[10px] font-medium transition-colors',
                        trendRange === range
                          ? 'bg-primary text-primary-foreground'
                          : 'hover:bg-muted text-muted-foreground'
                      )}
                    >
                      {rangeLabels[range]}
                    </button>
                  ))}
                </div>
                {trend && (
                  <span className="text-[10px] text-muted-foreground">
                    R² {(trend.r2 * 100).toFixed(0)}%
                  </span>
                )}
              </div>
            </div>
            {trend ? (
              <>
                <div className="grid grid-cols-3 gap-2">
                  {trend.projections.map((p) => (
                    <div key={p.label} className="text-center rounded-lg bg-muted/50 border px-2 py-2">
                      <div className="text-[10px] text-muted-foreground font-medium">{p.label}</div>
                      <div className="text-sm font-semibold tabular-nums mt-0.5">{formatPrice(p.price)}</div>
                      <div
                        className={cn(
                          'text-[10px] font-medium tabular-nums',
                          p.changePct >= 0 ? 'text-green-500' : 'text-red-500'
                        )}
                      >
                        {p.changePct >= 0 ? '+' : ''}{p.changePct.toFixed(1)}%
                      </div>
                    </div>
                  ))}
                </div>
                <div className="mt-3 text-center">
                  <div className="text-xs text-muted-foreground">
                    Daily trend: <span className={cn('font-medium tabular-nums', trend.dailyRate >= 0 ? 'text-green-500' : 'text-red-500')}>
                      {trend.dailyRate >= 0 ? '+' : ''}{formatPrice(trend.dailyRate)}/day
                    </span>
                  </div>
                </div>
                <p className="text-[10px] text-muted-foreground text-center mt-2">
                  Linear trend from {rangeLabels[trendRange].toLowerCase()} of data · Speculative, not financial advice
                </p>
              </>
            ) : (
              <div className="text-sm text-muted-foreground text-center py-4">
                Insufficient data for projection
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* AI Analysis */}
      <Card>
        <CardContent className="p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-1.5">
              <Bot className="h-4 w-4 text-primary" />
              <h4 className="text-sm font-semibold">AI Analysis & Strategy</h4>
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 text-xs"
              onClick={fetchAiAnalysis}
              disabled={isLoadingAi}
            >
              {isLoadingAi ? (
                <Loader2 className="h-3 w-3 mr-1 animate-spin" />
              ) : (
                <RefreshCw className="h-3 w-3 mr-1" />
              )}
              {aiAnalysis ? 'Refresh' : 'Analyze'}
            </Button>
          </div>

          {isLoadingAi ? (
            <div className="space-y-2">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" />
                Analyzing {symbol} — pulling data and computing strategy...
              </div>
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-4/5" />
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-3/4" />
            </div>
          ) : aiError ? (
            <div className="text-sm text-muted-foreground text-center py-4">
              <AlertTriangle className="h-5 w-5 mx-auto mb-1 text-yellow-500" />
              {aiError}
            </div>
          ) : aiAnalysis ? (
            <div className="prose prose-sm dark:prose-invert max-w-none text-sm leading-relaxed">
              {aiAnalysis.split('\n').map((line, i) => {
                if (!line.trim()) return <br key={i} />
                if (line.startsWith('### ')) return <h3 key={i} className="text-sm font-semibold mt-3 mb-1">{line.replace('### ', '')}</h3>
                if (line.startsWith('## ')) return <h2 key={i} className="text-base font-semibold mt-4 mb-1">{line.replace('## ', '')}</h2>
                if (line.startsWith('**') && line.endsWith('**')) return <p key={i} className="font-semibold mt-2">{line.replace(/\*\*/g, '')}</p>
                if (line.startsWith('- ')) return <li key={i} className="ml-4 list-disc text-sm">{renderInlineBold(line.slice(2))}</li>
                return <p key={i} className="text-sm">{renderInlineBold(line)}</p>
              })}
            </div>
          ) : (
            <div className="text-sm text-muted-foreground text-center py-4">
              Click Analyze to get AI-powered insights and strategy recommendations.
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

// Simple inline bold renderer for **text**
function renderInlineBold(text: string) {
  const parts = text.split(/(\*\*[^*]+\*\*)/)
  return parts.map((part, i) => {
    if (part.startsWith('**') && part.endsWith('**')) {
      return <strong key={i}>{part.slice(2, -2)}</strong>
    }
    return <span key={i}>{part}</span>
  })
}

// ---------------------------------------------------------------------------
// AI Prompt Builder
// ---------------------------------------------------------------------------

function buildAnalysisPrompt(
  symbol: string,
  signals: TechnicalSignals,
  trend: TrendData | null
): string {
  const parts: string[] = []

  parts.push(`Analyze ${symbol} and provide actionable trading strategy recommendations.`)
  parts.push('')
  parts.push('Here is the current technical data:')
  parts.push(`- Current Price: $${signals.currentPrice.toFixed(2)}`)
  parts.push(`- SMA 20: ${signals.sma20 != null ? '$' + signals.sma20.toFixed(2) : 'N/A'} (Price ${signals.sma20Signal})`)
  parts.push(`- SMA 50: ${signals.sma50 != null ? '$' + signals.sma50.toFixed(2) : 'N/A'} (Price ${signals.sma50Signal})`)
  parts.push(`- SMA 200: ${signals.sma200 != null ? '$' + signals.sma200.toFixed(2) : 'N/A'} (Price ${signals.sma200Signal})`)
  if (signals.goldenCross) parts.push('- SMA 50/200: GOLDEN CROSS (bullish)')
  if (signals.deathCross) parts.push('- SMA 50/200: DEATH CROSS (bearish)')
  parts.push(`- RSI (14): ${signals.rsi != null ? signals.rsi.toFixed(1) : 'N/A'} (${signals.rsiSignal})`)
  parts.push(`- MACD: ${signals.macdLine != null ? signals.macdLine.toFixed(2) : 'N/A'}, Signal: ${signals.macdSignal != null ? signals.macdSignal.toFixed(2) : 'N/A'} (${signals.macdSignalType})`)
  parts.push(`- Overall Technical Sentiment: ${signals.overallSentiment.replace('_', ' ').toUpperCase()}`)

  if (trend) {
    parts.push('')
    parts.push('6-Month Linear Trend:')
    parts.push(`- Daily rate: $${trend.dailyRate.toFixed(2)}/day`)
    parts.push(`- R² fit: ${(trend.r2 * 100).toFixed(1)}%`)
    for (const p of trend.projections) {
      parts.push(`- ${p.label} projection: $${p.price.toFixed(2)} (${p.changePct >= 0 ? '+' : ''}${p.changePct.toFixed(1)}%)`)
    }
  }

  parts.push('')
  parts.push('IMPORTANT: Do NOT use any tools. All the data you need is provided above. Analyze it directly.')
  parts.push('')
  parts.push('Based on the technical data above, provide your analysis in the following format:')
  parts.push('')
  parts.push('## What\'s Driving the Trend')
  parts.push('Based on the technical indicators and price action, explain what the data suggests about current momentum, trend strength, and likely drivers (sector dynamics, technical patterns, mean reversion, etc.).')
  parts.push('')
  parts.push('## Strategy Recommendations')
  parts.push('For each, give a specific price level derived from the technical data and reasoning:')
  parts.push('- **Market Buy**: Should an investor buy at current price? Why or why not based on the indicators?')
  parts.push('- **Limit Buy**: A better entry price to wait for based on support levels (SMAs, trend)')
  parts.push('- **Stop Loss**: Where to place protective stop — use nearest SMA or technical level')
  parts.push('- **Take Profit**: Target price levels based on resistance and projections')
  parts.push('- **Overall Recommendation**: Buy / Hold / Wait / Avoid — with a 1-sentence summary')
  parts.push('')
  parts.push('## Risk Factors')
  parts.push('Key technical and market risks that could invalidate this analysis.')
  parts.push('')
  parts.push('Be specific with dollar amounts. Keep it concise but actionable. Do not call any tools.')

  return parts.join('\n')
}
