/**
 * Market Data Hook for OpenAlgo Desktop
 *
 * Uses Tauri IPC commands and events for real-time market data
 * instead of browser WebSocket.
 */

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useCallback, useEffect, useRef, useState } from 'react'

export interface MarketData {
  ltp?: number
  open?: number
  high?: number
  low?: number
  close?: number
  volume?: number
  change?: number
  change_percent?: number
  timestamp?: string
  bid?: number
  ask?: number
  bid_qty?: number
  ask_qty?: number
  oi?: number
}

export interface SymbolData {
  symbol: string
  exchange: string
  data: MarketData
  lastUpdate?: number
}

interface MarketTick {
  symbol: string
  exchange: string
  token: string
  ltp: number
  open: number
  high: number
  low: number
  close: number
  volume: number
  bid: number
  ask: number
  bid_qty: number
  ask_qty: number
  oi: number
  timestamp: number
  change: number
  change_percent: number
}

interface UseMarketDataOptions {
  symbols: Array<{ symbol: string; exchange: string; token?: string }>
  mode?: 'LTP' | 'Quote' | 'Depth'
  enabled?: boolean
  autoReconnect?: boolean
}

interface UseMarketDataReturn {
  data: Map<string, SymbolData>
  isConnected: boolean
  isAuthenticated: boolean
  isConnecting: boolean
  /** True while the backend supervisor is retrying after an unexpected drop. */
  isReconnecting: boolean
  error: string | null
  connect: () => Promise<void>
  disconnect: () => void
}

// Map mode names to Tauri mode strings
function getModeString(mode: 'LTP' | 'Quote' | 'Depth'): string {
  switch (mode) {
    case 'LTP':
      return 'ltp'
    case 'Quote':
      return 'quote'
    case 'Depth':
      return 'full'
    default:
      return 'quote'
  }
}

export function useMarketData({
  symbols,
  mode = 'Quote',
  enabled = true,
  autoReconnect = true,
}: UseMarketDataOptions): UseMarketDataReturn {
  const [marketData, setMarketData] = useState<Map<string, SymbolData>>(new Map())
  const [isConnected, setIsConnected] = useState(false)
  const [isAuthenticated, setIsAuthenticated] = useState(false)
  const [isConnecting, setIsConnecting] = useState(false)
  const [isReconnecting, setIsReconnecting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const unlistenersRef = useRef<UnlistenFn[]>([])
  const subscribedSymbolsRef = useRef<Set<string>>(new Set())
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Ticks can arrive faster than React can paint (dozens/sec per symbol during
  // active markets). Writing state on every tick re-renders all consumers on
  // every tick and saturates the main thread, making the whole UI laggy. Instead
  // we accumulate ticks into a ref and flush to state on a fixed cadence below,
  // capping re-renders regardless of incoming tick volume.
  const pendingRef = useRef<Map<string, SymbolData>>(new Map())
  const dirtyRef = useRef(false)

  // Handle incoming market tick — mutate the pending buffer only, no setState.
  const handleMarketTick = useCallback((tick: MarketTick) => {
    const key = `${tick.exchange}:${tick.symbol}`
    pendingRef.current.set(key, {
      symbol: tick.symbol,
      exchange: tick.exchange,
      data: {
        ltp: tick.ltp,
        open: tick.open,
        high: tick.high,
        low: tick.low,
        close: tick.close,
        volume: tick.volume,
        change: tick.change,
        change_percent: tick.change_percent,
        bid: tick.bid,
        ask: tick.ask,
        bid_qty: tick.bid_qty,
        ask_qty: tick.ask_qty,
        oi: tick.oi,
        timestamp: tick.timestamp ? new Date(tick.timestamp).toISOString() : undefined,
      },
      lastUpdate: Date.now(),
    })
    dirtyRef.current = true
  }, [])

  // Flush the accumulated ticks to React state at most ~4x/sec.
  useEffect(() => {
    if (!enabled) return
    const id = setInterval(() => {
      if (dirtyRef.current) {
        dirtyRef.current = false
        setMarketData(new Map(pendingRef.current))
      }
    }, 250)
    return () => clearInterval(id)
  }, [enabled])

  // Subscribe to symbols
  const subscribeToSymbols = useCallback(
    async (symbolsToSubscribe: Array<{ symbol: string; exchange: string; token?: string }>) => {
      if (!isConnected || symbolsToSubscribe.length === 0) return

      const newSymbols = symbolsToSubscribe.filter((s) => {
        const key = `${s.exchange}:${s.symbol}`
        return !subscribedSymbolsRef.current.has(key)
      })

      if (newSymbols.length === 0) return

      try {
        // Build subscription requests
        const requests = newSymbols.map((s) => ({
          exchange: s.exchange,
          token: s.token || s.symbol, // Use token if provided, otherwise use symbol
          symbol: s.symbol,
          mode: getModeString(mode),
        }))

        await invoke('websocket_subscribe', { symbols: requests })

        // Track subscribed symbols
        newSymbols.forEach((s) => {
          const key = `${s.exchange}:${s.symbol}`
          subscribedSymbolsRef.current.add(key)

          // Initialize market data entry in the pending buffer; the throttled
          // flush above will surface it to consumers.
          if (!pendingRef.current.has(key)) {
            pendingRef.current.set(key, { symbol: s.symbol, exchange: s.exchange, data: {} })
            dirtyRef.current = true
          }
        })
      } catch (err) {
        console.error('Failed to subscribe to symbols:', err)
        setError(`Subscription failed: ${err}`)
      }
    },
    [isConnected, mode]
  )

  // Connect to WebSocket via Tauri
  const connect = useCallback(async () => {
    if (isConnected || isConnecting) return

    setIsConnecting(true)
    setError(null)

    try {
      // Connect via Tauri command
      await invoke('websocket_connect')

      setIsConnected(true)
      setIsAuthenticated(true) // Connection implies authentication in desktop mode
      setIsConnecting(false)
      setError(null)

      // Subscribe to pending symbols
      if (symbols.length > 0) {
        setTimeout(() => {
          subscribeToSymbols(symbols)
        }, 100)
      }
    } catch (err) {
      console.error('WebSocket connection failed:', err)
      setError(`Connection failed: ${err}`)
      setIsConnecting(false)

      // Auto-reconnect
      if (autoReconnect && enabled) {
        reconnectTimeoutRef.current = setTimeout(connect, 3000)
      }
    }
  }, [isConnected, isConnecting, symbols, subscribeToSymbols, autoReconnect, enabled])

  // Disconnect from WebSocket
  const disconnect = useCallback(async () => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }

    try {
      await invoke('websocket_disconnect')
    } catch (err) {
      console.error('WebSocket disconnect failed:', err)
    }

    setIsConnected(false)
    setIsAuthenticated(false)
    subscribedSymbolsRef.current.clear()
  }, [])

  // Set up Tauri event listeners
  useEffect(() => {
    if (!enabled) return

    // Listen to market tick events
    listen<MarketTick>('market_tick', (event) => {
      handleMarketTick(event.payload)
    }).then((unlisten) => {
      unlistenersRef.current.push(unlisten)
    })

    // Listen to connection status events. The BACKEND owns reconnection (it
    // retries with exponential backoff and re-subscribes stored symbols on any
    // unexpected drop), so the frontend only reflects status here — it never
    // re-issues a connect command on a drop, which would fight the supervisor.
    listen<string>('websocket_connected', () => {
      setIsConnected(true)
      setIsAuthenticated(true)
      setIsReconnecting(false)
      setError(null)
    }).then((unlisten) => {
      unlistenersRef.current.push(unlisten)
    })

    // Emitted at the start of each backend reconnect attempt after a drop.
    listen<{ broker: string; attempt: number; delaySecs: number }>(
      'websocket_reconnecting',
      (event) => {
        setIsConnected(false)
        setIsReconnecting(true)
        setError(
          `Connection lost — reconnecting (attempt ${event.payload.attempt})…`
        )
      }
    ).then((unlisten) => {
      unlistenersRef.current.push(unlisten)
    })

    listen<string>('websocket_disconnected', () => {
      // Reflect the drop; do NOT reconnect here — the backend supervisor does,
      // and keeps the subscription set so we leave subscribedSymbolsRef intact.
      setIsConnected(false)
      setIsAuthenticated(false)
    }).then((unlisten) => {
      unlistenersRef.current.push(unlisten)
    })

    listen<string>('websocket_error', (event) => {
      setError(`WebSocket error: ${event.payload}`)
    }).then((unlisten) => {
      unlistenersRef.current.push(unlisten)
    })

    return () => {
      unlistenersRef.current.forEach((unlisten) => unlisten())
      unlistenersRef.current = []
    }
  }, [enabled, handleMarketTick])

  // Auto-connect when enabled and symbols provided
  useEffect(() => {
    if (enabled && symbols.length > 0 && !isConnected && !isConnecting) {
      connect()
    }

    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current)
      }
    }
  }, [enabled, symbols.length, isConnected, isConnecting, connect])

  // Subscribe to new symbols when connected
  useEffect(() => {
    if (isConnected && symbols.length > 0) {
      subscribeToSymbols(symbols)
    }
  }, [isConnected, symbols, subscribeToSymbols])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      disconnect()
    }
  }, [disconnect])

  // Check initial WebSocket status
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const status = (await invoke('websocket_status')) as {
          connected: boolean
          broker: string | null
          subscriptions: number
        }
        setIsConnected(status.connected)
        setIsAuthenticated(status.connected)
      } catch {
        // Ignore - websocket command may not be available yet
      }
    }

    if (enabled) {
      checkStatus()
    }
  }, [enabled])

  return {
    data: marketData,
    isConnected,
    isAuthenticated,
    isConnecting,
    isReconnecting,
    error,
    connect,
    disconnect,
  }
}
