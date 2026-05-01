import React from 'react'
import {
  Document,
  Page,
  View,
  Text,
  Font,
  StyleSheet,
} from '@react-pdf/renderer'
import type { CopilotMessage } from '@/stores/copilotStore'
import type { ReportNote } from '@/stores/reportsStore'

// Disable hyphenation for cleaner text
Font.registerHyphenationCallback((word) => [word])

// ========== Colors ==========
const C = {
  accent: '#4f46e5',
  accentDark: '#3730a3',
  accentLight: '#c7d2fe',
  accentBg: '#eef2ff',
  heading: '#0f172a',
  body: '#374151',
  muted: '#6b7280',
  light: '#9ca3af',
  border: '#e5e7eb',
  borderLight: '#f3f4f6',
  surface: '#f8fafc',
  white: '#ffffff',
  noteYellow: '#fefce8',
  noteBorder: '#fde68a',
  noteText: '#92400e',
}

// ========== Styles ==========
const s = StyleSheet.create({
  // ----- Cover -----
  coverPage: {
    padding: '48pt 48pt 40pt',
    backgroundColor: C.white,
    fontFamily: 'Helvetica',
  },
  coverBar: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    height: 6,
    backgroundColor: C.accent,
  },
  coverBrand: {
    fontSize: 9,
    fontFamily: 'Helvetica-Bold',
    color: C.accent,
    letterSpacing: 3,
    textTransform: 'uppercase',
    marginTop: 24,
    marginBottom: 24,
  },
  coverTitle: {
    fontSize: 28,
    fontFamily: 'Helvetica-Bold',
    color: C.heading,
    lineHeight: 1.2,
    marginBottom: 16,
    maxWidth: '85%',
  },
  coverDate: {
    fontSize: 11,
    color: C.muted,
    marginBottom: 24,
  },
  coverTickerRow: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 8,
    marginBottom: 40,
  },
  coverTicker: {
    paddingVertical: 4,
    paddingHorizontal: 12,
    borderRadius: 4,
    backgroundColor: C.accentBg,
    borderWidth: 1,
    borderColor: C.accentLight,
  },
  coverTickerText: {
    fontSize: 11,
    fontFamily: 'Courier-Bold',
    color: C.accentDark,
    letterSpacing: 0.5,
  },
  coverDivider: {
    height: 1,
    backgroundColor: C.border,
    marginTop: 'auto',
    marginBottom: 20,
  },
  coverDisclaimer: {
    fontSize: 8,
    color: C.light,
    lineHeight: 1.5,
  },

  // ----- Content page -----
  contentPage: {
    paddingTop: 52,
    paddingBottom: 44,
    paddingHorizontal: 48,
    backgroundColor: C.white,
    fontFamily: 'Helvetica',
    fontSize: 10,
    color: C.body,
  },

  // ----- Fixed header -----
  header: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    paddingHorizontal: 48,
    paddingTop: 18,
    paddingBottom: 8,
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    borderBottomWidth: 0.5,
    borderBottomColor: C.border,
  },
  headerLabel: {
    fontSize: 7,
    fontFamily: 'Helvetica-Bold',
    color: C.light,
    letterSpacing: 2,
    textTransform: 'uppercase',
  },
  headerPage: {
    fontSize: 8,
    fontFamily: 'Courier',
    color: C.light,
  },

  // ----- Fixed footer -----
  footer: {
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    paddingHorizontal: 48,
    paddingBottom: 14,
    paddingTop: 6,
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    borderTopWidth: 0.5,
    borderTopColor: C.border,
  },
  footerText: {
    fontSize: 7,
    color: C.light,
  },
  footerBrand: {
    fontSize: 7,
    fontFamily: 'Helvetica-Bold',
    color: C.accent,
  },

  // ----- TOC -----
  tocHeading: {
    fontSize: 8,
    fontFamily: 'Helvetica-Bold',
    color: C.muted,
    letterSpacing: 2,
    textTransform: 'uppercase',
    marginBottom: 12,
  },
  tocRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    marginBottom: 7,
  },
  tocNum: {
    fontSize: 9,
    fontFamily: 'Courier-Bold',
    color: C.accent,
    width: 20,
  },
  tocLabel: {
    fontSize: 10,
    color: C.heading,
    flex: 1,
  },

  // ----- Section -----
  section: {
    marginBottom: 18,
  },
  sectionHead: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    marginBottom: 10,
  },
  sectionBadge: {
    width: 22,
    height: 22,
    borderRadius: 11,
    backgroundColor: C.accent,
    justifyContent: 'center',
    alignItems: 'center',
  },
  sectionBadgeNum: {
    fontSize: 10,
    fontFamily: 'Helvetica-Bold',
    color: C.white,
  },
  sectionTitle: {
    fontSize: 14,
    fontFamily: 'Helvetica-Bold',
    color: C.heading,
    flex: 1,
  },
  sectionTools: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 4,
    marginBottom: 8,
    paddingLeft: 32,
  },
  toolPill: {
    paddingVertical: 1,
    paddingHorizontal: 6,
    borderRadius: 3,
    backgroundColor: C.surface,
    borderWidth: 0.5,
    borderColor: C.border,
  },
  toolPillText: {
    fontSize: 7,
    fontFamily: 'Helvetica-Bold',
    color: C.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  sectionBody: {
    paddingLeft: 14,
    borderLeftWidth: 1.5,
    borderLeftColor: C.accentLight,
  },
  sectionDivider: {
    height: 0.5,
    backgroundColor: C.borderLight,
    marginTop: 14,
  },

  // ----- Markdown -----
  h1: {
    fontSize: 14,
    fontFamily: 'Helvetica-Bold',
    color: C.heading,
    marginTop: 14,
    marginBottom: 6,
    paddingBottom: 4,
    borderBottomWidth: 1.5,
    borderBottomColor: C.accent,
  },
  h2: {
    fontSize: 12,
    fontFamily: 'Helvetica-Bold',
    color: C.accent,
    marginTop: 12,
    marginBottom: 5,
    paddingBottom: 3,
    borderBottomWidth: 0.5,
    borderBottomColor: C.accentLight,
  },
  h3: {
    fontSize: 11,
    fontFamily: 'Helvetica-Bold',
    color: C.heading,
    marginTop: 10,
    marginBottom: 4,
  },
  h4: {
    fontSize: 9,
    fontFamily: 'Helvetica-Bold',
    color: C.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
    marginTop: 8,
    marginBottom: 3,
  },
  p: {
    fontSize: 10,
    lineHeight: 1.65,
    color: C.body,
    marginBottom: 3,
  },
  bold: {
    fontFamily: 'Helvetica-Bold',
    color: C.heading,
  },
  italic: {
    fontStyle: 'italic',
  },
  inlineCode: {
    fontFamily: 'Courier',
    fontSize: 9,
    backgroundColor: C.borderLight,
    color: C.accent,
  },
  codeBlock: {
    backgroundColor: C.surface,
    borderWidth: 0.5,
    borderColor: C.border,
    borderRadius: 4,
    padding: 10,
    marginVertical: 6,
  },
  codeBlockLang: {
    fontSize: 7,
    fontFamily: 'Helvetica-Bold',
    color: C.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    marginBottom: 6,
    paddingBottom: 4,
    borderBottomWidth: 0.5,
    borderBottomColor: C.border,
  },
  codeBlockBody: {
    fontFamily: 'Courier',
    fontSize: 8,
    lineHeight: 1.5,
    color: C.body,
  },
  listItem: {
    flexDirection: 'row',
    gap: 6,
    marginBottom: 2,
  },
  bullet: {
    fontSize: 10,
    color: C.accent,
    width: 8,
  },
  listText: {
    fontSize: 10,
    lineHeight: 1.6,
    color: C.body,
    flex: 1,
  },
  olNum: {
    fontSize: 9,
    fontFamily: 'Courier-Bold',
    color: C.accent,
    width: 16,
  },
  blockquote: {
    borderLeftWidth: 2,
    borderLeftColor: C.accent,
    backgroundColor: C.accentBg,
    paddingVertical: 8,
    paddingHorizontal: 12,
    borderTopRightRadius: 4,
    borderBottomRightRadius: 4,
    marginVertical: 6,
  },
  bqText: {
    fontSize: 10,
    fontStyle: 'italic',
    lineHeight: 1.5,
    color: C.accentDark,
  },
  hr: {
    height: 0.5,
    backgroundColor: C.border,
    marginVertical: 10,
  },
  blank: {
    height: 4,
  },

  // ----- Tables -----
  table: {
    borderWidth: 0.5,
    borderColor: C.border,
    borderRadius: 4,
    overflow: 'hidden',
    marginVertical: 8,
  },
  tHeaderRow: {
    flexDirection: 'row',
    backgroundColor: C.heading,
  },
  tHeaderCell: {
    paddingVertical: 6,
    paddingHorizontal: 8,
  },
  tHeaderText: {
    fontSize: 8,
    fontFamily: 'Helvetica-Bold',
    color: C.white,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  tRow: {
    flexDirection: 'row',
    borderBottomWidth: 0.5,
    borderBottomColor: C.borderLight,
  },
  tRowAlt: {
    flexDirection: 'row',
    borderBottomWidth: 0.5,
    borderBottomColor: C.borderLight,
    backgroundColor: C.surface,
  },
  tCell: {
    paddingVertical: 5,
    paddingHorizontal: 8,
  },
  tCellText: {
    fontSize: 9,
    color: C.body,
    lineHeight: 1.4,
  },

  // ----- Notes -----
  notesTitle: {
    fontSize: 13,
    fontFamily: 'Helvetica-Bold',
    color: C.heading,
    marginBottom: 10,
    marginTop: 8,
  },
  noteCard: {
    padding: 10,
    borderRadius: 4,
    backgroundColor: C.noteYellow,
    borderWidth: 0.5,
    borderColor: C.noteBorder,
    marginBottom: 6,
  },
  noteContent: {
    fontSize: 9,
    lineHeight: 1.6,
    color: C.noteText,
  },
  noteDate: {
    fontSize: 7,
    fontFamily: 'Courier',
    color: C.light,
    marginTop: 4,
  },
})

// ========== Table helpers ==========

function isTableRow(line: string): boolean {
  const t = line.trim()
  return t.startsWith('|') && t.endsWith('|') && t.split('|').length >= 3
}

function isTableSep(line: string): boolean {
  return /^\|[\s\-:|]+\|$/.test(line.trim())
}

function parseCells(line: string): string[] {
  return line
    .split('|')
    .slice(1, -1)
    .map((c) => c.trim())
}

function renderTable(lines: string[], key: number): React.ReactNode {
  const headers = parseCells(lines[0])
  const rows: string[][] = []
  for (let i = 2; i < lines.length; i++) {
    if (!isTableSep(lines[i])) rows.push(parseCells(lines[i]))
  }
  const colW = `${Math.floor(100 / headers.length)}%`

  const isNum = headers.map((_, ci) =>
    rows.length > 0 &&
    rows.every((r) => {
      const c = r[ci] || ''
      return (
        /^[\s$%,.\-+\d]+$/.test(c) || c === '' || c === '\u2014' || c === 'N/A'
      )
    })
  )

  return (
    <View key={`tbl-${key}`} style={s.table} wrap={false}>
      <View style={s.tHeaderRow}>
        {headers.map((h, ci) => (
          <View key={ci} style={[s.tHeaderCell, { width: colW }]}>
            <Text
              style={[
                s.tHeaderText,
                isNum[ci] ? { textAlign: 'right' as const } : undefined,
              ]}
            >
              {h}
            </Text>
          </View>
        ))}
      </View>
      {rows.map((row, ri) => (
        <View key={ri} style={ri % 2 === 1 ? s.tRowAlt : s.tRow}>
          {row.map((cell, ci) => (
            <View key={ci} style={[s.tCell, { width: colW }]}>
              <Text
                style={[
                  s.tCellText,
                  ci === 0
                    ? { fontFamily: 'Helvetica-Bold', color: C.heading }
                    : undefined,
                  isNum[ci]
                    ? { fontFamily: 'Courier', textAlign: 'right' as const }
                    : undefined,
                ]}
              >
                {cell}
              </Text>
            </View>
          ))}
        </View>
      ))}
    </View>
  )
}

// ========== Inline formatting ==========

function renderInline(text: string): React.ReactNode {
  const parts: React.ReactNode[] = []
  let rest = text
  let k = 0

  while (rest.length > 0) {
    const cm = rest.match(/^(.*?)`([^`]+)`(.*)$/)
    if (cm) {
      if (cm[1]) parts.push(...boldItalic(cm[1], k))
      k += 10
      parts.push(
        <Text key={`c-${k++}`} style={s.inlineCode}>
          {cm[2]}
        </Text>
      )
      rest = cm[3]
      continue
    }
    parts.push(...boldItalic(rest, k))
    break
  }

  if (parts.length === 0) return text
  if (parts.length === 1) return parts[0]
  return <>{parts}</>
}

function boldItalic(text: string, sk: number): React.ReactNode[] {
  const parts: React.ReactNode[] = []
  let rest = text
  let k = sk

  while (rest.length > 0) {
    const bm = rest.match(/^(.*?)\*\*(.+?)\*\*(.*)$/)
    if (bm) {
      if (bm[1]) parts.push(bm[1])
      parts.push(
        <Text key={`b-${k++}`} style={s.bold}>
          {bm[2]}
        </Text>
      )
      rest = bm[3]
      continue
    }
    const im = rest.match(/^(.*?)\*(.+?)\*(.*)$/)
    if (im) {
      if (im[1]) parts.push(im[1])
      parts.push(
        <Text key={`i-${k++}`} style={s.italic}>
          {im[2]}
        </Text>
      )
      rest = im[3]
      continue
    }
    parts.push(rest)
    break
  }
  return parts
}

// ========== Markdown -> react-pdf ==========

function renderMd(text: string): React.ReactNode[] {
  const lines = text.split('\n')
  const out: React.ReactNode[] = []
  let i = 0
  let cb: string[] | null = null
  let cbLang = ''

  while (i < lines.length) {
    const ln = lines[i]

    // Code fence
    if (ln.startsWith('```')) {
      if (cb === null) {
        cb = []
        cbLang = ln.slice(3).trim()
        i++
        continue
      }
      out.push(
        <View key={`cb-${i}`} style={s.codeBlock} wrap={false}>
          {cbLang && <Text style={s.codeBlockLang}>{cbLang}</Text>}
          <Text style={s.codeBlockBody}>{cb.join('\n')}</Text>
        </View>
      )
      cb = null
      cbLang = ''
      i++
      continue
    }
    if (cb !== null) {
      cb.push(ln)
      i++
      continue
    }

    // Table
    if (
      isTableRow(ln) &&
      i + 1 < lines.length &&
      isTableSep(lines[i + 1])
    ) {
      const tl: string[] = []
      let j = i
      while (
        j < lines.length &&
        (isTableRow(lines[j]) || isTableSep(lines[j]))
      ) {
        tl.push(lines[j])
        j++
      }
      out.push(renderTable(tl, i))
      i = j
      continue
    }

    // HR
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(ln.trim())) {
      out.push(<View key={`hr-${i}`} style={s.hr} />)
      i++
      continue
    }

    // Blank
    if (ln.trim() === '') {
      out.push(<View key={`bl-${i}`} style={s.blank} />)
      i++
      continue
    }

    // Headers
    if (ln.startsWith('#### ')) {
      out.push(
        <Text key={`h4-${i}`} style={s.h4}>
          {renderInline(ln.slice(5))}
        </Text>
      )
      i++
      continue
    }
    if (ln.startsWith('### ')) {
      out.push(
        <Text key={`h3-${i}`} style={s.h3}>
          {renderInline(ln.slice(4))}
        </Text>
      )
      i++
      continue
    }
    if (ln.startsWith('## ')) {
      out.push(
        <Text key={`h2-${i}`} style={s.h2}>
          {renderInline(ln.slice(3))}
        </Text>
      )
      i++
      continue
    }
    if (ln.startsWith('# ')) {
      out.push(
        <Text key={`h1-${i}`} style={s.h1}>
          {renderInline(ln.slice(2))}
        </Text>
      )
      i++
      continue
    }

    // Blockquote
    if (ln.startsWith('> ')) {
      const bq: string[] = []
      while (i < lines.length && lines[i].startsWith('> ')) {
        bq.push(lines[i].slice(2))
        i++
      }
      out.push(
        <View key={`bq-${i}`} style={s.blockquote}>
          <Text style={s.bqText}>{bq.join('\n')}</Text>
        </View>
      )
      continue
    }

    // Bullet
    if (ln.match(/^(\s*)[*-]\s/)) {
      const indent = ln.match(/^(\s*)/)?.[1]?.length || 0
      const content = ln.replace(/^(\s*)[*-]\s/, '')
      out.push(
        <View
          key={`li-${i}`}
          style={[s.listItem, { paddingLeft: indent * 3 }]}
        >
          <Text style={s.bullet}>{'\u2022'}</Text>
          <Text style={s.listText}>{renderInline(content)}</Text>
        </View>
      )
      i++
      continue
    }

    // Numbered
    const olm = ln.match(/^(\d+)\.\s(.*)/)
    if (olm) {
      out.push(
        <View key={`ol-${i}`} style={s.listItem}>
          <Text style={s.olNum}>{olm[1]}.</Text>
          <Text style={s.listText}>{renderInline(olm[2])}</Text>
        </View>
      )
      i++
      continue
    }

    // Paragraph
    out.push(
      <Text key={`p-${i}`} style={s.p}>
        {renderInline(ln)}
      </Text>
    )
    i++
  }

  // Unclosed code block
  if (cb !== null) {
    out.push(
      <View key="cb-end" style={s.codeBlock}>
        <Text style={s.codeBlockBody}>{cb.join('\n')}</Text>
      </View>
    )
  }

  return out
}

// ========== Section title extraction ==========

function extractTitle(content: string): { title: string; body: string } {
  const lines = content.split('\n')
  for (let i = 0; i < Math.min(lines.length, 5); i++) {
    const ln = lines[i].trim()
    if (ln === '') continue
    if (ln.startsWith('#')) {
      return {
        title: ln.replace(/^#+\s*/, ''),
        body: lines.slice(i + 1).join('\n').trim(),
      }
    }
    if (ln.startsWith('**') && ln.endsWith('**') && ln.length < 120) {
      return {
        title: ln.replace(/^\*\*|\*\*$/g, ''),
        body: lines.slice(i + 1).join('\n').trim(),
      }
    }
    if (ln.length <= 80) {
      return { title: ln, body: lines.slice(i + 1).join('\n').trim() }
    }
    break
  }
  const first =
    content.split('\n').find((l) => l.trim() !== '') || 'Analysis'
  const clean = first.replace(/^#+\s*/, '').replace(/^\*\*|\*\*$/g, '')
  return {
    title: clean.length > 60 ? `${clean.slice(0, 57)}...` : clean,
    body: content,
  }
}

function fmtTool(name: string): string {
  return name
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase())
}

// ========== Document ==========

export interface ReportDocumentProps {
  title: string
  date: string
  tickers: string[]
  sections: CopilotMessage[]
  notes?: ReportNote[]
}

export function ReportDocument({
  title,
  date,
  tickers,
  sections,
  notes,
}: ReportDocumentProps) {
  const parts = sections.map((msg) => {
    const { title: st, body } = extractTitle(msg.content)
    return { ...msg, sectionTitle: st, body }
  })

  return (
    <Document title={title} author="OpenAlgo Research">
      {/* ===== COVER PAGE ===== */}
      <Page size="A4" style={s.coverPage}>
        <View style={s.coverBar} fixed />

        <Text style={s.coverBrand}>OpenAlgo Research</Text>
        <Text style={s.coverTitle}>{title}</Text>
        <Text style={s.coverDate}>{date}</Text>

        {tickers.length > 0 && (
          <View style={s.coverTickerRow}>
            {tickers.map((t) => (
              <View key={t} style={s.coverTicker}>
                <Text style={s.coverTickerText}>${t}</Text>
              </View>
            ))}
          </View>
        )}

        <View style={s.coverDivider} />
        <Text style={s.coverDisclaimer}>
          This report was generated by OpenAlgo AI Research Assistant. The
          information contained herein is for informational purposes only and
          does not constitute financial advice.
        </Text>
      </Page>

      {/* ===== CONTENT PAGES ===== */}
      <Page size="A4" style={s.contentPage} wrap>
        {/* Fixed header */}
        <View style={s.header} fixed>
          <Text style={s.headerLabel}>OpenAlgo Research</Text>
          <Text
            style={s.headerPage}
            render={({ subPageNumber, subPageTotalPages }) =>
              `Page ${subPageNumber} of ${subPageTotalPages}`
            }
          />
        </View>

        {/* Table of Contents */}
        {parts.length > 1 && (
          <View style={{ marginBottom: 20 }} wrap={false}>
            <Text style={s.tocHeading}>Contents</Text>
            {parts.map((sec, idx) => (
              <View key={sec.id} style={s.tocRow}>
                <Text style={s.tocNum}>
                  {String(idx + 1).padStart(2, '0')}
                </Text>
                <Text style={s.tocLabel}>{sec.sectionTitle}</Text>
              </View>
            ))}
          </View>
        )}

        {/* Sections */}
        {parts.map((sec, idx) => (
          <View key={sec.id} style={s.section}>
            <View style={s.sectionHead} wrap={false}>
              <View style={s.sectionBadge}>
                <Text style={s.sectionBadgeNum}>{idx + 1}</Text>
              </View>
              <Text style={s.sectionTitle}>{sec.sectionTitle}</Text>
            </View>

            {sec.toolCalls && sec.toolCalls.length > 0 && (
              <View style={s.sectionTools} wrap={false}>
                {sec.toolCalls.map((tc, ti) => (
                  <View key={ti} style={s.toolPill}>
                    <Text style={s.toolPillText}>{fmtTool(tc.name)}</Text>
                  </View>
                ))}
              </View>
            )}

            <View style={s.sectionBody}>{renderMd(sec.body)}</View>

            {idx < parts.length - 1 && <View style={s.sectionDivider} />}
          </View>
        ))}

        {/* Strategy Notes */}
        {notes && notes.length > 0 && (
          <View>
            <View
              style={{
                height: 0.5,
                backgroundColor: C.border,
                marginVertical: 14,
              }}
            />
            <Text style={s.notesTitle}>Strategy Notes</Text>
            {notes.map((note) => {
              const nd = new Date(`${note.created_at}Z`)
              const ds = isNaN(nd.getTime())
                ? note.created_at
                : nd.toLocaleDateString('en-US', {
                    month: 'short',
                    day: 'numeric',
                    year: 'numeric',
                    hour: 'numeric',
                    minute: '2-digit',
                    hour12: true,
                    timeZone: 'America/New_York',
                  })
              return (
                <View key={note.id} style={s.noteCard} wrap={false}>
                  <Text style={s.noteContent}>{note.content}</Text>
                  <Text style={s.noteDate}>{ds}</Text>
                </View>
              )
            })}
          </View>
        )}

        {/* Fixed footer */}
        <View style={s.footer} fixed>
          <Text style={s.footerText}>
            Generated by{' '}
            <Text style={s.footerBrand}>OpenAlgo</Text> Research
          </Text>
          <Text style={s.footerText}>{date}</Text>
        </View>
      </Page>
    </Document>
  )
}
