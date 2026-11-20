/**
 * Chips row of the active indicators: name, a gear opening the numeric inputs
 * (applied on Enter or blur), and a remove button. 24 px tall, 6 px radius.
 */
import { Settings2, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import type { ActiveIndicator, IndicatorInputValue } from '@/lib/chartTools/activeIndicators'
import { type CatalogInput, getCatalogEntry } from '@/lib/chartTools/indicatorCatalog'

export interface IndicatorChipsProps {
  active: ActiveIndicator[]
  onChangeInputs: (key: string, inputs: ActiveIndicator['inputs']) => void
  onRemove: (key: string) => void
}

const CHIP_BUTTON =
  'flex h-5 w-5 shrink-0 items-center justify-center rounded-[4px] text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.97]'

interface NumericFieldProps {
  fieldId: string
  input: CatalogInput
  value: IndicatorInputValue | undefined
  onCommit: (value: number) => void
}

function NumericField({ fieldId, input, value, onCommit }: NumericFieldProps) {
  const current =
    typeof value === 'number' ? value : typeof input.default === 'number' ? input.default : 0
  const [text, setText] = useState(String(current))
  useEffect(() => {
    setText(String(current))
  }, [current])

  const commit = () => {
    const parsed = input.type === 'int' ? Number.parseInt(text, 10) : Number.parseFloat(text)
    if (!Number.isFinite(parsed)) {
      setText(String(current))
      return
    }
    let next = parsed
    if (typeof input.min === 'number') next = Math.max(input.min, next)
    if (typeof input.max === 'number') next = Math.min(input.max, next)
    setText(String(next))
    if (next !== current) onCommit(next)
  }

  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={fieldId} className="text-xs">
        {input.label}
      </Label>
      <Input
        id={fieldId}
        type="number"
        inputMode="decimal"
        className="h-8 text-sm"
        value={text}
        min={input.min}
        max={input.max}
        step={input.step ?? (input.type === 'int' ? 1 : 'any')}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            commit()
          }
        }}
      />
    </div>
  )
}

interface ChipProps {
  item: ActiveIndicator
  onChangeInputs: IndicatorChipsProps['onChangeInputs']
  onRemove: IndicatorChipsProps['onRemove']
}

function IndicatorChip({ item, onChangeInputs, onRemove }: ChipProps) {
  const entry = getCatalogEntry(item.id)
  const name = entry?.name ?? item.id
  const numeric = (entry?.inputs ?? []).filter((i) => i.type === 'int' || i.type === 'float')

  return (
    <li
      data-indicator-key={item.key}
      className="inline-flex h-6 shrink-0 items-center gap-1 rounded-[6px] border border-border bg-card pl-2 pr-1 text-xs font-semibold text-foreground"
    >
      <span className="max-w-[180px] truncate" title={name}>
        {name}
      </span>
      {numeric.length > 0 && (
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              className={CHIP_BUTTON}
              aria-label={`${name} settings`}
              title="Settings"
            >
              <Settings2 className="h-3 w-3" />
            </button>
          </PopoverTrigger>
          <PopoverContent className="w-56 p-3" align="start">
            <div className="flex flex-col gap-3">
              {numeric.map((input) => (
                <NumericField
                  key={input.id}
                  fieldId={`indicator-${item.key}-${input.id}`}
                  input={input}
                  value={item.inputs[input.id]}
                  onCommit={(value) =>
                    onChangeInputs(item.key, { ...item.inputs, [input.id]: value })
                  }
                />
              ))}
            </div>
          </PopoverContent>
        </Popover>
      )}
      <button
        type="button"
        className={CHIP_BUTTON}
        aria-label={`Remove ${name}`}
        title="Remove"
        onClick={() => onRemove(item.key)}
      >
        <X className="h-3 w-3" />
      </button>
    </li>
  )
}

export function IndicatorChips({ active, onChangeInputs, onRemove }: IndicatorChipsProps) {
  if (active.length === 0) return null
  return (
    <ul className="m-0 flex list-none flex-wrap items-center gap-2 p-0" aria-label="Active indicators">
      {active.map((item) => (
        <IndicatorChip
          key={item.key}
          item={item}
          onChangeInputs={onChangeInputs}
          onRemove={onRemove}
        />
      ))}
    </ul>
  )
}
