/**
 * Goldman Sax & Violins — research / briefing export.
 *
 * Used by the existing copilot-driven report pipeline (`pdf-report.tsx`).
 * Consumes the legacy `(title, date, tickers, messages, notes)` shape and
 * renders it through the Goldman house aesthetic: cream paper, Times serif,
 * gold/burgundy accents, ♪ bullets, ⚜ ♪ ⚜ closing flourish.
 *
 * Each assistant message becomes a "Movement" with a tempo annotation. Notes
 * (if any) are appended as a final movement called "Marginalia."
 */

import {
  Document,
  Page,
  StyleSheet,
  Text,
  View,
  type DocumentProps,
} from '@react-pdf/renderer'
import type { CopilotMessage } from '@/stores/copilotStore'
import type { ReportNote } from '@/stores/reportsStore'
import { extractTitle, renderMarkdown } from './markdown'
import { C, FONT } from './tokens'

const TEMPI = [
  'Andante, deliberate',
  'Allegro, with vigor',
  'Moderato, measured',
  'Vivace, lively',
  'Cantabile, singing',
  'Grazioso, graceful',
  'Marcato, marked',
  'Coda, rallentando',
]

const ROMAN = ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X']

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
  brandWord: { fontFamily: FONT.serif, fontSize: 22, color: C.gold },
  brandWordItalic: { fontFamily: FONT.serifItalic, fontSize: 22, color: C.gold },
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
    fontSize: 42,
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
    paddingTop: 24,
  },
  tickerRow: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 8,
    paddingTop: 18,
  },
  ticker: {
    fontFamily: FONT.sansBold,
    fontSize: 9,
    color: C.gold,
    paddingHorizontal: 7,
    paddingVertical: 3,
    borderWidth: 0.7,
    borderColor: C.goldSoft,
    letterSpacing: 0.5,
  },
  coverDivider: {
    borderBottomWidth: 0.6,
    borderBottomColor: C.burgundy,
    marginTop: 60,
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

  // Movement -----------------------------------------------------------
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
    fontSize: 24,
    color: C.ink,
    paddingTop: 6,
  },
  movementTitleAccent: {
    fontFamily: FONT.serifItalic,
    color: C.gold,
  },
  redRule: {
    borderBottomWidth: 0.5,
    borderBottomColor: C.burgundy,
    marginTop: 14,
    marginBottom: 12,
  },

  // Marginalia (notes) -------------------------------------------------
  noteCard: {
    borderWidth: 0.7,
    borderColor: C.goldSoft,
    paddingHorizontal: 11,
    paddingVertical: 8,
    marginTop: 10,
  },
  noteMeta: {
    fontFamily: FONT.serifItalic,
    fontSize: 8.5,
    color: C.muted,
    paddingBottom: 4,
  },
  noteBody: {
    fontFamily: FONT.serif,
    fontSize: 10.5,
    color: C.ink,
    lineHeight: 1.45,
  },

  // Closing flourish ---------------------------------------------------
  flourish: {
    paddingTop: 24,
    fontSize: 14,
    color: C.gold,
    textAlign: 'center',
  },

  // Page footer (running) ----------------------------------------------
  pageFooter: {
    position: 'absolute',
    bottom: 24,
    left: 64,
    right: 64,
    textAlign: 'center',
    fontFamily: FONT.serifItalic,
    fontSize: 7,
    color: C.muted,
  },
})

export interface GoldmanResearchReportProps extends Omit<DocumentProps, 'children'> {
  title: string
  date: string
  tickers: string[]
  sections: CopilotMessage[]
  notes?: ReportNote[]
}

export function GoldmanResearchReport({
  title,
  date,
  tickers,
  sections,
  notes,
  ...docProps
}: GoldmanResearchReportProps) {
  // Filter to assistant messages with content (skip user prompts and empty replies)
  const assistantSections = sections.filter(
    (m) => m.role === 'assistant' && m.content && m.content.trim().length > 0,
  )

  const confidentialLabel = 'CONFIDENTIAL RESEARCH BRIEF'
    .split('')
    .join(' ')

  // Long titles like "Scenario Analysis — Current Portfolio — Traditional IRA
  // (May 1, 2026)" wrap badly at 42pt. Split on em-dash so the first segment
  // becomes the cover headline and the rest becomes a subtitle context line.
  const titleParts = title.split(/\s+—\s+/)
  const coverHeadline = titleParts[0]
  const coverContext = titleParts.length > 1 ? titleParts.slice(1).join(' · ') : null

  // Cap ticker chips so a runaway list (e.g. when tickers got auto-extracted
  // from a long conversation) doesn't dominate the cover.
  const tickerLimit = 12
  const tickerChips = tickers.slice(0, tickerLimit)
  const tickerOverflow = tickers.length > tickerLimit ? tickers.length - tickerLimit : 0

  return (
    <Document title={title} author="Goldman Sax & Violins" {...docProps}>
      {/* Cover */}
      <Page size="LETTER" style={s.page}>
        <View style={s.brandRow}>
          <Text style={s.brandWord}>
            Goldman Sax{' '}
            <Text style={s.brandWordItalic}>&amp; </Text>
            Violins
          </Text>
          <Text style={s.brandDate}>{date.toUpperCase()}</Text>
        </View>

        <Text style={s.confidentialPill}>{confidentialLabel}</Text>

        <Text style={s.coverTitle}>
          <Text style={s.coverTitleAccent}>{coverHeadline}</Text>
        </Text>

        {coverContext && (
          <Text style={s.coverSubtitle}>{coverContext}</Text>
        )}

        {tickerChips.length > 0 && (
          <View style={s.tickerRow}>
            {tickerChips.map((t) => (
              <Text key={t} style={s.ticker}>
                {t}
              </Text>
            ))}
            {tickerOverflow > 0 && (
              <Text style={s.ticker}>+{tickerOverflow} more</Text>
            )}
          </View>
        )}

        <View style={s.coverDivider} />
        <View style={s.coverMetaRow}>
          <View>
            <Text style={s.coverMetaLabel}>D O C U M E N T</Text>
            <Text style={s.coverMetaValue}>{title}</Text>
          </View>
          <View>
            <Text style={s.coverMetaLabel}>T E M P O</Text>
            <Text style={s.coverMetaValue}>
              {assistantSections.length} {assistantSections.length === 1 ? 'Movement' : 'Movements'}
            </Text>
          </View>
        </View>

        <Text style={s.coverFooter}>
          Goldman Sax &amp; Violins, LLP · Equities &amp; Etudes Since 1869 · This document is satirical and is not financial advice.
        </Text>
      </Page>

      {/* Movements */}
      {assistantSections.map((msg, i) => {
        const { title: secTitle, body } = extractTitle(msg.content)
        const tempo = TEMPI[i % TEMPI.length]
        const numeral = ROMAN[i] ?? `${i + 1}`
        return (
          <Page size="LETTER" style={s.page} key={msg.id || i}>
            <Text style={s.movementLabel}>
              {`MOVEMENT ${numeral}`.split('').join(' ')}
              <Text style={s.movementTempo}> — {tempo}</Text>
            </Text>
            <Text style={s.movementTitle}>
              <Text style={s.movementTitleAccent}>{secTitle}</Text>
            </Text>
            <View style={s.redRule} />
            {renderMarkdown(body || msg.content)}
            <Text
              style={s.pageFooter}
              render={({ pageNumber, totalPages }) =>
                `${title} · ${pageNumber} / ${totalPages}`
              }
              fixed
            />
          </Page>
        )
      })}

      {/* Marginalia (notes) */}
      {notes && notes.length > 0 && (
        <Page size="LETTER" style={s.page}>
          <Text style={s.movementLabel}>
            {`MOVEMENT ${ROMAN[assistantSections.length] ?? assistantSections.length + 1}`
              .split('')
              .join(' ')}
            <Text style={s.movementTempo}> — Coda, rallentando</Text>
          </Text>
          <Text style={s.movementTitle}>
            <Text style={s.movementTitleAccent}>Marginalia</Text>
          </Text>
          <View style={s.redRule} />
          {notes.map((n) => (
            <View key={n.id} style={s.noteCard} wrap={false}>
              <Text style={s.noteMeta}>
                {new Date(n.created_at + 'Z').toLocaleString('en-US', {
                  month: 'short',
                  day: 'numeric',
                  year: 'numeric',
                  hour: 'numeric',
                  minute: '2-digit',
                  hour12: true,
                })}
              </Text>
              <Text style={s.noteBody}>{n.content}</Text>
            </View>
          ))}
          <Text style={s.flourish}>—   ·   —</Text>
          <Text
            style={s.pageFooter}
            render={({ pageNumber, totalPages }) =>
              `${title} · ${pageNumber} / ${totalPages}`
            }
            fixed
          />
        </Page>
      )}
      {/* No standalone closing-flourish page — when there are no notes the last
          movement's natural end is sufficient. The previous behavior emitted
          an extra page that frequently rendered with just a misencoded glyph. */}
    </Document>
  )
}
