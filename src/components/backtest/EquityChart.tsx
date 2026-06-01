import { AreaSeries, ColorType, CrosshairMode, LineSeries, createChart, type IChartApi, type ISeriesApi } from 'lightweight-charts'
import { useEffect, useRef } from 'react'
import { useThemeStore } from '@/stores/themeStore'
import type { EquityPoint } from '@/api/backtest'

interface EquityChartProps {
  equityCurve: EquityPoint[]
}

export function EquityChart({ equityCurve }: EquityChartProps) {
  const { mode } = useThemeStore()
  const isDark = mode === 'dark'
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const equitySeriesRef = useRef<ISeriesApi<'Line'> | null>(null)

  useEffect(() => {
    if (!containerRef.current || equityCurve.length === 0) return

    if (chartRef.current) {
      chartRef.current.remove()
      chartRef.current = null
    }

    const container = containerRef.current
    const chart = createChart(container, {
      width: container.offsetWidth || 600,
      height: 260,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: isDark ? '#a6adbb' : '#333',
      },
      grid: {
        vertLines: { color: isDark ? 'rgba(166,173,187,0.1)' : 'rgba(0,0,0,0.08)' },
        horzLines: { color: isDark ? 'rgba(166,173,187,0.1)' : 'rgba(0,0,0,0.08)' },
      },
      rightPriceScale: {
        borderColor: isDark ? 'rgba(166,173,187,0.2)' : 'rgba(0,0,0,0.2)',
        scaleMargins: { top: 0.1, bottom: 0.1 },
      },
      timeScale: {
        borderColor: isDark ? 'rgba(166,173,187,0.2)' : 'rgba(0,0,0,0.2)',
        timeVisible: false,
        secondsVisible: false,
        tickMarkFormatter: (time: number, tickMarkType: number) => {
          const d = new Date(time * 1000)
          const month = d.getUTCMonth()
          const monthNames = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
          if (tickMarkType === 0) return d.getUTCFullYear().toString()
          if (tickMarkType === 1) return monthNames[month]
          return d.getUTCDate().toString()
        },
      },
      crosshair: {
        mode: CrosshairMode.Normal,
        vertLine: { width: 1, color: isDark ? 'rgba(166,173,187,0.5)' : 'rgba(0,0,0,0.3)', style: 2 },
        horzLine: { width: 1, color: isDark ? 'rgba(166,173,187,0.5)' : 'rgba(0,0,0,0.3)', style: 2, labelBackgroundColor: isDark ? '#1f2937' : '#374151' },
      },
    })

    const equitySeries = chart.addSeries(LineSeries, {
      color: '#22c55e',
      lineWidth: 2,
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })

    const data = equityCurve.map((pt) => {
      const ts = pt.timestamp.slice(0, 10)
      const [y, m, dd] = ts.split('-').map(Number)
      const utc = Math.floor(Date.UTC(y, m - 1, dd) / 1000)
      return { time: utc as unknown as import('lightweight-charts').Time, value: pt.equity }
    })

    equitySeries.setData(data)
    equitySeriesRef.current = equitySeries
    chartRef.current = chart
    chart.timeScale().fitContent()

    const handleResize = () => {
      if (container && chartRef.current) {
        chartRef.current.applyOptions({ width: container.offsetWidth })
      }
    }
    const observer = new ResizeObserver(handleResize)
    observer.observe(container)

    return () => {
      observer.disconnect()
      if (chartRef.current) {
        chartRef.current.remove()
        chartRef.current = null
      }
    }
  }, [equityCurve, isDark])

  return <div ref={containerRef} className="w-full" style={{ height: 260 }} />
}

interface DrawdownChartProps {
  equityCurve: EquityPoint[]
}

export function DrawdownChart({ equityCurve }: DrawdownChartProps) {
  const { mode } = useThemeStore()
  const isDark = mode === 'dark'
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)

  useEffect(() => {
    if (!containerRef.current || equityCurve.length === 0) return

    if (chartRef.current) {
      chartRef.current.remove()
      chartRef.current = null
    }

    const container = containerRef.current
    const chart = createChart(container, {
      width: container.offsetWidth || 600,
      height: 160,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: isDark ? '#a6adbb' : '#333',
      },
      grid: {
        vertLines: { color: isDark ? 'rgba(166,173,187,0.08)' : 'rgba(0,0,0,0.06)' },
        horzLines: { color: isDark ? 'rgba(166,173,187,0.08)' : 'rgba(0,0,0,0.06)' },
      },
      rightPriceScale: {
        borderColor: isDark ? 'rgba(166,173,187,0.2)' : 'rgba(0,0,0,0.2)',
      },
      timeScale: {
        borderColor: isDark ? 'rgba(166,173,187,0.2)' : 'rgba(0,0,0,0.2)',
        timeVisible: false,
        secondsVisible: false,
        tickMarkFormatter: (time: number, tickMarkType: number) => {
          const d = new Date(time * 1000)
          const month = d.getUTCMonth()
          const monthNames = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
          if (tickMarkType === 0) return d.getUTCFullYear().toString()
          if (tickMarkType === 1) return monthNames[month]
          return d.getUTCDate().toString()
        },
      },
      crosshair: {
        mode: CrosshairMode.Normal,
        vertLine: { width: 1, color: isDark ? 'rgba(166,173,187,0.5)' : 'rgba(0,0,0,0.3)', style: 2 },
        horzLine: { width: 1, color: isDark ? 'rgba(166,173,187,0.5)' : 'rgba(0,0,0,0.3)', style: 2, labelBackgroundColor: isDark ? '#1f2937' : '#374151' },
      },
    })

    const ddSeries = chart.addSeries(AreaSeries, {
      topColor: 'rgba(239,68,68,0.3)',
      bottomColor: 'rgba(239,68,68,0.02)',
      lineColor: '#ef4444',
      lineWidth: 1,
      priceFormat: {
        type: 'custom',
        formatter: (v: number) => `${(v * 100).toFixed(1)}%`,
        minMove: 0.001,
      },
    })

    const data = equityCurve.map((pt) => {
      const ts = pt.timestamp.slice(0, 10)
      const [y, m, dd] = ts.split('-').map(Number)
      const utc = Math.floor(Date.UTC(y, m - 1, dd) / 1000)
      return { time: utc as unknown as import('lightweight-charts').Time, value: pt.drawdown }
    })

    ddSeries.setData(data)
    chartRef.current = chart
    chart.timeScale().fitContent()

    const handleResize = () => {
      if (container && chartRef.current) {
        chartRef.current.applyOptions({ width: container.offsetWidth })
      }
    }
    const observer = new ResizeObserver(observer => handleResize())
    observer.observe(container)

    return () => {
      observer.disconnect()
      if (chartRef.current) {
        chartRef.current.remove()
        chartRef.current = null
      }
    }
  }, [equityCurve, isDark])

  return <div ref={containerRef} className="w-full" style={{ height: 160 }} />
}
