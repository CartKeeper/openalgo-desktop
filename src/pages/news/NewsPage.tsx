import { open } from '@tauri-apps/plugin-shell'
import {
  AlertCircle,
  ArrowLeft,
  ExternalLink,
  Newspaper,
  RefreshCw,
  Search,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { providerCommands } from '@/api/tauri-client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'

// ============================================================================
// Types
// ============================================================================

interface NewsArticle {
  title: string
  url: string
  published_date: string
  source: string
  summary: string
  symbol: string
  image: string
}

type LoadingState = 'idle' | 'loading' | 'success' | 'error'

/** Strip HTML tags from a string, returning plain text */
function stripHtml(html: string): string {
  return html.replace(/<[^>]*>/g, '').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&nbsp;/g, ' ').replace(/\s+/g, ' ').trim()
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Formats a date string into a human-readable relative time.
 * Handles ISO strings and common date formats from FMP.
 */
function formatRelativeTime(dateStr: string): string {
  if (!dateStr) return ''

  const now = Date.now()
  const date = new Date(dateStr)
  const diffMs = now - date.getTime()

  if (isNaN(diffMs) || diffMs < 0) return ''

  const seconds = Math.floor(diffMs / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  const weeks = Math.floor(days / 7)
  const months = Math.floor(days / 30)

  if (seconds < 60) return 'just now'
  if (minutes === 1) return '1 minute ago'
  if (minutes < 60) return `${minutes} minutes ago`
  if (hours === 1) return '1 hour ago'
  if (hours < 24) return `${hours} hours ago`
  if (days === 1) return 'yesterday'
  if (days < 7) return `${days} days ago`
  if (weeks === 1) return '1 week ago'
  if (weeks < 4) return `${weeks} weeks ago`
  if (months === 1) return '1 month ago'
  if (months < 12) return `${months} months ago`

  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  })
}

/**
 * Opens a URL in the system default browser via Tauri shell plugin,
 * with a fallback to window.open for development environments.
 */
async function openExternal(url: string): Promise<void> {
  try {
    await open(url)
  } catch {
    window.open(url, '_blank', 'noopener,noreferrer')
  }
}

// ============================================================================
// Skeleton Loader
// ============================================================================

function NewsCardSkeleton() {
  return (
    <Card className="flex flex-row gap-0 overflow-hidden p-0">
      <div className="hidden sm:block w-[140px] min-h-[160px] shrink-0">
        <Skeleton className="h-full w-full rounded-none" />
      </div>
      <div className="flex flex-col gap-3 p-4 flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <Skeleton className="h-5 w-16 rounded-full" />
          <Skeleton className="h-4 w-24" />
        </div>
        <Skeleton className="h-5 w-full" />
        <Skeleton className="h-5 w-3/4" />
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-4 w-5/6" />
        <Skeleton className="h-4 w-2/3" />
      </div>
    </Card>
  )
}

// ============================================================================
// Empty State
// ============================================================================

function EmptyState({ symbols }: { symbols: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 gap-4">
      <div className="rounded-full bg-muted p-4">
        <Newspaper className="h-12 w-12 text-muted-foreground" />
      </div>
      <div className="text-center">
        <h3 className="text-base font-semibold">No news found</h3>
        <p className="text-sm text-muted-foreground mt-1">
          {symbols
            ? `No articles found for "${symbols}". Try different symbols or clear the filter.`
            : 'No market news available right now. Try again later.'}
        </p>
      </div>
    </div>
  )
}

// ============================================================================
// Error State
// ============================================================================

function ErrorState({ message }: { message: string }) {
  const isMissingKey =
    message.toLowerCase().includes('api key') ||
    message.toLowerCase().includes('unauthorized') ||
    message.toLowerCase().includes('forbidden') ||
    message.toLowerCase().includes('not configured')

  return (
    <div className="flex flex-col items-center justify-center py-16 gap-4">
      <div className="rounded-full bg-destructive/10 p-4">
        <AlertCircle className="h-12 w-12 text-destructive" />
      </div>
      <div className="text-center">
        <h3 className="text-base font-semibold">
          {isMissingKey ? 'FMP API Key Required' : 'Failed to Load News'}
        </h3>
        <p className="text-sm text-muted-foreground mt-1 max-w-md">
          {isMissingKey
            ? 'A Financial Modeling Prep (FMP) API key is needed to fetch market news. Configure it in the setup page.'
            : message}
        </p>
      </div>
      {isMissingKey && (
        <Link to="/generic-setup">
          <Button variant="outline" size="sm">
            Configure API Key
          </Button>
        </Link>
      )}
    </div>
  )
}

// ============================================================================
// News Card
// ============================================================================

function NewsCard({ article }: { article: NewsArticle }) {
  const [imgError, setImgError] = useState(false)
  const hasImage = article.image && !imgError

  const handleClick = () => {
    if (article.url) {
      openExternal(article.url)
    }
  }

  return (
    <Card
      className="flex flex-row gap-0 overflow-hidden p-0 cursor-pointer transition-colors duration-150 hover:bg-accent/40 min-h-[160px]"
      onClick={handleClick}
    >
      {/* Thumbnail */}
      <div className="hidden sm:flex w-[140px] shrink-0 items-center justify-center bg-muted/50 overflow-hidden">
        {hasImage ? (
          <img
            src={article.image}
            alt=""
            className="h-full w-full object-cover"
            onError={() => setImgError(true)}
            loading="lazy"
          />
        ) : (
          <Newspaper className="h-8 w-8 text-muted-foreground/50" />
        )}
      </div>

      {/* Content */}
      <div className="flex flex-col gap-2 p-4 flex-1 min-w-0">
        {/* Meta row: source + time + external icon */}
        <div className="flex items-center gap-2 flex-wrap">
          {article.source && (
            <Badge variant="secondary" className="text-[11px] font-semibold shrink-0">
              {article.source}
            </Badge>
          )}
          {article.symbol && (
            <Badge variant="outline" className="text-[11px] font-semibold shrink-0">
              {article.symbol}
            </Badge>
          )}
          {article.published_date && (
            <span className="text-xs text-muted-foreground">
              {formatRelativeTime(article.published_date)}
            </span>
          )}
          <ExternalLink className="h-3 w-3 text-muted-foreground/50 ml-auto shrink-0" />
        </div>

        {/* Title */}
        <h3 className="text-sm font-semibold leading-snug line-clamp-2">{stripHtml(article.title)}</h3>

        {/* Summary */}
        {article.summary && (
          <p className="text-xs text-muted-foreground leading-relaxed line-clamp-3">
            {stripHtml(article.summary)}
          </p>
        )}
      </div>
    </Card>
  )
}

// ============================================================================
// Main Component
// ============================================================================

export default function NewsPage() {
  const [articles, setArticles] = useState<NewsArticle[]>([])
  const [loadingState, setLoadingState] = useState<LoadingState>('idle')
  const [errorMessage, setErrorMessage] = useState('')
  const [symbolInput, setSymbolInput] = useState('')
  const [limit, setLimit] = useState('25')
  const [isRefreshing, setIsRefreshing] = useState(false)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const abortRef = useRef(0) // Simple counter to cancel stale requests

  const fetchNews = useCallback(
    async (symbols: string, newsLimit: string) => {
      const requestId = ++abortRef.current
      setLoadingState('loading')
      setErrorMessage('')

      try {
        const trimmedSymbols = symbols
          .split(',')
          .map((s) => s.trim().toUpperCase())
          .filter(Boolean)
          .join(',')

        const result = await providerCommands.getStockNews(
          trimmedSymbols || undefined,
          parseInt(newsLimit, 10)
        )

        // Bail if a newer request has been fired
        if (requestId !== abortRef.current) return

        const newsItems = (result as NewsArticle[]) || []
        setArticles(newsItems)
        setLoadingState(newsItems.length > 0 ? 'success' : 'success')
      } catch (err) {
        if (requestId !== abortRef.current) return
        const message = err instanceof Error ? err.message : String(err)
        setErrorMessage(message)
        setLoadingState('error')
        setArticles([])
      }
    },
    []
  )

  // Auto-load on mount
  useEffect(() => {
    fetchNews('', limit)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Debounced symbol input handler
  const handleSymbolChange = (value: string) => {
    setSymbolInput(value)

    if (debounceRef.current) {
      clearTimeout(debounceRef.current)
    }

    debounceRef.current = setTimeout(() => {
      fetchNews(value, limit)
    }, 500)
  }

  // Limit change fetches immediately
  const handleLimitChange = (newLimit: string) => {
    setLimit(newLimit)
    if (debounceRef.current) {
      clearTimeout(debounceRef.current)
    }
    fetchNews(symbolInput, newLimit)
  }

  // Refresh button
  const handleRefresh = async () => {
    setIsRefreshing(true)
    await fetchNews(symbolInput, limit)
    setIsRefreshing(false)
    toast.success('News refreshed')
  }

  const isLoading = loadingState === 'loading' && !isRefreshing

  return (
    <div className="flex flex-col gap-6 p-4 md:p-6 max-w-5xl mx-auto">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Newspaper className="h-6 w-6" />
            Market News
          </h1>
        </div>
        <p className="text-sm text-muted-foreground ml-6">
          Latest market news and analysis powered by Financial Modeling Prep
        </p>
      </div>

      {/* Filter Bar */}
      <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
          <Input
            placeholder="Filter by symbols (e.g. AAPL, MSFT, TSLA)"
            value={symbolInput}
            onChange={(e) => handleSymbolChange(e.target.value)}
            className="pl-9"
          />
        </div>

        <div className="flex items-center gap-3">
          <Select value={limit} onValueChange={handleLimitChange}>
            <SelectTrigger className="w-[100px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="10">10 items</SelectItem>
              <SelectItem value="25">25 items</SelectItem>
              <SelectItem value="50">50 items</SelectItem>
            </SelectContent>
          </Select>

          <Button
            variant="outline"
            size="icon"
            onClick={handleRefresh}
            disabled={loadingState === 'loading'}
            title="Refresh news"
          >
            <RefreshCw
              className={`h-4 w-4 ${isRefreshing || loadingState === 'loading' ? 'animate-spin' : ''}`}
            />
          </Button>
        </div>
      </div>

      {/* Content */}
      {isLoading ? (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <NewsCardSkeleton key={i} />
          ))}
        </div>
      ) : loadingState === 'error' ? (
        <ErrorState message={errorMessage} />
      ) : articles.length === 0 && loadingState === 'success' ? (
        <EmptyState symbols={symbolInput} />
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {articles.map((article, index) => (
            <NewsCard key={`${article.url}-${index}`} article={article} />
          ))}
        </div>
      )}
    </div>
  )
}
