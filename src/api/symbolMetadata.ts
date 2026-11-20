/**
 * FinanceDatabase symbol metadata (sector, industry, country, identifiers)
 * stored in the app's DuckDB. Backed by the `symbol_metadata_*` commands.
 */

import { invoke } from '@tauri-apps/api/core'

export interface SymbolMetadataRow {
  symbol: string
  exchange: string
  name: string
  sector: string | null
  industry: string | null
  industry_group: string | null
  country: string | null
  market_cap: number | null
  size_bucket: string | null
  isin: string | null
  cusip: string | null
  figi: string | null
  website: string | null
  summary: string | null
  /** "equity" or "etf" */
  asset_type: string
  delisted: boolean
}

export interface MetadataStatus {
  /** Rows loaded (listed and delisted). */
  rows: number
  /** FinanceDatabase commit the data came from. */
  source_commit: string
  /** Data date of that commit, YYYY-MM-DD. */
  source_as_of: string
  /** UTC ISO-8601 time of the last successful download; null before the first. */
  last_refresh: string | null
  /** Whether the last download included OTC pink sheets. */
  include_otc: boolean
}

export interface SymbolMetadataSearchOptions {
  /** Default 25, maximum 200. */
  limit?: number
  /** Include delisted symbols (default false). */
  includeDelisted?: boolean
}

export const symbolMetadataApi = {
  /** Download the pinned FinanceDatabase files and load them; resolves to the new status. */
  refresh: (includeOtc: boolean) =>
    invoke<MetadataStatus>('symbol_metadata_refresh', { includeOtc }),
  status: () => invoke<MetadataStatus>('symbol_metadata_status'),
  search: (query: string, options: SymbolMetadataSearchOptions = {}) =>
    invoke<SymbolMetadataRow[]>('symbol_metadata_search', { query, ...options }),
}
