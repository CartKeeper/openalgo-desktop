import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle,
  ArrowLeft,
  Bot,
  Eye,
  FileDown,
  FileText,
  Key,
  Loader2,
  Pin,
  Search,
  Send,
  ShoppingCart,
  Sparkles,
  Trash2,
  Wrench,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { ActionReviewModal } from '@/components/trading/ActionReviewModal'
import { PlaceOrderDialog } from '@/components/trading/PlaceOrderDialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { ReportBuilderDialog } from '@/components/reports/ReportBuilderDialog'
import { parseActionsFromMarkdown } from '@/lib/parseActions'
import { renderMarkdown, formatToolName, extractTickers } from '@/lib/markdown'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import { useCopilotStore } from '@/stores/copilotStore'
import { useReportsStore } from '@/stores/reportsStore'
import type { CopilotMessage } from '@/stores/copilotStore'

// ---------- Suggested prompts ----------

const SUGGESTED_PROMPTS = [
  "Analyze AAPL's financial health",
  'Find undervalued tech stocks under $50',
  "What's the latest news on NVDA?",
  'Compare MSFT and GOOGL fundamentals',
  'Explain the current economic outlook',
  'Screen for high-dividend stocks',
]

// ---------- Stock action bar ----------

function StockActionBar({ tickers }: { tickers: string[] }) {
  const navigate = useNavigate()

  const handleWatch = async (symbol: string) => {
    try {
      await invoke('add_watchlist_symbol', { symbol })
      toast.success(`${symbol} added to watchlist`)
    } catch {
      toast.error(`Failed to add ${symbol}`)
    }
  }

  if (tickers.length === 0) return null

  return (
    <div className="flex flex-wrap items-center gap-1.5 mt-3 pt-3 border-t border-border/40">
      <span className="text-[10px] font-semibold text-muted-foreground uppercase tracking-[0.05em] mr-1">
        Stocks:
      </span>
      {tickers.map((ticker) => (
        <div
          key={ticker}
          className="flex items-center gap-0.5 rounded-md border bg-background/80 px-1.5 py-0.5"
        >
          <span className="text-[12px] font-semibold mr-1">{ticker}</span>
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5"
            onClick={() => navigate(`/fundamentals/${ticker}`)}
            title={`Research ${ticker}`}
          >
            <Search className="h-3 w-3" />
          </Button>
          <PlaceOrderDialog
            defaultSymbol={ticker}
            trigger={
              <Button
                variant="ghost"
                size="icon"
                className="h-5 w-5 text-green-500 hover:text-green-600"
                title={`Trade ${ticker}`}
              >
                <ShoppingCart className="h-3 w-3" />
              </Button>
            }
          />
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5"
            onClick={() => handleWatch(ticker)}
            title={`Watch ${ticker}`}
          >
            <Eye className="h-3 w-3" />
          </Button>
        </div>
      ))}
    </div>
  )
}

// ---------- Message bubble ----------

function MessageBubble({
  message,
  isPinned,
  onTogglePin,
}: {
  message: CopilotMessage
  isPinned?: boolean
  onTogglePin?: (id: string) => void
}) {
  const isUser = message.role === 'user'
  const tickers = !isUser ? extractTickers(message.content) : []
  const actions = !isUser ? parseActionsFromMarkdown(message.content, 'copilot') : []
  const reviewWithFundsCheck = useActionQueueStore((s) => s.setItemsAndOpenWithFundsCheck)

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-4`}>
      <div
        className={`max-w-[70%] rounded-[12px] px-4 py-3 group ${
          isUser
            ? 'bg-primary text-primary-foreground'
            : isPinned
              ? 'bg-muted/60 border border-l-[3px] border-l-primary'
              : 'bg-muted/60 border'
        }`}
      >
        {isUser ? (
          <p className="text-[14px] leading-[1.5] whitespace-pre-wrap">{message.content}</p>
        ) : (
          <div className="space-y-0">{renderMarkdown(message.content)}</div>
        )}

        {/* Stock action bar */}
        {tickers.length > 0 && <StockActionBar tickers={tickers} />}

        {/* Action recommendations */}
        {actions.length > 0 && (
          <div className="mt-3 pt-3 border-t border-border/40">
            <Button
              variant="outline"
              size="sm"
              className="h-8 gap-1.5 text-[12px]"
              onClick={() => reviewWithFundsCheck(actions)}
            >
              <ShoppingCart className="h-3.5 w-3.5" />
              Review {actions.length} trade{actions.length !== 1 ? 's' : ''}
            </Button>
          </div>
        )}

        {/* Tool call badges + Pin button row */}
        {!isUser && (
          <div className="flex flex-wrap items-center gap-1.5 mt-3 pt-3 border-t border-border/40">
            {message.toolCalls && message.toolCalls.length > 0 && (
              <>
                <Wrench className="h-3 w-3 text-muted-foreground mt-[1px] shrink-0" />
                {message.toolCalls.map((tc, idx) => (
                  <Badge
                    key={idx}
                    variant="secondary"
                    className="text-[10px] font-semibold px-2 py-0"
                  >
                    {formatToolName(tc.name)}
                  </Badge>
                ))}
              </>
            )}
            {onTogglePin && (
              <button
                type="button"
                onClick={() => onTogglePin(message.id)}
                className={`ml-auto h-8 px-3 rounded-[8px] flex items-center gap-1.5 text-[12px] font-semibold transition-colors duration-150 cursor-pointer ${
                  isPinned
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-accent hover:text-foreground'
                }`}
                title={isPinned ? 'Unpin from report' : 'Pin to report'}
              >
                <Pin className="h-3.5 w-3.5" />
                {isPinned ? 'Pinned' : 'Pin'}
              </button>
            )}
          </div>
        )}

        {/* Tool call badges for user messages (shouldn't happen, but safe) */}
        {isUser && message.toolCalls && message.toolCalls.length > 0 && (
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

// ---------- Loading dots ----------

function LoadingDots() {
  return (
    <div className="flex justify-start mb-4">
      <div className="bg-muted/60 border rounded-[12px] px-4 py-3">
        <div className="flex items-center gap-1.5">
          <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-pulse" />
          <span
            className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-pulse"
            style={{ animationDelay: '150ms' }}
          />
          <span
            className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-pulse"
            style={{ animationDelay: '300ms' }}
          />
        </div>
      </div>
    </div>
  )
}

// ---------- Empty state ----------

function EmptyState({ onPromptClick }: { onPromptClick: (prompt: string) => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full px-4">
      <div className="flex items-center justify-center h-12 w-12 rounded-full bg-primary/10 mb-4">
        <Sparkles className="h-6 w-6 text-primary" />
      </div>
      <h2 className="text-[20px] font-semibold mb-1">Research Assistant</h2>
      <p className="text-[14px] text-muted-foreground mb-6 text-center max-w-md">
        Ask questions about stocks, markets, and economic data. The assistant can look up real-time
        quotes, financials, news, and more.
      </p>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 w-full max-w-lg">
        {SUGGESTED_PROMPTS.map((prompt) => (
          <button
            key={prompt}
            type="button"
            onClick={() => onPromptClick(prompt)}
            className="text-left px-4 py-3 rounded-[12px] border bg-card hover:bg-accent/50 transition-colors duration-150 cursor-pointer"
          >
            <p className="text-[14px] leading-[1.5]">{prompt}</p>
          </button>
        ))}
      </div>
    </div>
  )
}

// ---------- Not configured state ----------

function NotConfiguredState() {
  return (
    <div className="flex items-center justify-center h-full px-4">
      <Card className="max-w-md w-full rounded-[12px]">
        <CardContent className="flex flex-col items-center text-center p-6">
          <div className="flex items-center justify-center h-12 w-12 rounded-full bg-yellow-500/10 mb-4">
            <Key className="h-6 w-6 text-yellow-500" />
          </div>
          <h2 className="text-[20px] font-semibold mb-1">API Key Required</h2>
          <p className="text-[14px] text-muted-foreground mb-4">
            To use the Research Assistant, you need to configure your Anthropic API key in the data
            provider settings.
          </p>
          <Button asChild className="h-10">
            <Link to="/generic-setup">
              <Key className="h-4 w-4 mr-2" />
              Configure API Keys
            </Link>
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}

// ---------- Main page ----------

export default function CopilotPage() {
  const navigate = useNavigate()
  const {
    messages,
    pinnedMessageIds,
    isLoading,
    isConfigured,
    error,
    checkConfiguration,
    sendMessage,
    togglePin,
    clearMessages,
    clearError,
  } = useCopilotStore()
  const { saveReport } = useReportsStore()

  const [input, setInput] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [showReportBuilder, setShowReportBuilder] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const pinnedCount = pinnedMessageIds.length

  // Check configuration on mount
  useEffect(() => {
    checkConfiguration()
  }, [checkConfiguration])

  // Auto-scroll to bottom when messages change or loading starts
  useEffect(() => {
    if (scrollRef.current) {
      // ScrollArea viewport is the first child element
      const viewport = scrollRef.current.querySelector('[data-slot="scroll-area-viewport"]')
      if (viewport) {
        viewport.scrollTop = viewport.scrollHeight
      }
    }
  }, [messages, isLoading])

  // Show error via toast and clear
  useEffect(() => {
    if (error) {
      toast.error(error)
      clearError()
    }
  }, [error, clearError])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim() || isLoading) return
    const text = input.trim()
    setInput('')
    await sendMessage(text)
    inputRef.current?.focus()
  }

  const handlePromptClick = async (prompt: string) => {
    if (isLoading) return
    await sendMessage(prompt)
    inputRef.current?.focus()
  }

  const handleClear = () => {
    clearMessages()
    toast.success('Conversation cleared')
    inputRef.current?.focus()
  }

  const handleSaveReport = async () => {
    if (messages.length === 0 || isSaving) return
    setIsSaving(true)
    try {
      // Use first user message as title, first assistant message as summary
      const firstUser = messages.find((m) => m.role === 'user')
      const firstAssistant = messages.find((m) => m.role === 'assistant')
      const title = firstUser?.content.slice(0, 120) || 'Research Report'
      const summary = firstAssistant?.content.slice(0, 300) || ''

      // Extract unique tool names as tags
      const toolNames = new Set<string>()
      for (const msg of messages) {
        if (msg.toolCalls) {
          for (const tc of msg.toolCalls) {
            toolNames.add(tc.name.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()))
          }
        }
      }
      const tags = [...toolNames].slice(0, 10)

      const report = await saveReport(title, summary, tags, messages)
      toast.success('Report saved')
      navigate(`/reports/${report.id}`)
    } catch (err) {
      toast.error('Failed to save report')
    } finally {
      setIsSaving(false)
    }
  }

  // Initial configuration check loading
  if (isConfigured === null) {
    return (
      <div className="flex items-center justify-center h-[calc(100vh-8rem)]">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="flex flex-col h-[calc(100vh-8rem)]">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 shrink-0">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
              <ArrowLeft className="h-4 w-4" />
            </Link>
            <h1 className="text-[20px] font-semibold flex items-center gap-2">
              <Bot className="h-5 w-5" />
              Research Assistant
            </h1>
          </div>
          <p className="text-[14px] text-muted-foreground">
            AI-powered market research and analysis
          </p>
        </div>
        {isConfigured && messages.length > 0 && (
          <div className="flex items-center gap-2">
            {pinnedCount > 0 && (
              <Badge variant="secondary" className="text-[10px] font-semibold px-2 py-0 gap-1">
                <Pin className="h-3 w-3" />
                {pinnedCount} pinned
              </Badge>
            )}
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowReportBuilder(true)}
              disabled={isLoading}
              className="h-8"
            >
              <FileDown className="h-4 w-4 mr-1" />
              Generate PDF
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={handleSaveReport}
              disabled={isSaving || isLoading}
              className="h-8"
            >
              {isSaving ? (
                <Loader2 className="h-4 w-4 mr-1 animate-spin" />
              ) : (
                <FileText className="h-4 w-4 mr-1" />
              )}
              Save Report
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleClear}
              className="h-8 text-muted-foreground hover:text-foreground"
            >
              <Trash2 className="h-4 w-4 mr-1" />
              Clear
            </Button>
          </div>
        )}
      </div>

      {/* Not configured */}
      {!isConfigured && <NotConfiguredState />}

      {/* Configured - chat interface */}
      {isConfigured && (
        <div className="flex flex-col flex-1 min-h-0 rounded-[12px] border bg-card">
          {/* Message area */}
          <div className="flex-1 min-h-0" ref={scrollRef}>
            <ScrollArea className="h-full">
              {messages.length === 0 && !isLoading ? (
                <div className="h-full min-h-[400px] flex items-start justify-center pt-12">
                  <EmptyState onPromptClick={handlePromptClick} />
                </div>
              ) : (
                <div className="p-4">
                  {messages.map((msg) => (
                    <MessageBubble
                      key={msg.id}
                      message={msg}
                      isPinned={pinnedMessageIds.includes(msg.id)}
                      onTogglePin={togglePin}
                    />
                  ))}
                  {isLoading && <LoadingDots />}
                </div>
              )}
            </ScrollArea>
          </div>

          {/* Input area */}
          <div className="shrink-0 border-t p-4">
            {error && (
              <div className="flex items-center gap-2 mb-3 text-[12px] font-semibold text-destructive">
                <AlertCircle className="h-3 w-3 shrink-0" />
                <span className="truncate">{error}</span>
              </div>
            )}
            <form onSubmit={handleSubmit} className="flex items-center gap-3">
              <input
                ref={inputRef}
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                placeholder="Ask about stocks, markets, or economic data..."
                disabled={isLoading}
                className="flex-1 h-10 px-3 rounded-[8px] border bg-background text-[14px] placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary disabled:opacity-50 disabled:cursor-not-allowed"
              />
              <Button
                type="submit"
                size="sm"
                disabled={isLoading || !input.trim()}
                className="h-10 w-10 p-0 shrink-0"
              >
                {isLoading ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Send className="h-4 w-4" />
                )}
              </Button>
            </form>
          </div>
        </div>
      )}

      {/* Report builder dialog */}
      <ReportBuilderDialog
        open={showReportBuilder}
        onOpenChange={setShowReportBuilder}
        messages={messages}
        pinnedIds={pinnedMessageIds}
        defaultTitle={messages.find((m) => m.role === 'user')?.content.slice(0, 120)}
      />

      <ActionReviewModal />
    </div>
  )
}
