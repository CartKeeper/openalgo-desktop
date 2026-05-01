/**
 * Generates and downloads a Goldman Sax & Violins PDF for a given brief.
 * Use from any UI surface that produces a "recommendations & performance" report.
 */

import { pdf } from '@react-pdf/renderer'
import { GoldmanReport } from './GoldmanReport'
import type { GoldmanBrief } from './types'

export async function downloadGoldmanBriefPdf(brief: GoldmanBrief): Promise<void> {
  const blob = await pdf(<GoldmanReport brief={brief} />).toBlob()
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  const safeName = brief.client_name.replace(/[^a-zA-Z0-9]/g, '_')
  const safeDoc = brief.document_label.replace(/[^a-zA-Z0-9]/g, '_')
  a.download = `Goldman_Sax_and_Violins_${safeDoc}_${safeName}.pdf`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}
