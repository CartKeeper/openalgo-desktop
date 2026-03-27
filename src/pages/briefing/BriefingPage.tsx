import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle,
  ArrowLeft,
  BarChart3,
  CheckCircle2,
  Clock,
  FileText,
  Loader2,
  Play,
  RefreshCw,
  Shield,
  Target,
  TrendingUp,
  Zap,
} from 'lucide-react'
import React, { useCallback, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import type { ToolCallInfo } from '@/stores/copilotStore'
import { useReportsStore } from '@/stores/reportsStore'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'

// ============================================================================
// Types
// ============================================================================

interface BriefingResponse {
  response_text: string
  tool_calls_made: Array<{ tool_name: string; result_summary: string }>
}

interface BriefingSection {
  id: string
  title: string
  icon: React.ReactNode
  content: string
}

// ============================================================================
// Markdown Rendering (shared with CopilotPage)
// ============================================================================

function renderMarkdown(text: string): React.ReactNode[] {
  const lines = text.split('\n')
  const elements: React.ReactNode[] = []
  let codeBlock: string[] | null = null
  let codeBlockLang = ''
  let tableRows: string[] | null = null

  const flushTable = (idx: number) => {
    if (!tableRows || tableRows.length === 0) return
    const headerCells = tableRows[0].split('|').map((c) => c.trim()).filter(Boolean)
    const dataRows = tableRows.slice(2) // skip header + separator
    elements.push(
      <div key={`table-${idx}`} className="my-3 rounded-lg border overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="h-10 border-b bg-muted/50 text-xs text-muted-foreground uppercase tracking-wider">
                {headerCells.map((cell, ci) => (
                  <th key={ci} className="px-4 text-left font-semibold">{cell}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {dataRows.map((row, ri) => {
                const cells = row.split('|').map((c) => c.trim()).filter(Boolean)
                return (
                  <tr key={ri} className="h-12 border-b last:border-b-0 hover:bg-muted/30 transition-colors duration-150">
                    {cells.map((cell, ci) => (
                      <td key={ci} className={`px-4 ${ci === 0 ? 'font-semibold' : ''} ${/^[-+]?\$?[\d,.]+%?$/.test(cell.replace(/[▲▼→]/g, '').trim()) ? 'tabular-nums font-mono' : ''}`}>
                        {inlineFormat(cell)}
                      </td>
                    ))}
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      </div>
    )
    tableRows = null
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]

    // Code block toggle
    if (line.startsWith('```')) {
      flushTable(i)
      if (codeBlock === null) {
        codeBlock = []
        codeBlockLang = line.slice(3).trim()
      } else {
        elements.push(
          <div key={`code-${i}`} className="my-2 rounded-lg border bg-muted/50 overflow-hidden">
            {codeBlockLang && (
              <div className="px-3 py-1 border-b text-[10px] font-semibold uppercase tracking-[0.05em] text-muted-foreground">
                {codeBlockLang}
              </div>
            )}
            <pre className="p-3 overflow-x-auto text-[13px] leading-relaxed font-mono">
              <code>{codeBlock.join('\n')}</code>
            </pre>
          </div>
        )
        codeBlock = null
        codeBlockLang = ''
      }
      continue
    }

    if (codeBlock !== null) {
      codeBlock.push(line)
      continue
    }

    // Table detection (line has | characters and isn't a separator-only line)
    const isTableRow = line.includes('|') && line.trim().startsWith('|')
    const isTableSep = /^\|[\s\-:|]+\|$/.test(line.trim())

    if (isTableRow || isTableSep) {
      if (tableRows === null) tableRows = []
      tableRows.push(line.trim())
      continue
    } else {
      flushTable(i)
    }

    // Blank line
    if (line.trim() === '') {
      elements.push(<div key={`blank-${i}`} className="h-2" />)
      continue
    }

    // Headers — these become section dividers in the briefing
    if (line.startsWith('### ')) {
      elements.push(
        <h4 key={`h3-${i}`} className="text-[14px] font-semibold mt-4 mb-1">
          {inlineFormat(line.slice(4))}
        </h4>
      )
      continue
    }
    if (line.startsWith('## ')) {
      // Skip — we extract ## headers as section titles
      continue
    }
    if (line.startsWith('# ')) {
      elements.push(
        <h3 key={`h1-${i}`} className="text-[16px] font-bold mt-4 mb-2">
          {inlineFormat(line.slice(2))}
        </h3>
      )
      continue
    }

    // Horizontal rule
    if (/^---+$/.test(line.trim())) {
      elements.push(<hr key={`hr-${i}`} className="my-4 border-border" />)
      continue
    }

    // Bullet lists
    if (line.match(/^(\s*)[*-]\s/)) {
      const indent = line.match(/^(\s*)/)?.[1]?.length || 0
      const content = line.replace(/^(\s*)[*-]\s/, '')
      elements.push(
        <div
          key={`li-${i}`}
          className="flex gap-2 text-[14px] leading-[1.5]"
          style={{ paddingLeft: `${Math.max(indent * 4, 0) + 8}px` }}
        >
          <span className="text-muted-foreground mt-[2px] shrink-0">&#8226;</span>
          <span>{inlineFormat(content)}</span>
        </div>
      )
      continue
    }

    // Numbered lists
    if (line.match(/^\d+\.\s/)) {
      const match = line.match(/^(\d+)\.\s(.*)/)
      if (match) {
        elements.push(
          <div key={`ol-${i}`} className="flex gap-2 text-[14px] leading-[1.5] pl-2">
            <span className="text-muted-foreground shrink-0 tabular-nums">{match[1]}.</span>
            <span>{inlineFormat(match[2])}</span>
          </div>
        )
        continue
      }
    }

    // Normal paragraph
    elements.push(
      <p key={`p-${i}`} className="text-[14px] leading-[1.5]">
        {inlineFormat(line)}
      </p>
    )
  }

  flushTable(lines.length)

  if (codeBlock !== null) {
    elements.push(
      <pre key="code-unclosed" className="my-2 p-3 rounded-lg border bg-muted/50 overflow-x-auto text-[13px] font-mono">
        <code>{codeBlock.join('\n')}</code>
      </pre>
    )
  }

  return elements
}

function inlineFormat(text: string): React.ReactNode {
  const parts: React.ReactNode[] = []
  let remaining = text
  let key = 0

  while (remaining.length > 0) {
    const codeMatch = remaining.match(/^(.*?)`([^`]+)`(.*)$/)
    if (codeMatch) {
      if (codeMatch[1]) {
        parts.push(...inlineBoldItalic(codeMatch[1], key))
        key += 10
      }
      parts.push(
        <code key={`ic-${key++}`} className="px-1.5 py-0.5 rounded bg-muted text-[13px] font-mono">
          {codeMatch[2]}
        </code>
      )
      remaining = codeMatch[3]
      continue
    }
    parts.push(...inlineBoldItalic(remaining, key))
    break
  }

  return parts.length === 1 ? parts[0] : <>{parts}</>
}

function inlineBoldItalic(text: string, startKey: number): React.ReactNode[] {
  const parts: React.ReactNode[] = []
  let remaining = text
  let key = startKey

  while (remaining.length > 0) {
    const boldMatch = remaining.match(/^(.*?)\*\*(.+?)\*\*(.*)$/)
    if (boldMatch) {
      if (boldMatch[1]) parts.push(<span key={`t-${key++}`}>{boldMatch[1]}</span>)
      parts.push(<span key={`b-${key++}`} className="font-semibold">{boldMatch[2]}</span>)
      remaining = boldMatch[3]
      continue
    }
    const italicMatch = remaining.match(/^(.*?)\*(.+?)\*(.*)$/)
    if (italicMatch) {
      if (italicMatch[1]) parts.push(<span key={`t-${key++}`}>{italicMatch[1]}</span>)
      parts.push(<em key={`i-${key++}`}>{italicMatch[2]}</em>)
      remaining = italicMatch[3]
      continue
    }
    parts.push(<span key={`t-${key++}`}>{remaining}</span>)
    break
  }

  return parts
}

// ============================================================================
// Section Parser
// ============================================================================

const SECTION_META: Record<string, { icon: React.ReactNode }> = {
  'market pulse': { icon: <Zap className="h-5 w-5" /> },
  'portfolio snapshot': { icon: <BarChart3 className="h-5 w-5" /> },
  'alerts & watchlist': { icon: <AlertCircle className="h-5 w-5" /> },
  'alerts': { icon: <AlertCircle className="h-5 w-5" /> },
  'watchlist': { icon: <Target className="h-5 w-5" /> },
  'opportunities': { icon: <TrendingUp className="h-5 w-5" /> },
  'risk radar': { icon: <Shield className="h-5 w-5" /> },
  'deep dive': { icon: <Target className="h-5 w-5" /> },
  'deep dives': { icon: <Target className="h-5 w-5" /> },
  'new stock corner': { icon: <TrendingUp className="h-5 w-5" /> },
  'stock corner': { icon: <TrendingUp className="h-5 w-5" /> },
}

function parseSections(text: string): BriefingSection[] {
  const sections: BriefingSection[] = []
  const lines = text.split('\n')
  let currentTitle = ''
  let currentContent: string[] = []

  const flush = () => {
    if (currentTitle) {
      const key = currentTitle.toLowerCase()
      const meta = Object.entries(SECTION_META).find(([k]) => key.includes(k))
      sections.push({
        id: currentTitle.toLowerCase().replace(/\s+/g, '-'),
        title: currentTitle,
        icon: meta?.[1].icon || <FileText className="h-5 w-5" />,
        content: currentContent.join('\n').trim(),
      })
    }
  }

  for (const line of lines) {
    if (line.startsWith('## ')) {
      flush()
      currentTitle = line.slice(3).trim()
      currentContent = []
    } else {
      currentContent.push(line)
    }
  }
  flush()

  // If no sections found, treat the whole text as one section
  if (sections.length === 0 && text.trim()) {
    sections.push({
      id: 'briefing',
      title: 'Briefing',
      icon: <FileText className="h-5 w-5" />,
      content: text.trim(),
    })
  }

  return sections
}

// ============================================================================
// Loading Skeleton
// ============================================================================

function BriefingSkeleton() {
  return (
    <div className="space-y-6">
      {Array.from({ length: 5 }).map((_, i) => (
        <Card key={i} className="p-4">
          <div className="flex items-center gap-3 mb-4">
            <Skeleton className="h-10 w-10 rounded-lg" />
            <Skeleton className="h-5 w-48" />
          </div>
          <div className="space-y-2">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
            <Skeleton className="h-4 w-4/6" />
            <Skeleton className="h-4 w-3/6" />
          </div>
        </Card>
      ))}
    </div>
  )
}

// ============================================================================
// Tool Activity Indicator
// ============================================================================

function ToolActivity({ tools }: { tools: ToolCallInfo[] }) {
  if (tools.length === 0) return null
  return (
    <div className="flex flex-wrap gap-2">
      {tools.map((t, i) => (
        <Badge key={i} variant="secondary" className="text-[11px] gap-1">
          <CheckCircle2 className="h-3 w-3 text-green-500" />
          {t.name.replace(/_/g, ' ')}
        </Badge>
      ))}
    </div>
  )
}

// ============================================================================
// Main Component
// ============================================================================

export default function BriefingPage() {
  const [sections, setSections] = useState<BriefingSection[]>([])
  const [toolCalls, setToolCalls] = useState<ToolCallInfo[]>([])
  const [isGenerating, setIsGenerating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [generatedAt, setGeneratedAt] = useState<string | null>(null)
  const [rawText, setRawText] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const navigate = useNavigate()
  const { saveReport } = useReportsStore()

  const generate = useCallback(async () => {
    setIsGenerating(true)
    setError(null)
    setSections([])
    setToolCalls([])
    setRawText('')

    try {
      const response = await invoke<BriefingResponse>('generate_briefing')

      const tools: ToolCallInfo[] = response.tool_calls_made.map((tc) => ({
        name: tc.tool_name,
        summary: tc.result_summary || '',
      }))

      const parsed = parseSections(response.response_text)

      setSections(parsed)
      setToolCalls(tools)
      setRawText(response.response_text)
      setGeneratedAt(
        new Date().toLocaleTimeString('en-US', {
          hour: 'numeric',
          minute: '2-digit',
          hour12: true,
          timeZone: 'America/New_York',
        }) + ' ET'
      )
    } catch (err: unknown) {
      let msg: string
      if (err instanceof Error) {
        msg = err.message
      } else if (typeof err === 'object' && err !== null && 'message' in err) {
        msg = String((err as { message: string }).message)
      } else {
        msg = String(err)
      }
      setError(msg)
    } finally {
      setIsGenerating(false)
    }
  }, [])

  const handleSaveAsReport = async () => {
    if (!rawText || isSaving) return
    setIsSaving(true)
    try {
      const title = `10-Min Briefing — ${new Date().toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}`
      const summary = sections[0]?.content.slice(0, 300) || ''
      const tags = ['Briefing', ...toolCalls.map((t) => t.name.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())).slice(0, 5)]
      const messages = [
        { id: '1', role: 'user' as const, content: 'Generate 10-Minute Market Briefing', timestamp: Date.now() },
        { id: '2', role: 'assistant' as const, content: rawText, toolCalls, timestamp: Date.now() },
      ]
      const report = await saveReport(title, summary, tags, messages)
      toast.success('Briefing saved as report')
      navigate(`/reports/${report.id}`)
    } catch {
      toast.error('Failed to save briefing')
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="flex flex-col gap-6 p-4 md:p-6 max-w-4xl mx-auto">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Clock className="h-6 w-6" />
            10-Minute Briefing
          </h1>
        </div>
        <p className="text-sm text-muted-foreground ml-6">
          AI-powered market brief based on your live portfolio. One click, full picture.
        </p>
      </div>

      {/* Action Bar */}
      <div className="flex items-center gap-3 flex-wrap">
        <Button
          onClick={generate}
          disabled={isGenerating}
          className="h-10 gap-2"
        >
          {isGenerating ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              Generating Briefing...
            </>
          ) : sections.length > 0 ? (
            <>
              <RefreshCw className="h-4 w-4" />
              Regenerate
            </>
          ) : (
            <>
              <Play className="h-4 w-4" />
              Generate Briefing
            </>
          )}
        </Button>

        {sections.length > 0 && (
          <Button
            variant="outline"
            size="sm"
            className="h-10 gap-2"
            onClick={handleSaveAsReport}
            disabled={isSaving}
          >
            <FileText className="h-4 w-4" />
            {isSaving ? 'Saving...' : 'Save as Report'}
          </Button>
        )}

        {generatedAt && (
          <span className="text-xs text-muted-foreground ml-auto">
            Generated at {generatedAt}
          </span>
        )}
      </div>

      {/* Tool Activity */}
      {toolCalls.length > 0 && <ToolActivity tools={toolCalls} />}

      {/* Loading State */}
      {isGenerating && <BriefingSkeleton />}

      {/* Error State */}
      {error && !isGenerating && (
        <Card className="p-8 text-center border-destructive/50">
          <AlertCircle className="h-12 w-12 mx-auto text-destructive mb-3" />
          <p className="text-sm font-semibold">Failed to Generate Briefing</p>
          <p className="text-xs text-muted-foreground mt-1 max-w-md mx-auto">{error}</p>
          <Button variant="outline" size="sm" className="mt-4" onClick={generate}>
            Try Again
          </Button>
        </Card>
      )}

      {/* Empty State */}
      {!isGenerating && !error && sections.length === 0 && (
        <Card className="p-12 text-center">
          <div className="rounded-full bg-muted p-4 w-fit mx-auto mb-4">
            <Clock className="h-12 w-12 text-muted-foreground" />
          </div>
          <h3 className="text-base font-semibold">Ready to Brief</h3>
          <p className="text-sm text-muted-foreground mt-1 max-w-md mx-auto">
            Hit "Generate Briefing" and the AI will pull your live portfolio positions,
            fetch current market data, news, and financials, then deliver a structured
            10-minute research brief.
          </p>
        </Card>
      )}

      {/* Briefing Sections */}
      {sections.length > 0 && !isGenerating && (
        <div className="space-y-4">
          {sections.map((section) => (
            <Card key={section.id} className="overflow-hidden">
              {/* Section Header */}
              <div className="flex items-center gap-3 px-4 py-3 border-b bg-muted/30">
                <div className="rounded-lg bg-primary/10 p-2 text-primary">
                  {section.icon}
                </div>
                <h2 className="text-[16px] font-semibold">{section.title}</h2>
              </div>

              {/* Section Content */}
              <div className="px-4 py-4">
                {renderMarkdown(section.content)}
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
