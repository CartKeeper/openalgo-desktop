# Scenario Accept In Place — Design

**Date:** 2026-04-23
**Status:** Approved (design); implementation pending
**Scope:** Sandbox Scenarios — "Analyze with Claude" accept flow

---

## Problem

Inside a sandbox scenario, clicking "Apply to Cloned Scenario" on Claude's recommendations unconditionally creates a new scenario row (`cloneScenario` → INSERT into `client_scenarios`, copy positions, then apply the trades to the new clone). Repeated acceptances produce a growing trail of sibling clones instead of letting the user iterate on a single strategy clone.

The user wants the accept flow to mirror the main-portfolio briefing pattern: select which recommended actions to apply, hit Apply, and have them written in place — the same way `ActionReviewModal` applies briefing actions to the live portfolio via `placeBasketOrder`.

## Goal

Accepting Claude's recommendations modifies the current scenario in place. Cloning only happens when the user is on a baseline, and only once per acceptance, with a user-typed name that identifies the strategy.

## Non-Goals

- No changes to the briefing → live-portfolio flow.
- No database schema changes.
- No changes to the scenario detail page layout.
- No changes to the manual "+ New Scenario" button or baseline auto-sync behavior.
- No changes to how Claude generates the recommendations or parses markdown.

---

## Behavior

### Non-baseline scenario (existing clone)

1. User clicks "Analyze with Claude" — chat dialog opens (unchanged).
2. Claude replies with markdown containing recommended actions.
3. User clicks a new "Review Recommendations" button in the dialog.
4. `parseActionsFromMarkdown(messageContent, 'copilot')` produces the action list (unchanged call).
5. `ActionReviewModal` opens with those actions.
6. User toggles which actions to apply, optionally edits quantities (existing modal behavior), clicks Apply.
7. For each selected action: the scenario-trade handler calls the existing `apply_scenario_trade` Tauri command with `scenarioId = currentScenarioId`.
8. Modal closes. Holdings in the scenario detail page refresh. No navigation.

### Baseline scenario

Same as above, with one difference in `ActionReviewModal`:

- A required text input is rendered above the action list: **"Name this strategy clone"**.
- Apply button is disabled until the name is non-empty.
- On Apply, the handler calls `clone_client_scenario(baselineId, userTypedName)` first, then loops `apply_scenario_trade` against the new clone's id, then navigates to the new clone's detail page.

A baseline is never mutated. Claude's recommendations against a baseline always produce exactly one new clone per acceptance, with a deliberate name.

---

## Components

### `ActionReviewModal` (src/components/trading/ActionReviewModal.tsx)

Two new optional props. Default behavior (briefing) unchanged when props are absent.

- `onApply?: (selectedItems, cloneName?: string) => Promise<void>` — when provided, the modal delegates the Apply action to this callback instead of calling `tradingApi.placeBasketOrder`. Briefing page does not pass this; scenario flow does.
- `cloneNameRequired?: boolean` — when true, renders the required "Name this strategy clone" input at the top of the modal. Apply is disabled until the input is non-empty. The typed name is passed to `onApply` as the second argument.

Submit button label:
- Default (briefing): unchanged.
- `onApply` provided, `cloneNameRequired` false: "Apply to Scenario".
- `onApply` provided, `cloneNameRequired` true: "Create Clone & Apply".

No other visual or interaction changes to the modal.

### `AnalyzeWithClaudeDialog` (src/pages/clients/AnalyzeWithClaudeDialog.tsx)

Changes to the accept flow:

- Remove the unconditional `cloneScenario()` call in the current `handleApplyToClone` handler.
- Remove the auto-generated `"{scenarioName} — AI Recommendations"` name string.
- Rename the button from "Apply to Cloned Scenario" to "Review Recommendations".
- Clicking the button:
  1. Parses actions with the existing `parseActionsFromMarkdown(messageContent, 'copilot')` call.
  2. If no actions were found, show the existing "no actions" feedback (unchanged).
  3. Otherwise open `ActionReviewModal` with:
     - `items` = parsed actions
     - `cloneNameRequired` = `currentScenario.is_baseline`
     - `onApply` = the scenario-apply handler described below

Scenario-apply handler (inside the dialog component):

```
async function applyToScenario(selectedItems, cloneName?) {
  let targetId = currentScenarioId;
  if (currentScenario.is_baseline) {
    const cloned = await cloneScenario(currentScenarioId, clientId, cloneName, /*description*/ null);
    targetId = cloned.id;
  }
  for (const item of selectedItems) {
    await invoke('apply_scenario_trade', { scenarioId: targetId, ...item });
  }
  if (targetId !== currentScenarioId) {
    navigate(`/clients/${clientId}/scenarios/${targetId}`);
  } else {
    // trigger holdings refresh on the current scenario detail page
  }
}
```

Refresh mechanism for the non-navigation case: use the scenario detail page's existing positions-query invalidation (the same mechanism the page already uses when positions change). Exact hook/query key identified during implementation.

### Backend

No changes. Uses existing:
- `clone_client_scenario` (src-tauri/src/db/sqlite/client_scenarios.rs, lines 146–179)
- `apply_scenario_trade` (existing Tauri command)

---

## Data flow

```
User on scenario → Claude chat → markdown with actions
  ↓
parseActionsFromMarkdown(content, 'copilot')
  ↓
ActionReviewModal { items, cloneNameRequired, onApply }
  ↓ (user selects + clicks Apply)
onApply(selectedItems, cloneName?)
  ↓
  if baseline: clone_client_scenario(...) → new scenarioId
  ↓
  for each item: apply_scenario_trade(scenarioId, ...)
  ↓
  if cloned: navigate to new scenario
  else:       invalidate positions query, modal closes
```

---

## Error handling

- If `clone_client_scenario` fails on a baseline apply, surface the error in the modal, keep the modal open, do not call `apply_scenario_trade` for any item. The baseline is untouched.
- If any `apply_scenario_trade` call fails mid-loop, stop the loop, surface the error, and leave already-applied items applied. Do not roll back the clone if one was created. Show a message naming which actions were applied and which failed so the user knows the state.
- All Tauri command errors are rendered inside the existing modal error region (same pattern used by the briefing flow today).

## Testing

Manual verification:

1. Non-baseline scenario: analyze with Claude → review → apply → positions update in place, no new scenario row created, no navigation.
2. Baseline scenario: analyze with Claude → review → Apply disabled until name typed → apply → new clone row created with the typed name, positions applied to clone, navigated to the clone.
3. Baseline scenario: dismiss the modal without typing a name → no clone created, no positions changed.
4. Apply failure mid-loop: surface error, partial state visible, no silent corruption.
5. Briefing flow unchanged: live-portfolio briefing → review → submit still calls `placeBasketOrder` against the broker.

---

## Out of scope (explicit)

- Undo/rollback for partial apply failures.
- Batching multiple `apply_scenario_trade` calls into a single transaction (can be done later if performance requires it).
- Restructuring the scenario detail page layout.
- Changing which actions Claude recommends or how markdown is parsed.

---

## Files touched

- `src/components/trading/ActionReviewModal.tsx` — add two optional props, wire the Apply button.
- `src/pages/clients/AnalyzeWithClaudeDialog.tsx` — remove unconditional clone, rename button, open `ActionReviewModal`, implement scenario-apply handler.

No backend, no schema, no migration.
