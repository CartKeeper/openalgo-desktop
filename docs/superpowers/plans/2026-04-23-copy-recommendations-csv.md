# Copy Recommendations as CSV — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Copy as CSV" action on two surfaces — the `ActionReviewModal` header and each Claude chat message with recommendations — so the user can paste trade recommendations into a spreadsheet.

**Architecture:** One pure RFC-4180 formatter in `src/lib/csvRecommendations.ts` (unit-tested), plus a small `Copy`-icon button wired into each surface. Both surfaces share the same formatter and use `navigator.clipboard.writeText`. No backend, no schema, no store changes.

**Tech Stack:** React 18 + TypeScript, Vitest, Biome, shadcn/ui. Tauri 2.0 webview (supports `navigator.clipboard.writeText`).

**Spec:** [`docs/superpowers/specs/2026-04-23-copy-recommendations-csv-design.md`](../specs/2026-04-23-copy-recommendations-csv-design.md)

---

## File structure

| File | Role | Change |
|------|------|--------|
| `src/lib/csvRecommendations.ts` | Pure CSV formatter with RFC-4180 field escaping. | Create |
| `src/lib/csvRecommendations.test.ts` | Unit tests for the formatter. | Create |
| `src/components/trading/ActionReviewModal.tsx` | Add a `Copy` icon button to the `DialogTitle` row, gated on `!showResults && items.length > 0`. | Modify |
| `src/pages/clients/AnalyzeWithClaudeDialog.tsx` | Add a `Copy` icon button next to the existing "Review Recommendations" button on assistant messages with `hasActions`. | Modify |

---

## Task 1 — Pure CSV formatter with unit tests

**Files:**
- Create: `src/lib/csvRecommendations.ts`
- Create: `src/lib/csvRecommendations.test.ts`

- [ ] **Step 1: Write the failing test file**

Create `src/lib/csvRecommendations.test.ts`:

```typescript
import { describe, expect, it } from 'vitest'
import { formatRecommendationsAsCsv } from './csvRecommendations'
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
    rationale: 'Strong Q2 earnings',
    source: 'copilot',
    ...overrides,
  }
}

const HEADER = 'symbol,exchange,side,quantity,price,rationale'

describe('formatRecommendationsAsCsv', () => {
  it('renders header + one row per item with \\r\\n separators', () => {
    const csv = formatRecommendationsAsCsv([
      item({ symbol: 'AAPL', price: 180 }),
      item({ symbol: 'TSLA', side: 'SELL', quantity: 5, price: 240, rationale: 'Reducing risk' }),
    ])
    expect(csv).toBe(
      `${HEADER}\r\nAAPL,NASDAQ,BUY,10,180.00,Strong Q2 earnings\r\nTSLA,NASDAQ,SELL,5,240.00,Reducing risk\r\n`
    )
  })

  it('returns header alone when items is empty', () => {
    const csv = formatRecommendationsAsCsv([])
    expect(csv).toBe(`${HEADER}\r\n`)
  })

  it('formats integer prices to two decimals', () => {
    const csv = formatRecommendationsAsCsv([item({ price: 180 })])
    expect(csv).toContain(',180.00,')
  })

  it('formats fractional prices to two decimals', () => {
    const csv = formatRecommendationsAsCsv([item({ price: 180.5 })])
    expect(csv).toContain(',180.50,')
  })

  it('renders empty rationale as an empty field (not empty quotes)', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: '' })])
    // trailing field is empty: ends with `,\r\n` not `,""\r\n`
    expect(csv.endsWith(',\r\n')).toBe(true)
    expect(csv).not.toContain(',""\r\n')
  })

  it('quotes and escapes rationale containing a comma', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: 'hedge, short term' })])
    expect(csv).toContain(',"hedge, short term"\r\n')
  })

  it('quotes and doubles embedded quotes in rationale', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: 'She said "hi".' })])
    expect(csv).toContain(',"She said ""hi""."\r\n')
  })

  it('quotes rationale containing a newline', () => {
    const csv = formatRecommendationsAsCsv([item({ rationale: 'line one\nline two' })])
    expect(csv).toContain(',"line one\nline two"\r\n')
  })
})
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd /Users/jasonborst/openalgo-desktop && npx vitest run src/lib/csvRecommendations.test.ts`

Expected: All 8 tests FAIL — module `./csvRecommendations` cannot be resolved.

- [ ] **Step 3: Implement the pure formatter**

Create `src/lib/csvRecommendations.ts`:

```typescript
import type { OrderRecommendation } from '@/types/actionQueue'

const HEADER = 'symbol,exchange,side,quantity,price,rationale'
const ROW_TERMINATOR = '\r\n'

/**
 * Wraps a field in double quotes and doubles embedded quotes when the value
 * contains a comma, double quote, or newline. Per RFC 4180.
 * Empty strings pass through unquoted (rendered as an empty field).
 */
function escapeCsvField(value: string): string {
  if (value === '') return ''
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

/**
 * Format a list of order recommendations as RFC 4180 CSV.
 * Always returns the header row; each item contributes one data row.
 * Price is always rendered with two decimal places.
 */
export function formatRecommendationsAsCsv(items: OrderRecommendation[]): string {
  const rows = items.map((item) => {
    const fields = [
      item.symbol,
      item.exchange,
      item.side,
      String(item.quantity),
      item.price.toFixed(2),
      item.rationale,
    ].map(escapeCsvField)
    return fields.join(',')
  })

  return [HEADER, ...rows].join(ROW_TERMINATOR) + ROW_TERMINATOR
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cd /Users/jasonborst/openalgo-desktop && npx vitest run src/lib/csvRecommendations.test.ts`

Expected: 8/8 PASS.

- [ ] **Step 5: Lint**

Run: `npm run lint`

Expected: No NEW warnings/errors in the two new files.

- [ ] **Step 6: Commit**

```bash
git -C /Users/jasonborst/openalgo-desktop add src/lib/csvRecommendations.ts src/lib/csvRecommendations.test.ts
git -C /Users/jasonborst/openalgo-desktop commit -m "Add formatRecommendationsAsCsv pure function + tests"
```

---

## Task 2 — Copy button in `ActionReviewModal`

**Files:** Modify: `src/components/trading/ActionReviewModal.tsx`

Adds a `Copy` icon button inside the `DialogTitle` row, right-aligned, gated on `!showResults && items.length > 0`. Reuses the shared `formatRecommendationsAsCsv` from Task 1.

- [ ] **Step 1: Add imports**

In the imports at the top of `src/components/trading/ActionReviewModal.tsx`:

- Find the existing `lucide-react` import (currently `import { Check, Loader2, ShoppingCart, X } from 'lucide-react'`). Add `Copy` to it, keeping alphabetical order:

```typescript
import { Check, Copy, Loader2, ShoppingCart, X } from 'lucide-react'
```

- Add a new import for the formatter, grouping it with the other `@/lib` / utility imports (below the existing `cn` import):

```typescript
import { formatRecommendationsAsCsv } from '@/lib/csvRecommendations'
```

- [ ] **Step 2: Add the copy handler**

Inside the `ActionReviewModal` component body, directly below the existing `handleClose` definition, add:

```typescript
const handleCopyCsv = async () => {
  try {
    const csv = formatRecommendationsAsCsv(items)
    await navigator.clipboard.writeText(csv)
    toast.success(`Copied ${items.length} trade${items.length !== 1 ? 's' : ''} to clipboard`)
  } catch (err) {
    toast.error(err instanceof Error ? err.message : 'Failed to copy')
  }
}
```

- [ ] **Step 3: Add the copy button to `DialogTitle`**

Find the `DialogHeader` block (near the top of the returned JSX). Currently:

```tsx
<DialogHeader>
  <DialogTitle className="flex items-center gap-2">
    <ShoppingCart className="h-5 w-5" />
    Review Recommendations
    {items.length > 0 && (
      <Badge variant="secondary" className="ml-1 text-xs">
        {items.length} order{items.length !== 1 ? 's' : ''}
      </Badge>
    )}
  </DialogTitle>
  <DialogDescription>
    Review AI-recommended trades before placing. Edit quantities, prices, or remove any
    you don't want.
  </DialogDescription>
</DialogHeader>
```

Replace with:

```tsx
<DialogHeader>
  <DialogTitle className="flex items-center gap-2">
    <ShoppingCart className="h-5 w-5" />
    Review Recommendations
    {items.length > 0 && (
      <Badge variant="secondary" className="ml-1 text-xs">
        {items.length} order{items.length !== 1 ? 's' : ''}
      </Badge>
    )}
    {!showResults && items.length > 0 && (
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={handleCopyCsv}
        title="Copy as CSV"
        aria-label="Copy as CSV"
        className="ml-auto h-8 w-8 p-0 text-muted-foreground hover:text-foreground"
      >
        <Copy className="h-4 w-4" />
      </Button>
    )}
  </DialogTitle>
  <DialogDescription>
    Review AI-recommended trades before placing. Edit quantities, prices, or remove any
    you don't want.
  </DialogDescription>
</DialogHeader>
```

The `ml-auto` on the Button pushes it to the right edge of the `DialogTitle` flex row.

- [ ] **Step 4: Typecheck**

Run: `cd /Users/jasonborst/openalgo-desktop && npx tsc -b --pretty false`

Expected: No new errors.

- [ ] **Step 5: Lint**

Run: `npm run lint`

Expected: No new warnings/errors in the touched file.

- [ ] **Step 6: Commit**

```bash
git -C /Users/jasonborst/openalgo-desktop add src/components/trading/ActionReviewModal.tsx
git -C /Users/jasonborst/openalgo-desktop commit -m "ActionReviewModal: add Copy as CSV header button"
```

---

## Task 3 — Per-message copy button in `AnalyzeWithClaudeDialog`

**Files:** Modify: `src/pages/clients/AnalyzeWithClaudeDialog.tsx`

Adds a `Copy` icon button next to the existing "Review Recommendations" button on each Claude assistant message with `hasActions`. Re-parses the message with `parseActionsFromMarkdown` and copies via the shared formatter.

- [ ] **Step 1: Add imports**

In the existing `lucide-react` import (`import { Bot, ClipboardCheck, Loader2, Send, Sparkles } from 'lucide-react'`), add `Copy` in alphabetical order:

```typescript
import {
  Bot,
  ClipboardCheck,
  Copy,
  Loader2,
  Send,
  Sparkles,
} from 'lucide-react'
```

Then add a new import for the formatter below the existing `applyScenarioRecommendations` import:

```typescript
import { formatRecommendationsAsCsv } from '@/lib/csvRecommendations'
```

- [ ] **Step 2: Add the copy handler**

Inside the component body, directly below the existing `handleApplyToScenario` definition, add:

```typescript
const handleCopyMessageCsv = async (messageContent: string) => {
  const actions = parseActionsFromMarkdown(messageContent, 'copilot')
  if (actions.length === 0) {
    toast.error('No recommended trades found in this message')
    return
  }
  try {
    const csv = formatRecommendationsAsCsv(actions)
    await navigator.clipboard.writeText(csv)
    toast.success(`Copied ${actions.length} trade${actions.length !== 1 ? 's' : ''} to clipboard`)
  } catch (err) {
    toast.error(err instanceof Error ? err.message : 'Failed to copy')
  }
}
```

- [ ] **Step 3: Replace the single-button block with a two-button row**

Find the `{hasActions && (...)}` block inside the assistant-message render. It currently contains a single Button:

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

Replace with a flex row containing both buttons:

```tsx
{hasActions && (
  <div className="mt-2 flex items-center gap-2">
    <Button
      variant="outline"
      size="sm"
      className="text-xs h-8"
      onClick={() => handleReviewRecommendations(msg.content)}
    >
      <ClipboardCheck className="h-3.5 w-3.5 mr-2" />
      Review Recommendations
    </Button>
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={() => handleCopyMessageCsv(msg.content)}
      title="Copy as CSV"
      aria-label="Copy as CSV"
      className="h-8 w-8 p-0 text-muted-foreground hover:text-foreground"
    >
      <Copy className="h-3.5 w-3.5" />
    </Button>
  </div>
)}
```

The `mt-2` is now on the outer wrapper `div` so the two buttons share one top margin rather than each having their own.

- [ ] **Step 4: Typecheck**

Run: `cd /Users/jasonborst/openalgo-desktop && npx tsc -b --pretty false`

Expected: No new errors.

- [ ] **Step 5: Lint**

Run: `npm run lint`

Expected: No new warnings/errors in the touched file.

- [ ] **Step 6: Commit**

```bash
git -C /Users/jasonborst/openalgo-desktop add src/pages/clients/AnalyzeWithClaudeDialog.tsx
git -C /Users/jasonborst/openalgo-desktop commit -m "AnalyzeWithClaudeDialog: add per-message Copy as CSV button"
```

---

## Task 4 — Manual verification

No code changes. User-driven ship gate.

- [ ] **Step 1: Build and run**

Run: `npm run tauri:dev`

Expected: App launches cleanly.

- [ ] **Step 2: Modal copy — non-baseline scenario**

1. Open a scenario with recommendations (Analyze with Claude → send a prompt that returns actions).
2. Click **Review Recommendations** to open `ActionReviewModal`.
3. Click the `Copy` icon in the header.
4. Toast reads: `Copied N trade(s) to clipboard`.
5. Paste into a text editor. Verify: header row is `symbol,exchange,side,quantity,price,rationale`. Rows match the listed items. Prices are two decimals (e.g. `180.00`). Any rationale with a comma or quote is quoted and escaped.

- [ ] **Step 3: Modal copy reflects edits**

1. With the modal open, edit a quantity on one of the recommendations.
2. Click the header copy button.
3. Paste — the new quantity is reflected in that row.

- [ ] **Step 4: Modal copy — button gated**

1. Open `ActionReviewModal` with zero items (if reproducible — e.g. remove all items via the per-row X). The header Copy button disappears (gated on `items.length > 0`).
2. In the results view (after a successful briefing submit), the Copy button should not be shown (`!showResults`).

- [ ] **Step 5: Per-message copy**

1. In `AnalyzeWithClaudeDialog`, ask Claude for trade recommendations.
2. When Claude replies, locate the two-button row under the message: **Review Recommendations** + a small `Copy` icon button.
3. Click the `Copy` icon.
4. Toast reads: `Copied N trade(s) to clipboard`.
5. Paste into a text editor. Verify the CSV matches the raw (as-generated) actions from that message, not any edits (because no edits have been made).

- [ ] **Step 6: Briefing regression**

1. Open the Briefing page, generate a briefing with actions, open `ActionReviewModal`.
2. Header Copy button is present and works.
3. Apply still places live orders as before — the copy button is purely additive.

- [ ] **Step 7: Verify clipboard-permission failure UX (optional)**

If you have a way to simulate clipboard failure (e.g. strip clipboard permissions in a dev build), confirm the error toast surfaces a sensible message. Otherwise, skip this step.

---

## Rollback

If the feature needs to be reverted post-merge:

```bash
# Revert Tasks 2 and 3 (UI buttons)
git revert <task-3-sha> <task-2-sha>
```

Task 1 (pure function + tests) is harmless to leave; reverting it is optional.

---

## Out of scope (do not touch)

- Other formats (human-readable, JSON, Markdown table).
- File download (`Blob` → anchor) — clipboard only.
- Keyboard shortcuts.
- Backend Rust code.
- Tauri clipboard plugin — `navigator.clipboard` in the webview is sufficient.
- Any other component that renders recommendations outside the two surfaces listed above.
