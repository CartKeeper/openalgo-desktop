/**
 * Schema for a Goldman Sax & Violins Brief.
 *
 * The Tauri command `generate_client_brief` returns this shape after Claude
 * analyzes a client's holdings/orders/violations. The shape is stable so it can
 * also be hand-authored or generated for non-portfolio reports later.
 */

export interface GoldmanBrief {
  /** "Casey Borst" — used in cover PREPARED FOR field. */
  client_name: string
  /** "Opus 2 / Combined Brief" — DOCUMENT field. */
  document_label: string
  /** "Allegro · Q2 2026" — TEMPO field. */
  tempo: string
  /** Long human date for letterhead, e.g. "April 30, 2026". */
  generated_date: string
  /** Italic subtitle on cover ("A combined diagnostic and tactical brief…"). */
  subtitle: string
  /** Confidential line on the cover, set by the app from the account type, e.g. "Confidential client brief — 401(k)". */
  confidential_label?: string
  /** Account figures the app computes from live quotes (never Claude). Shown on the cover. */
  facts?: BriefFacts
  /** Ordered movements rendered after cover. Renderer treats `kind` as discriminator. */
  movements: GoldmanMovement[]
}

export interface BriefFacts {
  /** "401(k)", "Traditional IRA", … */
  account_type: string
  positions: number
  /** Market value of the positions that have a live price. */
  market_value: number
  /** Cost basis of those same positions. */
  cost_basis: number
  unrealized_pnl: number
  unrealized_pct: number | null
  /** "November 11, 2026, 12:06 PM ET" */
  prices_as_of: string
  /** Positions with no live price (left out of the figures above). */
  unpriced: string[]
}

export type GoldmanMovement =
  | DiagnosisMovement
  | RestructuringMovement
  | TacticalMovement
  | CodaMovement
  | NarrativeMovement

export interface MovementBase {
  /** Roman numeral or label, e.g. "I", "II". */
  numeral: string
  /** Italic tempo annotation after the dash, e.g. "Andante, deliberate". */
  tempo: string
  /** Plain part of the section title, e.g. "The". */
  title_main: string
  /** Italic-gold accent of the section title, e.g. "Diagnosis". */
  title_accent: string
  /** Optional muted-italic intro paragraph beneath the title. */
  intro?: string
}

export interface DiagnosisMovement extends MovementBase {
  kind: 'diagnosis'
  issues: DiagnosisIssue[]
  /** Bottom "Headline." line in italic gray. */
  headline?: { lead: string; body: string }
}

export interface DiagnosisIssue {
  title: string
  body: string
  bullets: string[]
}

export interface RestructuringMovement extends MovementBase {
  kind: 'restructuring'
  allocation_cards: AllocationCard[]
  proposed_core_table?: { heading: string; rows: PortfolioRow[] }
  sector_sleeve?: { heading: string; bullets: BulletItem[] }
  immediate_eliminations?: { heading: string; bullets: BulletItem[] }
}

export interface AllocationCard {
  /** The sleeve's share of account value today, computed by the app from `symbols`, e.g. "38%". */
  percentage: string
  /** Sleeve name, e.g. "Core — index & mega-cap". */
  label: string
  /** Body paragraph beneath. */
  body: string
  /** Held tickers in this sleeve. */
  symbols?: string[]
  /** What `percentage` measures, e.g. "of account value today". */
  caption?: string
}

export interface TacticalMovement extends MovementBase {
  kind: 'tactical'
  sections: TacticalSection[]
}

export interface TacticalSection {
  /** "1", "2", … shown in burgundy italic numeral. */
  number: string
  /** Bold gold heading, e.g. "Profit-Taking on Overextended Winners". */
  title: string
  /** Optional intro paragraph. */
  body?: string
  /** Optional ♪ bullets. */
  bullets?: BulletItem[]
  /** Optional table. */
  table?: { columns: TableColumn[]; rows: PortfolioRow[] }
}

export interface CodaMovement extends MovementBase {
  kind: 'coda'
  groups: ChecklistGroup[]
  /** Italic note above the ⚜ ♪ ⚜ flourish. */
  closing_note?: { lead: string; body: string }
}

export interface ChecklistGroup {
  heading: string
  items: { lead: string; body?: string }[]
}

/** Generic free-form section (used when content doesn't fit the four-movement archetype). */
export interface NarrativeMovement extends MovementBase {
  kind: 'narrative'
  paragraphs: string[]
  bullets?: BulletItem[]
}

// ----- Shared primitives -----

export interface BulletItem {
  /** Optional bold lead, e.g. "Healthcare:" or "AAPL". */
  lead?: string
  /** Body text after the lead. */
  body: string
}

export type TableColumn = 'TICKER' | 'NAME' | 'P/L' | 'POSITION' | 'CURRENT P/L' | 'ACTION' | 'DECISION'

export interface PortfolioRow {
  ticker: string
  name: string
  /** Display string e.g. "+29.2%" or "-31%". */
  pl: string
  /** Determines green vs burgundy color. */
  pl_positive: boolean
  /** Action / Decision column body. */
  action: string
}
