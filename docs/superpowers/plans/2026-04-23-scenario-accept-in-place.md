# Scenario Accept In Place — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Accepting Claude's recommendations inside a sandbox scenario applies the trades in place to the current scenario instead of cloning every time. Baselines are the single exception — they clone once, with a user-typed strategy name.

**Architecture:** Reuse the existing `ActionReviewModal` by adding two optional props (`onApply`, `cloneNameRequired`). Extract the scenario-apply logic into a pure, unit-testable function. Rewire `AnalyzeWithClaudeDialog` to push parsed actions into the shared `actionQueueStore` and open the modal — which then calls the custom `onApply` handler that clones only on a baseline and applies trades via the existing `apply_scenario_trade` Tauri command.

**Tech Stack:** React 18 + TypeScript, Tauri 2.0 frontend, zustand store (`useActionQueueStore`, `useClientScenarioStore`), Vitest for unit tests, Biome for lint/format. No backend changes.

**Spec:** [`docs/superpowers/specs/2026-04-23-scenario-accept-in-place-design.md`](../specs/2026-04-23-scenario-accept-in-place-design.md)

---

## File structure

| File | Role | Change |
|------|------|--------|
| `src/lib/applyScenarioRecommendations.ts` | Pure function wrapping clone-if-baseline + per-trade apply logic. Takes injected deps so it is unit-testable. | Create |
| `src/lib/applyScenarioRecommendations.test.ts` | Unit tests for the pure function. | Create |
| `src/components/trading/ActionReviewModal.tsx` | Add `onApply`, `cloneNameRequired` props. Render clone-name input when required. Branch `handlePlaceAll` to call `onApply` when provided. | Modify |
| `src/pages/clients/AnalyzeWithClaudeDialog.tsx` | Rename button, replace `handleApplyToClone` with store-push + `onApply` callback using the pure function. Accept new `isBaseline` prop. Mount a local `ActionReviewModal` instance with scenario-mode props. | Modify |
| `src/pages/clients/ScenarioDetail.tsx` | Pass `isBaseline={activeScenario?.is_baseline ?? false}` into `AnalyzeWithClaudeDialog`. | Modify |

Briefing, copilot, and reports flows all use the default `<ActionReviewModal />` (no new props) → unchanged.

---

## Task 1 — Pure scenario-apply function with unit tests

**Files:**
- Create: `src/lib/applyScenarioRecommendations.ts`
- Create: `src/lib/applyScenarioRecommendations.test.ts`

This task extracts the logic described in the spec's "Scenario-apply handler" pseudocode into a testable function. The function takes injected dependencies so the unit tests can assert branching behavior without mounting React or running Tauri.

- [ ] **Step 1: Write the failing test file**

Create `src/lib/applyScenarioRecommendations.test.ts`:

```typescript
import { describe, expect, it, vi } from 'vitest'
import { applyScenarioRecommendations } from './applyScenarioRecommendations'
import type { OrderRecommendation } from '@/types/actionQueue'

function item(overrides: Partial<OrderRecommendation> = {}): OrderRecommendation {
  return {
    id: 'a',
    symbol: 'AAPL',
    exchange: 'NASDAQ',
    side: 'BUY',
    quantity: 10,
    orderType: 'MARKET',
    product: 'CNC',
    price: 180,
    rationale: 'test',
    source: 'copilot',
    ...overrides,
  }
}

describe('applyScenarioRecommendations', () => {
  it('on non-baseline: applies each trade to current scenario, does not clone, returns targetId = current', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    const cloneScenario = vi.fn()
    const result = await applyScenarioRecommendations({
      items: [item({ symbol: 'AAPL' }), item({ id: 'b', symbol: 'MSFT' })],
      clientId: 1,
      currentScenarioId: 42,
      isBaseline: false,
      cloneName: undefined,
      invoke,
      cloneScenario,
    })

    expect(cloneScenario).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke).toHaveBeenNthCalledWith(1, 'apply_scenario_trade', {
      scenarioId: 42,
      symbol: 'AAPL',
      exchange: 'NASDAQ',
      side: 'long',
      quantity: 10,
      price: 180,
    })
    expect(result).toEqual({ targetScenarioId: 42, cloned: false, applied: 2, failures: [] })
  })

  it('on baseline: clones once with the provided name, then applies trades to the new clone', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    const cloneScenario = vi.fn().mockResolvedValue({ id: 99 })

    const result = await applyScenarioRecommendations({
      items: [item()],
      clientId: 1,
      currentScenarioId: 7,
      isBaseline: true,
      cloneName: 'Aggressive Growth',
      invoke,
      cloneScenario,
    })

    expect(cloneScenario).toHaveBeenCalledTimes(1)
    expect(cloneScenario).toHaveBeenCalledWith(7, 1, 'Aggressive Growth', null)
    expect(invoke).toHaveBeenCalledWith('apply_scenario_trade', expect.objectContaining({ scenarioId: 99 }))
    expect(result).toEqual({ targetScenarioId: 99, cloned: true, applied: 1, failures: [] })
  })

  it('on baseline with empty cloneName: throws before any invoke/clone call', async () => {
    const invoke = vi.fn()
    const cloneScenario = vi.fn()
    await expect(
      applyScenarioRecommendations({
        items: [item()],
        clientId: 1,
        currentScenarioId: 7,
        isBaseline: true,
        cloneName: '  ',
        invoke,
        cloneScenario,
      })
    ).rejects.toThrow(/clone name/i)
    expect(invoke).not.toHaveBeenCalled()
    expect(cloneScenario).not.toHaveBeenCalled()
  })

  it('maps BUY -> long and SELL -> short for the backend side field', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    const cloneScenario = vi.fn()
    await applyScenarioRecommendations({
      items: [item({ side: 'SELL' })],
      clientId: 1,
      currentScenarioId: 42,
      isBaseline: false,
      cloneName: undefined,
      invoke,
      cloneScenario,
    })
    expect(invoke).toHaveBeenCalledWith('apply_scenario_trade', expect.objectContaining({ side: 'short' }))
  })

  it('collects per-trade failures without aborting the loop and reports them', async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(undefined) // first succeeds
      .mockRejectedValueOnce(new Error('insufficient qty')) // second fails
      .mockResolvedValueOnce(undefined) // third succeeds
    const cloneScenario = vi.fn()

    const result = await applyScenarioRecommendations({
      items: [
        item({ id: 'a', symbol: 'AAPL' }),
        item({ id: 'b', symbol: 'MSFT' }),
        item({ id: 'c', symbol: 'GOOG' }),
      ],
      clientId: 1,
      currentScenarioId: 42,
      isBaseline: false,
      cloneName: undefined,
      invoke,
      cloneScenario,
    })

    expect(result.applied).toBe(2)
    expect(result.failures).toEqual([{ symbol: 'MSFT', message: 'insufficient qty' }])
  })

  it('on baseline clone failure: does not call invoke for any trade, rethrows', async () => {
    const invoke = vi.fn()
    const cloneScenario = vi.fn().mockRejectedValue(new Error('db locked'))
    await expect(
      applyScenarioRecommendations({
        items: [item()],
        clientId: 1,
        currentScenarioId: 7,
        isBaseline: true,
        cloneName: 'X',
        invoke,
        cloneScenario,
      })
    ).rejects.toThrow(/db locked/)
    expect(invoke).not.toHaveBeenCalled()
  })
})
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `npx vitest run src/lib/applyScenarioRecommendations.test.ts`

Expected: All 6 tests FAIL — module `./applyScenarioRecommendations` cannot be resolved.

- [ ] **Step 3: Implement the pure function**

Create `src/lib/applyScenarioRecommendations.ts`:

```typescript
import type { InvokeArgs } from '@tauri-apps/api/core'
import type { OrderRecommendation } from '@/types/actionQueue'
import type { ClientScenario } from '@/types/client-scenarios'

export interface ApplyScenarioRecommendationsDeps {
  /** Tauri invoke — injected so the logic is testable without Tauri. */
  invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>
  /** Store method that wraps the `clone_client_scenario` Tauri command. */
  cloneScenario: (
    sourceId: number,
    clientId: number,
    name: string,
    description?: string
  ) => Promise<ClientScenario>
}

export interface ApplyScenarioRecommendationsParams extends ApplyScenarioRecommendationsDeps {
  items: OrderRecommendation[]
  clientId: number
  currentScenarioId: number
  isBaseline: boolean
  /** Required when isBaseline === true; names the new clone. */
  cloneName: string | undefined
}

export interface ApplyScenarioRecommendationsResult {
  /** ID the trades were applied against — either the current scenario or the new clone. */
  targetScenarioId: number
  /** True when a new clone was created as part of this apply. */
  cloned: boolean
  /** Count of successfully applied trades. */
  applied: number
  /** Per-trade failures (loop does not abort on individual failures). */
  failures: Array<{ symbol: string; message: string }>
}

export async function applyScenarioRecommendations(
  params: ApplyScenarioRecommendationsParams
): Promise<ApplyScenarioRecommendationsResult> {
  const { items, clientId, currentScenarioId, isBaseline, cloneName, invoke, cloneScenario } =
    params

  let targetScenarioId = currentScenarioId
  let cloned = false

  if (isBaseline) {
    const trimmed = (cloneName ?? '').trim()
    if (trimmed.length === 0) {
      throw new Error('A clone name is required when applying to a baseline scenario.')
    }
    const clone = await cloneScenario(currentScenarioId, clientId, trimmed, undefined)
    if (clone.id == null) {
      throw new Error('Clone succeeded but returned no id.')
    }
    targetScenarioId = clone.id
    cloned = true
  }

  const failures: Array<{ symbol: string; message: string }> = []
  let applied = 0

  for (const item of items) {
    try {
      await invoke('apply_scenario_trade', {
        scenarioId: targetScenarioId,
        symbol: item.symbol,
        exchange: item.exchange,
        side: item.side === 'BUY' ? 'long' : 'short',
        quantity: item.quantity,
        price: item.price,
      })
      applied++
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      failures.push({ symbol: item.symbol, message })
    }
  }

  return { targetScenarioId, cloned, applied, failures }
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `npx vitest run src/lib/applyScenarioRecommendations.test.ts`

Expected: 6 tests PASS.

- [ ] **Step 5: Lint**

Run: `npm run lint`

Expected: No errors in the two new files (other repo-wide warnings are fine — do not touch them).

- [ ] **Step 6: Commit**

```bash
git add src/lib/applyScenarioRecommendations.ts src/lib/applyScenarioRecommendations.test.ts
git commit -m "Add applyScenarioRecommendations pure function + tests"
```

---

## Task 2 — Add optional scenario-mode props to `ActionReviewModal`

**Files:**
- Modify: `src/components/trading/ActionReviewModal.tsx`

This task introduces two optional props that let callers override the modal's default "live basket order" Apply behavior. When the props are absent, the briefing/copilot/reports flows continue to work unchanged.

- [ ] **Step 1: Add the prop types and destructure them**

In `src/components/trading/ActionReviewModal.tsx`, change the component signature from:

```typescript
export function ActionReviewModal() {
  const { items, isReviewOpen, isSubmitting, lastResults, updateItem, removeItem, close } =
    useActionQueueStore()
```

to:

```typescript
export interface ActionReviewModalProps {
  /**
   * Optional override for the Apply action. When provided, the modal calls this
   * instead of submitting a live basket order. Receives the current items array
   * and (if cloneNameRequired is true) the user-typed clone name.
   */
  onApply?: (items: OrderRecommendation[], cloneName?: string) => Promise<void>
  /** When true, renders a required "Name this strategy clone" input above the list. */
  cloneNameRequired?: boolean
  /** Overrides the Apply button label. Defaults to "Place N Order(s)". */
  applyButtonLabel?: string
}

export function ActionReviewModal({
  onApply,
  cloneNameRequired = false,
  applyButtonLabel,
}: ActionReviewModalProps = {}) {
  const { items, isReviewOpen, isSubmitting, lastResults, updateItem, removeItem, close } =
    useActionQueueStore()
  const [cloneName, setCloneName] = useState('')
```

Note: `useState` is already imported from `react`. `OrderRecommendation` is already imported from `@/types/actionQueue`.

- [ ] **Step 2: Reset clone name on close**

Find `handleClose` (line 220):

```typescript
const handleClose = () => {
  setShowResults(false)
  close()
}
```

Replace with:

```typescript
const handleClose = () => {
  setShowResults(false)
  setCloneName('')
  close()
}
```

- [ ] **Step 3: Branch `handlePlaceAll` to use `onApply` when provided**

Replace the entire `handlePlaceAll` function (currently lines 173–218) with:

```typescript
const handlePlaceAll = async () => {
  if (items.length === 0) return

  // Scenario-mode path: caller-provided onApply handler.
  if (onApply) {
    setSubmitting(true)
    try {
      await onApply(items, cloneNameRequired ? cloneName.trim() : undefined)
      // onApply owns post-apply UX (toasts + navigation). Close the modal.
      setCloneName('')
      close()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to apply recommendations')
      setSubmitting(false)
    }
    return
  }

  // Default path: live basket order.
  setSubmitting(true)
  try {
    const response = await tradingApi.placeBasketOrder(
      items.map((item) => ({
        symbol: item.symbol,
        exchange: item.exchange,
        action: item.side,
        product: item.product,
        pricetype: item.orderType,
        price: item.orderType === 'MARKET' || item.orderType === 'TRAILING_STOP' ? 0 : item.price,
        quantity: item.quantity,
        trigger_price: item.triggerPrice,
        validity: 'DAY',
        amo: false,
        trail_price: item.orderType === 'TRAILING_STOP' ? item.trailPrice : undefined,
        trail_percent: item.orderType === 'TRAILING_STOP' ? item.trailPercent : undefined,
      }))
    )

    if (response.status === 'success' && response.data) {
      const successCount = response.data.filter((r) => r.success).length
      const failCount = response.data.length - successCount

      setResults(response.data)
      setShowResults(true)

      emit('basket_order_event', { count: successCount })

      if (failCount === 0) {
        toast.success(`All ${successCount} orders placed successfully`)
      } else {
        toast.warning(`${successCount} placed, ${failCount} failed`)
      }
    } else {
      toast.error(response.message || 'Failed to place basket order')
      setSubmitting(false)
    }
  } catch (err) {
    toast.error(err instanceof Error ? err.message : 'Failed to place orders')
    setSubmitting(false)
  }
}
```

Only the addition of the `if (onApply)` branch + `setCloneName('')` on success is new; the default path is unchanged.

- [ ] **Step 4: Render the clone-name input when required**

Find the scrollable body block (currently around line 244):

```typescript
<div className="flex-1 overflow-y-auto space-y-3 py-2">
  {showResults && lastResults ? (
```

Insert a clone-name input section **immediately above** that div, inside the same `DialogContent` wrapper, after `</DialogHeader>` (line 242) and before the scrollable body div. The new block:

```tsx
{!showResults && cloneNameRequired && items.length > 0 && (
  <div className="px-1 pb-2 space-y-1.5 shrink-0">
    <Label htmlFor="scenario-clone-name" className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
      Name this strategy clone
    </Label>
    <Input
      id="scenario-clone-name"
      value={cloneName}
      onChange={(e) => setCloneName(e.target.value)}
      placeholder="e.g. Aggressive Growth, Dividend Tilt"
      className="h-10"
      autoFocus
      disabled={isSubmitting}
    />
    <p className="text-[11px] text-muted-foreground">
      Baseline scenarios are never modified. A new clone will be created with this name.
    </p>
  </div>
)}
```

- [ ] **Step 5: Update the Apply button label + disabled condition**

Find the Apply button (currently around lines 295–304):

```tsx
<Button
  onClick={handlePlaceAll}
  disabled={isSubmitting || items.length === 0}
  className="bg-primary"
>
  {isSubmitting ? (
    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
  ) : null}
  Place {items.length} Order{items.length !== 1 ? 's' : ''}
</Button>
```

Replace with:

```tsx
<Button
  onClick={handlePlaceAll}
  disabled={
    isSubmitting ||
    items.length === 0 ||
    (cloneNameRequired && cloneName.trim().length === 0)
  }
  className="bg-primary"
>
  {isSubmitting ? (
    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
  ) : null}
  {applyButtonLabel ??
    (onApply
      ? cloneNameRequired
        ? 'Create Clone & Apply'
        : 'Apply to Scenario'
      : `Place ${items.length} Order${items.length !== 1 ? 's' : ''}`)}
</Button>
```

- [ ] **Step 6: Typecheck the whole app**

Run: `npx tsc -b --pretty false`

Expected: No new errors in `ActionReviewModal.tsx`. (If the repo already has unrelated type errors, they remain — do not touch them.)

- [ ] **Step 7: Lint the touched file**

Run: `npm run lint`

Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add src/components/trading/ActionReviewModal.tsx
git commit -m "ActionReviewModal: accept optional onApply + cloneNameRequired props"
```

---

## Task 3 — Rewire `AnalyzeWithClaudeDialog`

**Files:**
- Modify: `src/pages/clients/AnalyzeWithClaudeDialog.tsx`

This task replaces the unconditional clone with the new flow: parse actions → push to store → open the review modal → on Apply, call the pure function. It also renames the chat-message button from "Apply to Cloned Scenario" to "Review Recommendations".

- [ ] **Step 1: Add `isBaseline` prop and required new imports**

At the top of `src/pages/clients/AnalyzeWithClaudeDialog.tsx`, update the imports section:

In the existing `lucide-react` import (lines 2–8), remove `Copy` and add `ClipboardCheck`:

```typescript
import {
  Bot,
  ClipboardCheck,
  Loader2,
  Send,
  Sparkles,
} from 'lucide-react'
```

Add the following new imports below the lucide-react import:
```typescript
import { ActionReviewModal } from '@/components/trading/ActionReviewModal'
import { applyScenarioRecommendations } from '@/lib/applyScenarioRecommendations'
import { useActionQueueStore } from '@/stores/actionQueueStore'
import type { OrderRecommendation } from '@/types/actionQueue'
```

Update the `AnalyzeWithClaudeDialogProps` interface (currently lines 50–57) to include `isBaseline`:

```typescript
interface AnalyzeWithClaudeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  clientId: number
  scenarioId: number
  scenarioName: string
  isBaseline: boolean
  onTradesApplied?: () => void
}
```

Update the component signature (lines 59–66) to destructure it:

```typescript
export default function AnalyzeWithClaudeDialog({
  open,
  onOpenChange,
  clientId,
  scenarioId,
  scenarioName,
  isBaseline,
  onTradesApplied,
}: AnalyzeWithClaudeDialogProps) {
```

- [ ] **Step 2: Wire the action queue store**

Below the existing `const { cloneScenario, applyTrade } = useClientScenarioStore()` line (67–68), add:

```typescript
const setItemsAndOpen = useActionQueueStore((s) => s.setItemsAndOpen)
```

The `applyTrade` store method is no longer used in this file after this task — remove it from the destructure:

```typescript
const { cloneScenario } = useClientScenarioStore()
```

- [ ] **Step 3: Replace `handleApplyToClone` with `handleReviewRecommendations`**

Delete the entire existing `handleApplyToClone` function (currently lines 166–232) and also delete the now-unused `isCloning` state and its setter (line 74: `const [isCloning, setIsCloning] = useState(false)`).

In its place, add:

```typescript
const handleReviewRecommendations = (messageContent: string) => {
  const actions = parseActionsFromMarkdown(messageContent, 'copilot')
  if (actions.length === 0) {
    toast.error('No recommended trades found in this message')
    return
  }
  // parseActionsFromMarkdown returns fully-formed OrderRecommendation objects
  // (see src/lib/parseActions.ts) — pass through directly.
  setItemsAndOpen(actions)
}

const handleApplyToScenario = async (
  items: OrderRecommendation[],
  cloneName: string | undefined
) => {
  const result = await applyScenarioRecommendations({
    items,
    clientId,
    currentScenarioId: scenarioId,
    isBaseline,
    cloneName,
    invoke,
    cloneScenario,
  })

  if (result.failures.length > 0) {
    const failedList = result.failures.map((f) => f.symbol).join(', ')
    toast.warning(
      `${result.applied} applied, ${result.failures.length} failed (${failedList})`
    )
  } else if (result.cloned) {
    toast.success(`Created "${cloneName}" with ${result.applied} trades applied`)
  } else {
    toast.success(`Applied ${result.applied} trades to "${scenarioName}"`)
  }

  if (result.cloned) {
    onOpenChange(false)
    navigate(`/clients/${clientId}/scenarios/${result.targetScenarioId}`)
  } else if (onTradesApplied) {
    onTradesApplied()
  }
}
```

**Verification before moving on:** Confirm that `parseActionsFromMarkdown` returns items with the shape `{ symbol, exchange, side: 'BUY' | 'SELL', quantity, price, rationale }`. Open `src/lib/parseActions.ts` and check the returned type. If fields differ, adjust the mapping in `handleReviewRecommendations` accordingly — do not silently rename fields.

- [ ] **Step 4: Replace the "Apply to Cloned Scenario" button**

Find the button block inside the message list (currently lines 316–331):

```tsx
{hasActions && (
  <Button
    variant="outline"
    size="sm"
    className="mt-2 text-xs h-8"
    disabled={isCloning}
    onClick={() => handleApplyToClone(msg.content)}
  >
    {isCloning ? (
      <Loader2 className="h-3.5 w-3.5 mr-2 animate-spin" />
    ) : (
      <Copy className="h-3.5 w-3.5 mr-2" />
    )}
    {isCloning ? 'Cloning...' : 'Apply to Cloned Scenario'}
  </Button>
)}
```

Replace with:

```tsx
{hasActions && (
  <Button
    variant="outline"
    size="sm"
    className="mt-2 text-xs h-8"
    onClick={() => handleReviewRecommendations(msg.content)}
  >
    <ClipboardCheck className="h-3.5 w-3.5 mr-2" />
    Review Recommendations
  </Button>
)}
```

- [ ] **Step 5: Mount the scenario-mode `ActionReviewModal`**

Inside `AnalyzeWithClaudeDialog`'s returned JSX, find the closing `</Dialog>` (the outermost one, currently line 374). **Before** that `</Dialog>`, add the modal:

```tsx
<ActionReviewModal
  onApply={handleApplyToScenario}
  cloneNameRequired={isBaseline}
/>
```

Final outer render structure looks like:

```tsx
return (
  <>
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/* existing DialogContent ... */}
    </Dialog>
    <ActionReviewModal
      onApply={handleApplyToScenario}
      cloneNameRequired={isBaseline}
    />
  </>
)
```

Wrap the existing return in a React fragment (`<>...</>`) since we now render two sibling elements.

- [ ] **Step 6: Remove unused imports**

After the changes above, these symbols are no longer used in this file:
- `Copy` from `lucide-react` (replaced by `ClipboardCheck`)
- `Loader2` is still used for the "Analyzing..." indicator — keep it

Double-check by running the typecheck:

Run: `npx tsc -b --pretty false`

Expected: No unused-import errors. No new errors in `AnalyzeWithClaudeDialog.tsx`.

- [ ] **Step 7: Lint**

Run: `npm run lint`

Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add src/pages/clients/AnalyzeWithClaudeDialog.tsx
git commit -m "AnalyzeWithClaudeDialog: review flow instead of unconditional clone"
```

---

## Task 4 — Pass `isBaseline` from `ScenarioDetail`

**Files:**
- Modify: `src/pages/clients/ScenarioDetail.tsx`

One-line wiring change so the new prop gets populated.

- [ ] **Step 1: Update the `AnalyzeWithClaudeDialog` usage**

Open `src/pages/clients/ScenarioDetail.tsx`, find the `<AnalyzeWithClaudeDialog ... />` usage (around line 834).

Add the `isBaseline` prop:

```tsx
<AnalyzeWithClaudeDialog
  /* ... existing props ... */
  scenarioName={activeScenario?.name || 'Scenario'}
  isBaseline={activeScenario?.is_baseline ?? false}
  /* ... remaining existing props ... */
/>
```

(Only add the one new line. Do not change anything else in this file.)

- [ ] **Step 2: Typecheck**

Run: `npx tsc -b --pretty false`

Expected: No new errors.

- [ ] **Step 3: Commit**

```bash
git add src/pages/clients/ScenarioDetail.tsx
git commit -m "ScenarioDetail: pass isBaseline to AnalyzeWithClaudeDialog"
```

---

## Task 5 — Manual verification

No code changes. This is the ship-gate: reproduce each scenario in a running build.

- [ ] **Step 1: Build and start the dev app**

Run: `npm run tauri:dev`

Expected: App launches with no compile errors.

- [ ] **Step 2: Non-baseline in-place apply**

1. Open a client that has a non-baseline clone scenario (e.g. "Ai - Current Portfolio — Individual Brokerage" from the screenshot).
2. Click **Analyze with Claude** → send a prompt that yields actions (e.g. "Recommend trades to increase diversification").
3. When Claude replies, click **Review Recommendations**.
4. Verify: `ActionReviewModal` opens with the parsed actions. **No** "Name this strategy clone" input is shown. Button label reads "Apply to Scenario".
5. Click **Apply to Scenario**.
6. Verify: modal closes; toast reads "Applied N trades to ..."; the scenario detail page holdings reflect the new positions (via `onTradesApplied` refresh); **no new scenario row** appears in the scenarios list.

- [ ] **Step 3: Baseline clone-once apply**

1. Open a client, navigate to a **baseline** scenario (tagged "Baseline", e.g. "Current Portfolio — 401(k)").
2. Click **Analyze with Claude** → prompt for trades.
3. Click **Review Recommendations**.
4. Verify: "Name this strategy clone" input is shown. Apply button is **disabled** and labeled "Create Clone & Apply".
5. Type a name (e.g. "Aggressive Growth"). Apply button enables.
6. Click **Create Clone & Apply**.
7. Verify: a new non-baseline scenario with that exact name appears in the scenarios list; the app navigates to that new scenario; toast reads 'Created "Aggressive Growth" with N trades applied'; the baseline itself is untouched.

- [ ] **Step 4: Baseline cancel path**

1. Repeat steps 1–3 of the baseline flow.
2. Leave the clone-name input empty. Click **Cancel**.
3. Verify: no clone created, no positions changed.

- [ ] **Step 5: Partial apply failure**

Hard to reproduce without a failing backend call. At minimum, confirm via the unit tests (Task 1) that the failure path surfaces each failed symbol. If a natural failure presents during manual testing (e.g. an invalid symbol), confirm the toast lists the failing symbols and the successful ones are still applied.

- [ ] **Step 6: Briefing flow regression check**

1. Navigate to the Briefing page.
2. Generate a briefing that produces actions.
3. Click **Review Actions**.
4. Verify: `ActionReviewModal` opens. **No** "Name this strategy clone" input. Button label reads "Place N Orders" (unchanged). Clicking it still submits to `placeBasketOrder` — i.e. the briefing flow is byte-for-byte the same as before.

- [ ] **Step 7: Final verification commit**

If everything above passes, no code changes. Skip the commit step. If you fixed something during manual verification, commit it as a separate fix and note what you found.

---

## Rollback

If the shipped change needs to be reverted:

```bash
# Revert Tasks 2–4 (keep Task 1 — the pure function and tests are harmless)
git revert <task-4-sha> <task-3-sha> <task-2-sha>
```

Task 1 is safe to leave in place; it is unused code that adds no runtime cost and keeps the tests.

---

## Out of scope (do not touch in this plan)

- Backend Rust code (`src-tauri/**`). The existing `apply_scenario_trade` and `clone_client_scenario` commands are used as-is.
- `useClientScenarioStore` internals.
- The scenario detail page layout.
- The "+ New Scenario" dialog and manual clone button.
- Briefing, copilot page, reports `ActionReviewModal` callers — they receive no props and default behavior is preserved.
- Database schema / migrations.
