import {
  AlertCircle,
  ArrowLeft,
  BookOpen,
  Bot,
  Loader2,
  Trash2,
  Wrench,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { useReportsStore, type ResearchReportSummary } from '@/stores/reportsStore'
import type { ToolCallInfo } from '@/stores/copilotStore'

function formatDate(dateStr: string): string {
  const d = new Date(dateStr + 'Z')
  if (isNaN(d.getTime())) return dateStr
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

function parseTags(tagsJson: string | null): string[] {
  if (!tagsJson) return []
  try {
    // Dedupe: older reports were saved with duplicate tool tags (e.g. two
    // "Get Market Overview"), which collide as React keys when rendered.
    return [...new Set(JSON.parse(tagsJson) as string[])]
  } catch {
    return []
  }
}

function parseToolCalls(json: string | null): ToolCallInfo[] {
  if (!json) return []
  try {
    return JSON.parse(json)
  } catch {
    return []
  }
}

function formatToolName(name: string): string {
  return name
    .replace(/_/g, ' ')
    .replace(/\bget\b/gi, '')
    .trim()
    .replace(/^\w/, (c) => c.toUpperCase())
}

export default function ReportsLibrary() {
  const navigate = useNavigate()
  const { reports, isLoading, error, fetchReports, deleteReport } = useReportsStore()
  const [search, setSearch] = useState('')

  useEffect(() => {
    fetchReports()
  }, [fetchReports])

  const handleDelete = async (e: React.MouseEvent, id: number) => {
    e.stopPropagation()
    try {
      await deleteReport(id)
      toast.success('Report deleted')
    } catch (err) {
      toast.error('Failed to delete report')
    }
  }

  const filtered = reports.filter((r) => {
    if (!search) return true
    const q = search.toLowerCase()
    return (
      r.title.toLowerCase().includes(q) ||
      (r.summary && r.summary.toLowerCase().includes(q)) ||
      parseTags(r.tags).some((t) => t.toLowerCase().includes(q))
    )
  })

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold">Research Reports</h1>
        </div>
        <p className="text-sm text-muted-foreground">
          Saved research analyses from the AI Copilot. Add strategy notes for later.
        </p>
      </div>

      {/* Search */}
      <Input
        placeholder="Search reports by title or tag..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="h-10 max-w-sm"
      />

      {/* Loading */}
      {isLoading && (
        <div className="grid gap-4 md:grid-cols-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-32 rounded-lg" />
          ))}
        </div>
      )}

      {/* Error */}
      {error && !isLoading && (
        <Card className="p-8 text-center">
          <AlertCircle className="h-12 w-12 mx-auto text-muted-foreground mb-3" />
          <p className="text-sm font-medium">Failed to load reports</p>
          <p className="text-xs text-muted-foreground mt-1">{error}</p>
          <Button variant="outline" size="sm" className="mt-4" onClick={() => fetchReports()}>
            Retry
          </Button>
        </Card>
      )}

      {/* Empty */}
      {!isLoading && !error && filtered.length === 0 && (
        <Card className="p-8 text-center">
          <BookOpen className="h-12 w-12 mx-auto text-muted-foreground mb-3" />
          <p className="text-sm font-medium">
            {search ? 'No matching reports' : 'No saved reports yet'}
          </p>
          <p className="text-xs text-muted-foreground mt-1">
            {search
              ? 'Try a different search term.'
              : 'Use the Research Copilot to analyze markets, then save as a report.'}
          </p>
          {!search && (
            <Button size="sm" className="mt-4" onClick={() => navigate('/copilot')}>
              <Bot className="h-4 w-4 mr-1" />
              Go to Research
            </Button>
          )}
        </Card>
      )}

      {/* Reports Grid */}
      {!isLoading && filtered.length > 0 && (
        <div className="grid gap-4 md:grid-cols-2">
          {filtered.map((report) => (
            <ReportCard
              key={report.id}
              report={report}
              onClick={() => navigate(`/reports/${report.id}`)}
              onDelete={(e) => handleDelete(e, report.id)}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function ReportCard({
  report,
  onClick,
  onDelete,
}: {
  report: ResearchReportSummary
  onClick: () => void
  onDelete: (e: React.MouseEvent) => void
}) {
  const tags = parseTags(report.tags)
  const toolCalls = parseToolCalls(report.tool_calls_json)
  const uniqueTools = [...new Set(toolCalls.map((tc) => tc.name))]

  return (
    <Card
      className="p-4 cursor-pointer hover:bg-muted/30 transition-colors duration-150 relative group"
      onClick={onClick}
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-semibold line-clamp-2">{report.title}</h3>
        <Button
          variant="ghost"
          size="sm"
          className="h-8 w-8 p-0 opacity-0 group-hover:opacity-100 transition-opacity text-destructive hover:text-destructive shrink-0"
          onClick={onDelete}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>

      {report.summary && (
        <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{report.summary}</p>
      )}

      <div className="flex flex-wrap gap-1 mt-2">
        {tags.slice(0, 5).map((tag) => (
          <Badge key={tag} variant="secondary" className="text-[10px] px-1.5 py-0">
            {tag}
          </Badge>
        ))}
      </div>

      {uniqueTools.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-2">
          {uniqueTools.slice(0, 4).map((tool) => (
            <Badge key={tool} variant="outline" className="text-[10px] px-1.5 py-0 gap-0.5">
              <Wrench className="h-2.5 w-2.5" />
              {formatToolName(tool)}
            </Badge>
          ))}
        </div>
      )}

      <p className="text-[10px] text-muted-foreground mt-2 tabular-nums">
        {formatDate(report.created_at)}
      </p>
    </Card>
  )
}
