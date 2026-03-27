import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import type { CopilotMessage, ToolCallInfo } from './copilotStore'

// ---------- Types ----------

export interface ResearchReportSummary {
  id: number
  title: string
  summary: string | null
  tags: string | null
  tool_calls_json: string | null
  created_at: string
}

export interface ResearchReport extends ResearchReportSummary {
  messages_json: string
  updated_at: string
}

export interface ReportNote {
  id: number
  report_id: number
  content: string
  created_at: string
  updated_at: string
}

interface ReportsStore {
  reports: ResearchReportSummary[]
  currentReport: ResearchReport | null
  currentMessages: CopilotMessage[]
  notes: ReportNote[]
  isLoading: boolean
  error: string | null

  fetchReports: () => Promise<void>
  fetchReport: (id: number) => Promise<void>
  saveReport: (
    title: string,
    summary: string,
    tags: string[],
    messages: CopilotMessage[],
  ) => Promise<ResearchReport>
  deleteReport: (id: number) => Promise<void>
  updateReportTitle: (id: number, title: string) => Promise<void>
  fetchNotes: (reportId: number) => Promise<void>
  addNote: (reportId: number, content: string) => Promise<void>
  updateNote: (noteId: number, content: string) => Promise<void>
  deleteNote: (noteId: number) => Promise<void>
  clearCurrentReport: () => void
  clearError: () => void
}

// ---------- Helpers ----------

function extractToolCalls(messages: CopilotMessage[]): ToolCallInfo[] {
  const calls: ToolCallInfo[] = []
  for (const msg of messages) {
    if (msg.toolCalls) {
      calls.push(...msg.toolCalls)
    }
  }
  return calls
}

// ---------- Store ----------

export const useReportsStore = create<ReportsStore>((set, get) => ({
  reports: [],
  currentReport: null,
  currentMessages: [],
  notes: [],
  isLoading: false,
  error: null,

  fetchReports: async () => {
    set({ isLoading: true, error: null })
    try {
      const reports = await invoke<ResearchReportSummary[]>('get_research_reports')
      set({ reports, isLoading: false })
    } catch (err) {
      set({ error: String(err), isLoading: false })
    }
  },

  fetchReport: async (id: number) => {
    set({ isLoading: true, error: null })
    try {
      const report = await invoke<ResearchReport>('get_research_report', { id })
      let messages: CopilotMessage[] = []
      try {
        messages = JSON.parse(report.messages_json)
      } catch {
        // If messages can't be parsed, just show empty
      }
      set({ currentReport: report, currentMessages: messages, isLoading: false })
    } catch (err) {
      set({ error: String(err), isLoading: false })
    }
  },

  saveReport: async (title, summary, tags, messages) => {
    const messagesJson = JSON.stringify(messages)
    const toolCalls = extractToolCalls(messages)
    const toolCallsJson = toolCalls.length > 0 ? JSON.stringify(toolCalls) : null
    const tagsJson = tags.length > 0 ? JSON.stringify(tags) : null

    const report = await invoke<ResearchReport>('save_research_report', {
      title,
      summary: summary || null,
      tags: tagsJson,
      messagesJson,
      toolCallsJson,
    })

    // Refresh the list
    get().fetchReports()
    return report
  },

  deleteReport: async (id: number) => {
    await invoke<boolean>('delete_research_report', { id })
    set((s) => ({
      reports: s.reports.filter((r) => r.id !== id),
      currentReport: s.currentReport?.id === id ? null : s.currentReport,
    }))
  },

  updateReportTitle: async (id: number, title: string) => {
    const updated = await invoke<ResearchReport>('update_research_report_title', { id, title })
    set((s) => ({
      reports: s.reports.map((r) => (r.id === id ? { ...r, title: updated.title } : r)),
      currentReport: s.currentReport?.id === id ? updated : s.currentReport,
    }))
  },

  fetchNotes: async (reportId: number) => {
    try {
      const notes = await invoke<ReportNote[]>('get_report_notes', { reportId })
      set({ notes })
    } catch (err) {
      console.error('Failed to fetch notes:', err)
    }
  },

  addNote: async (reportId: number, content: string) => {
    const note = await invoke<ReportNote>('add_report_note', { reportId, content })
    set((s) => ({ notes: [...s.notes, note] }))
  },

  updateNote: async (noteId: number, content: string) => {
    const updated = await invoke<ReportNote>('update_report_note', { noteId, content })
    set((s) => ({
      notes: s.notes.map((n) => (n.id === noteId ? updated : n)),
    }))
  },

  deleteNote: async (noteId: number) => {
    await invoke<boolean>('delete_report_note', { noteId })
    set((s) => ({ notes: s.notes.filter((n) => n.id !== noteId) }))
  },

  clearCurrentReport: () => set({ currentReport: null, currentMessages: [], notes: [] }),
  clearError: () => set({ error: null }),
}))
