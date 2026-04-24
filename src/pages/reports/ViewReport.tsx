import {
  AlertCircle,
  ArrowLeft,
  BookOpen,
  Check,
  FileDown,
  Loader2,
  Pencil,
  Plus,
  ShoppingCart,
  StickyNote,
  Trash2,
  Wrench,
  X,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { ActionReviewModal } from '@/components/trading/ActionReviewModal'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import { ReportBuilderDialog } from '@/components/reports/ReportBuilderDialog'
import { parseActionsFromMarkdown } from '@/lib/parseActions'
import { renderMarkdown, formatToolName } from '@/lib/markdown'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import { useReportsStore, type ReportNote } from '@/stores/reportsStore'
import type { CopilotMessage, ToolCallInfo } from '@/stores/copilotStore'

function formatDate(dateStr: string): string {
  const d = new Date(dateStr + 'Z')
  if (isNaN(d.getTime())) return dateStr
  return d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
    timeZone: 'America/New_York',
  })
}

function parseTags(tagsJson: string | null): string[] {
  if (!tagsJson) return []
  try {
    return JSON.parse(tagsJson)
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

// ---------- Message bubble (read-only) ----------

function MessageBubble({ message }: { message: CopilotMessage }) {
  const isUser = message.role === 'user'

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-4`}>
      <div
        className={`max-w-[70%] rounded-[12px] px-4 py-3 ${
          isUser
            ? 'bg-primary text-primary-foreground'
            : 'bg-muted/60 border'
        }`}
      >
        {isUser ? (
          <p className="text-[14px] leading-[1.5] whitespace-pre-wrap">{message.content}</p>
        ) : (
          <div className="space-y-0">{renderMarkdown(message.content)}</div>
        )}

        {message.toolCalls && message.toolCalls.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-3 pt-3 border-t border-border/40">
            <Wrench className="h-3 w-3 text-muted-foreground mt-[3px] shrink-0" />
            {message.toolCalls.map((tc, idx) => (
              <Badge
                key={idx}
                variant="secondary"
                className="text-[10px] font-semibold px-2 py-0"
              >
                {formatToolName(tc.name)}
              </Badge>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

// ---------- Note card ----------

function NoteCard({
  note,
  onUpdate,
  onDelete,
}: {
  note: ReportNote
  onUpdate: (id: number, content: string) => void
  onDelete: (id: number) => void
}) {
  const [editing, setEditing] = useState(false)
  const [content, setContent] = useState(note.content)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (editing && textareaRef.current) {
      textareaRef.current.focus()
      textareaRef.current.selectionStart = textareaRef.current.value.length
    }
  }, [editing])

  const handleSave = () => {
    const trimmed = content.trim()
    if (!trimmed) return
    onUpdate(note.id, trimmed)
    setEditing(false)
  }

  const handleCancel = () => {
    setContent(note.content)
    setEditing(false)
  }

  const noteDate = new Date(note.created_at + 'Z')
  const dateStr = isNaN(noteDate.getTime())
    ? note.created_at
    : noteDate.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
        hour12: true,
        timeZone: 'America/New_York',
      })

  return (
    <div className="rounded-[8px] border bg-card p-3 group">
      {editing ? (
        <div className="space-y-2">
          <textarea
            ref={textareaRef}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            className="w-full min-h-[80px] px-3 py-2 rounded-[8px] border bg-background text-[14px] leading-[1.5] resize-y focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary"
          />
          <div className="flex gap-2">
            <Button size="sm" className="h-8" onClick={handleSave} disabled={!content.trim()}>
              <Check className="h-3.5 w-3.5 mr-1" />
              Save
            </Button>
            <Button size="sm" variant="ghost" className="h-8" onClick={handleCancel}>
              <X className="h-3.5 w-3.5 mr-1" />
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <>
          <p className="text-[14px] leading-[1.5] whitespace-pre-wrap">{note.content}</p>
          <div className="flex items-center justify-between mt-2">
            <span className="text-[10px] text-muted-foreground tabular-nums">{dateStr}</span>
            <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
              <Button
                variant="ghost"
                size="sm"
                className="h-8 w-8 p-0"
                onClick={() => setEditing(true)}
              >
                <Pencil className="h-3 w-3" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 w-8 p-0 text-destructive hover:text-destructive"
                onClick={() => onDelete(note.id)}
              >
                <Trash2 className="h-3 w-3" />
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  )
}

// ---------- Main page ----------

export default function ViewReport() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const {
    currentReport,
    currentMessages,
    notes,
    isLoading,
    error,
    fetchReport,
    fetchNotes,
    addNote,
    updateNote,
    deleteNote,
    deleteReport,
    updateReportTitle,
    clearCurrentReport,
  } = useReportsStore()

  const [editingTitle, setEditingTitle] = useState(false)
  const [titleDraft, setTitleDraft] = useState('')
  const [newNote, setNewNote] = useState('')
  const [showNoteInput, setShowNoteInput] = useState(false)
  const [showReportBuilder, setShowReportBuilder] = useState(false)
  const titleInputRef = useRef<HTMLInputElement>(null)
  const noteInputRef = useRef<HTMLTextAreaElement>(null)

  const reportId = id ? parseInt(id, 10) : null

  useEffect(() => {
    if (reportId) {
      fetchReport(reportId)
      fetchNotes(reportId)
    }
    return () => clearCurrentReport()
  }, [reportId, fetchReport, fetchNotes, clearCurrentReport])

  useEffect(() => {
    if (editingTitle && titleInputRef.current) {
      titleInputRef.current.focus()
      titleInputRef.current.select()
    }
  }, [editingTitle])

  useEffect(() => {
    if (showNoteInput && noteInputRef.current) {
      noteInputRef.current.focus()
    }
  }, [showNoteInput])

  const handleTitleSave = async () => {
    if (!reportId || !titleDraft.trim()) return
    try {
      await updateReportTitle(reportId, titleDraft.trim())
      setEditingTitle(false)
      toast.success('Title updated')
    } catch {
      toast.error('Failed to update title')
    }
  }

  const handleAddNote = async () => {
    if (!reportId || !newNote.trim()) return
    try {
      await addNote(reportId, newNote.trim())
      setNewNote('')
      setShowNoteInput(false)
      toast.success('Note added')
    } catch {
      toast.error('Failed to add note')
    }
  }

  const handleUpdateNote = async (noteId: number, content: string) => {
    try {
      await updateNote(noteId, content)
      toast.success('Note updated')
    } catch {
      toast.error('Failed to update note')
    }
  }

  const handleDeleteNote = async (noteId: number) => {
    try {
      await deleteNote(noteId)
      toast.success('Note deleted')
    } catch {
      toast.error('Failed to delete note')
    }
  }

  const handleDeleteReport = async () => {
    if (!reportId) return
    try {
      await deleteReport(reportId)
      toast.success('Report deleted')
      navigate('/reports')
    } catch {
      toast.error('Failed to delete report')
    }
  }

  // Loading
  if (isLoading && !currentReport) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-4 w-96" />
        <div className="space-y-4">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-24 rounded-[12px]" />
          ))}
        </div>
      </div>
    )
  }

  // Error
  if (error && !currentReport) {
    return (
      <div className="space-y-6">
        <Link to="/reports" className="inline-flex items-center gap-1 text-[14px] text-muted-foreground hover:text-foreground">
          <ArrowLeft className="h-4 w-4" />
          Back to Reports
        </Link>
        <Card className="p-8 text-center rounded-[12px]">
          <AlertCircle className="h-12 w-12 mx-auto text-muted-foreground mb-3" />
          <p className="text-[14px] font-semibold">Failed to load report</p>
          <p className="text-[12px] text-muted-foreground mt-1">{error}</p>
          <Button variant="outline" size="sm" className="h-8 mt-4" onClick={() => reportId && fetchReport(reportId)}>
            Retry
          </Button>
        </Card>
      </div>
    )
  }

  if (!currentReport) return null

  const tags = parseTags(currentReport.tags)
  const toolCalls = parseToolCalls(currentReport.tool_calls_json)
  const uniqueTools = [...new Set(toolCalls.map((tc) => tc.name))]

  const reportActions = useMemo(
    () =>
      currentMessages
        .filter((m) => m.role === 'assistant')
        .flatMap((m) => parseActionsFromMarkdown(m.content, 'report')),
    [currentMessages]
  )
  const setItemsAndOpen = useActionQueueStore((s) => s.setItemsAndOpen)

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <Link to="/reports" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          {editingTitle ? (
            <div className="flex items-center gap-2 flex-1">
              <input
                ref={titleInputRef}
                value={titleDraft}
                onChange={(e) => setTitleDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleTitleSave()
                  if (e.key === 'Escape') setEditingTitle(false)
                }}
                className="flex-1 h-10 px-3 rounded-[8px] border bg-background text-[20px] font-semibold focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary"
              />
              <Button size="sm" className="h-8" onClick={handleTitleSave}>
                <Check className="h-3.5 w-3.5" />
              </Button>
              <Button size="sm" variant="ghost" className="h-8" onClick={() => setEditingTitle(false)}>
                <X className="h-3.5 w-3.5" />
              </Button>
            </div>
          ) : (
            <div className="flex items-center gap-2 flex-1 min-w-0">
              <h1 className="text-[20px] font-semibold truncate">{currentReport.title}</h1>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 w-8 p-0 shrink-0"
                onClick={() => {
                  setTitleDraft(currentReport.title)
                  setEditingTitle(true)
                }}
              >
                <Pencil className="h-3.5 w-3.5" />
              </Button>
              <div className="ml-auto shrink-0 flex items-center gap-2">
                {reportActions.length > 0 && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8"
                    onClick={() => setItemsAndOpen(reportActions)}
                  >
                    <ShoppingCart className="h-4 w-4 mr-1" />
                    Review {reportActions.length} Action{reportActions.length !== 1 ? 's' : ''}
                  </Button>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8"
                  onClick={() => setShowReportBuilder(true)}
                >
                  <FileDown className="h-4 w-4 mr-1" />
                  Generate PDF
                </Button>
              </div>
            </div>
          )}
        </div>
        <div className="flex items-center gap-3 text-[12px] text-muted-foreground">
          <span className="tabular-nums">{formatDate(currentReport.created_at)}</span>
          {tags.length > 0 && (
            <div className="flex gap-1">
              {tags.map((tag) => (
                <Badge key={tag} variant="secondary" className="text-[10px] px-1.5 py-0">
                  {tag}
                </Badge>
              ))}
            </div>
          )}
        </div>
        {uniqueTools.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-2">
            {uniqueTools.map((tool) => (
              <Badge key={tool} variant="outline" className="text-[10px] px-1.5 py-0 gap-0.5">
                <Wrench className="h-2.5 w-2.5" />
                {formatToolName(tool)}
              </Badge>
            ))}
          </div>
        )}
      </div>

      {/* Two-column layout: Conversation + Notes */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Conversation */}
        <div className="lg:col-span-2">
          <Card className="rounded-[12px] overflow-hidden">
            <div className="border-b px-4 py-3">
              <h2 className="text-[14px] font-semibold flex items-center gap-2">
                <BookOpen className="h-4 w-4 text-muted-foreground" />
                Research Conversation
              </h2>
            </div>
            <ScrollArea className="h-[calc(100vh-20rem)]">
              <div className="p-4">
                {currentMessages.length === 0 ? (
                  <div className="text-center py-8">
                    <p className="text-[14px] text-muted-foreground">No conversation data</p>
                  </div>
                ) : (
                  currentMessages.map((msg) => (
                    <MessageBubble key={msg.id} message={msg} />
                  ))
                )}
              </div>
            </ScrollArea>
          </Card>
        </div>

        {/* Notes sidebar */}
        <div className="lg:col-span-1 space-y-4">
          <Card className="rounded-[12px] overflow-hidden">
            <div className="border-b px-4 py-3 flex items-center justify-between">
              <h2 className="text-[14px] font-semibold flex items-center gap-2">
                <StickyNote className="h-4 w-4 text-muted-foreground" />
                Strategy Notes
              </h2>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 w-8 p-0"
                onClick={() => setShowNoteInput(true)}
              >
                <Plus className="h-4 w-4" />
              </Button>
            </div>
            <ScrollArea className="h-[calc(100vh-26rem)]">
              <div className="p-4 space-y-3">
                {/* New note input */}
                {showNoteInput && (
                  <div className="rounded-[8px] border border-primary bg-card p-3 space-y-2">
                    <textarea
                      ref={noteInputRef}
                      value={newNote}
                      onChange={(e) => setNewNote(e.target.value)}
                      placeholder="Add a strategy note..."
                      className="w-full min-h-[80px] px-3 py-2 rounded-[8px] border bg-background text-[14px] leading-[1.5] resize-y focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-muted-foreground"
                    />
                    <div className="flex gap-2">
                      <Button size="sm" className="h-8" onClick={handleAddNote} disabled={!newNote.trim()}>
                        <Plus className="h-3.5 w-3.5 mr-1" />
                        Add Note
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-8"
                        onClick={() => {
                          setShowNoteInput(false)
                          setNewNote('')
                        }}
                      >
                        Cancel
                      </Button>
                    </div>
                  </div>
                )}

                {/* Notes list */}
                {notes.length === 0 && !showNoteInput && (
                  <div className="text-center py-6">
                    <StickyNote className="h-8 w-8 mx-auto text-muted-foreground mb-2" />
                    <p className="text-[14px] text-muted-foreground">No notes yet</p>
                    <p className="text-[12px] text-muted-foreground mt-1">
                      Add strategy notes to build on this research later.
                    </p>
                  </div>
                )}

                {notes.map((note) => (
                  <NoteCard
                    key={note.id}
                    note={note}
                    onUpdate={handleUpdateNote}
                    onDelete={handleDeleteNote}
                  />
                ))}
              </div>
            </ScrollArea>
          </Card>

          {/* Delete report */}
          <Button
            variant="outline"
            size="sm"
            className="h-8 w-full text-destructive hover:text-destructive"
            onClick={handleDeleteReport}
          >
            <Trash2 className="h-3.5 w-3.5 mr-1" />
            Delete Report
          </Button>
        </div>
      </div>

      {/* Report builder dialog */}
      <ReportBuilderDialog
        open={showReportBuilder}
        onOpenChange={setShowReportBuilder}
        messages={currentMessages}
        pinnedIds={[]}
        defaultTitle={currentReport.title}
        notes={notes}
      />

      <ActionReviewModal />
    </div>
  )
}
