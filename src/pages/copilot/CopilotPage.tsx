import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle,
  ArrowLeft,
  Bot,
  Eye,
  FileText,
  Key,
  Loader2,
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
import { PlaceOrderDialog } from '@/components/trading/PlaceOrderDialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
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

// ---------- Simple markdown renderer ----------

function renderMarkdown(text: string): React.ReactNode[] {
  const lines = text.split('\n')
  const elements: React.ReactNode[] = []
  let codeBlock: string[] | null = null
  let codeBlockLang = ''

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]

    // Code block toggle
    if (line.startsWith('```')) {
      if (codeBlock === null) {
        codeBlock = []
        codeBlockLang = line.slice(3).trim()
      } else {
        elements.push(
          <div key={`code-${i}`} className="my-2 rounded-[8px] border bg-muted/50 overflow-hidden">
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

    // Blank line
    if (line.trim() === '') {
      elements.push(<div key={`blank-${i}`} className="h-2" />)
      continue
    }

    // Headers
    if (line.startsWith('### ')) {
      elements.push(
        <p key={`h3-${i}`} className="text-[14px] font-semibold mt-3 mb-1">
          {inlineFormat(line.slice(4))}
        </p>
      )
      continue
    }
    if (line.startsWith('## ')) {
      elements.push(
        <p key={`h2-${i}`} className="text-[14px] font-semibold mt-3 mb-1">
          {inlineFormat(line.slice(3))}
        </p>
      )
      continue
    }
    if (line.startsWith('# ')) {
      elements.push(
        <p key={`h1-${i}`} className="text-[16px] font-semibold mt-3 mb-1">
          {inlineFormat(line.slice(2))}
        </p>
      )
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

  // Handle unclosed code blocks
  if (codeBlock !== null) {
    elements.push(
      <pre key="code-unclosed" className="my-2 p-3 rounded-[8px] border bg-muted/50 overflow-x-auto text-[13px] font-mono">
        <code>{codeBlock.join('\n')}</code>
      </pre>
    )
  }

  return elements
}

function inlineFormat(text: string): React.ReactNode {
  // Process inline code, bold, italic
  const parts: React.ReactNode[] = []
  let remaining = text
  let key = 0

  while (remaining.length > 0) {
    // Inline code
    const codeMatch = remaining.match(/^(.*?)`([^`]+)`(.*)$/)
    if (codeMatch) {
      if (codeMatch[1]) {
        parts.push(...inlineBoldItalic(codeMatch[1], key))
        key += 10
      }
      parts.push(
        <code
          key={`ic-${key++}`}
          className="px-1.5 py-0.5 rounded-[4px] bg-muted text-[13px] font-mono"
        >
          {codeMatch[2]}
        </code>
      )
      remaining = codeMatch[3]
      continue
    }

    // No more inline code, process bold/italic on the rest
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
    // Bold
    const boldMatch = remaining.match(/^(.*?)\*\*(.+?)\*\*(.*)$/)
    if (boldMatch) {
      if (boldMatch[1]) parts.push(<span key={`t-${key++}`}>{boldMatch[1]}</span>)
      parts.push(
        <span key={`b-${key++}`} className="font-semibold">
          {boldMatch[2]}
        </span>
      )
      remaining = boldMatch[3]
      continue
    }

    // Italic
    const italicMatch = remaining.match(/^(.*?)\*(.+?)\*(.*)$/)
    if (italicMatch) {
      if (italicMatch[1]) parts.push(<span key={`t-${key++}`}>{italicMatch[1]}</span>)
      parts.push(
        <em key={`i-${key++}`}>{italicMatch[2]}</em>
      )
      remaining = italicMatch[3]
      continue
    }

    // Plain text
    parts.push(<span key={`t-${key++}`}>{remaining}</span>)
    break
  }

  return parts
}

// ---------- Tool call name formatter ----------

function formatToolName(name: string): string {
  return name
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase())
}

// ---------- Ticker extraction ----------

// Common words that look like tickers but aren't
const TICKER_BLOCKLIST = new Set([
  'A', 'I', 'AM', 'AN', 'AS', 'AT', 'BE', 'BY', 'DO', 'GO', 'IF', 'IN', 'IS', 'IT',
  'MY', 'NO', 'OF', 'ON', 'OR', 'SO', 'TO', 'UP', 'US', 'WE', 'AI', 'CEO', 'CFO',
  'CTO', 'COO', 'IPO', 'ETF', 'GDP', 'SEC', 'FED', 'USA', 'USD', 'EUR', 'GBP',
  'THE', 'FOR', 'AND', 'NOT', 'BUT', 'ALL', 'CAN', 'HER', 'WAS', 'ONE', 'OUR',
  'OUT', 'ARE', 'HAS', 'HIS', 'HOW', 'MAN', 'NEW', 'NOW', 'OLD', 'SEE', 'WAY',
  'WHO', 'BOY', 'DID', 'GET', 'HIM', 'LET', 'SAY', 'SHE', 'TOO', 'USE',
  'PE', 'EPS', 'ROE', 'ROA', 'YOY', 'QOQ', 'TTM', 'FCF', 'DCF', 'YTD',
  'HIGH', 'LOW', 'OPEN', 'SELL', 'BUY', 'HOLD', 'LONG', 'SHORT', 'CALL', 'PUT',
  'CASH', 'DEBT', 'RISK', 'RATE', 'FUND', 'BOND', 'GAIN', 'LOSS', 'BEAR', 'BULL',
  'EBITDA', 'GAAP', 'NYSE', 'NASDAQ',
])

function extractTickers(text: string): string[] {
  const tickers = new Set<string>()

  // Match $TICKER patterns (most reliable)
  const dollarMatches = text.matchAll(/\$([A-Z]{1,5})\b/g)
  for (const m of dollarMatches) {
    tickers.add(m[1])
  }

  // Match **TICKER** bold patterns (common in AI financial analysis)
  const boldMatches = text.matchAll(/\*\*([A-Z]{1,5})\*\*/g)
  for (const m of boldMatches) {
    if (!TICKER_BLOCKLIST.has(m[1])) {
      tickers.add(m[1])
    }
  }

  // Match (TICKER) parenthesized patterns like "Apple (AAPL)"
  const parenMatches = text.matchAll(/\(([A-Z]{1,5})\)/g)
  for (const m of parenMatches) {
    if (!TICKER_BLOCKLIST.has(m[1])) {
      tickers.add(m[1])
    }
  }

  return [...tickers]
}

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

function MessageBubble({ message }: { message: CopilotMessage }) {
  const isUser = message.role === 'user'
  const tickers = !isUser ? extractTickers(message.content) : []

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

        {/* Stock action bar */}
        {tickers.length > 0 && <StockActionBar tickers={tickers} />}

        {/* Tool call badges */}
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
    isLoading,
    isConfigured,
    error,
    checkConfiguration,
    sendMessage,
    clearMessages,
    clearError,
  } = useCopilotStore()
  const { saveReport } = useReportsStore()

  const [input, setInput] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

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
                    <MessageBubble key={msg.id} message={msg} />
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
    </div>
  )
}
