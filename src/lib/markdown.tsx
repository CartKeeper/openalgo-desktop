import type React from 'react'

// ---------- Simple markdown renderer ----------

export function renderMarkdown(text: string): React.ReactNode[] {
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

export function formatToolName(name: string): string {
  return name
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase())
}

// ---------- Ticker extraction ----------

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

export function extractTickers(text: string): string[] {
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
