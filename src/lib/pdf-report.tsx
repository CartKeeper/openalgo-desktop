import { pdf } from '@react-pdf/renderer'
import {
  ReportDocument,
  type ReportDocumentProps,
} from '@/components/reports/PdfReportTemplate'

export interface GenerateReportPdfOptions {
  title: string
  messages: import('@/stores/copilotStore').CopilotMessage[]
  notes?: import('@/stores/reportsStore').ReportNote[]
  tickers: string[]
}

export async function generateReportPdf(
  options: GenerateReportPdfOptions
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

  const props: ReportDocumentProps = {
    title,
    date: dateStr,
    tickers,
    sections: messages,
    notes,
  }

  // Generate PDF blob client-side via @react-pdf/renderer
  const blob = await pdf(<ReportDocument {...props} />).toBlob()

  // Trigger download
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  const timestamp = now
    .toISOString()
    .replace(/[:.]/g, '-')
    .slice(0, -5)
  const safeTitle = title
    .replace(/[^a-zA-Z0-9 ]/g, '')
    .trim()
    .replace(/\s+/g, '_')
    .slice(0, 40)
  a.download = `Research_Report_${safeTitle}_${timestamp}.pdf`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}
