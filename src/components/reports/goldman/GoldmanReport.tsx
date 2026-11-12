/**
 * Goldman Sax & Violins — house report template (React-PDF).
 *
 * Visual reference: cream paper, Times serif, gold/burgundy accents,
 * four-movement layout with tempo annotations.
 *
 * All exported reports for "recommendations and performance" use this template.
 * Brand wordmark, tagline, and disclaimer are intentionally hardcoded — house brand.
 *
 * Tracked-out labels use `letterSpacing`, never spaces between letters: spaces
 * let a line break inside a word ("I N D E / X"). Words are never hyphenated.
 */

import {
  Document,
  Font,
  Page,
  Path,
  StyleSheet,
  Svg,
  Text,
  View,
  type DocumentProps,
} from '@react-pdf/renderer'
import type {
  AllocationCard,
  BriefComparison,
  BriefFacts,
  BulletItem,
  ChangeRow,
  ChangesMovement,
  ChecklistGroup,
  CodaMovement,
  DiagnosisIssue,
  DiagnosisMovement,
  DrawdownTable,
  GoldmanBrief,
  GoldmanMovement,
  HealthCell,
  HealthMovement,
  HealthStatus,
  MeasuresMovement,
  MetricsMovement,
  MonteCarloLine,
  MonthlyTable,
  NarrativeMovement,
  PortfolioRow,
  RestructuringMovement,
  TableColumn,
  TacticalMovement,
  TacticalSection,
} from './types'
import type { ComponentProps } from 'react'
import { C, FONT } from './tokens'
import { pdfSafeDeep } from './pdfText'

// Break lines only between words.
Font.registerHyphenationCallback((word) => [word])

const s = StyleSheet.create({
  page: {
    backgroundColor: C.paper,
    paddingTop: 56,
    paddingHorizontal: 64,
    paddingBottom: 64,
    fontFamily: FONT.serif,
    color: C.ink,
    fontSize: 10.5,
  },

  // Cover --------------------------------------------------------------
  brandRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    paddingBottom: 24,
  },
  brandWord: { fontFamily: FONT.serif, fontSize: 22, color: C.gold },
  brandWordItalic: { fontFamily: FONT.serifItalic, fontSize: 22, color: C.gold },
  brandDate: { fontFamily: FONT.serif, fontSize: 9.5, color: C.muted, letterSpacing: 1.4 },
  confidentialPill: {
    fontFamily: FONT.serif,
    fontSize: 9.5,
    color: C.burgundy,
    letterSpacing: 3,
    paddingTop: 52,
  },
  coverTitle: { fontSize: 40, color: C.ink, lineHeight: 1.1, paddingTop: 20 },
  coverTitleAccent: { fontFamily: FONT.serifBoldItalic, color: C.gold },
  coverSubtitle: {
    fontFamily: FONT.serifItalic,
    fontSize: 12,
    color: C.body,
    lineHeight: 1.45,
    paddingTop: 24,
  },
  coverDivider: {
    borderBottomWidth: 0.6,
    borderBottomColor: C.burgundy,
    marginTop: 56,
    marginBottom: 12,
  },
  coverMetaRow: { flexDirection: 'row' },
  coverMetaCell: { width: '33.33%', paddingRight: 12 },
  coverMetaLabel: { fontSize: 8, color: C.muted, letterSpacing: 2, paddingBottom: 4 },
  coverMetaValue: { fontSize: 11, color: C.ink },
  coverFooter: {
    position: 'absolute',
    bottom: 30,
    left: 64,
    right: 64,
    textAlign: 'center',
    fontFamily: FONT.serifItalic,
    fontSize: 7,
    color: C.muted,
  },

  // Cover fact panel ---------------------------------------------------
  facts: {
    marginTop: 28,
    borderTopWidth: 0.6,
    borderTopColor: C.goldSoft,
    borderBottomWidth: 0.6,
    borderBottomColor: C.goldSoft,
    paddingVertical: 12,
  },
  factsHeading: {
    fontFamily: FONT.serifItalic,
    fontSize: 10,
    color: C.gold,
    paddingBottom: 8,
  },
  factsRow: { flexDirection: 'row', paddingTop: 6 },
  factCell: { width: '33.33%', paddingRight: 12 },
  factLabel: { fontSize: 8, color: C.muted, letterSpacing: 1.6, paddingBottom: 3 },
  factValue: { fontFamily: FONT.serifBold, fontSize: 12.5, color: C.ink },
  factNote: {
    fontFamily: FONT.serifItalic,
    fontSize: 9,
    color: C.body,
    lineHeight: 1.4,
    paddingTop: 10,
  },

  // Cover comparison panel ---------------------------------------------
  compareHeaderRow: { flexDirection: 'row', paddingBottom: 5, borderBottomWidth: 0.6, borderBottomColor: C.goldSoft },
  compareRow: { flexDirection: 'row', paddingVertical: 5, borderBottomWidth: 0.4, borderBottomColor: C.goldSoft },
  compareLabelCell: { width: '34%', paddingRight: 10 },
  compareValueCell: { width: '33%', paddingRight: 10 },
  compareName: { fontFamily: FONT.serifItalic, fontSize: 10, color: C.gold, lineHeight: 1.3 },
  compareRole: { fontSize: 8, color: C.muted, letterSpacing: 1.6, paddingBottom: 2 },
  compareValue: { fontFamily: FONT.serifBold, fontSize: 12, color: C.ink },

  // Running footer (movement pages) ------------------------------------
  footer: {
    position: 'absolute',
    bottom: 28,
    left: 64,
    right: 64,
    flexDirection: 'row',
    justifyContent: 'space-between',
    borderTopWidth: 0.5,
    borderTopColor: C.goldSoft,
    paddingTop: 6,
  },
  footerText: { fontFamily: FONT.serifItalic, fontSize: 7.5, color: C.muted },

  // Movement ----------------------------------------------------------
  movementLabel: {
    fontFamily: FONT.serif,
    fontSize: 9,
    color: C.burgundy,
    letterSpacing: 2.4,
    paddingTop: 4,
  },
  movementTempo: { fontFamily: FONT.serifItalic, fontSize: 9, color: C.muted, letterSpacing: 0.4 },
  movementTitle: { fontFamily: FONT.serif, fontSize: 26, color: C.ink, paddingTop: 6 },
  movementTitleAccent: { fontFamily: FONT.serifItalic, color: C.gold },
  movementIntro: {
    fontFamily: FONT.serifItalic,
    fontSize: 12,
    color: C.body,
    lineHeight: 1.45,
    paddingTop: 5,
  },
  redRule: {
    borderBottomWidth: 0.5,
    borderBottomColor: C.burgundy,
    marginTop: 14,
    marginBottom: 12,
  },

  // Body --------------------------------------------------------------
  paragraph: { fontSize: 10.5, color: C.ink, lineHeight: 1.45, paddingTop: 6 },
  h2: { fontFamily: FONT.serifBold, fontSize: 13.5, color: C.gold, paddingTop: 14, paddingBottom: 4 },
  h3: { fontFamily: FONT.serifBold, fontSize: 11.5, color: C.ink, paddingTop: 10, lineHeight: 1.4 },

  // Numbered rows (diagnosis i./ii., tactical 1./2.) --------------------
  numberedRow: { flexDirection: 'row', paddingTop: 12 },
  numeral: { fontFamily: FONT.serifItalic, fontSize: 13.5, color: C.burgundy, width: 22, paddingTop: 14 },
  numberedBody: { flex: 1 },

  // Bullets -----------------------------------------------------------
  bulletRow: { flexDirection: 'row', paddingTop: 6, alignItems: 'flex-start' },
  noteGlyph: { fontFamily: FONT.serif, fontSize: 11, color: C.gold, width: 14, paddingTop: 0.5 },
  bulletBody: { flex: 1, fontSize: 10.5, lineHeight: 1.4 },
  bulletLead: { fontFamily: FONT.serifBold, color: C.ink },
  bulletTickerLead: { fontFamily: FONT.sansBold, color: C.gold, fontSize: 9.5 },

  // Compact ticker ledger (long lists of single holdings) ---------------
  ledger: { flexDirection: 'row', flexWrap: 'wrap', paddingTop: 6 },
  ledgerCell: {
    width: '50%',
    flexDirection: 'row',
    paddingVertical: 4,
    paddingRight: 14,
    borderBottomWidth: 0.5,
    borderBottomColor: C.goldSoft,
  },
  ledgerTicker: { fontFamily: FONT.sansBold, color: C.gold, fontSize: 9, width: 52 },
  ledgerBody: { flex: 1, fontSize: 9.5, color: C.ink, lineHeight: 1.35 },

  // Allocation cards --------------------------------------------------
  allocationRow: { flexDirection: 'row', paddingTop: 12 },
  allocationCard: {
    flex: 1,
    borderWidth: 0.7,
    borderColor: C.goldSoft,
    paddingHorizontal: 11,
    paddingTop: 11,
    paddingBottom: 14,
  },
  allocationGap: { width: 12 },
  allocationPct: { fontFamily: FONT.serifBold, fontSize: 24, color: C.gold },
  allocationCaption: { fontFamily: FONT.serifItalic, fontSize: 8.5, color: C.muted, paddingTop: 1 },
  allocationLabel: {
    fontFamily: FONT.serif,
    fontSize: 8.5,
    color: C.burgundy,
    letterSpacing: 1.6,
    lineHeight: 1.35,
    paddingTop: 8,
  },
  allocationSymbols: { fontFamily: FONT.sansBold, fontSize: 7.5, color: C.gold, lineHeight: 1.45, paddingTop: 6 },
  allocationBody: { fontFamily: FONT.serif, fontSize: 9.5, color: C.body, lineHeight: 1.4, paddingTop: 6 },

  // Tables ------------------------------------------------------------
  tableHeaderRow: {
    flexDirection: 'row',
    borderBottomWidth: 1,
    borderBottomColor: C.gold,
    paddingBottom: 4,
    paddingTop: 4,
  },
  tableHeaderCell: { fontSize: 7.5, color: C.muted, letterSpacing: 1.2 },
  tableRow: {
    flexDirection: 'row',
    borderBottomWidth: 0.6,
    borderBottomColor: C.goldSoft,
    paddingVertical: 6,
  },
  tableCell: { fontSize: 10, color: C.ink, lineHeight: 1.3 },
  tickerCell: { fontFamily: FONT.sansBold, color: C.gold, fontSize: 10 },
  plCell: { fontFamily: FONT.serifBold, fontSize: 10, textAlign: 'right' },

  // Metrics and health (comparison report) ---------------------------
  windowLine: { fontFamily: FONT.serifItalic, fontSize: 9, color: C.muted, paddingBottom: 4 },
  gridRow: {
    flexDirection: 'row',
    borderBottomWidth: 0.6,
    borderBottomColor: C.goldSoft,
    paddingVertical: 4,
  },
  gridHeaderCell: { fontSize: 6.5, color: C.muted, letterSpacing: 0.8 },
  gridCell: { fontSize: 8, color: C.ink, lineHeight: 1.3 },
  thresholdCell: { fontSize: 8.5, color: C.body, lineHeight: 1.3 },
  cellDetail: { fontFamily: FONT.serifItalic, fontSize: 8, color: C.muted, lineHeight: 1.3, paddingTop: 2, paddingLeft: 14 },
  legendRow: { flexDirection: 'row', alignItems: 'center', paddingTop: 8 },
  legendText: { fontSize: 8.5, color: C.muted, paddingRight: 14 },

  // Coda checklist ----------------------------------------------------
  checkRow: { flexDirection: 'row', paddingTop: 7, alignItems: 'flex-start' },
  checkBox: {
    width: 8,
    height: 8,
    borderWidth: 0.8,
    borderColor: C.gold,
    marginTop: 3,
    marginRight: 10,
  },
  checkBody: { flex: 1, fontSize: 10.5, lineHeight: 1.45 },
  checkLead: { fontFamily: FONT.serifBold, color: C.ink },

  // Closing note + flourish ------------------------------------------
  closingNote: { fontFamily: FONT.serifBold, fontSize: 10.5, color: C.ink, paddingTop: 18, lineHeight: 1.45 },
  closingBody: { fontFamily: FONT.serifItalic, color: C.body, fontWeight: 'normal' },
  flourish: { paddingTop: 24, fontSize: 14, color: C.gold, textAlign: 'center' },
})

// ----- Formatting -----

const usd = (n: number) =>
  n.toLocaleString('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 })

const signedUsd = (n: number) => `${n >= 0 ? '+' : '-'}${usd(Math.abs(n))}`

const signedPct = (n: number) => `${n >= 0 ? '+' : ''}${n.toFixed(2)}%`

/** Green for a gain, burgundy for a loss, muted when the cell holds no number. */
function plColor(pl: string): string {
  const t = pl.trim()
  if (t.startsWith('+')) return C.green
  if (t.startsWith('-') || t.startsWith('−')) return C.burgundy
  return C.muted
}

const TICKER = /^[A-Z0-9.-]{1,6}$/

// ----- Metrics formatting (keep in step with scenario_comparison.rs, which prints the same figures for Claude) -----

/** Round half away from zero, as Rust's `f64::round` does (`Math.round` sends -62.5 to -62). */
const roundHalfAway = (x: number) => Math.sign(x) * Math.round(Math.abs(x))

/** A return fraction as a signed percentage with one decimal: "+12.3%", "-8.1%", "0.0%". Mirrors `signed_pct_text`. */
const returnPct = (v: number) => {
  const x = roundHalfAway(v * 1000) / 10
  return `${x > 0 ? '+' : ''}${x.toFixed(1)}%`
}

/** A threshold as a signed whole percentage: "-20%", "+10%". Mirrors `signed_whole_pct`. */
const signedWholePct = (v: number) => {
  const x = roundHalfAway(v * 100)
  return `${x > 0 ? '+' : ''}${x}%`
}

/** A probability as a whole percentage: 0.234 -> "23%". Mirrors `odds_text`. */
const odds = (p: number) => `${roundHalfAway(p * 100)}%`

/** The Monte Carlo sentence, word for word as `monte_carlo_text` prints it for Claude. */
function monteCarloText(l: MonteCarloLine): string {
  return `In ${l.paths.toLocaleString('en-US')} bootstrapped years (${l.horizon_days} trading days each): median ${returnPct(l.p50)}; 5th to 95th percentile ${returnPct(l.p5)} to ${returnPct(l.p95)}; odds of a ${signedWholePct(l.bust_threshold)} drawdown ${odds(l.bust_probability)}; odds of ${signedWholePct(l.goal_threshold)} or better ${odds(l.goal_probability)}.`
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

/** "2026-03-13" -> "Mar 13, 2026", read from the string so no timezone can shift the day. */
function isoDate(iso: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  return `${MONTHS[m - 1]} ${d}, ${y}`
}

/** A monthly-grid cell in percent without the sign: "+4.1", "-2.3"; blank outside the window. */
const monthCell = (v: number | null) => (v === null ? '' : returnPct(v).replace('%', ''))

// ----- Public component -----

export interface GoldmanReportProps extends Omit<DocumentProps, 'children'> {
  brief: GoldmanBrief
}

export function GoldmanReport({ brief: raw, ...docProps }: GoldmanReportProps) {
  // Every string is made drawable by the PDF fonts first (a "−" would vanish).
  const brief = pdfSafeDeep(raw)
  const confidentialLabel = (brief.confidential_label ?? 'Confidential client brief').toUpperCase()

  return (
    <Document title={brief.document_label} author="Goldman Sax & Violins" {...docProps}>
      {/* Cover Page */}
      <Page size="LETTER" style={s.page}>
        <View style={s.brandRow}>
          <Text style={s.brandWord}>
            Goldman Sax <Text style={s.brandWordItalic}>&amp;</Text> Violins
          </Text>
          <Text style={s.brandDate}>{brief.generated_date.toUpperCase()}</Text>
        </View>

        <Text style={s.confidentialPill}>{confidentialLabel}</Text>

        <Text style={s.coverTitle}>{brief.cover_title?.main ?? 'Portfolio Analysis &'}</Text>
        <Text style={[s.coverTitle, s.coverTitleAccent, { paddingTop: 0 }]}>
          {brief.cover_title?.accent ?? 'Strategic Recommendations'}
        </Text>

        <Text style={s.coverSubtitle}>{brief.subtitle}</Text>

        <View style={s.coverDivider} />

        <View style={s.coverMetaRow}>
          <MetaCell label="PREPARED FOR" value={brief.client_name} />
          <MetaCell label="DOCUMENT" value={brief.document_label} />
          <MetaCell label="TEMPO" value={brief.tempo} />
        </View>

        {brief.facts && <FactPanel facts={brief.facts} />}
        {brief.comparison && <ComparisonPanel comparison={brief.comparison} />}

        <Text style={s.coverFooter}>
          Goldman Sax &amp; Violins, LLP · Equities &amp; Etudes Since 1869 · This document is satirical and
          is not financial advice.
        </Text>
      </Page>

      {/* Movements — each starts on its own page */}
      {brief.movements.map((m, i) => (
        <Page size="LETTER" style={s.page} key={i}>
          {renderMovement(m)}
          <RunningFooter brief={brief} />
        </Page>
      ))}
    </Document>
  )
}

function MetaCell({ label, value }: { label: string; value: string }) {
  return (
    <View style={s.coverMetaCell}>
      <Text style={s.coverMetaLabel}>{label}</Text>
      <Text style={s.coverMetaValue}>{value}</Text>
    </View>
  )
}

/** The account as the app measured it — every number here is computed, not written by Claude. */
function FactPanel({ facts }: { facts: BriefFacts }) {
  const pnl = `${signedUsd(facts.unrealized_pnl)}${
    facts.unrealized_pct !== null ? ` (${signedPct(facts.unrealized_pct)})` : ''
  }`
  return (
    <View style={s.facts} wrap={false}>
      <Text style={s.factsHeading}>The account, at the time of writing</Text>
      <View style={s.factsRow}>
        <Fact label="ACCOUNT" value={facts.account_type} />
        <Fact label="POSITIONS" value={String(facts.positions)} />
        <Fact label="PRICES AS OF" value={facts.prices_as_of} small />
      </View>
      <View style={s.factsRow}>
        <Fact label="MARKET VALUE" value={usd(facts.market_value)} />
        <Fact label="COST BASIS" value={usd(facts.cost_basis)} />
        <Fact label="UNREALIZED P/L" value={pnl} color={plColor(pnl)} />
      </View>
      <Text style={s.factNote}>
        Live quotes, which may be delayed up to 15 minutes. Value, cost and P/L cover the positions with a live
        price
        {facts.unpriced.length > 0
          ? `; ${facts.unpriced.length} without one ${facts.unpriced.length === 1 ? 'is' : 'are'} left out: ${facts.unpriced.join(', ')}.`
          : '.'}
      </Text>
    </View>
  )
}

/** Both portfolios at the same live prices — every number computed by the app. */
function ComparisonPanel({ comparison }: { comparison: BriefComparison }) {
  const { original: o, recommended: r } = comparison
  const cash = (v: number | null) => (v === null ? 'Not recorded' : usd(v))
  const total = (v: number | null) => (v === null ? 'Cash not recorded' : usd(v))
  const rows: [string, string, string][] = [
    ['ACCOUNT VALUE', total(o.total_value), total(r.total_value)],
    ['HOLDINGS', usd(o.holdings_value), usd(r.holdings_value)],
    ['CASH', cash(o.cash), cash(r.cash)],
    ['POSITIONS', String(o.positions), String(r.positions)],
  ]
  const unpriced = [...new Set([...o.unpriced, ...r.unpriced])]
  return (
    <View style={s.facts} wrap={false}>
      <Text style={s.factsHeading}>
        Both portfolios at {comparison.prices_as_of} · {comparison.account_type}
      </Text>
      <View style={s.compareHeaderRow}>
        <View style={s.compareLabelCell} />
        <SideHeading heading="ORIGINAL" name={o.name} style={s.compareValueCell} />
        <SideHeading heading="RECOMMENDED" name={r.name} style={s.compareValueCell} />
      </View>
      {rows.map(([label, a, b]) => (
        <View key={label} style={s.compareRow}>
          <Text style={[s.compareLabelCell, s.factLabel, { paddingBottom: 0, paddingTop: 3 }]}>{label}</Text>
          <Text style={[s.compareValueCell, s.compareValue]}>{a}</Text>
          <Text style={[s.compareValueCell, s.compareValue]}>{b}</Text>
        </View>
      ))}
      <Text style={s.factNote}>
        Live quotes, which may be delayed up to 15 minutes, applied to both portfolios. Holdings value covers the
        positions with a live price
        {unpriced.length > 0
          ? `; ${unpriced.length} without one ${unpriced.length === 1 ? 'is' : 'are'} left out: ${unpriced.join(', ')}.`
          : '.'}
      </Text>
    </View>
  )
}

/** A portfolio column heading: the column's heading ("ORIGINAL") over the scenario's name. */
function SideHeading({ heading, name, style }: { heading: string; name: string; style: ComponentProps<typeof View>["style"] }) {
  return (
    <View style={style}>
      <Text style={s.compareRole}>{heading}</Text>
      <Text style={s.compareName}>{name}</Text>
    </View>
  )
}

function Fact({
  label,
  value,
  color,
  small,
}: {
  label: string
  value: string
  color?: string
  small?: boolean
}) {
  return (
    <View style={s.factCell}>
      <Text style={s.factLabel}>{label}</Text>
      <Text style={[s.factValue, small ? { fontSize: 10, lineHeight: 1.3 } : {}, color ? { color } : {}]}>
        {value}
      </Text>
    </View>
  )
}

function RunningFooter({ brief }: { brief: GoldmanBrief }) {
  return (
    <View style={s.footer} fixed>
      <Text style={s.footerText}>
        Goldman Sax &amp; Violins · {brief.client_name} · {brief.generated_date}
      </Text>
      <Text
        style={s.footerText}
        render={({ pageNumber, totalPages }) => `Page ${pageNumber} of ${totalPages}`}
      />
    </View>
  )
}

// ----- Movement dispatcher -----

function renderMovement(m: GoldmanMovement) {
  switch (m.kind) {
    case 'diagnosis':
      return <DiagnosisView m={m} />
    case 'restructuring':
      return <RestructuringView m={m} />
    case 'tactical':
      return <TacticalView m={m} />
    case 'coda':
      return <CodaView m={m} />
    case 'narrative':
      return <NarrativeView m={m} />
    case 'changes':
      return <ChangesView m={m} />
    case 'measures':
      return <MeasuresView m={m} />
    case 'metrics':
      return <MetricsView m={m} />
    case 'health':
      return <HealthView m={m} />
  }
}

function MovementHeader({ m }: { m: GoldmanMovement }) {
  return (
    <View>
      <Text style={s.movementLabel}>
        {`MOVEMENT ${m.numeral}`} <Text style={s.movementTempo}>— {m.tempo}</Text>
      </Text>
      <Text style={s.movementTitle}>
        {m.title_main} <Text style={s.movementTitleAccent}>{m.title_accent}</Text>
      </Text>
      {m.intro && <Text style={s.movementIntro}>{m.intro}</Text>}
      <View style={s.redRule} />
    </View>
  )
}

/** A heading that never sits alone at the bottom of a page. */
function Heading({ children }: { children: React.ReactNode }) {
  return (
    <Text style={s.h2} minPresenceAhead={48}>
      {children}
    </Text>
  )
}

// ----- Diagnosis -----

function DiagnosisView({ m }: { m: DiagnosisMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      {m.issues.map((issue, i) => (
        <DiagnosisIssueRow key={i} issue={issue} index={i} />
      ))}
      {m.headline && (
        <View wrap={false}>
          <View style={{ height: 14 }} />
          <Text style={s.h3}>
            {m.headline.lead}{' '}
            <Text style={{ fontFamily: FONT.serifItalic, color: C.body }}>{m.headline.body}</Text>
          </Text>
        </View>
      )}
    </View>
  )
}

function DiagnosisIssueRow({ issue, index }: { issue: DiagnosisIssue; index: number }) {
  const numerals = ['i.', 'ii.', 'iii.', 'iv.', 'v.', 'vi.', 'vii.']
  return (
    <View style={s.numberedRow}>
      <Text style={s.numeral}>{numerals[index] ?? `${index + 1}.`}</Text>
      <View style={s.numberedBody}>
        <Heading>{issue.title}</Heading>
        <Text style={s.paragraph}>{issue.body}</Text>
        {issue.bullets.map((b, i) => (
          <PlainBullet key={i}>{b}</PlainBullet>
        ))}
      </View>
    </View>
  )
}

// ----- Restructuring -----

function RestructuringView({ m }: { m: RestructuringMovement }) {
  return (
    <View>
      <MovementHeader m={m} />

      <View style={s.allocationRow} wrap={false}>
        {m.allocation_cards.map((c, i) => (
          <View key={i} style={{ flex: 1, flexDirection: 'row' }}>
            {i > 0 && <View style={s.allocationGap} />}
            <AllocationCardView card={c} />
          </View>
        ))}
      </View>

      {m.proposed_core_table && (
        <CoreTable
          heading={m.proposed_core_table.heading}
          rows={m.proposed_core_table.rows}
          columns={['TICKER', 'NAME', 'CURRENT P/L', 'ACTION']}
        />
      )}

      {m.sector_sleeve && (
        <View>
          <Heading>{m.sector_sleeve.heading}</Heading>
          <Bullets items={m.sector_sleeve.bullets} />
        </View>
      )}

      {m.immediate_eliminations && (
        <View>
          <Heading>{m.immediate_eliminations.heading}</Heading>
          <Bullets items={m.immediate_eliminations.bullets} />
        </View>
      )}
    </View>
  )
}

function AllocationCardView({ card }: { card: AllocationCard }) {
  return (
    <View style={s.allocationCard}>
      <Text style={s.allocationPct}>{card.percentage}</Text>
      {card.caption && <Text style={s.allocationCaption}>{card.caption}</Text>}
      <Text style={s.allocationLabel}>{card.label.toUpperCase()}</Text>
      {card.symbols && card.symbols.length > 0 && (
        <Text style={s.allocationSymbols}>{card.symbols.join(', ')}</Text>
      )}
      <Text style={s.allocationBody}>{card.body}</Text>
    </View>
  )
}

// ----- Tactical -----

function TacticalView({ m }: { m: TacticalMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      {m.sections.map((sec, i) => (
        <TacticalSectionRow key={i} section={sec} />
      ))}
    </View>
  )
}

function TacticalSectionRow({ section }: { section: TacticalSection }) {
  // A section built around a short table moves to the next page as a unit, so
  // its heading and intro never sit apart from the table.
  const keepTogether = !!section.table && section.table.rows.length <= 8
  return (
    <View style={s.numberedRow} wrap={keepTogether ? false : undefined}>
      <Text style={s.numeral}>{section.number}.</Text>
      <View style={s.numberedBody}>
        <Heading>{section.title}</Heading>
        {section.body && <Text style={s.paragraph}>{section.body}</Text>}
        {section.table && (
          <View style={{ paddingTop: 8 }}>
            <CoreTable rows={section.table.rows} columns={section.table.columns} />
          </View>
        )}
        {section.bullets && <Bullets items={section.bullets} />}
      </View>
    </View>
  )
}

// ----- Coda -----

function CodaView({ m }: { m: CodaMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      {m.groups.map((g, i) => (
        <ChecklistGroupView key={i} group={g} />
      ))}
      {m.closing_note && (
        <Text style={s.closingNote} wrap={false}>
          {m.closing_note.lead} <Text style={s.closingBody}>{m.closing_note.body}</Text>
        </Text>
      )}
      <Text style={s.flourish}>—   ·   —</Text>
    </View>
  )
}

function ChecklistGroupView({ group }: { group: ChecklistGroup }) {
  return (
    <View>
      <Heading>{group.heading}</Heading>
      {group.items.map((item, i) => (
        <View key={i} style={s.checkRow} wrap={false}>
          <View style={s.checkBox} />
          <Text style={s.checkBody}>
            <Text style={s.checkLead}>{item.lead}</Text>
            {item.body ? <Text>{` ${item.body}`}</Text> : null}
          </Text>
        </View>
      ))}
    </View>
  )
}

// ----- Changes (comparison report) -----

const CHANGE_GROUPS: { kind: ChangeRow['kind']; heading: string }[] = [
  { kind: 'sold', heading: 'Sold' },
  { kind: 'trimmed', heading: 'Trimmed' },
  { kind: 'added', heading: 'Added to' },
  { kind: 'bought', heading: 'Bought' },
]

/** "$6,129.40"; sub-dollar prices keep four decimals ("$0.0012"). */
const price = (p: number | null) => {
  if (p === null) return 'No live price'
  const digits = p < 1 ? 4 : 2
  return p.toLocaleString('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })
}

/** Whole dollars from $10, cents below ("$0.20"), "under $0.01" for a sliver. Mirrors `value_text` in scenario_comparison.rs. */
const valueToday = (v: number) => {
  if (v >= 10) return usd(v)
  if (v > 0 && v < 0.005) return 'under $0.01'
  return v.toLocaleString('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

function ChangesView({ m }: { m: ChangesMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      {CHANGE_GROUPS.map(({ kind, heading }) => {
        const rows = m.rows.filter((r) => r.kind === kind)
        return rows.length > 0 ? <ChangeTable key={kind} heading={heading} rows={rows} /> : null
      })}
    </View>
  )
}

function ChangeTable({ heading, rows }: { heading: string; rows: ChangeRow[] }) {
  const cols: { label: string; width: number; right?: boolean }[] = [
    { label: 'TICKER', width: 0.11 },
    { label: 'CHANGE', width: 0.16 },
    { label: 'PRICE TODAY', width: 0.14, right: true },
    { label: 'VALUE TODAY', width: 0.14, right: true },
    { label: 'WHY', width: 0.45 },
  ]
  const cell = (i: number) => ({
    width: `${cols[i].width * 100}%`,
    paddingLeft: i === 0 ? 4 : i === 4 ? 12 : 0,
    paddingRight: cols[i].right ? 12 : 8,
    textAlign: (cols[i].right ? 'right' : 'left') as 'right' | 'left',
  })
  // A short table moves to the next page whole; a long one keeps its header with its first rows
  // and repeats it at the top of each page it continues onto.
  const unbreakable = rows.length <= 6
  return (
    <View wrap={!unbreakable}>
      <Heading>{heading}</Heading>
      <View style={s.tableHeaderRow} minPresenceAhead={unbreakable ? undefined : 60} fixed={!unbreakable}>
        {cols.map((c, i) => (
          <Text key={c.label} style={[s.tableHeaderCell, cell(i)]}>
            {c.label}
          </Text>
        ))}
      </View>
      {rows.map((r) => (
        <View key={r.ticker} style={s.tableRow} wrap={false}>
          <Text style={[s.tickerCell, cell(0)]}>{r.ticker}</Text>
          <Text style={[s.tableCell, cell(1)]}>{r.change}</Text>
          <Text style={[s.tableCell, cell(2)]}>{price(r.price)}</Text>
          <Text style={[s.tableCell, cell(3)]}>{r.value_today === null ? '—' : valueToday(r.value_today)}</Text>
          <Text style={[s.tableCell, cell(4), { fontSize: 9.5 }]}>{r.reason || '—'}</Text>
        </View>
      ))}
    </View>
  )
}

// ----- Measures (comparison report) -----

/** Label | original | recommended rows under the two portfolio headings. */
function SideBySideTable({
  column,
  originalLabel,
  recommendedLabel,
  rows,
}: {
  column: string
  originalLabel: string
  recommendedLabel: string
  rows: { label: string; original: string; recommended: string }[]
}) {
  const widths = ['36%', '32%', '32%']
  return (
    <View wrap={false}>
      <View style={s.tableHeaderRow}>
        <Text style={[s.tableHeaderCell, { width: widths[0], paddingLeft: 4 }]}>{column}</Text>
        <SideHeading heading="ORIGINAL" name={originalLabel} style={{ width: widths[1], paddingRight: 10 }} />
        <SideHeading heading="RECOMMENDED" name={recommendedLabel} style={{ width: widths[2], paddingRight: 10 }} />
      </View>
      {rows.map((row) => (
        <View key={row.label} style={s.tableRow}>
          <Text style={[s.tableCell, { width: widths[0], paddingLeft: 4, paddingRight: 10 }]}>{row.label}</Text>
          <Text style={[s.tableCell, { width: widths[1], paddingRight: 10 }]}>{row.original}</Text>
          <Text style={[s.tableCell, s.checkLead, { width: widths[2], paddingRight: 10 }]}>{row.recommended}</Text>
        </View>
      ))}
    </View>
  )
}

function MeasuresView({ m }: { m: MeasuresMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      <SideBySideTable
        column="MEASURE"
        originalLabel={m.original_label}
        recommendedLabel={m.recommended_label}
        rows={m.rows}
      />
      {m.note && <Text style={s.factNote}>{m.note}</Text>}
    </View>
  )
}

// ----- Metrics (comparison report) -----

function MetricsView({ m }: { m: MetricsMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      <Text style={s.windowLine}>{m.window}</Text>
      {m.rows.length > 0 && (
        <SideBySideTable
          column="METRIC"
          originalLabel={m.original_label}
          recommendedLabel={m.recommended_label}
          rows={m.rows}
        />
      )}
      {m.monthly.map((table) => (
        <MonthlyGrid key={table.label} table={table} />
      ))}
      {m.drawdowns.map((table) => (
        <DrawdownTableView key={table.label} table={table} />
      ))}
      {m.monte_carlo.length > 0 && (
        <View>
          <Heading>Monte Carlo</Heading>
          {m.monte_carlo.map((line) => (
            <Text key={line.label} style={s.paragraph} wrap={false}>
              <Text style={s.checkLead}>{line.label}.</Text> {monteCarloText(line)}
            </Text>
          ))}
        </View>
      )}
      <Text style={s.factNote}>{m.note}</Text>
    </View>
  )
}

/** Year + twelve months + YTD, in percent; a short table that moves to the next page whole. */
function MonthlyGrid({ table }: { table: MonthlyTable }) {
  const columns = ['YEAR', ...MONTHS.map((m) => m.toUpperCase()), 'YTD']
  const width = (i: number) => (i === 0 ? '9%' : i === 13 ? '10%' : '6.75%')
  const cell = (i: number) => ({
    width: width(i),
    paddingLeft: i === 0 ? 4 : 0,
    paddingRight: 3,
    textAlign: (i === 0 ? 'left' : 'right') as 'left' | 'right',
  })
  return (
    <View wrap={false}>
      <Text style={s.h3}>Monthly returns, % — {table.label}</Text>
      <View style={s.tableHeaderRow}>
        {columns.map((c, i) => (
          <Text key={c} style={[s.gridHeaderCell, cell(i)]}>
            {c}
          </Text>
        ))}
      </View>
      {table.years.map((y) => (
        <View key={y.year} style={s.gridRow}>
          <Text style={[s.gridCell, cell(0)]}>{y.year}</Text>
          {y.months.map((v, i) => (
            <Text key={MONTHS[i]} style={[s.gridCell, cell(i + 1), { color: plColor(monthCell(v)) }]}>
              {monthCell(v)}
            </Text>
          ))}
          <Text style={[s.gridCell, s.checkLead, cell(13), { color: plColor(monthCell(y.ytd)) }]}>
            {monthCell(y.ytd)}
          </Text>
        </View>
      ))}
    </View>
  )
}

/** The deepest peak-to-recovery episodes, deepest first. */
function DrawdownTableView({ table }: { table: DrawdownTable }) {
  const cols: { label: string; width: string; right?: boolean }[] = [
    { label: 'START', width: '22%' },
    { label: 'TROUGH', width: '22%' },
    { label: 'END', width: '26%' },
    { label: 'DEPTH', width: '16%', right: true },
    { label: 'DAYS', width: '14%', right: true },
  ]
  const cell = (i: number) => ({
    width: cols[i].width,
    paddingLeft: i === 0 ? 4 : 0,
    paddingRight: cols[i].right ? 12 : 8,
    textAlign: (cols[i].right ? 'right' : 'left') as 'right' | 'left',
  })
  return (
    <View wrap={false}>
      <Text style={s.h3}>Drawdowns — {table.label}</Text>
      <View style={s.tableHeaderRow}>
        {cols.map((c, i) => (
          <Text key={c.label} style={[s.tableHeaderCell, cell(i)]}>
            {c.label}
          </Text>
        ))}
      </View>
      {table.rows.map((r) => (
        <View key={`${r.start}-${r.trough}`} style={s.tableRow}>
          <Text style={[s.tableCell, cell(0)]}>{isoDate(r.start)}</Text>
          <Text style={[s.tableCell, cell(1)]}>{isoDate(r.trough)}</Text>
          <Text style={[s.tableCell, cell(2)]}>{r.end === null ? 'Not yet recovered' : isoDate(r.end)}</Text>
          <Text style={[s.plCell, cell(3), { color: plColor(returnPct(r.depth_pct)) }]}>{returnPct(r.depth_pct)}</Text>
          <Text style={[s.tableCell, cell(4)]}>{r.days}</Text>
        </View>
      ))}
    </View>
  )
}

// ----- Health (comparison report) -----

const STATUS_COLOR: Record<HealthStatus, string> = {
  pass: C.green,
  warn: C.gold,
  fail: C.burgundy,
  na: C.muted,
}

/** Check, exclamation, cross and dash paths in a 10×10 box. */
const STATUS_PATH: Record<HealthStatus, string> = {
  pass: 'M1.5 5.4 L4 7.8 L8.5 2.2',
  warn: 'M5 1.4 L5 6 M5 7.8 L5 8.6',
  fail: 'M2 2 L8 8 M8 2 L2 8',
  na: 'M2 5 L8 5',
}

const STATUS_WORD: Record<HealthStatus, string> = { pass: 'pass', warn: 'warn', fail: 'fail', na: 'not judged' }

/** A status mark drawn as a path: the built-in PDF fonts have no check or cross glyphs. */
function StatusGlyph({ status }: { status: HealthStatus }) {
  return (
    <Svg width={9} height={9} viewBox="0 0 10 10" style={{ marginTop: 2, marginRight: 5 }}>
      <Path
        d={STATUS_PATH[status]}
        stroke={STATUS_COLOR[status]}
        strokeWidth={1.6}
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </Svg>
  )
}

function HealthCellView({ cell, style }: { cell: HealthCell; style: ComponentProps<typeof View>['style'] }) {
  return (
    <View style={style}>
      <View style={{ flexDirection: 'row', alignItems: 'flex-start' }}>
        <StatusGlyph status={cell.status} />
        <Text style={[s.tableCell, { flex: 1 }]}>{cell.value_text}</Text>
      </View>
      {cell.detail && <Text style={s.cellDetail}>{cell.detail}</Text>}
    </View>
  )
}

function HealthView({ m }: { m: HealthMovement }) {
  const widths = ['22%', '27%', '27%', '24%']
  // As in ChangeTable: a short table moves to the next page whole; a long one keeps its
  // header with its first rows and repeats it at the top of each page it continues onto.
  const unbreakable = m.rows.length <= 6
  return (
    <View>
      <MovementHeader m={m} />
      <View wrap={!unbreakable}>
        <View style={s.tableHeaderRow} minPresenceAhead={unbreakable ? undefined : 60} fixed={!unbreakable}>
          <Text style={[s.tableHeaderCell, { width: widths[0], paddingLeft: 4 }]}>CHECK</Text>
          <SideHeading heading="ORIGINAL" name={m.original_label} style={{ width: widths[1], paddingRight: 8 }} />
          <SideHeading heading="RECOMMENDED" name={m.recommended_label} style={{ width: widths[2], paddingRight: 8 }} />
          <Text style={[s.tableHeaderCell, { width: widths[3] }]}>THRESHOLD</Text>
        </View>
        {m.rows.map((row) => (
          <View key={row.key} style={s.tableRow} wrap={false}>
            <Text style={[s.tableCell, s.checkLead, { width: widths[0], paddingLeft: 4, paddingRight: 8 }]}>{row.title}</Text>
            <HealthCellView cell={row.original} style={{ width: widths[1], paddingRight: 8 }} />
            <HealthCellView cell={row.recommended} style={{ width: widths[2], paddingRight: 8 }} />
            <Text style={[s.thresholdCell, { width: widths[3] }]}>{row.threshold}</Text>
          </View>
        ))}
      </View>
      <View style={s.legendRow} wrap={false}>
        {(['pass', 'warn', 'fail', 'na'] as HealthStatus[]).map((status) => (
          <View key={status} style={{ flexDirection: 'row', alignItems: 'flex-start' }}>
            <StatusGlyph status={status} />
            <Text style={s.legendText}>{STATUS_WORD[status]}</Text>
          </View>
        ))}
      </View>
      <Text style={s.factNote}>{m.note}</Text>
    </View>
  )
}

// ----- Narrative (fallback for non-portfolio reports) -----

function NarrativeView({ m }: { m: NarrativeMovement }) {
  return (
    <View>
      <MovementHeader m={m} />
      {m.paragraphs.map((p, i) => (
        <Text key={i} style={s.paragraph}>
          {p}
        </Text>
      ))}
      {m.bullets && <Bullets items={m.bullets} />}
    </View>
  )
}

// ----- Shared primitives -----

function PlainBullet({ children }: { children: React.ReactNode }) {
  return (
    <View style={s.bulletRow} wrap={false}>
      <Text style={s.noteGlyph}>•</Text>
      <Text style={s.bulletBody}>{children}</Text>
    </View>
  )
}

/**
 * Bullets, or — when eight or more each lead with a ticker — a two-column
 * ledger, so a long elimination list reads as a list instead of filling a page.
 */
function Bullets({ items }: { items: BulletItem[] }) {
  const allTickers = items.length >= 8 && items.every((b) => !!b.lead && TICKER.test(b.lead))
  if (allTickers) {
    return (
      <View style={s.ledger}>
        {items.map((b, i) => (
          <View key={i} style={s.ledgerCell} wrap={false}>
            <Text style={s.ledgerTicker}>{b.lead}</Text>
            {/* Drop a leading "— " separator, never a minus sign. */}
            <Text style={s.ledgerBody}>{b.body.replace(/^\s*[—–]\s+/, '')}</Text>
          </View>
        ))}
      </View>
    )
  }
  return (
    <>
      {items.map((b, i) => (
        <BulletItemView key={i} item={b} />
      ))}
    </>
  )
}

function BulletItemView({ item }: { item: BulletItem }) {
  const isTicker = !!item.lead && TICKER.test(item.lead)
  return (
    <View style={s.bulletRow} wrap={false}>
      <Text style={s.noteGlyph}>•</Text>
      <Text style={s.bulletBody}>
        {item.lead && (
          <Text style={isTicker ? s.bulletTickerLead : s.bulletLead}>
            {item.lead}
            {item.lead.endsWith(':') ? ' ' : isTicker ? ' — ' : ' '}
          </Text>
        )}
        <Text>{item.body}</Text>
      </Text>
    </View>
  )
}

function CoreTable({
  heading,
  rows,
  columns,
}: {
  /** Kept on the same page as the table. */
  heading?: string
  rows: PortfolioRow[]
  columns: TableColumn[]
}) {
  // Column widths (must sum to 1).
  const widthsByCols: Record<string, number[]> = {
    'TICKER,NAME,CURRENT P/L,ACTION': [0.12, 0.3, 0.16, 0.42],
    'TICKER,POSITION,P/L,ACTION': [0.12, 0.3, 0.14, 0.44],
    'TICKER,NAME,P/L,DECISION': [0.12, 0.28, 0.14, 0.46],
  }
  const widths = widthsByCols[columns.join(',')] ?? Array(columns.length).fill(1 / columns.length)
  const isPl = (col: string) => col === 'P/L' || col === 'CURRENT P/L'
  // Every cell gets the same gutter so neighbouring columns never touch.
  const cell = (i: number, col: string) => ({
    width: `${widths[i] * 100}%`,
    paddingLeft: i === 0 ? 4 : 0,
    paddingRight: isPl(col) ? 14 : 10,
  })

  // A short table moves to the next page whole, heading included; a long one
  // keeps its header with at least its first rows.
  const unbreakable = rows.length <= 8
  return (
    <View wrap={!unbreakable}>
      {heading && <Heading>{heading}</Heading>}
      <View style={s.tableHeaderRow} minPresenceAhead={unbreakable ? undefined : 60}>
        {columns.map((col, i) => (
          <Text key={col} style={[s.tableHeaderCell, cell(i, col), { textAlign: isPl(col) ? 'right' : 'left' }]}>
            {col}
          </Text>
        ))}
      </View>
      {rows.map((row, ri) => (
        <View key={ri} style={s.tableRow} wrap={unbreakable ? undefined : false}>
          {columns.map((col, ci) => {
            switch (col) {
              case 'TICKER':
                return (
                  <Text key={ci} style={[s.tickerCell, cell(ci, col)]}>
                    {row.ticker}
                  </Text>
                )
              case 'NAME':
              case 'POSITION':
                return (
                  <Text key={ci} style={[s.tableCell, cell(ci, col)]}>
                    {row.name}
                  </Text>
                )
              case 'P/L':
              case 'CURRENT P/L':
                return (
                  <Text key={ci} style={[s.plCell, cell(ci, col), { color: plColor(row.pl) }]}>
                    {row.pl}
                  </Text>
                )
              case 'ACTION':
              case 'DECISION':
                return (
                  <Text key={ci} style={[s.tableCell, cell(ci, col)]}>
                    {row.action}
                  </Text>
                )
            }
          })}
        </View>
      ))}
    </View>
  )
}
