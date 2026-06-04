import { Loader2, PieChart } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { providerCommands } from '@/api/tauri-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { EnrichedPosition } from '@/hooks/useLivePositions'
import { formatUSD } from '@/lib/format'

interface AssetAllocationCardProps {
  positions: EnrichedPosition[]
}

const COLORS = [
  '#3b82f6',
  '#22c55e',
  '#f59e0b',
  '#a855f7',
  '#ef4444',
  '#06b6d4',
  '#ec4899',
  '#84cc16',
  '#64748b',
]

export function AssetAllocationCard({ positions }: AssetAllocationCardProps) {
  // symbol -> sector, kept in state (so segments recompute) and reused as a cache.
  const [sectors, setSectors] = useState<Record<string, string>>({})
  const [isLoading, setIsLoading] = useState(true)

  const symbols = useMemo(
    () => Array.from(new Set(positions.map((p) => p.symbol))),
    [positions]
  )

  useEffect(() => {
    let cancelled = false
    const missing = symbols.filter((s) => !(s in sectors))
    if (missing.length === 0) {
      setIsLoading(false)
      return
    }
    ;(async () => {
      const entries = await Promise.all(
        missing.map(async (s): Promise<[string, string]> => {
          try {
            const profile = await providerCommands.getCompanyProfile(s)
            return [s, profile?.sector || 'Other']
          } catch {
            return [s, 'Other']
          }
        })
      )
      if (!cancelled) {
        setSectors((prev) => ({ ...prev, ...Object.fromEntries(entries) }))
        setIsLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [symbols, sectors])

  const segments = useMemo(() => {
    const bySector = new Map<string, number>()
    for (const p of positions) {
      const sector = sectors[p.symbol] || 'Other'
      const value = Math.abs(p.liveValue)
      bySector.set(sector, (bySector.get(sector) || 0) + value)
    }
    const total = Array.from(bySector.values()).reduce((s, v) => s + v, 0)
    return {
      total,
      items: Array.from(bySector.entries())
        .map(([sector, value], i) => ({
          sector,
          value,
          pct: total > 0 ? (value / total) * 100 : 0,
          color: COLORS[i % COLORS.length],
        }))
        .sort((a, b) => b.value - a.value),
    }
  }, [positions, sectors])

  // SVG donut geometry.
  const size = 160
  const stroke = 24
  const r = (size - stroke) / 2
  const C = 2 * Math.PI * r
  let offset = 0

  return (
    <Card className="h-140">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <PieChart className="h-4 w-4 text-primary" />
          Asset Allocation
          <span className="text-xs font-normal text-muted-foreground">by sector</span>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading && segments.items.length === 0 ? (
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : segments.total === 0 ? (
          <div className="py-10 text-center text-sm text-muted-foreground">
            No holdings to allocate
          </div>
        ) : (
          <div className="flex flex-col items-center gap-5 sm:flex-row">
            <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="shrink-0">
              <g transform={`rotate(-90 ${size / 2} ${size / 2})`}>
                {segments.items.map((seg) => {
                  const len = (seg.pct / 100) * C
                  const dash = `${len} ${C - len}`
                  const el = (
                    <circle
                      key={seg.sector}
                      cx={size / 2}
                      cy={size / 2}
                      r={r}
                      fill="none"
                      stroke={seg.color}
                      strokeWidth={stroke}
                      strokeDasharray={dash}
                      strokeDashoffset={-offset}
                    />
                  )
                  offset += len
                  return el
                })}
              </g>
              <text
                x="50%"
                y="50%"
                textAnchor="middle"
                dominantBaseline="central"
                className="fill-foreground text-sm font-semibold"
              >
                {formatUSD(segments.total)}
              </text>
            </svg>
            <ul className="w-full space-y-1.5">
              {segments.items.map((seg) => (
                <li key={seg.sector} className="flex items-center gap-2 text-sm">
                  <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: seg.color }} />
                  <span className="min-w-0 flex-1 truncate">{seg.sector}</span>
                  <span className="shrink-0 tabular-nums text-muted-foreground">
                    {seg.pct.toFixed(1)}%
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
