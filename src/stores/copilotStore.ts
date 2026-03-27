import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'

// ---------- Types ----------

export interface ToolCallInfo {
  name: string
  summary: string
}

export interface CopilotMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  toolCalls?: ToolCallInfo[]
  timestamp: number
}

interface CopilotResponse {
  response_text: string
  tool_calls_made: Array<{ tool_name: string; result_summary: string }>
}

interface ConversationEntry {
  role: 'user' | 'assistant'
  content: string
}

interface CopilotStore {
  messages: CopilotMessage[]
  isLoading: boolean
  isConfigured: boolean | null
  error: string | null

  checkConfiguration: () => Promise<void>
  sendMessage: (text: string) => Promise<void>
  clearMessages: () => void
  clearError: () => void
}

// ---------- Helpers ----------

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
}

function buildConversationHistory(messages: CopilotMessage[]): ConversationEntry[] {
  return messages.map((m) => ({
    role: m.role,
    content: m.content,
  }))
}

// ---------- Store ----------

export const useCopilotStore = create<CopilotStore>()((set, get) => ({
  messages: [],
  isLoading: false,
  isConfigured: null,
  error: null,

  checkConfiguration: async () => {
    try {
      const configured = await invoke<boolean>('copilot_check_configured')
      set({ isConfigured: configured })
    } catch (err) {
      console.error('Failed to check copilot configuration:', err)
      set({ isConfigured: false })
    }
  },

  sendMessage: async (text: string) => {
    const trimmed = text.trim()
    if (!trimmed) return

    const userMessage: CopilotMessage = {
      id: generateId(),
      role: 'user',
      content: trimmed,
      timestamp: Date.now(),
    }

    set((state) => ({
      messages: [...state.messages, userMessage],
      isLoading: true,
      error: null,
    }))

    try {
      const history = buildConversationHistory(get().messages)
      const response = await invoke<CopilotResponse>('copilot_send_message', {
        message: trimmed,
        conversationHistoryJson: JSON.stringify(history),
      })

      const assistantMessage: CopilotMessage = {
        id: generateId(),
        role: 'assistant',
        content: response.response_text,
        toolCalls: response.tool_calls_made.map((tc) => ({
          name: tc.tool_name,
          summary: tc.result_summary,
        })),
        timestamp: Date.now(),
      }

      set((state) => ({
        messages: [...state.messages, assistantMessage],
        isLoading: false,
      }))
    } catch (err: unknown) {
      let msg: string
      if (err instanceof Error) {
        msg = err.message
      } else if (typeof err === 'object' && err !== null && 'message' in err) {
        msg = String((err as { message: string }).message)
      } else {
        msg = String(err)
      }
      console.error('Copilot send failed:', err)
      set({ isLoading: false, error: msg })
    }
  },

  clearMessages: () => set({ messages: [], error: null }),

  clearError: () => set({ error: null }),
}))

export default useCopilotStore
