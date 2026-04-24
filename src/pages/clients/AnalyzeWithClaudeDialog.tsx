import { invoke } from '@tauri-apps/api/core'
import {
  Bot,
  ClipboardCheck,
  Loader2,
  Send,
  Sparkles,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { renderMarkdown, formatToolName } from '@/lib/markdown'
import { parseActionsFromMarkdown } from '@/lib/parseActions'
import { ActionReviewModal } from '@/components/trading/ActionReviewModal'
import { applyScenarioRecommendations } from '@/lib/applyScenarioRecommendations'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import type { OrderRecommendation } from '@/types/actionQueue'
import { useClientScenarioStore } from '@/stores/clientScenarioStore'

interface CopilotResponse {
  response_text: string
  tool_calls_made: Array<{ tool_name: string; result_summary: string }>
}

interface ConversationEntry {
  role: 'user' | 'assistant'
  content: string
}

interface MessageEntry {
  role: 'user' | 'assistant'
  content: string
  toolCalls?: Array<{ name: string; summary: string }>
}

const SUGGESTED_PROMPTS = [
  'Analyze this portfolio and suggest improvements',
  'What are the biggest risks in this scenario?',
  'Suggest a rebalance to reduce risk',
  'How would a 10% market correction impact this portfolio?',
  'Recommend trades to increase diversification',
]

interface AnalyzeWithClaudeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  clientId: number
  scenarioId: number
  scenarioName: string
  isBaseline: boolean
  onTradesApplied?: () => void
}

export default function AnalyzeWithClaudeDialog({
  open,
  onOpenChange,
  clientId,
  scenarioId,
  scenarioName,
  isBaseline,
  onTradesApplied,
}: AnalyzeWithClaudeDialogProps) {
  const navigate = useNavigate()
  const { cloneScenario } = useClientScenarioStore()
  const setItemsAndOpen = useActionQueueStore((s) => s.setItemsAndOpen)

  const [messages, setMessages] = useState<MessageEntry[]>([])
  const [conversationHistory, setConversationHistory] = useState<ConversationEntry[]>([])
  const [inputValue, setInputValue] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isConfigured, setIsConfigured] = useState<boolean | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (open) {
      invoke<boolean>('copilot_check_configured').then(setIsConfigured).catch(() => setIsConfigured(false))
    }
  }, [open])

  useEffect(() => {
    if (!open) {
      setMessages([])
      setConversationHistory([])
      setInputValue('')
    }
  }, [open])

  useEffect(() => {
    if (scrollRef.current) {
      const viewport = scrollRef.current.querySelector('[data-radix-scroll-area-viewport]')
      if (viewport) {
        viewport.scrollTop = viewport.scrollHeight
      }
    }
  }, [messages, isLoading])

  const sendMessage = async (text: string) => {
    const trimmed = text.trim()
    if (!trimmed || isLoading) return

    // Build the message with scenario context baked in
    const isFirstMessage = messages.length === 0
    const prompt = isFirstMessage
      ? `I'm looking at a sandbox scenario called "${scenarioName}" (scenario_id: ${scenarioId}). Please use the get_client_scenario tool to fetch the positions and pricing first, then: ${trimmed}`
      : trimmed

    const userMessage: MessageEntry = { role: 'user', content: trimmed }
    setMessages((prev) => [...prev, userMessage])
    setInputValue('')
    setIsLoading(true)

    try {
      const history: ConversationEntry[] = [
        ...conversationHistory,
        { role: 'user', content: prompt },
      ]

      const response = await invoke<CopilotResponse>('copilot_send_message', {
        message: prompt,
        conversationHistoryJson: JSON.stringify(
          conversationHistory.length > 0 ? conversationHistory : []
        ),
      })

      const assistantMessage: MessageEntry = {
        role: 'assistant',
        content: response.response_text,
        toolCalls: response.tool_calls_made.map((tc) => ({
          name: tc.tool_name,
          summary: tc.result_summary,
        })),
      }

      setMessages((prev) => [...prev, assistantMessage])
      setConversationHistory([
        ...history,
        { role: 'assistant', content: response.response_text },
      ])

      // Check if trades were applied
      const appliedTrades = response.tool_calls_made.some(
        (tc) => tc.tool_name === 'apply_scenario_trades'
      )
      if (appliedTrades && onTradesApplied) {
        onTradesApplied()
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Copilot error: ${msg}`)
    } finally {
      setIsLoading(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      sendMessage(inputValue)
    }
  }

  const handleReviewRecommendations = (messageContent: string) => {
    const actions = parseActionsFromMarkdown(messageContent, 'copilot')
    if (actions.length === 0) {
      toast.error('No recommended trades found in this message')
      return
    }
    // parseActionsFromMarkdown returns fully-formed OrderRecommendation objects
    // (see src/lib/parseActions.ts) — pass through directly.
    setItemsAndOpen(actions)
  }

  const handleApplyToScenario = async (
    items: OrderRecommendation[],
    cloneName: string | undefined
  ) => {
    const result = await applyScenarioRecommendations({
      items,
      clientId,
      currentScenarioId: scenarioId,
      isBaseline,
      cloneName,
      invoke,
      cloneScenario,
    })

    if (result.failures.length > 0) {
      const failedList = result.failures.map((f) => f.symbol).join(', ')
      toast.warning(
        `${result.applied} applied, ${result.failures.length} failed (${failedList})`
      )
    } else if (result.cloned) {
      toast.success(`Created "${cloneName}" with ${result.applied} trades applied`)
    } else {
      toast.success(`Applied ${result.applied} trades to "${scenarioName}"`)
    }

    if (result.cloned) {
      onOpenChange(false)
      navigate(`/clients/${clientId}/scenarios/${result.targetScenarioId}`)
    } else if (onTradesApplied) {
      onTradesApplied()
    }
  }

  return (
    <>
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl h-[75vh] flex flex-col p-0">
        <DialogHeader className="px-6 pt-6 pb-3 border-b shrink-0">
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-primary" />
            Analyze with Claude
          </DialogTitle>
          <p className="text-xs text-muted-foreground">
            Scenario: {scenarioName}
          </p>
        </DialogHeader>

        {isConfigured === false ? (
          <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6">
            <Bot className="h-12 w-12 text-muted-foreground" />
            <p className="text-sm text-muted-foreground text-center">
              Anthropic API key not configured. Add your API key in Settings &gt; Providers to use the AI copilot.
            </p>
          </div>
        ) : (
          <>
            {/* Messages */}
            <div className="flex-1 min-h-0">
            <ScrollArea className="h-full px-6" ref={scrollRef}>
              <div className="py-4 space-y-4">
                {messages.length === 0 && !isLoading && (
                  <div className="flex flex-col items-center gap-4 py-8">
                    <Sparkles className="h-10 w-10 text-muted-foreground" />
                    <p className="text-sm text-muted-foreground text-center">
                      Ask Claude to analyze this scenario. It will fetch positions and pricing automatically.
                    </p>
                    <div className="flex flex-wrap gap-2 justify-center max-w-lg">
                      {SUGGESTED_PROMPTS.map((prompt) => (
                        <Button
                          key={prompt}
                          variant="outline"
                          size="sm"
                          className="text-xs h-8"
                          onClick={() => sendMessage(prompt)}
                        >
                          {prompt}
                        </Button>
                      ))}
                    </div>
                  </div>
                )}

                {messages.map((msg, idx) => {
                  const hasActions =
                    msg.role === 'assistant' &&
                    parseActionsFromMarkdown(msg.content, 'copilot').length > 0

                  return (
                    <div
                      key={idx}
                      className={`flex flex-col ${msg.role === 'user' ? 'items-end' : 'items-start'}`}
                    >
                      <div
                        className={`max-w-[85%] rounded-[12px] px-4 py-3 ${
                          msg.role === 'user'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted'
                        }`}
                      >
                        {msg.role === 'assistant' && msg.toolCalls && msg.toolCalls.length > 0 && (
                          <div className="flex flex-wrap gap-1 mb-2">
                            {msg.toolCalls.map((tc, i) => (
                              <Badge key={i} variant="secondary" className="text-[10px]">
                                {formatToolName(tc.name)}
                              </Badge>
                            ))}
                          </div>
                        )}
                        {msg.role === 'assistant' ? (
                          <div className="prose prose-sm dark:prose-invert max-w-none text-sm leading-relaxed">
                            {renderMarkdown(msg.content)}
                          </div>
                        ) : (
                          <p className="text-sm">{msg.content}</p>
                        )}
                      </div>
                      {hasActions && (
                        <Button
                          variant="outline"
                          size="sm"
                          className="mt-2 text-xs h-8"
                          onClick={() => handleReviewRecommendations(msg.content)}
                        >
                          <ClipboardCheck className="h-3.5 w-3.5 mr-2" />
                          Review Recommendations
                        </Button>
                      )}
                    </div>
                  )
                })}

                {isLoading && (
                  <div className="flex justify-start">
                    <div className="bg-muted rounded-[12px] px-4 py-3">
                      <div className="flex items-center gap-2 text-sm text-muted-foreground">
                        <Loader2 className="h-4 w-4 animate-spin" />
                        Analyzing...
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </ScrollArea>
            </div>

            {/* Input */}
            <div className="px-6 py-3 border-t shrink-0">
              <div className="flex gap-2">
                <Input
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="Ask about this scenario..."
                  disabled={isLoading}
                  className="flex-1"
                />
                <Button
                  size="icon"
                  onClick={() => sendMessage(inputValue)}
                  disabled={isLoading || !inputValue.trim()}
                  className="h-10 w-10 shrink-0"
                >
                  <Send className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
    <ActionReviewModal
      onApply={handleApplyToScenario}
      cloneNameRequired={isBaseline}
    />
    </>
  )
}
