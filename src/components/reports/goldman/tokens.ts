/**
 * Shared design tokens for the Goldman Sax & Violins house template.
 * Used by both the structured `GoldmanReport` (Client Briefs) and the
 * markdown-driven `GoldmanResearchReport` (Research / Briefing exports).
 */

export const C = {
  paper: '#F4ECDF',
  ink: '#1A1A1A',
  body: '#4A4A4A',
  muted: '#797979',
  gold: '#795C26',
  goldSoft: '#C8B88F',
  burgundy: '#791F1F',
  green: '#2D5E2D',
} as const

export const FONT = {
  serif: 'Times-Roman',
  serifBold: 'Times-Bold',
  serifItalic: 'Times-Italic',
  serifBoldItalic: 'Times-BoldItalic',
  sansBold: 'Helvetica-Bold',
  mono: 'Courier',
  monoBold: 'Courier-Bold',
} as const
