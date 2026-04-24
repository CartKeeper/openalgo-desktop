import { FileDown, Loader2, Wrench } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { ScrollArea } from '@/components/ui/scroll-area'
import { extractTickers, formatToolName } from '@/lib/markdown'
import type { CopilotMessage } from '@/stores/copilotStore'
import type { ReportNote } from '@/stores/reportsStore'

interface ReportBuilderDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  messages: CopilotMessage[]
  pinnedIds: string[]
  defaultTitle?: string
  notes?: ReportNote[]
}

export function ReportBuilderDialog({
  open,
  onOpenChange,
  messages,
  pinnedIds,
  defaultTitle,
  notes,
}: ReportBuilderDialogProps) {
  const assistantMessages = messages.filter((m) => m.role === 'assistant')

  // Initialize selected IDs from pinned, or all assistant messages if none pinned
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [reportTitle, setReportTitle] = useState('')
  const [includeNotes, setIncludeNotes] = useState(true)
  const [isGenerating, setIsGenerating] = useState(false)

  // Reset state when dialog opens
  useEffect(() => {
    if (open) {
      const initial = pinnedIds.length > 0
        ? new Set(pinnedIds)
        : new Set(assistantMessages.map((m) => m.id))
      setSelectedIds(initial)
      setReportTitle(defaultTitle || 'Research Report')
      setIncludeNotes(true)
    }
  }, [open])

  const selectedCount = selectedIds.size
  const allSelected = selectedCount === assistantMessages.length
  const noneSelected = selectedCount === 0

  const toggleMessage = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const selectAll = () => {
    setSelectedIds(new Set(assistantMessages.map((m) => m.id)))
  }

  const deselectAll = () => {
    setSelectedIds(new Set())
  }

  const handleGenerate = async () => {
    if (noneSelected) return
    setIsGenerating(true)

    try {
      const selectedMessages = assistantMessages.filter((m) => selectedIds.has(m.id))

      // Collect tickers from all selected messages
      const allTickers = new Set<string>()
      for (const msg of selectedMessages) {
        for (const t of extractTickers(msg.content)) {
          allTickers.add(t)
        }
      }

      const { generateReportPdf } = await import('@/lib/pdf-report')
      await generateReportPdf({
        title: reportTitle,
        messages: selectedMessages,
        notes: includeNotes ? notes : undefined,
        tickers: [...allTickers],
      })

      toast.success('Report PDF generated')
      onOpenChange(false)
    } catch (err) {
      console.error('PDF generation failed:', err)
      toast.error('Failed to generate PDF')
    } finally {
      setIsGenerating(false)
    }
  }

  // Truncate message preview
  const getPreview = (content: string, maxLines = 3): string => {
    const lines = content.split('\n').filter((l) => l.trim() !== '')
    const preview = lines.slice(0, maxLines).join('\n')
    if (lines.length > maxLines) return `${preview}...`
    return preview
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl h-[75vh] flex flex-col rounded-[16px] p-0 gap-0">
        {/* Header */}
        <DialogHeader className="shrink-0 px-6 pt-6 pb-4 border-b">
          <DialogTitle className="text-[16px] font-semibold">Generate PDF Report</DialogTitle>
          <DialogDescription className="text-[12px] text-muted-foreground">
            Select which insights to include in your report.
          </DialogDescription>
          <div className="mt-3">
            <label htmlFor="report-title" className="text-[12px] font-semibold text-muted-foreground block mb-1.5">
              Report Title
            </label>
            <input
              id="report-title"
              type="text"
              value={reportTitle}
              onChange={(e) => setReportTitle(e.target.value)}
              className="w-full h-10 px-3 rounded-[8px] border bg-background text-[14px] placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary"
              placeholder="Report title..."
            />
          </div>
        </DialogHeader>

        {/* Selection toolbar */}
        <div className="shrink-0 px-6 py-3 border-b flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span className="text-[12px] font-semibold text-muted-foreground">
              {selectedCount} of {assistantMessages.length} selected
            </span>
            <Button
              variant="ghost"
              size="sm"
              className="h-8 text-[12px]"
              onClick={allSelected ? deselectAll : selectAll}
            >
              {allSelected ? 'Deselect All' : 'Select All'}
            </Button>
          </div>
          {notes && notes.length > 0 && (
            <label className="flex items-center gap-2 cursor-pointer">
              <Checkbox
                checked={includeNotes}
                onCheckedChange={(checked) => setIncludeNotes(checked === true)}
              />
              <span className="text-[12px] font-semibold text-muted-foreground">
                Include strategy notes ({notes.length})
              </span>
            </label>
          )}
        </div>

        {/* Message list */}
        <ScrollArea className="flex-1 min-h-0">
          <div className="p-4 space-y-3">
            {assistantMessages.length === 0 ? (
              <div className="text-center py-8">
                <p className="text-[14px] text-muted-foreground">No assistant messages to include.</p>
              </div>
            ) : (
              assistantMessages.map((msg) => {
                const isSelected = selectedIds.has(msg.id)
                return (
                  <button
                    key={msg.id}
                    type="button"
                    onClick={() => toggleMessage(msg.id)}
                    className={`w-full text-left rounded-[12px] border p-4 transition-colors duration-150 cursor-pointer ${
                      isSelected
                        ? 'border-l-[3px] border-l-primary bg-primary/4'
                        : 'hover:bg-accent/50'
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <div className="pt-0.5 shrink-0">
                        <Checkbox
                          checked={isSelected}
                          onCheckedChange={() => toggleMessage(msg.id)}
                          onClick={(e) => e.stopPropagation()}
                        />
                      </div>
                      <div className="flex-1 min-w-0">
                        {/* Tool badges */}
                        {msg.toolCalls && msg.toolCalls.length > 0 && (
                          <div className="flex flex-wrap gap-1 mb-2">
                            <Wrench className="h-3 w-3 text-muted-foreground mt-[1px] shrink-0" />
                            {msg.toolCalls.map((tc, idx) => (
                              <Badge
                                key={idx}
                                variant="secondary"
                                className="text-[10px] font-semibold px-1.5 py-0"
                              >
                                {formatToolName(tc.name)}
                              </Badge>
                            ))}
                          </div>
                        )}
                        {/* Content preview */}
                        <p className="text-[13px] leading-[1.5] text-muted-foreground whitespace-pre-wrap line-clamp-3">
                          {getPreview(msg.content)}
                        </p>
                      </div>
                    </div>
                  </button>
                )
              })
            )}
          </div>
        </ScrollArea>

        {/* Footer */}
        <DialogFooter className="shrink-0 px-6 py-4 border-t">
          <Button
            variant="outline"
            size="sm"
            className="h-10"
            onClick={() => onOpenChange(false)}
            disabled={isGenerating}
          >
            Cancel
          </Button>
          <Button
            size="sm"
            className="h-10"
            onClick={handleGenerate}
            disabled={noneSelected || isGenerating || !reportTitle.trim()}
          >
            {isGenerating ? (
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            ) : (
              <FileDown className="h-4 w-4 mr-2" />
            )}
            {isGenerating ? 'Generating...' : 'Generate PDF'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
