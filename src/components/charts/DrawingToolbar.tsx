/**
 * Vertical drawing rail for the Historify chart (WP1).
 *
 * Pointer + the six primary tools decided in the spec, a "More tools" menu with
 * the rest of the library grouped by family, then Hide all / Clear all / Delete
 * selection. 32x32 ghost icon buttons; the armed tool is `aria-pressed`.
 */
import type { DrawingKind } from 'lightweight-charts-drawing'
import {
  Eraser,
  Eye,
  EyeOff,
  type LucideIcon,
  Minus,
  MoreHorizontal,
  MousePointer2,
  MoveUpRight,
  Rows3,
  Square,
  Trash2,
  TrendingUp,
  Type,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Separator } from '@/components/ui/separator'
import type { ToolKind } from '@/hooks/useChartDrawings'
import { cn } from '@/lib/utils'

export interface DrawingToolbarProps {
  activeTool: ToolKind | null
  onSelectTool: (t: ToolKind | null) => void
  allHidden: boolean
  onToggleHidden: () => void
  onClearAll: () => void
  onDeleteSelection: () => void
  hasSelection: boolean
}

export interface PrimaryTool {
  kind: DrawingKind
  label: string
  icon: LucideIcon
}

export interface ToolGroup {
  label: string
  tools: DrawingKind[]
}

const TOOL_LABEL_OVERRIDES: Partial<Record<DrawingKind, string>> = {
  'fib-retracement': 'Fibonacci retracement',
  'trend-based-fib-extension': 'Trend-based Fib extension',
  'trend-based-fib-time': 'Trend-based Fib time',
}

const PROPER_WORDS: Record<string, string> = {
  fib: 'Fib',
  gann: 'Gann',
  elliott: 'Elliott',
  schiff: 'Schiff',
  vwap: 'VWAP',
  xabcd: 'XABCD',
  abcd: 'ABCD',
}

/** Human label of a library tool id ("parallel-channel" -> "Parallel channel"). */
export function toolLabel(kind: DrawingKind): string {
  const override = TOOL_LABEL_OVERRIDES[kind]
  if (override) return override
  return kind
    .split('-')
    .map((word, i) => {
      const proper = PROPER_WORDS[word]
      if (proper) return proper
      return i === 0 ? word.charAt(0).toUpperCase() + word.slice(1) : word
    })
    .join(' ')
}

/** The six primary tools (spec, open item 5), in rail order after the pointer. */
export const PRIMARY_TOOLS: PrimaryTool[] = [
  { kind: 'trend-line', label: toolLabel('trend-line'), icon: TrendingUp },
  { kind: 'horizontal-line', label: toolLabel('horizontal-line'), icon: Minus },
  { kind: 'ray', label: toolLabel('ray'), icon: MoveUpRight },
  { kind: 'rectangle', label: toolLabel('rectangle'), icon: Square },
  { kind: 'fib-retracement', label: toolLabel('fib-retracement'), icon: Rows3 },
  { kind: 'text', label: toolLabel('text'), icon: Type },
]

/**
 * Tools that need host features this page does not provide (an image file
 * picker, a table cell editor, a glyph picker). Left out of the menu on purpose.
 */
export const HOST_ONLY_TOOLS: DrawingKind[] = ['image', 'table', 'font-icon']

/** The remaining library tools, grouped by family (the package ships no category metadata). */
export const MORE_TOOL_GROUPS: ToolGroup[] = [
  {
    label: 'Lines',
    tools: [
      'horizontal-ray',
      'vertical-line',
      'cross-line',
      'extended-line',
      'info-line',
      'trend-angle',
      'arrow',
      'parallel-channel',
      'disjoint-channel',
      'flat-top-bottom',
      'regression-trend',
    ],
  },
  {
    label: 'Pitchforks',
    tools: [
      'pitchfork',
      'schiff-pitchfork',
      'modified-schiff-pitchfork',
      'inside-pitchfork',
      'pitchfan',
    ],
  },
  {
    label: 'Fibonacci',
    tools: [
      'trend-based-fib-extension',
      'fib-channel',
      'fib-time-zone',
      'trend-based-fib-time',
      'fib-speed-resistance-fan',
      'fib-speed-resistance-arcs',
      'fib-circles',
      'fib-spiral',
      'fib-wedge',
    ],
  },
  {
    label: 'Gann',
    tools: ['gann-box', 'gann-square-fixed', 'gann-square', 'gann-fan'],
  },
  {
    label: 'Patterns',
    tools: [
      'xabcd-pattern',
      'cypher-pattern',
      'abcd-pattern',
      'triangle-pattern',
      'three-drives-pattern',
      'head-and-shoulders',
    ],
  },
  {
    label: 'Elliott waves',
    tools: [
      'elliott-impulse',
      'elliott-correction',
      'elliott-triangle',
      'elliott-double-combo',
      'elliott-triple-combo',
    ],
  },
  {
    label: 'Cycles',
    tools: ['cyclic-lines', 'time-cycles', 'sine-line'],
  },
  {
    label: 'Forecasting & measuring',
    tools: [
      'long-position',
      'short-position',
      'position-forecast',
      'bar-pattern',
      'ghost-feed',
      'price-range',
      'date-range',
      'date-and-price-range',
    ],
  },
  {
    label: 'Volume & data',
    tools: ['anchored-vwap', 'fixed-range-volume-profile', 'anchored-volume-profile'],
  },
  {
    label: 'Shapes',
    tools: [
      'circle',
      'ellipse',
      'triangle',
      'rotated-rectangle',
      'curve',
      'arc',
      'double-curve',
      'sector',
      'polyline',
      'path',
      'brush',
      'highlighter',
    ],
  },
  {
    label: 'Annotations',
    tools: [
      'note',
      'pin',
      'comment',
      'price-note',
      'signpost',
      'callout',
      'price-label',
      'arrow-marker',
      'arrow-mark-up',
      'arrow-mark-down',
      'flag-mark',
    ],
  },
]

const PRIMARY_KINDS: ReadonlySet<string> = new Set(PRIMARY_TOOLS.map((t) => t.kind))

interface RailButtonProps {
  label: string
  icon: LucideIcon
  pressed?: boolean
  disabled?: boolean
  onClick?: () => void
}

function RailButton({ label, icon: Icon, pressed = false, disabled = false, onClick }: RailButtonProps) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'h-8 w-8 rounded-[8px] transition-colors active:scale-[0.97]',
        pressed && 'bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary'
      )}
    >
      <Icon className="h-4 w-4" />
    </Button>
  )
}

export function DrawingToolbar({
  activeTool,
  onSelectTool,
  allHidden,
  onToggleHidden,
  onClearAll,
  onDeleteSelection,
  hasSelection,
}: DrawingToolbarProps) {
  const moreToolActive = activeTool !== null && !PRIMARY_KINDS.has(activeTool)
  const moreLabel = moreToolActive ? `More tools (${toolLabel(activeTool)})` : 'More tools'

  return (
    <div
      role="toolbar"
      aria-label="Drawing tools"
      aria-orientation="vertical"
      className="flex w-10 shrink-0 flex-col items-center gap-1 border-r border-border bg-card py-1"
    >
      <RailButton
        label="Pointer"
        icon={MousePointer2}
        pressed={activeTool === null}
        onClick={() => onSelectTool(null)}
      />
      {PRIMARY_TOOLS.map((tool) => (
        <RailButton
          key={tool.kind}
          label={tool.label}
          icon={tool.icon}
          pressed={activeTool === tool.kind}
          onClick={() => onSelectTool(tool.kind)}
        />
      ))}

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label="More tools"
            title={moreLabel}
            aria-pressed={moreToolActive}
            className={cn(
              'h-8 w-8 rounded-[8px] transition-colors active:scale-[0.97]',
              moreToolActive && 'bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary'
            )}
          >
            <MoreHorizontal className="h-4 w-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent
          side="right"
          align="start"
          className="max-h-[70vh] w-56 overflow-y-auto"
        >
          {MORE_TOOL_GROUPS.map((group, index) => (
            <DropdownMenuGroup key={group.label}>
              {index > 0 && <DropdownMenuSeparator />}
              <DropdownMenuLabel className="text-xs uppercase tracking-wider text-muted-foreground">
                {group.label}
              </DropdownMenuLabel>
              {group.tools.map((kind) => (
                <DropdownMenuItem
                  key={kind}
                  onSelect={() => onSelectTool(kind)}
                  className={cn('text-sm', activeTool === kind && 'bg-primary/10 text-primary')}
                >
                  {toolLabel(kind)}
                </DropdownMenuItem>
              ))}
            </DropdownMenuGroup>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <Separator className="my-1 w-6" />

      <RailButton
        label={allHidden ? 'Show all drawings' : 'Hide all drawings'}
        icon={allHidden ? EyeOff : Eye}
        pressed={allHidden}
        onClick={onToggleHidden}
      />
      <RailButton label="Clear all drawings" icon={Eraser} onClick={onClearAll} />
      <RailButton
        label="Delete selection"
        icon={Trash2}
        disabled={!hasSelection}
        onClick={onDeleteSelection}
      />
    </div>
  )
}
