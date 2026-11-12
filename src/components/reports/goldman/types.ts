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
  /** A comparison report's cover panel: original vs recommended, computed by the app. */
  comparison?: BriefComparison
  /** Replaces the default cover title. */
  cover_title?: { main: string; accent: string }
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

/** Two portfolios priced at the same live quotes. */
export interface BriefComparison {
  account_type: string
  prices_as_of: string
  original: ComparisonSide
  recommended: ComparisonSide
}

export interface ComparisonSide {
  /** Scenario name. */
  name: string
  positions: number
  /** Market value of the positions with a live price. */
  holdings_value: number
  /** null = cash not recorded. */
  cash: number | null
  /** Holdings + cash; null when cash is not recorded. */
  total_value: number | null
  unpriced: string[]
}

export type GoldmanMovement =
  | DiagnosisMovement
  | RestructuringMovement
  | TacticalMovement
  | CodaMovement
  | NarrativeMovement
  | ChangesMovement
  | MeasuresMovement
  | MetricsMovement
  | HealthMovement

/** Both portfolios' trailing-window figures, computed by the app; only `tempo` and `intro` are Claude's. */
export interface MetricsMovement extends MovementBase {
  kind: 'metrics'
  original_label: string
  recommended_label: string
  /** The window the figures cover, e.g. "Trailing 365 days of daily closes, November 12, 2025 to November 11, 2026". */
  window: string
  /** Metric | original | recommended, already formatted ("12.3%", "1.23", "n/a"). */
  rows: { label: string; original: string; recommended: string }[]
  /** One grid per side with enough history. */
  monthly: MonthlyTable[]
  /** One table per side with enough history. */
  drawdowns: DrawdownTable[]
  /** One line per side with a Monte Carlo result. */
  monte_carlo: MonteCarloLine[]
  /** Each side's window and what was left out, or why there are no figures. */
  note: string
}

export interface MonthlyTable {
  /** The portfolio's name. */
  label: string
  years: MonthlyYear[]
}

/** One calendar year of monthly returns as fractions; `months[0]` is January, null = outside the window. */
export interface MonthlyYear {
  year: number
  months: (number | null)[]
  ytd: number | null
}

export interface DrawdownTable {
  label: string
  rows: DrawdownRow[]
}

/** A peak-to-recovery episode. Dates are "YYYY-MM-DD"; `end` is null while unrecovered. */
export interface DrawdownRow {
  start: string
  trough: string
  end: string | null
  /** Trough over peak minus one: -0.20 for a 20% fall. */
  depth_pct: number
  days: number
}

/** Bootstrapped one-year outcomes for one portfolio; returns, thresholds and odds are fractions. */
export interface MonteCarloLine {
  label: string
  paths: number
  horizon_days: number
  p5: number
  p50: number
  p95: number
  bust_threshold: number
  bust_probability: number
  goal_threshold: number
  goal_probability: number
}

/** The health checklist for both portfolios, computed by the app; only `tempo` and `intro` are Claude's. */
export interface HealthMovement extends MovementBase {
  kind: 'health'
  original_label: string
  recommended_label: string
  rows: HealthCompareRow[]
  note: string
}

export type HealthStatus = 'pass' | 'warn' | 'fail' | 'na'

export interface HealthCompareRow {
  key: string
  title: string
  original: HealthCell
  recommended: HealthCell
  /** The rule's threshold in words. */
  threshold: string
}

export interface HealthCell {
  status: HealthStatus
  value_text: string
  /** What was left out or could not be judged. */
  detail?: string
}

/** Every difference between two portfolios; only `reason` is written by Claude. */
export interface ChangesMovement extends MovementBase {
  kind: 'changes'
  rows: ChangeRow[]
}

export interface ChangeRow {
  ticker: string
  kind: 'sold' | 'trimmed' | 'added' | 'bought'
  /** "Sold all 3", "Bought 9", … */
  change: string
  before: number
  after: number
  /** Today's live price; null = no live price. */
  price: number | null
  /** Shares changed × today's price. */
  value_today: number | null
  reason: string
}

/** Both portfolios' measures side by side, computed by the app. */
export interface MeasuresMovement extends MovementBase {
  kind: 'measures'
  original_label: string
  recommended_label: string
  rows: { label: string; original: string; recommended: string }[]
  note?: string
}

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
