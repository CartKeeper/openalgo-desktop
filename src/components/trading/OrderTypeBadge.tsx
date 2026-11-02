import type { ReactNode } from 'react'
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { ORDER_GLOSSARY, type GlossaryOrderType } from '@/lib/orderGlossary'

/**
 * Wraps any element with a plain-English explainer of the given order type on
 * hover (and keyboard focus), for users who don't yet know what LIMIT / SL-M /
 * MARKET mean. Renders children unchanged when the glossary has no entry, so
 * callers keep their exact existing styling either way.
 */
export function OrderTypeTooltip({
  pricetype,
  children,
}: {
  pricetype: string
  children: ReactNode
}) {
  const entry = ORDER_GLOSSARY.orderTypes[pricetype as GlossaryOrderType]
  if (!entry) {
    return <>{children}</>
  }
  return (
    <Tooltip delayDuration={200}>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side="top" className="max-w-70 text-xs">
        <p className="font-semibold mb-1">{entry.label}</p>
        <p>{entry.desc}</p>
      </TooltipContent>
    </Tooltip>
  )
}

/**
 * Standard order-type badge with the glossary tooltip — the default way to
 * display an order's price type (Order Book table, modify dialog, etc.).
 */
export function OrderTypeBadge({
  pricetype,
  className,
}: {
  pricetype: string
  className?: string
}) {
  return (
    <OrderTypeTooltip pricetype={pricetype}>
      <Badge
        variant="secondary"
        className={`cursor-help ${className ?? ''}`.trim()}
      >
        {pricetype}
      </Badge>
    </OrderTypeTooltip>
  )
}
