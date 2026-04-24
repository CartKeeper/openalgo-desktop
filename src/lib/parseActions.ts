import type { OrderRecommendation } from '@/types/actionQueue'

const ACTIONS_REGEX = /<!--\s*ACTIONS_JSON\s*\n([\s\S]*?)\n\s*-->/g

/**
 * Extract structured order recommendations from AI markdown output.
 * The AI emits `<!-- ACTIONS_JSON [...] -->` blocks that are invisible
 * when rendered as markdown but parseable here.
 */
export function parseActionsFromMarkdown(
  text: string,
  source: OrderRecommendation['source']
): OrderRecommendation[] {
  const results: OrderRecommendation[] = []
  let match: RegExpExecArray | null

  // Reset regex state for each call
  ACTIONS_REGEX.lastIndex = 0

  while ((match = ACTIONS_REGEX.exec(text)) !== null) {
    try {
      const parsed = JSON.parse(match[1])
      if (!Array.isArray(parsed)) continue

      for (const item of parsed) {
        // Skip items missing required fields
        if (!item.symbol || !item.side || !item.quantity) continue

        results.push({
          id: crypto.randomUUID(),
          symbol: String(item.symbol).toUpperCase(),
          exchange: item.exchange || 'US',
          side: item.side === 'SELL' ? 'SELL' : 'BUY',
          quantity: Math.max(1, Math.floor(Number(item.quantity) || 1)),
          orderType: item.orderType || 'MARKET',
          product: item.product || 'CNC',
          price: Number(item.price) || 0,
          triggerPrice: item.triggerPrice ? Number(item.triggerPrice) : undefined,
          trailPrice: item.trailPrice ? Number(item.trailPrice) : undefined,
          trailPercent: item.trailPercent ? Number(item.trailPercent) : undefined,
          rationale: item.rationale || '',
          source,
        })
      }
    } catch {
      // Malformed JSON block — skip silently
    }
  }

  return results
}
