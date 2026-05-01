/**
 * Goldman Sax & Violins markdown → React-PDF renderer.
 *
 * Handles: H1–H4, paragraphs, blank lines, ordered/unordered lists, tables,
 * blockquotes, code blocks, horizontal rules, inline `code`, **bold**, *italic*.
 * Output uses the Goldman color/font tokens — gold headings, ♪ bullets,
 * gold-underlined tables, burgundy inline code on cream.
 */

import React from 'react'
import { StyleSheet, Text, View } from '@react-pdf/renderer'
import { C, FONT } from './tokens'

const m = StyleSheet.create({
  paragraph: {
    fontFamily: FONT.serif,
    fontSize: 10.5,
    color: C.ink,
    lineHeight: 1.5,
    paddingTop: 4,
  },
  blank: { height: 6 },
  hr: {
    borderBottomWidth: 0.5,
    borderBottomColor: C.goldSoft,
    marginVertical: 10,
  },
  h1: {
    fontFamily: FONT.serifBold,
    fontSize: 18,
    color: C.gold,
    paddingTop: 14,
    paddingBottom: 4,
  },
  h2: {
    fontFamily: FONT.serifBold,
    fontSize: 13.5,
    color: C.gold,
    paddingTop: 12,
    paddingBottom: 3,
  },
  h3: {
    fontFamily: FONT.serifBold,
    fontSize: 11.5,
    color: C.ink,
    paddingTop: 10,
    paddingBottom: 2,
  },
  h4: {
    fontFamily: FONT.serifBold,
    fontSize: 10.5,
    color: C.body,
    paddingTop: 8,
    paddingBottom: 2,
  },
  bold: { fontFamily: FONT.serifBold, color: C.ink },
  italic: { fontFamily: FONT.serifItalic, color: C.body },
  inlineCode: {
    fontFamily: FONT.mono,
    fontSize: 9.5,
    color: C.burgundy,
    backgroundColor: '#EFE6D2',
  },
  bulletRow: {
    flexDirection: 'row',
    paddingTop: 4,
    alignItems: 'flex-start',
  },
  bulletGlyph: {
    fontFamily: FONT.serif,
    fontSize: 11,
    color: C.gold,
    width: 14,
    paddingTop: 0.5,
  },
  bulletNum: {
    fontFamily: FONT.serifItalic,
    fontSize: 11,
    color: C.burgundy,
    width: 18,
    paddingTop: 0.5,
  },
  bulletBody: {
    flex: 1,
    fontSize: 10.5,
    fontFamily: FONT.serif,
    color: C.ink,
    lineHeight: 1.45,
  },
  blockquote: {
    borderLeftWidth: 2,
    borderLeftColor: C.goldSoft,
    paddingLeft: 10,
    paddingVertical: 4,
    marginVertical: 6,
  },
  blockquoteText: {
    fontFamily: FONT.serifItalic,
    fontSize: 10.5,
    color: C.body,
    lineHeight: 1.45,
  },
  codeBlock: {
    backgroundColor: '#EFE6D2',
    borderWidth: 0.5,
    borderColor: C.goldSoft,
    paddingVertical: 6,
    paddingHorizontal: 10,
    marginVertical: 6,
  },
  codeBlockText: {
    fontFamily: FONT.mono,
    fontSize: 9,
    color: C.burgundy,
    lineHeight: 1.4,
  },
  table: {
    marginVertical: 8,
  },
  tableHeaderRow: {
    flexDirection: 'row',
    borderBottomWidth: 1,
    borderBottomColor: C.gold,
    paddingBottom: 4,
    paddingTop: 4,
  },
  tableHeaderCell: {
    fontFamily: FONT.serif,
    fontSize: 8,
    color: C.muted,
    letterSpacing: 1.4,
    paddingHorizontal: 4,
  },
  tableRow: {
    flexDirection: 'row',
    borderBottomWidth: 0.6,
    borderBottomColor: C.goldSoft,
    paddingVertical: 5,
  },
  tableCell: {
    fontFamily: FONT.serif,
    fontSize: 10,
    color: C.ink,
    lineHeight: 1.3,
    paddingHorizontal: 4,
  },
  tableCellNumeric: {
    fontFamily: FONT.mono,
    fontSize: 10,
    color: C.ink,
    textAlign: 'right',
    paddingHorizontal: 4,
  },
  tableCellFirstCol: {
    fontFamily: FONT.sansBold,
    fontSize: 10,
    color: C.gold,
    paddingHorizontal: 4,
  },
})

// ----- Public renderer -----

export function renderMarkdown(markdown: string): React.ReactNode[] {
  const lines = markdown.replace(/\r\n/g, '\n').split('\n')
  const out: React.ReactNode[] = []
  let i = 0
  let codeBuf: string[] | null = null

  while (i < lines.length) {
    const ln = lines[i]

    // Code block fences --------------------------------------------------
    if (ln.trim().startsWith('```')) {
      if (codeBuf === null) {
        codeBuf = []
      } else {
        out.push(
          <View key={`cb-${i}`} style={m.codeBlock} wrap={false}>
            <Text style={m.codeBlockText}>{codeBuf.join('\n')}</Text>
          </View>,
        )
        codeBuf = null
      }
      i++
      continue
    }
    if (codeBuf !== null) {
      codeBuf.push(ln)
      i++
      continue
    }

    // Tables -------------------------------------------------------------
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

    // Horizontal rule ----------------------------------------------------
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(ln.trim())) {
      out.push(<View key={`hr-${i}`} style={m.hr} />)
      i++
      continue
    }

    // Blank line ---------------------------------------------------------
    if (ln.trim() === '') {
      out.push(<View key={`bl-${i}`} style={m.blank} />)
      i++
      continue
    }

    // Headers ------------------------------------------------------------
    if (ln.startsWith('#### ')) {
      out.push(
        <Text key={`h4-${i}`} style={m.h4}>
          {renderInline(ln.slice(5))}
        </Text>,
      )
      i++
      continue
    }
    if (ln.startsWith('### ')) {
      out.push(
        <Text key={`h3-${i}`} style={m.h3}>
          {renderInline(ln.slice(4))}
        </Text>,
      )
      i++
      continue
    }
    if (ln.startsWith('## ')) {
      out.push(
        <Text key={`h2-${i}`} style={m.h2}>
          {renderInline(ln.slice(3))}
        </Text>,
      )
      i++
      continue
    }
    if (ln.startsWith('# ')) {
      out.push(
        <Text key={`h1-${i}`} style={m.h1}>
          {renderInline(ln.slice(2))}
        </Text>,
      )
      i++
      continue
    }

    // Blockquote ---------------------------------------------------------
    if (ln.startsWith('> ')) {
      const bq: string[] = []
      while (i < lines.length && lines[i].startsWith('> ')) {
        bq.push(lines[i].slice(2))
        i++
      }
      out.push(
        <View key={`bq-${i}`} style={m.blockquote}>
          <Text style={m.blockquoteText}>{bq.join('\n')}</Text>
        </View>,
      )
      continue
    }

    // Unordered list (♪ glyph) ------------------------------------------
    const liMatch = ln.match(/^(\s*)[*\-+]\s(.*)/)
    if (liMatch) {
      const indent = (liMatch[1].length || 0) * 3
      out.push(
        <View key={`li-${i}`} style={[m.bulletRow, { paddingLeft: indent }]}>
          <Text style={m.bulletGlyph}>♪</Text>
          <Text style={m.bulletBody}>{renderInline(liMatch[2])}</Text>
        </View>,
      )
      i++
      continue
    }

    // Ordered list (italic burgundy numerals) ---------------------------
    const olMatch = ln.match(/^(\d+)\.\s(.*)/)
    if (olMatch) {
      out.push(
        <View key={`ol-${i}`} style={m.bulletRow}>
          <Text style={m.bulletNum}>{olMatch[1]}.</Text>
          <Text style={m.bulletBody}>{renderInline(olMatch[2])}</Text>
        </View>,
      )
      i++
      continue
    }

    // Paragraph ---------------------------------------------------------
    out.push(
      <Text key={`p-${i}`} style={m.paragraph}>
        {renderInline(ln)}
      </Text>,
    )
    i++
  }

  // Unclosed code block — flush whatever we collected
  if (codeBuf !== null) {
    out.push(
      <View key="cb-end" style={m.codeBlock}>
        <Text style={m.codeBlockText}>{codeBuf.join('\n')}</Text>
      </View>,
    )
  }

  return out
}

// ----- Inline -----

function renderInline(text: string): React.ReactNode {
  const parts: React.ReactNode[] = []
  let rest = text
  let key = 0

  while (rest.length > 0) {
    // Inline code first (highest precedence) -------------------------------
    const cm = rest.match(/^([\s\S]*?)`([^`]+)`([\s\S]*)$/)
    if (cm) {
      if (cm[1]) parts.push(...renderEmphasis(cm[1], key))
      key += 100
      parts.push(
        <Text key={`c-${key++}`} style={m.inlineCode}>
          {cm[2]}
        </Text>,
      )
      rest = cm[3]
      continue
    }
    parts.push(...renderEmphasis(rest, key))
    break
  }

  if (parts.length === 0) return text
  if (parts.length === 1) return parts[0]
  return <>{parts}</>
}

function renderEmphasis(text: string, sk: number): React.ReactNode[] {
  const parts: React.ReactNode[] = []
  let rest = text
  let key = sk

  while (rest.length > 0) {
    // **bold**
    const bm = rest.match(/^([\s\S]*?)\*\*(.+?)\*\*([\s\S]*)$/)
    if (bm) {
      if (bm[1]) parts.push(bm[1])
      parts.push(
        <Text key={`b-${key++}`} style={m.bold}>
          {bm[2]}
        </Text>,
      )
      rest = bm[3]
      continue
    }
    // *italic*
    const im = rest.match(/^([\s\S]*?)\*(.+?)\*([\s\S]*)$/)
    if (im) {
      if (im[1]) parts.push(im[1])
      parts.push(
        <Text key={`i-${key++}`} style={m.italic}>
          {im[2]}
        </Text>,
      )
      rest = im[3]
      continue
    }
    parts.push(rest)
    break
  }

  return parts
}

// ----- Table helpers -----

function isTableRow(line: string): boolean {
  const t = line.trim()
  return t.startsWith('|') && t.endsWith('|') && t.split('|').length >= 3
}

function isTableSep(line: string): boolean {
  return /^\|[\s\-:|]+\|$/.test(line.trim())
}

function parseCells(line: string): string[] {
  return line.split('|').slice(1, -1).map((c) => c.trim())
}

function renderTable(lines: string[], key: number): React.ReactNode {
  const headers = parseCells(lines[0])
  const rows: string[][] = []
  for (let i = 2; i < lines.length; i++) {
    if (!isTableSep(lines[i])) rows.push(parseCells(lines[i]))
  }
  const colW = `${Math.floor(100 / headers.length)}%`

  const isNumeric = headers.map((_, ci) =>
    rows.length > 0 &&
    rows.every((r) => {
      const c = r[ci] || ''
      return /^[\s$%,.\-+\d]+$/.test(c) || c === '' || c === '—' || c === 'N/A'
    }),
  )

  return (
    <View key={`tbl-${key}`} style={m.table} wrap={false}>
      <View style={m.tableHeaderRow}>
        {headers.map((h, ci) => (
          <Text
            key={ci}
            style={[
              m.tableHeaderCell,
              { width: colW, textAlign: isNumeric[ci] ? 'right' : 'left' },
            ]}
          >
            {h.toUpperCase().split('').join(' ')}
          </Text>
        ))}
      </View>
      {rows.map((row, ri) => (
        <View key={ri} style={m.tableRow}>
          {row.map((cell, ci) => {
            const style =
              ci === 0
                ? m.tableCellFirstCol
                : isNumeric[ci]
                  ? m.tableCellNumeric
                  : m.tableCell
            return (
              <Text key={ci} style={[style, { width: colW }]}>
                {cell}
              </Text>
            )
          })}
        </View>
      ))}
    </View>
  )
}

/**
 * Extract a human title from the start of a markdown blob.
 * Used to generate Movement section titles from copilot messages.
 */
export function extractTitle(content: string): { title: string; body: string } {
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
  const first = content.split('\n').find((l) => l.trim() !== '') || 'Analysis'
  const clean = first.replace(/^#+\s*/, '').replace(/^\*\*|\*\*$/g, '')
  return {
    title: clean.length > 60 ? `${clean.slice(0, 57)}…` : clean,
    body: content,
  }
}
