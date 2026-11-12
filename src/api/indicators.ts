/**
 * Indicators API for OpenAlgo Desktop
 *
 * Server-side technical indicators (TA-Lib) through Tauri IPC: the catalog the
 * Analysis tab picks from, and the compute call that returns values aligned to
 * the symbol's bars.
 */

import { invoke } from '@tauri-apps/api/core'

export type ParamKind = 'int' | 'float' | 'matype'

export interface ParamSpec {
  /** Request key, e.g. `period`. */
  name: string
  default: number
  min: number
  max: number
  /** `matype` is a moving-average type: 0 = SMA … 8 = T3. */
  kind: ParamKind
}

export type IndicatorGroup =
  | 'Overlap'
  | 'Momentum'
  | 'Volatility'
  | 'Volume'
  | 'Cycle'
  | 'Statistics'
  | 'Patterns'

/** Catalog groups in display order (mirrors the Rust catalog). */
export const INDICATOR_GROUPS: IndicatorGroup[] = [
  'Overlap',
  'Momentum',
  'Volatility',
  'Volume',
  'Cycle',
  'Statistics',
  'Patterns',
]

export interface IndicatorSpec {
  /** TA-Lib name, e.g. `ADX`; also the request name. */
  name: string
  label: string
  group: IndicatorGroup
  /** Bar fields the indicator reads: a subset of open, high, low, close, volume. */
  inputs: string[]
  params: ParamSpec[]
  /** Output names in TA-Lib order (`value` for single-output, `pattern` for CDL*). */
  outputs: string[]
}

export interface IndicatorPoint {
  timestamp: string
  /** Primary output; null before the indicator's lookback. */
  value: number | null
  secondary_value: number | null
  tertiary_value: number | null
}

export interface IndicatorResult {
  /** Result id, e.g. `SMA_20`, `MACD_12_26`, `CDLDOJI`. */
  name: string
  values: IndicatorPoint[]
  /** Every output by name, aligned to the bars: `outputs[k][1][i]` is output k at bar i. */
  outputs: [string, (number | null)[]][]
}

export interface IndicatorRequest {
  name: string
  /** Missing parameters take the catalog defaults. */
  params?: Record<string, number>
}

export const indicatorsApi = {
  catalog: () => invoke<IndicatorSpec[]>('indicator_catalog'),
  compute: (symbol: string, exchange: string, timeframe: string, indicators: IndicatorRequest[]) =>
    invoke<IndicatorResult[]>('compute_indicators', { symbol, exchange, timeframe, indicators }),
}
