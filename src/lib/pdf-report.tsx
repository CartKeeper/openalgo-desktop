/**
 * Generates a PDF for the saved-research / copilot-briefing pipeline.
 *
 * Routed through the Goldman Sax & Violins house template — every
 * "recommendations and performance" report exits the app with the same
 * visual identity. The Client Brief flow uses `goldman/generator.tsx`
 * directly with a structured `GoldmanBrief`; this module is the adapter
 * for the legacy markdown-based `(messages, notes, tickers)` shape.
 */

import { pdf } from '@react-pdf/renderer'
import { GoldmanResearchReport } from '@/components/reports/goldman/GoldmanResearchReport'

export interface GenerateReportPdfOptions {
  title: string
  messages: import('@/stores/copilotStore').CopilotMessage[]
  notes?: import('@/stores/reportsStore').ReportNote[]
  tickers: string[]
}

export async function generateReportPdf(
  options: GenerateReportPdfOptions,
): Promise<void> {
  const { title, messages, notes, tickers } = options

  const now = new Date()
  const dateStr = now.toLocaleDateString('en-US', {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
    timeZone: 'America/New_York',
  })

  const blob = await pdf(
    <GoldmanResearchReport
      title={title}
      date={dateStr}
      tickers={tickers}
      sections={messages}
      notes={notes}
    />,
  ).toBlob()

  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  const timestamp = now.toISOString().replace(/[:.]/g, '-').slice(0, -5)
  const safeTitle = title
    .replace(/[^a-zA-Z0-9 ]/g, '')
    .trim()
    .replace(/\s+/g, '_')
    .slice(0, 40)
  a.download = `Goldman_Sax_and_Violins_${safeTitle}_${timestamp}.pdf`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}
