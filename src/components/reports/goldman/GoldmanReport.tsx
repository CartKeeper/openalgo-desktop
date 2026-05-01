/**
 * Goldman Sax & Violins — house report template (React-PDF).
 *
 * Visual reference: cream paper, Times serif, gold/burgundy accents,
 * four-movement layout with tempo annotations and ♪ bullet motif.
 *
 * All exported reports for "recommendations and performance" use this template.
 * Brand wordmark, tagline, and disclaimer are intentionally hardcoded — house brand.
 */

import {
  Document,
  Page,
  StyleSheet,
  Text,
  View,
  type DocumentProps,
} from '@react-pdf/renderer'
import type {
  AllocationCard,
  BulletItem,
  ChecklistGroup,
  CodaMovement,
  DiagnosisIssue,
  DiagnosisMovement,
  GoldmanBrief,
  GoldmanMovement,
  NarrativeMovement,
  PortfolioRow,
  RestructuringMovement,
  TableColumn,
  TacticalMovement,
  TacticalSection,
} from './types'
import { C, FONT } from './tokens'

const s = StyleSheet.create({
  page: {
    backgroundColor: C.paper,
    paddingTop: 56,
    paddingHorizontal: 64,
    paddingBottom: 48,
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
  brandWord: {
    fontFamily: FONT.serif,
    fontSize: 22,
    color: C.gold,
  },
  brandWordItalic: {
    fontFamily: FONT.serifItalic,
    fontSize: 22,
    color: C.gold,
  },
  brandDate: {
    fontFamily: FONT.serif,
    fontSize: 9.5,
    color: C.muted,
    letterSpacing: 1.4,
  },
  confidentialPill: {
    fontFamily: FONT.serif,
    fontSize: 9.5,
    color: C.burgundy,
    letterSpacing: 3.2,
    paddingTop: 60,
  },
  coverTitle: {
    fontSize: 48,
    color: C.ink,
    lineHeight: 1.05,
    paddingTop: 24,
  },
  coverTitleAccent: {
    fontFamily: FONT.serifBoldItalic,
    color: C.gold,
  },
  coverSubtitle: {
    fontFamily: FONT.serifItalic,
    fontSize: 12,
    color: C.body,
    lineHeight: 1.45,
    paddingTop: 28,
  },
  coverDivider: {
    borderBottomWidth: 0.6,
    borderBottomColor: C.burgundy,
    marginTop: 80,
    marginBottom: 12,
  },
  coverMetaRow: {
    flexDirection: 'row',
    gap: 64,
  },
  coverMetaLabel: {
    fontSize: 8,
    color: C.muted,
    letterSpacing: 2.2,
    paddingBottom: 4,
  },
  coverMetaValue: {
    fontSize: 11,
    color: C.ink,
  },
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

  // Movement ----------------------------------------------------------
  movementLabel: {
    fontFamily: FONT.serif,
    fontSize: 9,
    color: C.burgundy,
    letterSpacing: 2.4,
    paddingTop: 4,
  },
  movementTempo: {
    fontFamily: FONT.serifItalic,
    fontSize: 9,
    color: C.muted,
  },
  movementTitle: {
    fontFamily: FONT.serif,
    fontSize: 26,
    color: C.ink,
    paddingTop: 6,
  },
  movementTitleAccent: {
    fontFamily: FONT.serifItalic,
    color: C.gold,
  },
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
  goldRule: {
    borderBottomWidth: 0.5,
    borderBottomColor: C.goldSoft,
    marginTop: 8,
    marginBottom: 8,
  },

  // Body --------------------------------------------------------------
  paragraph: {
    fontSize: 10.5,
    color: C.ink,
    lineHeight: 1.45,
    paddingTop: 6,
  },
  h2: {
    fontFamily: FONT.serifBold,
    fontSize: 13.5,
    color: C.gold,
    paddingTop: 14,
    paddingBottom: 4,
  },
  h3: {
    fontFamily: FONT.serifBold,
    fontSize: 11.5,
    color: C.ink,
    paddingTop: 10,
  },

  // Diagnosis numerals -------------------------------------------------
  romanRow: {
    flexDirection: 'row',
    paddingTop: 12,
  },
  romanNumeral: {
    fontFamily: FONT.serifItalic,
    fontSize: 13.5,
    color: C.burgundy,
    width: 22,
  },
  romanBody: { flex: 1 },

  // ♪ bullets ---------------------------------------------------------
  bulletRow: {
    flexDirection: 'row',
    paddingTop: 6,
    alignItems: 'flex-start',
  },
  noteGlyph: {
    fontFamily: FONT.serif,
    fontSize: 11,
    color: C.gold,
    width: 14,
    paddingTop: 0.5,
  },
  bulletBody: { flex: 1, fontSize: 10.5, lineHeight: 1.4 },
  bulletLead: { fontFamily: FONT.serifBold, color: C.ink },
  bulletTickerLead: { fontFamily: FONT.sansBold, color: C.gold, fontSize: 9.5 },

  // Allocation cards --------------------------------------------------
  allocationRow: {
    flexDirection: 'row',
    gap: 12,
    paddingTop: 12,
  },
  allocationCard: {
    flex: 1,
    borderWidth: 0.7,
    borderColor: C.goldSoft,
    paddingHorizontal: 11,
    paddingTop: 11,
    paddingBottom: 14,
    minHeight: 142,
  },
  allocationPct: {
    fontFamily: FONT.serifBold,
    fontSize: 22,
    color: C.gold,
  },
  allocationLabel: {
    fontFamily: FONT.serif,
    fontSize: 9,
    color: C.burgundy,
    letterSpacing: 2.4,
    paddingTop: 6,
  },
  allocationBody: {
    fontFamily: FONT.serif,
    fontSize: 9.5,
    color: C.body,
    lineHeight: 1.4,
    paddingTop: 8,
  },

  // Tables ------------------------------------------------------------
  tableHeaderRow: {
    flexDirection: 'row',
    borderBottomWidth: 1,
    borderBottomColor: C.gold,
    paddingBottom: 4,
    paddingTop: 4,
  },
  tableHeaderCell: {
    fontSize: 8,
    color: C.muted,
    letterSpacing: 1.6,
  },
  tableRow: {
    flexDirection: 'row',
    borderBottomWidth: 0.6,
    borderBottomColor: C.goldSoft,
    paddingVertical: 6,
  },
  tableCell: { fontSize: 10, color: C.ink, lineHeight: 1.3 },
  tickerCell: { fontFamily: FONT.sansBold, color: C.gold, fontSize: 10 },
  plPositive: { fontFamily: FONT.serifBold, color: C.green, fontSize: 10, textAlign: 'right' },
  plNegative: { fontFamily: FONT.serifBold, color: C.burgundy, fontSize: 10, textAlign: 'right' },

  // Tactical numbered sections ----------------------------------------
  tacticalRow: {
    flexDirection: 'row',
    paddingTop: 14,
  },
  tacticalNumeral: {
    fontFamily: FONT.serifItalic,
    fontSize: 13.5,
    color: C.burgundy,
    width: 22,
  },
  tacticalBody: { flex: 1 },

  // Coda checklist ----------------------------------------------------
  checkRow: {
    flexDirection: 'row',
    paddingTop: 6,
    alignItems: 'flex-start',
  },
  checkBox: {
    fontFamily: FONT.serif,
    fontSize: 13,
    color: C.gold,
    width: 18,
    paddingTop: 0.5,
  },
  checkBody: { flex: 1, fontSize: 10.5, lineHeight: 1.45 },
  checkLead: { fontFamily: FONT.serifBold, color: C.ink },

  // Closing note + flourish ------------------------------------------
  closingNote: {
    fontFamily: FONT.serifBold,
    fontSize: 10.5,
    color: C.ink,
    paddingTop: 18,
  },
  closingBody: { fontFamily: FONT.serifItalic, color: C.body, fontWeight: 'normal' },
  flourish: {
    paddingTop: 24,
    fontSize: 14,
    color: C.gold,
    textAlign: 'center',
  },
})

// ----- Public component -----

export interface GoldmanReportProps extends Omit<DocumentProps, 'children'> {
  brief: GoldmanBrief
}

export function GoldmanReport({ brief, ...docProps }: GoldmanReportProps) {
  const confidentialLabel = (
    brief.confidential_label ?? 'CONFIDENTIAL CLIENT BRIEF — 401(K)'
  )
    .toUpperCase()
    .split('')
    .join(' ')

  return (
    <Document
      title={brief.document_label}
      author="Goldman Sax & Violins"
      {...docProps}
    >
      {/* Cover Page */}
      <Page size="LETTER" style={s.page}>
        <View style={s.brandRow}>
          <Text style={s.brandWord}>
            Goldman Sax{' '}
            <Text style={s.brandWordItalic}>&amp; </Text>
            Violins
          </Text>
          <Text style={s.brandDate}>{brief.generated_date.toUpperCase()}</Text>
        </View>

        <Text style={s.confidentialPill}>{confidentialLabel}</Text>

        <Text style={s.coverTitle}>
          {/* Title: plain text + italic gold accent */}
          Portfolio Analysis &amp;{' '}
          <Text style={s.coverTitleAccent}>Strategic Recommendations</Text>
        </Text>

        <Text style={s.coverSubtitle}>{brief.subtitle}</Text>

        <View style={s.coverDivider} />

        <View style={s.coverMetaRow}>
          <View>
            <Text style={s.coverMetaLabel}>P R E P A R E D  F O R</Text>
            <Text style={s.coverMetaValue}>{brief.client_name}</Text>
          </View>
          <View>
            <Text style={s.coverMetaLabel}>D O C U M E N T</Text>
            <Text style={s.coverMetaValue}>{brief.document_label}</Text>
          </View>
          <View>
            <Text style={s.coverMetaLabel}>T E M P O</Text>
            <Text style={s.coverMetaValue}>{brief.tempo}</Text>
          </View>
        </View>

        <Text style={s.coverFooter}>
          Goldman Sax &amp; Violins, LLP · Equities &amp; Etudes Since 1869 · This document is satirical and is not financial advice.
        </Text>
      </Page>

      {/* Movements */}
      {brief.movements.map((m, i) => (
        <Page size="LETTER" style={s.page} key={i}>
          {renderMovement(m)}
        </Page>
      ))}
    </Document>
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
  }
}

function MovementHeader({ m }: { m: GoldmanMovement }) {
  return (
    <>
      <Text style={s.movementLabel}>
        {`MOVEMENT ${m.numeral}`.split('').join(' ')}{' '}
        <Text style={s.movementTempo}> — {m.tempo}</Text>
      </Text>
      <Text style={s.movementTitle}>
        {m.title_main}{' '}
        <Text style={s.movementTitleAccent}>{m.title_accent}</Text>
      </Text>
      {m.intro && <Text style={s.movementIntro}>{m.intro}</Text>}
      <View style={s.redRule} />
    </>
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
        <View>
          <View style={{ height: 14 }} />
          <Text style={s.h3}>
            {m.headline.lead}{' '}
            <Text style={{ fontFamily: FONT.serifItalic, color: C.body }}>
              {m.headline.body}
            </Text>
          </Text>
        </View>
      )}
    </View>
  )
}

function DiagnosisIssueRow({ issue, index }: { issue: DiagnosisIssue; index: number }) {
  const numerals = ['i.', 'ii.', 'iii.', 'iv.', 'v.', 'vi.', 'vii.']
  return (
    <View style={s.romanRow}>
      <Text style={s.romanNumeral}>{numerals[index] ?? `${index + 1}.`}</Text>
      <View style={s.romanBody}>
        <Text style={s.h2}>{issue.title}</Text>
        <Text style={s.paragraph}>{issue.body}</Text>
        {issue.bullets.map((b, i) => (
          <MusicalBullet key={i}>{b}</MusicalBullet>
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

      <View style={s.allocationRow}>
        {m.allocation_cards.map((c, i) => (
          <AllocationCardView key={i} card={c} />
        ))}
      </View>

      {m.proposed_core_table && (
        <View>
          <Text style={s.h2}>{m.proposed_core_table.heading}</Text>
          <CoreTable rows={m.proposed_core_table.rows} columns={['TICKER', 'NAME', 'CURRENT P/L', 'ACTION']} />
        </View>
      )}

      {m.sector_sleeve && (
        <View>
          <Text style={s.h2}>{m.sector_sleeve.heading}</Text>
          {m.sector_sleeve.bullets.map((b, i) => (
            <BulletItemView key={i} item={b} />
          ))}
        </View>
      )}

      {m.immediate_eliminations && (
        <View>
          <Text style={s.h2}>{m.immediate_eliminations.heading}</Text>
          {m.immediate_eliminations.bullets.map((b, i) => (
            <BulletItemView key={i} item={b} />
          ))}
        </View>
      )}
    </View>
  )
}

function AllocationCardView({ card }: { card: AllocationCard }) {
  return (
    <View style={s.allocationCard}>
      <Text style={s.allocationPct}>{card.percentage}</Text>
      <Text style={s.allocationLabel}>
        {card.label.toUpperCase().split('').join(' ')}
      </Text>
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
  return (
    <View style={s.tacticalRow}>
      <Text style={s.tacticalNumeral}>{section.number}.</Text>
      <View style={s.tacticalBody}>
        <Text style={s.h2}>{section.title}</Text>
        {section.body && <Text style={s.paragraph}>{section.body}</Text>}
        {section.table && (
          <View style={{ paddingTop: 8 }}>
            <CoreTable rows={section.table.rows} columns={section.table.columns} />
          </View>
        )}
        {section.bullets &&
          section.bullets.map((b, i) => <BulletItemView key={i} item={b} />)}
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
        <Text style={s.closingNote}>
          {m.closing_note.lead}{' '}
          <Text style={s.closingBody}>{m.closing_note.body}</Text>
        </Text>
      )}
      <Text style={s.flourish}>⚜  ♪  ⚜</Text>
    </View>
  )
}

function ChecklistGroupView({ group }: { group: ChecklistGroup }) {
  return (
    <View>
      <Text style={s.h2}>{group.heading}</Text>
      {group.items.map((item, i) => (
        <View key={i} style={s.checkRow}>
          <Text style={s.checkBox}>□</Text>
          <Text style={s.checkBody}>
            <Text style={s.checkLead}>{item.lead}</Text>
            {item.body ? <Text>{` ${item.body}`}</Text> : null}
          </Text>
        </View>
      ))}
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
      {m.bullets &&
        m.bullets.map((b, i) => <BulletItemView key={i} item={b} />)}
    </View>
  )
}

// ----- Shared primitives -----

function MusicalBullet({ children }: { children: React.ReactNode }) {
  return (
    <View style={s.bulletRow}>
      <Text style={s.noteGlyph}>♪</Text>
      <Text style={s.bulletBody}>{children}</Text>
    </View>
  )
}

function BulletItemView({ item }: { item: BulletItem }) {
  // Heuristic: if `lead` is all uppercase letters/digits ≤ 5 chars, render as ticker (Arial gold)
  const isTicker = !!item.lead && /^[A-Z0-9.\-]{1,5}$/.test(item.lead)
  return (
    <View style={s.bulletRow}>
      <Text style={s.noteGlyph}>♪</Text>
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

function CoreTable({ rows, columns }: { rows: PortfolioRow[]; columns: TableColumn[] }) {
  // Column widths (must sum to 1) — picked to match HTML proportions.
  const widthsByCols: Record<string, number[]> = {
    'TICKER,NAME,CURRENT P/L,ACTION': [0.12, 0.32, 0.18, 0.38],
    'TICKER,POSITION,P/L,ACTION': [0.12, 0.36, 0.14, 0.38],
    'TICKER,NAME,P/L,DECISION': [0.14, 0.26, 0.12, 0.48],
  }
  const key = columns.join(',')
  const widths = widthsByCols[key] ?? Array(columns.length).fill(1 / columns.length)

  return (
    <View>
      <View style={s.tableHeaderRow}>
        {columns.map((col, i) => {
          const isPlOrAction = col === 'P/L' || col === 'CURRENT P/L'
          return (
            <Text
              key={col}
              style={[
                s.tableHeaderCell,
                {
                  width: `${widths[i] * 100}%`,
                  textAlign: isPlOrAction ? 'right' : 'left',
                  paddingRight: isPlOrAction ? 6 : 0,
                  paddingLeft: i === 0 ? 4 : 0,
                },
              ]}
            >
              {col.split('').join(' ')}
            </Text>
          )
        })}
      </View>
      {rows.map((row, ri) => (
        <View key={ri} style={s.tableRow}>
          {columns.map((col, ci) => {
            const w = `${widths[ci] * 100}%`
            switch (col) {
              case 'TICKER':
                return (
                  <Text key={ci} style={[s.tickerCell, { width: w, paddingLeft: 4 }]}>
                    {row.ticker}
                  </Text>
                )
              case 'NAME':
              case 'POSITION':
                return (
                  <Text key={ci} style={[s.tableCell, { width: w }]}>
                    {row.name}
                  </Text>
                )
              case 'P/L':
              case 'CURRENT P/L':
                return (
                  <Text
                    key={ci}
                    style={[row.pl_positive ? s.plPositive : s.plNegative, { width: w, paddingRight: 6 }]}
                  >
                    {row.pl}
                  </Text>
                )
              case 'ACTION':
              case 'DECISION':
                return (
                  <Text key={ci} style={[s.tableCell, { width: w }]}>
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
