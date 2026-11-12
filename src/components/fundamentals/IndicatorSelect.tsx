import { INDICATOR_GROUPS, type IndicatorSpec } from '@/api/indicators'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

interface IndicatorSelectProps {
  /** Catalog name of the current selection, e.g. `ADX`. */
  value: string
  onChange: (name: string) => void
  specs: IndicatorSpec[]
  disabled?: boolean
  id?: string
}

/**
 * The indicator picker: one group per catalog group, in catalog order, each
 * entry showing the TA-Lib name and its label.
 */
export function IndicatorSelect({ value, onChange, specs, disabled, id }: IndicatorSelectProps) {
  const groups = INDICATOR_GROUPS.map((group) => ({
    group,
    items: specs.filter((spec) => spec.group === group),
  })).filter(({ items }) => items.length > 0)

  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger id={id} size="sm" className="h-8 w-full" aria-label="Indicator">
        <SelectValue placeholder="Choose an indicator" />
      </SelectTrigger>
      <SelectContent position="popper" className="max-h-80">
        {groups.map(({ group, items }) => (
          <SelectGroup key={group}>
            <SelectLabel>{group}</SelectLabel>
            {items.map((spec) => (
              <SelectItem key={spec.name} value={spec.name} textValue={spec.name}>
                <span className="font-mono text-xs">{spec.name}</span>
                <span className="truncate text-muted-foreground">{spec.label}</span>
              </SelectItem>
            ))}
          </SelectGroup>
        ))}
      </SelectContent>
    </Select>
  )
}
