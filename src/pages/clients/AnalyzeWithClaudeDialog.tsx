import { invoke } from '@tauri-apps/api/core'
import {
  Bot,
  ClipboardCheck,
  Copy,
  FileText,
  HelpCircle,
  ListChecks,
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
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ScrollArea } from '@/components/ui/scroll-area'
import { renderMarkdown, formatToolName } from '@/lib/markdown'
import { parseActionsFromMarkdown } from '@/lib/parseActions'
import {
  applyProgress,
  initialStages,
  onStagedProgress,
  runStagedAnalysis,
  type StageRow,
} from '@/api/stagedAnalysis'
import { StagedProgress } from '@/components/clients/StagedProgress'
import { ActionReviewModal } from '@/components/trading/ActionReviewModal'
import { applyScenarioRecommendations } from '@/lib/applyScenarioRecommendations'
import { formatRecommendationsAsCsv } from '@/lib/csvRecommendations'
import { filterRecommendationsForAccount } from '@/lib/filterRecommendationsForAccount'
import { allowedScenarioTrades } from '@/lib/scenarioReview'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import type { OrderRecommendation } from '@/types/actionQueue'
import { useClientScenarioStore } from '@/stores/clientScenarioStore'
import { useReportsStore } from '@/stores/reportsStore'
import type { CopilotMessage } from '@/stores/copilotStore'
import { downloadGoldmanBriefPdf } from '@/components/reports/goldman/generator'
import type { GoldmanBrief } from '@/components/reports/goldman/types'

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

/** What the dialog's help icon explains: the chat, the full analysis, its stages and its checks. */
const HELP_ITEMS: Array<{ title: string; description: string }> = [
  {
    title: 'Send',
    description:
      'The tool-driven chat: Claude fetches the scenario and its prices, then answers your question. Chat and the full analysis share one conversation, so you can ask about a report after it arrives.',
  },
  {
    title: 'Run full analysis',
    description:
      'A staged analysis of this scenario. The app first builds a dossier (positions, live prices, fundamentals, headlines, sector and macro data); Claude then works through four analysts, a bull case, a bear case, a research manager, a risk panel and a portfolio manager. It takes several minutes, about nine Claude calls and roughly a hundred market-data calls (FMP, FRED, Yahoo). Closing this dialog does not stop a run; reopen it to see the stages or the report.',
  },
  {
    title: 'The stages',
    description:
      'Dossier, Analysts, Bull case, Bear case, Research manager, Risk panel, Portfolio manager, Checks, Report. The list above the input shows each stage as it runs. A stage that fails is marked and the error names it; stages already finished stay marked done, and there is no partial report, so start the run again.',
  },
  {
    title: 'Focus',
    description:
      'Type a focus in the box before clicking Run full analysis (for example "tax efficiency" or "reduce single-stock risk"); the portfolio manager is told. Leave the box empty for a general analysis.',
  },
  {
    title: 'Trades are checked by the app',
    description:
      "Before any trade is offered, the app rounds it to whole shares, drops sells of anything the scenario does not hold (and trims sells to what is held), caps buys to the scenario's cash plus the proceeds of the proposed sells, and drops buys the account's guidelines flag, such as leveraged or inverse funds in a 401(k) or IRA. Dropped trades are listed in the report with the reason.",
  },
  {
    title: 'Nothing is applied automatically',
    description:
      "The report's trades wait under Review Recommendations. On a baseline scenario, Review applies them to a new clone and leaves the baseline untouched.",
  },
  {
    title: 'Prices and times',
    description:
      'The report quotes live prices taken when the dossier was built, with the time shown in Eastern time. Data a provider could not supply is named in the report rather than guessed.',
  },
]

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
  /** Account type ('401k', 'roth_ira', etc.) — used to enforce short-selling restrictions on AI output. */
  accountType?: string | null
  /** Symbol → net long quantity in the active scenario. Used by the restriction filter. */
  longQtyBySymbol?: Record<string, number>
  onTradesApplied?: () => void
}

export default function AnalyzeWithClaudeDialog({
  open,
  onOpenChange,
  clientId,
  scenarioId,
  scenarioName,
  isBaseline,
  accountType = null,
  longQtyBySymbol = {},
  onTradesApplied,
}: AnalyzeWithClaudeDialogProps) {
  const navigate = useNavigate()
  const { cloneScenario } = useClientScenarioStore()
  const setItemsAndOpen = useActionQueueStore((s) => s.setItemsAndOpen)
  const saveReport = useReportsStore((s) => s.saveReport)

  const [messages, setMessages] = useState<MessageEntry[]>([])
  const [conversationHistory, setConversationHistory] = useState<ConversationEntry[]>([])
  const [inputValue, setInputValue] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  // The staged "Run full analysis": its stage list while it runs (or after it failed).
  const [stages, setStages] = useState<StageRow[] | null>(null)
  const [isRunningFull, setIsRunningFull] = useState(false)
  // Mirrors isRunningFull for the close effect, which must not re-run when a
  // run ends while the dialog is closed (that would discard the report).
  const runningFullRef = useRef(false)
  const [isSaving, setIsSaving] = useState(false)
  const [isGeneratingBrief, setIsGeneratingBrief] = useState(false)
  const [isConfigured, setIsConfigured] = useState<boolean | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (open) {
      invoke<boolean>('copilot_check_configured').then(setIsConfigured).catch(() => setIsConfigured(false))
    }
  }, [open])

  useEffect(() => {
    if (!open) {
      // Flush any in-flight review so the items don't bleed into other
      // ActionReviewModal hosts (briefing, copilot, reports) on next navigation.
      if (useActionQueueStore.getState().isReviewOpen) {
        useActionQueueStore.getState().close()
      }
      // A full analysis keeps running after the dialog closes (it cannot be
      // cancelled); keep its request, stage list and eventual report so that
      // reopening shows them. Everything else resets as before.
      if (runningFullRef.current) return
      setMessages([])
      setConversationHistory([])
      setInputValue('')
      setStages(null)
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
    if (!trimmed || isLoading || isRunningFull) return

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
        // A client's sandbox: the backend must not size buys to the user's own broker cash.
        context: 'client_scenario',
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

  /**
   * The staged pipeline (dossier → analysts → bull/bear → research manager →
   * risk panel → portfolio manager → app checks → report). The input text, if
   * any, is the focus handed to the portfolio manager. The report lands in the
   * chat exactly as a chat answer does, so Review Recommendations appears when
   * it carries trades, and a follow-up question sees it in the history.
   */
  const runFullAnalysis = async () => {
    if (isLoading || isRunningFull) return
    const focus = inputValue.trim()
    const request = focus || 'Run a full analysis of this scenario.'
    const prompt = `I'm looking at a sandbox scenario called "${scenarioName}" (scenario_id: ${scenarioId}). ${request}`

    setMessages((prev) => [...prev, { role: 'user', content: request }])
    setInputValue('')
    setStages(initialStages())
    setIsRunningFull(true)
    runningFullRef.current = true

    let unlisten: (() => void) | undefined
    try {
      unlisten = await onStagedProgress(scenarioId, (progress) => {
        setStages((prev) => (prev ? applyProgress(prev, progress) : prev))
      })
      const response = await runStagedAnalysis(scenarioId, focus || undefined)
      setMessages((prev) => [...prev, { role: 'assistant', content: response.response_text }])
      setConversationHistory((prev) => [
        ...prev,
        { role: 'user', content: prompt },
        { role: 'assistant', content: response.response_text },
      ])
      setStages(null)
    } catch (err) {
      // The stage list stays up so the failed stage is visible until the next run.
      const msg = err instanceof Error ? err.message : String(err)
      toast.error(`Full analysis failed: ${msg}`)
    } finally {
      unlisten?.()
      runningFullRef.current = false
      setIsRunningFull(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      sendMessage(inputValue)
    }
  }

  const handleReviewRecommendations = (messageContent: string) => {
    const allowed = allowedScenarioTrades(parseActionsFromMarkdown(messageContent, 'copilot'), {
      accountType,
      longQtyBySymbol,
    })
    if (allowed.length > 0) setItemsAndOpen(allowed)
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

  const handleSaveAsReport = async () => {
    if (messages.length === 0 || isSaving) return
    setIsSaving(true)
    try {
      const dateLabel = new Date().toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
        timeZone: 'America/New_York',
      })
      const title = `Scenario Analysis — ${scenarioName} (${dateLabel})`
      const firstAssistant = messages.find((m) => m.role === 'assistant')
      const summary = firstAssistant?.content.slice(0, 300) || ''
      const allToolNames = new Set<string>()
      for (const m of messages) {
        if (m.toolCalls) for (const tc of m.toolCalls) allToolNames.add(tc.name)
      }
      const tags = [
        'Scenario Analysis',
        ...Array.from(allToolNames)
          .map((n) => n.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()))
          .slice(0, 5),
      ]
      const ts = Date.now()
      const reportMessages: CopilotMessage[] = messages.map((m, i) => ({
        id: String(i + 1),
        role: m.role,
        content: m.content,
        toolCalls: m.toolCalls,
        timestamp: ts + i,
      }))
      // The link lets the saved report apply its trades to this scenario later.
      const report = await saveReport(title, summary, tags, reportMessages, scenarioId)
      toast.success('Conversation saved as report')
      onOpenChange(false)
      navigate(`/reports/${report.id}`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to save report')
    } finally {
      setIsSaving(false)
    }
  }

  /**
   * Bypass the chat → save → ReportBuilder → research-template chain and produce
   * a structured Goldman Sax & Violins brief directly from the scenario data.
   *
   * Calls `generate_scenario_brief` (Claude returns four-movement structured
   * JSON), then renders via `<GoldmanReport>` — same template that produces
   * client briefs after import. Output matches the reference HTML structurally
   * (movements, allocation cards, gold-bordered tables, ⚜ flourish).
   */
  const handleGenerateBriefPdf = async () => {
    if (isGeneratingBrief) return
    setIsGeneratingBrief(true)
    try {
      const brief = await invoke<GoldmanBrief>('generate_scenario_brief', {
        scenarioId,
      })
      await downloadGoldmanBriefPdf(brief)
      toast.success('Goldman brief downloaded.')
    } catch (err) {
      toast.error(
        err instanceof Error
          ? `Brief generation failed: ${err.message}`
          : 'Brief generation failed.',
      )
    } finally {
      setIsGeneratingBrief(false)
    }
  }

  const handleCopyMessageCsv = async (messageContent: string) => {
    const actions = parseActionsFromMarkdown(messageContent, 'copilot')
    if (actions.length === 0) {
      toast.error('No recommended trades found in this message')
      return
    }
    const { allowed, dropped } = filterRecommendationsForAccount(actions, {
      accountType,
      longQtyBySymbol,
    })
    if (allowed.length === 0) {
      toast.error('All recommendations were disallowed for this account type')
      return
    }
    try {
      const csv = formatRecommendationsAsCsv(allowed)
      await navigator.clipboard.writeText(csv)
      const suffix = dropped.length > 0 ? ` (${dropped.length} dropped)` : ''
      toast.success(`Copied ${allowed.length} trade${allowed.length !== 1 ? 's' : ''} to clipboard${suffix}`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to copy')
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
            <Popover>
              <PopoverTrigger asChild>
                <button
                  type="button"
                  aria-label="Help"
                  className="ml-1 flex h-8 w-8 items-center justify-center rounded-[8px] text-muted-foreground transition-colors duration-150 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 active:scale-[0.97]"
                >
                  <HelpCircle className="h-5 w-5" strokeWidth={1.5} />
                </button>
              </PopoverTrigger>
              <PopoverContent align="start" className="w-80 max-h-[60vh] overflow-y-auto space-y-3 p-4">
                <p className="text-sm font-semibold">Analyze with Claude</p>
                {HELP_ITEMS.map((item) => (
                  <div key={item.title} className="space-y-1">
                    <p className="text-xs font-semibold">{item.title}</p>
                    <p className="text-xs font-normal leading-relaxed text-muted-foreground">{item.description}</p>
                  </div>
                ))}
              </PopoverContent>
            </Popover>
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
                        <div className="mt-2 flex items-center gap-2">
                          <Button
                            variant="outline"
                            size="sm"
                            className="text-xs h-8"
                            onClick={() => handleReviewRecommendations(msg.content)}
                          >
                            <ClipboardCheck className="h-3.5 w-3.5 mr-2" />
                            Review Recommendations
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => handleCopyMessageCsv(msg.content)}
                            title="Copy as CSV"
                            aria-label="Copy as CSV"
                            className="h-8 w-8 p-0 text-muted-foreground hover:text-foreground"
                          >
                            <Copy className="h-3.5 w-3.5" />
                          </Button>
                        </div>
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
            <div className="px-6 py-3 border-t shrink-0 space-y-2">
              <div className="flex items-center justify-end gap-2">
                <Button
                  variant="default"
                  size="sm"
                  className="h-8 gap-2"
                  onClick={handleGenerateBriefPdf}
                  disabled={isGeneratingBrief}
                >
                  {isGeneratingBrief ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Sparkles className="h-3.5 w-3.5" />
                  )}
                  {isGeneratingBrief ? 'Composing brief…' : 'Generate Brief PDF'}
                </Button>
                {messages.some((m) => m.role === 'assistant') && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8 gap-2"
                    onClick={handleSaveAsReport}
                    disabled={isSaving}
                  >
                    {isSaving ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <FileText className="h-3.5 w-3.5" />
                    )}
                    {isSaving ? 'Saving...' : 'Save chat as report'}
                  </Button>
                )}
              </div>
              {stages && <StagedProgress stages={stages} />}
              <div className="flex gap-2">
                <Input
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="Ask about this scenario..."
                  disabled={isLoading || isRunningFull}
                  className="flex-1"
                />
                <Button
                  size="icon"
                  onClick={() => sendMessage(inputValue)}
                  disabled={isLoading || isRunningFull || !inputValue.trim()}
                  className="h-10 w-10 shrink-0"
                >
                  <Send className="h-4 w-4" />
                </Button>
                <Button
                  variant="secondary"
                  onClick={runFullAnalysis}
                  disabled={isLoading || isRunningFull}
                  className="h-10 shrink-0 gap-2"
                  title="Dossier, four analysts, bull and bear, research manager, risk panel, portfolio manager, then the app's checks. Several minutes."
                >
                  {isRunningFull ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <ListChecks className="h-4 w-4" />
                  )}
                  {isRunningFull ? 'Running…' : 'Run full analysis'}
                </Button>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
    {/* Mounted per-dialog-instance (like BriefingPage/CopilotPage/ViewReport)
        so scenario-mode props are local to this flow. Scenario Detail's
        Allocation card ("Review plan" in AllocationCard.tsx) depends on this
        modal staying mounted unconditionally; it opens it via setItemsAndOpen. */}
    <ActionReviewModal
      onApply={handleApplyToScenario}
      cloneNameRequired={isBaseline}
    />
    </>
  )
}
