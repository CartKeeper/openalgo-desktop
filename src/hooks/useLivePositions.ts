/**
 * Live positions hook.
 *
 * Pulls the position book from the broker on an interval (the baseline, which
 * also surfaces newly opened / closed positions) and overlays the real-time
 * WebSocket price feed on top, recomputing unrealized P&L tick-by-tick. When no
 * live tick has arrived yet for a symbol (e.g. market closed), it falls back to
 * the broker-provided LTP/P&L so the table is never empty.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import type { Position } from '@/api/tauri-client'
import { tradingApi } from '@/api/trading'
import { useMarketData } from '@/hooks/useMarketData'

export interface EnrichedPosition extends Position {
  liveLtp: number
  liveUnrealized: number
  liveValue: number
  isLive: boolean
}

export interface PositionTotals {
  unrealized: number
  realized: number
  marketValue: number
  costBasis: number
  openCount: number
}

export interface UseLivePositionsReturn {
  positions: EnrichedPosition[]
  totals: PositionTotals
  isLoading: boolean
  error: string | null
  isLive: boolean
  refresh: () => Promise<void>
}

export function useLivePositions(pollMs = 10000): UseLivePositionsReturn {
  const [positions, setPositions] = useState<Position[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetchPositions = useCallback(async () => {
    try {
      const res = await tradingApi.getPositions('')
      if (res.status === 'success' && res.data) {
        setPositions(res.data.filter((p) => p.quantity !== 0))
        setError(null)
      } else if (res.status === 'error') {
        setError(res.message ?? 'Failed to load positions')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchPositions()
    const id = setInterval(fetchPositions, pollMs)
    return () => clearInterval(id)
  }, [fetchPositions, pollMs])

  // Stable symbol set keyed by content so useMarketData only re-subscribes when
  // the actual set of held symbols changes (not on every poll).
  const symKey = useMemo(
    () =>
      positions
        .map((p) => `${p.exchange}:${p.symbol}`)
        .sort()
        .join(','),
    [positions]
  )
  const symbols = useMemo(
    () =>
      symKey
        ? symKey.split(',').map((k) => {
            const [exchange, symbol] = k.split(':')
            return { exchange, symbol }
          })
        : [],
    [symKey]
  )

  const { data: live, isConnected } = useMarketData({
    symbols,
    mode: 'LTP',
    enabled: symbols.length > 0,
  })

  const enriched: EnrichedPosition[] = useMemo(
    () =>
      positions.map((p) => {
        const tick = live.get(`${p.exchange}:${p.symbol}`)?.data.ltp
        const hasTick = typeof tick === 'number' && tick > 0
        const liveLtp = hasTick ? (tick as number) : p.ltp
        // quantity is signed (negative = short), so this handles both directions.
        const liveUnrealized = (liveLtp - p.average_price) * p.quantity
        const liveValue = liveLtp * p.quantity
        return { ...p, liveLtp, liveUnrealized, liveValue, isLive: hasTick }
      }),
    [positions, live]
  )

  const totals: PositionTotals = useMemo(
    () => ({
      unrealized: enriched.reduce((s, p) => s + p.liveUnrealized, 0),
      realized: enriched.reduce((s, p) => s + (p.realized_pnl || 0), 0),
      marketValue: enriched.reduce((s, p) => s + p.liveValue, 0),
      costBasis: enriched.reduce((s, p) => s + p.average_price * p.quantity, 0),
      openCount: enriched.length,
    }),
    [enriched]
  )

  return { positions: enriched, totals, isLoading, error, isLive: isConnected, refresh: fetchPositions }
}
