# Copy Recommendations as CSV — Design

**Date:** 2026-04-23
**Status:** Approved (design); implementation pending
**Scope:** Adds a "Copy as CSV" action to two surfaces — `ActionReviewModal` and `AnalyzeWithClaudeDialog` chat messages — so users can paste trade recommendations into spreadsheets or other tools.

---

## Problem

When Claude produces trade recommendations inside a sandbox scenario (or any flow that feeds `ActionReviewModal` — briefing, copilot, reports), there is no way to pull those recommendations out as plain text. Copying directly from rendered markdown yields HTML. Users want a plain-text, spreadsheet-friendly copy.

## Goal

A one-click "Copy as CSV" button in two locations:

1. Inside `ActionReviewModal` — copies the currently displayed (possibly edited) items.
2. On each Claude chat message in `AnalyzeWithClaudeDialog` that contains recommendations — copies the as-generated actions before any modal edits.

Both buttons write the same CSV format to the clipboard.

## Non-Goals

- No other output formats (no human-readable, no JSON, no Markdown table). CSV only.
- No format picker UI.
- No changes to how Claude generates recommendations or to `parseActionsFromMarkdown`.
- No backend changes.
- No changes to the briefing / copilot / reports flows beyond the shared `ActionReviewModal` header button (which they inherit automatically).

---

## CSV format

Header row followed by one row per recommendation:

```
symbol,exchange,side,quantity,price,rationale
AAPL,NASDAQ,BUY,10,180.00,"Strong Q2 earnings momentum"
TSLA,NASDAQ,SELL,5,240.00,"Reducing concentration risk"
```

**Field rules:**

- `symbol` — uppercase, straight from `OrderRecommendation.symbol`.
- `exchange` — straight from `OrderRecommendation.exchange`.
- `side` — `'BUY'` or `'SELL'` (matches `OrderRecommendation.side`).
- `quantity` — integer, no decimals, no thousands separators.
- `price` — two decimal places via `price.toFixed(2)`. So `180` → `180.00`, `180.5` → `180.50`.
- `rationale` — raw string from `OrderRecommendation.rationale`; empty string if none.

**Escaping (RFC 4180):**

- A field containing a comma, double quote, or newline is wrapped in double quotes.
- Embedded double quotes are doubled. `She said "hi".` → `"She said ""hi""."`
- Empty rationale produces an empty field (`...,180.00,\n`), not `""`.
- Line separator: `\r\n` between rows (matches RFC 4180 and is what most spreadsheets expect).

**No fields included:**

- `id`, `orderType`, `product`, `triggerPrice`, `trailPrice`, `trailPercent`, `source` — excluded to keep the CSV compact and human-friendly. These are internal concerns; users pasting into a spreadsheet don't need them. If a future need arises, we add a second format rather than widening this one.

---

## Components

### New: `src/lib/csvRecommendations.ts`

Pure function, no React, no Tauri, no DOM:

```typescript
export function formatRecommendationsAsCsv(items: OrderRecommendation[]): string
```

- Returns a single string ending with `\r\n` after the last row.
- If `items.length === 0`, returns the header row alone (still ending with `\r\n`), so the user gets a consistent document shape.
- Internal helper `escapeCsvField(value: string): string` handles the quoting rules.

### New: `src/lib/csvRecommendations.test.ts`

Vitest unit tests covering:

1. Happy path: two items, header + two rows, `\r\n` separators.
2. Empty-items edge case: returns only the header row.
3. Rationale containing commas, quotes, and newlines — all escaped correctly.
4. Empty rationale — renders as an empty field, not `""`.
5. Price formatting — integer input (`180`) and fractional (`180.5`) both land on two-decimal output.

### Modified: `src/components/trading/ActionReviewModal.tsx`

Add a `Copy` icon-only button to the `DialogTitle` row, right-aligned. Rendered only when `!showResults && items.length > 0`.

On click:
- Calls `formatRecommendationsAsCsv(items)`.
- `await navigator.clipboard.writeText(csv)`.
- On success: `toast.success("Copied N trade(s) to clipboard")`.
- On failure: `toast.error(err.message ?? 'Failed to copy')`.

The button uses `h-8 w-8` square sizing and `variant="ghost"` so it reads as a subtle header action rather than a primary control. Tooltip/`title` attribute: `"Copy as CSV"`.

### Modified: `src/pages/clients/AnalyzeWithClaudeDialog.tsx`

Next to the existing "Review Recommendations" button — only rendered when a chat message `hasActions` — add a small `Copy` icon-only button.

On click:
- Re-parses the message: `parseActionsFromMarkdown(msg.content, 'copilot')`.
- Same clipboard + toast behavior as above.

Both the existing "Review Recommendations" and the new "Copy" button sit on the same line below the assistant message, separated by an 8px gap.

---

## Data flow

```
ActionReviewModal:
  items (from store) → formatRecommendationsAsCsv → clipboard

AnalyzeWithClaudeDialog message:
  msg.content → parseActionsFromMarkdown → formatRecommendationsAsCsv → clipboard
```

The same helper serves both paths; no branching logic.

---

## Error handling

- `navigator.clipboard.writeText` can reject (permission, secure-context failures, or non-clipboard-capable WebViews). Catch and show a toast. Do not log to console.
- Empty-items is a valid input to `formatRecommendationsAsCsv` (returns the header alone). Both call sites gate the button so it cannot fire with zero items — no special case required at the call site.
- No retry logic.

## Testing

**Unit (automated):** `formatRecommendationsAsCsv` — see `csvRecommendations.test.ts` cases above.

**Manual verification:** After the feature ships,

1. Non-baseline scenario — open review modal with 2+ items, click the header copy button, paste into a text editor. Verify header row, CSV shape, price decimals, escape on any rationale with punctuation.
2. Edit a quantity in the modal, then copy. Verify the edited value is reflected.
3. Open `AnalyzeWithClaudeDialog`, ask Claude for trade recommendations, click the per-message copy button on Claude's reply. Paste and verify output.
4. Copy with no items (close all items, if possible) — should never be able to click the button (gated).
5. Briefing flow — generate a briefing, open review modal, copy. Verify it still works identically (modal is shared).

---

## Out of scope

- CSV for order-row internals (trigger price, order type, product). If needed later, add a separate "detailed export" action.
- Export to file (download .csv). This is clipboard only.
- Keyboard shortcut (`Cmd+C` / `Ctrl+C`) — the existing native behavior on selected text is preserved; this is a deliberate button action, not a shortcut.
- Clipboard permission prompts — Tauri's webview grants clipboard access; no prompt flow needed.

---

## Files touched

| File | Role | Change |
|------|------|--------|
| `src/lib/csvRecommendations.ts` | Pure formatter + escape helper | Create |
| `src/lib/csvRecommendations.test.ts` | Unit tests | Create |
| `src/components/trading/ActionReviewModal.tsx` | Header copy button | Modify |
| `src/pages/clients/AnalyzeWithClaudeDialog.tsx` | Per-message copy button | Modify |

No backend. No schema. No store changes. No new dependencies.
