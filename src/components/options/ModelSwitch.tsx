import { cn } from '@/lib/utils'

/** Pricing model accepted by the `compute_greeks*` commands. */
export type PricingModel = 'black_scholes' | 'binomial'

export const PRICING_MODEL_LABELS: Record<PricingModel, string> = {
  black_scholes: 'Black-Scholes',
  binomial: 'Binomial (American)',
}

const MODELS: PricingModel[] = ['black_scholes', 'binomial']

interface ModelSwitchProps {
  value: PricingModel
  onChange: (model: PricingModel) => void
  /** Hold the switch while a calculation is in flight so results cannot race. */
  disabled?: boolean
}

/**
 * Two-way pricing model switch: Black-Scholes (European, closed form) or
 * Binomial (American, 200-step lattice with early exercise).
 */
export function ModelSwitch({ value, onChange, disabled = false }: ModelSwitchProps) {
  return (
    // The fieldset carries the group semantics; the inner div carries the flex
    // layout (fieldsets have long been unreliable flex containers in WebKit).
    <fieldset aria-label="Pricing model" className="min-w-0">
      <div className="inline-flex h-8 rounded-md border border-input overflow-hidden">
        {MODELS.map((model, index) => {
          const selected = value === model
          return (
            <button
              key={model}
              type="button"
              aria-pressed={selected}
              disabled={disabled}
              onClick={() => onChange(model)}
              className={cn(
                'h-8 min-w-16 px-3 text-xs font-medium whitespace-nowrap',
                'transition-colors duration-150 active:scale-[0.97]',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
                'disabled:pointer-events-none disabled:opacity-50',
                index > 0 && 'border-l border-input',
                selected
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground'
              )}
            >
              {PRICING_MODEL_LABELS[model]}
            </button>
          )
        })}
      </div>
    </fieldset>
  )
}
