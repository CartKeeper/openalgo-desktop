/**
 * "Indicators" button (32 px) opening a searchable picker over the indicator
 * catalog: the first 50 matches, grouped by category.
 */
import { ChartLine } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { type CatalogEntry, searchIndicators } from '@/lib/chartTools/indicatorCatalog'

export const INDICATOR_PICKER_LIMIT = 50

export interface IndicatorPickerProps {
  onAdd: (entry: CatalogEntry) => void
}

export function IndicatorPicker({ onAdd }: IndicatorPickerProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')

  const groups = useMemo(() => {
    const map = new Map<string, CatalogEntry[]>()
    for (const entry of searchIndicators(query, INDICATOR_PICKER_LIMIT)) {
      const list = map.get(entry.group) ?? []
      list.push(entry)
      map.set(entry.group, list)
    }
    return [...map.entries()]
  }, [query])

  const close = () => {
    setOpen(false)
    setQuery('')
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        if (next) setOpen(true)
        else close()
      }}
    >
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-label="Indicators"
          title="Add an indicator"
          className="h-8 rounded-[8px] px-3 text-xs transition-colors active:scale-[0.97]"
        >
          <ChartLine className="h-4 w-4" />
          Indicators
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-80 p-0" align="start">
        <Command shouldFilter={false}>
          <CommandInput
            placeholder="Search indicators..."
            value={query}
            onValueChange={setQuery}
          />
          <CommandList className="max-h-[320px]">
            <CommandEmpty>No indicators found</CommandEmpty>
            {groups.map(([group, entries]) => (
              <CommandGroup key={group} heading={group}>
                {entries.map((entry) => (
                  <CommandItem
                    key={entry.id}
                    value={entry.id}
                    onSelect={() => {
                      onAdd(entry)
                      close()
                    }}
                  >
                    <span className="truncate">{entry.name}</span>
                    <span className="ml-auto flex shrink-0 items-center gap-1">
                      {!entry.overlay && (
                        <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                          pane
                        </span>
                      )}
                      {entry.origin === 'community' && (
                        <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                          community
                        </span>
                      )}
                    </span>
                  </CommandItem>
                ))}
              </CommandGroup>
            ))}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}
