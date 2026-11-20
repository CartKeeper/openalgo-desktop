# Ports Program — Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement each work-package plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build eight work packages in parallel, each on its own branch and worktree, and merge them into `feat/historify-desktop-backend` in a fixed order with no surprise conflicts and a UI check after every merge.

**Architecture:** Comment anchors in the six shared files give each WP its own insertion point, so parallel additions three-way-merge cleanly. Each WP owns a disjoint file set (listed in its plan). Lock files are never hand-merged; they are regenerated. Agents prove work with tests and `npm run build`; the user verifies in the running app after each merge.

**Tech Stack:** git worktrees, cargo 1.93, npm / vitest 4 / biome, Sidekick (`mcp__sidekick__restart_project`, project id `1NrfShzwVNxcovM1u7XjB`) to restart the app from the main checkout.

**Spec:** `docs/superpowers/specs/2026-11-12-ports-program-design.md`

## Global Constraints

- Base branch for every WP: `feat/historify-desktop-backend`. Nothing is cut from `master`.
- Branch names: `wp1-chart-tools`, `wp2-portfolio-analytics`, `wp3-staged-analyzer`, `wp4-report-metrics`, `wp5-american-options`, `wp6-talib-indicators`, `wp7-symbol-metadata`, `wp8a-flow-core`, `wp8b-flow-ui`, `wp8c-flow-triggers`, `wp8d-flow-parity`.
- Worktree path: `.claude/worktrees/<branch>`. Each worktree runs its own `npm ci` and its own `target/` (no shared cargo target dir).
- No agent runs the Tauri app from a worktree (vite `strictPort 4379`). No agent restarts the user's app. No agent pushes.
- Gates before a branch is merge-ready, run in the worktree: `cd src-tauri && cargo test --lib` → all pass; `npx vitest run` → all pass; `npm run build` → exit 0; `npm run lint` → exit 0.
- Conventional commit subjects (`feat(scope): …`, `fix(scope): …`, `test(scope): …`), one commit per task.
- Reserved prefixes: Tauri commands `chart_` (WP1), `analytics_` (WP2), `staged_` (WP3), `american_` (WP5 — only if a new command is needed), `indicator_` (WP6), `symbol_metadata_` (WP7), `flow_` and `telegram_` (WP8). Migrations: `061_chart_drawings` (WP1), `062_client_allocation_targets` (WP2), `100_flow_core` (WP8), DuckDB `symbol_metadata` columns (WP7).
- Every page touched keeps or gains the bare help icon (`HelpButton`), and its help text describes the new controls.

## Review Focus

1. Two WPs register the same command name → the merged build fails. Reserved prefixes above; the merge checklist greps `generate_handler!` for duplicates.
2. Two WPs add the same migration name → `migrations.name UNIQUE` fails at app start. Numbers reserved above; the merge checklist greps `run_migration(` for duplicates.
3. A lock-file conflict resolved by picking one side → a dependency silently missing. Rule: keep the integration branch's lock, re-run `npm install` / `cargo build`, commit the regenerated lock.
4. A WP branch cut before Task 0 lands → anchors missing → hand conflicts in `lib.rs`. Branches are cut only after the Task 0 commit is on the base branch.
5. Merged code compiles but the user tests the old binary. The merge checklist restarts the app and checks the binary's modification time and a `strings` marker, as done for `15f3a9b`.

---

### Task 0: Integration anchors (coordinator, once, on the base branch)

**Files:**
- Modify: `src-tauri/src/lib.rs` (module declarations near the top; the `generate_handler![ … ]` list ending at the `])` around line 398)
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/db/sqlite/mod.rs`
- Modify: `src-tauri/src/db/sqlite/migrations.rs` (after the `060_scenario_cash` line, and at end of file)
- Modify: `src-tauri/Cargo.toml` (end of `[dependencies]`)

**Interfaces:**
- Produces: anchor lines of the form `// [wp-anchor] WPn <name>` followed by a bare `//` separator line. A WP inserts its lines **directly beneath its own anchor line**, never touching another WP's anchor or the separator.

- [ ] **Step 1: Add module anchors to `src-tauri/src/lib.rs`** after the last top-level `mod`/`pub mod` line:

```rust
// [wp-anchor] modules: each WP adds its top-level `mod` line under its own marker.
// [wp-anchor] WP2 analytics
//
// [wp-anchor] WP8 flow
//
```

- [ ] **Step 2: Add handler anchors to `src-tauri/src/lib.rs`** as the last lines inside `generate_handler![ … ]`, before `])`:

```rust
            // [wp-anchor] WP1 chart-tools
            //
            // [wp-anchor] WP2 analytics
            //
            // [wp-anchor] WP3 staged-analysis
            //
            // [wp-anchor] WP5 american-options
            //
            // [wp-anchor] WP6 talib-indicators
            //
            // [wp-anchor] WP7 symbol-metadata
            //
            // [wp-anchor] WP8 flow
            //
```

- [ ] **Step 3: Add the same seven-marker block** (WP1, WP2, WP3, WP5, WP6, WP7, WP8) at the end of `src-tauri/src/commands/mod.rs`, and a WP2/WP3/WP4/WP5/WP6/WP7 block at the end of `src-tauri/src/services/mod.rs`, and a WP1/WP2 block at the end of `src-tauri/src/db/sqlite/mod.rs`.

- [ ] **Step 4: Migration anchors.** In `run_migrations`, directly after `run_migration(conn, "060_scenario_cash", ADD_SCENARIO_CASH)?;`:

```rust
    // [wp-anchor] WP1 migrations
    //
    // [wp-anchor] WP2 migrations
    //
    // [wp-anchor] WP8 migrations
    //
```

and at the end of the file:

```rust
// [wp-anchor] WP1 sql
//
// [wp-anchor] WP2 sql
//
// [wp-anchor] WP8 sql
//
```

- [ ] **Step 5: Cargo anchors.** As the last lines of `[dependencies]` in `src-tauri/Cargo.toml` (before the next `[` section):

```toml
# [wp-anchor] WP5 deps
#
# [wp-anchor] WP6 deps
#
# [wp-anchor] WP7 deps
#
# [wp-anchor] WP8 deps
#
```

- [ ] **Step 6: Verify nothing changed but comments**

Run: `cd src-tauri && cargo check 2>&1 | tail -1` → `Finished`. Run: `git diff --stat` → only the six files, additions only.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/mod.rs src-tauri/src/services/mod.rs src-tauri/src/db/sqlite/mod.rs src-tauri/src/db/sqlite/migrations.rs src-tauri/Cargo.toml
git commit -m "chore: integration anchors for the ports program"
```

---

### Task 1: Branches and worktrees (coordinator)

- [ ] **Step 1: Cut every first-wave branch from the anchored base**

```bash
cd /Users/jasonborst/openalgo-desktop
for b in wp1-chart-tools wp2-portfolio-analytics wp3-staged-analyzer wp4-report-metrics wp5-american-options wp6-talib-indicators wp7-symbol-metadata wp8a-flow-core; do
  git worktree add ".claude/worktrees/$b" -b "$b" feat/historify-desktop-backend
done
git worktree list
```

Expected: eight new worktrees, each at the Task 0 commit. `wp8b`, `wp8c`, `wp8d` are cut later, each from the integration branch after the previous Flow phase merged.

- [ ] **Step 2: Prime each worktree** (per agent, first thing): `npm ci` in the worktree root; `cd src-tauri && cargo build` (first build is long; it is per-worktree on purpose).

---

### Task 2: Agent briefing (coordinator writes one per WP from this template)

Each agent receives, verbatim:

1. Paths to read first: `docs/superpowers/specs/2026-11-12-ports-program-design.md`, this file, and its own `docs/superpowers/plans/2026-11-12-wpN-….md`. Also `CLAUDE.md` at the repo root (design standards, help icon rule, timezone rule).
2. Its worktree path and branch name. "Work only there. Do not touch the main checkout."
3. "You cannot run the app. Prove each task with its tests, and the branch with the four gates. Say plainly which gate output you saw."
4. "Edit only the files your plan lists, plus the anchor lines reserved for your WP. If you need another file, stop and report why."
5. "Never delete or drop data stores, never log or commit secrets, never push, never change the baseline-scenario guard."
6. Report format at the end: what changed (files), each gate's last line of output, the Definition of Done from the plan rewritten as "the user clicks … and sees …", and anything unresolved.

---

### Task 3: Merge protocol (coordinator, per branch, in the order below)

- [ ] **Step 1: Merge**

```bash
git checkout feat/historify-desktop-backend
git merge --no-ff <branch>
```

- [ ] **Step 2: Resolve conflicts by rule**
  - `package-lock.json` or `src-tauri/Cargo.lock`: `git checkout --ours <lockfile>`, then `npm install` / `cd src-tauri && cargo build`, then `git add` the regenerated lock.
  - Anchor files: keep both sides' lines under their own anchors.
  - Any other conflict: stop, resolve on the WP branch, re-merge.

- [ ] **Step 3: Duplicate-name checks**

```bash
grep -o 'commands::[a-z_]*::[a-z_]*' src-tauri/src/lib.rs | sort | uniq -d        # expected: no output
grep -o 'run_migration(conn, "[^"]*"' src-tauri/src/db/sqlite/migrations.rs | sort | uniq -d   # expected: no output
sed -n '/^\[dependencies\]/,/^\[/p' src-tauri/Cargo.toml | grep -E '^[a-zA-Z0-9_-]+ *=' | cut -d= -f1 | tr -d ' ' | sort | uniq -d   # expected: no output (duplicate crate keys)
```

- [ ] **Step 4: Gates on the integration branch**: `cd src-tauri && cargo test --lib`; `npx vitest run`; `npm run build`; `npm run lint`. All must pass before the next step.

- [ ] **Step 5: Restart and verify the binary**: `mcp__sidekick__restart_project` (id `1NrfShzwVNxcovM1u7XjB`); then confirm the new build is running (binary mtime after the merge, `lsof -i :5000` shows the app, and a `strings` grep for a string the WP introduced).

- [ ] **Step 6: User UI check** against the WP's Definition of Done. Only after the user confirms: commit any follow-up fix, then push (the user allowed pushing verified commits on this branch).

- [ ] **Step 7: Next branch.** For WP8, cut the next phase branch from the integration branch now.

---

## Merge order and why

| # | Branch | Depends on | Why this slot |
|---|---|---|---|
| 1 | `wp1-chart-tools` | — | frontend-only plus one small table; also bumps `lightweight-charts`, which every other UI branch should rebase onto early |
| 2 | `wp6-talib-indicators` | — | self-contained backend swap |
| 3 | `wp5-american-options` | — | raises `rust-version` to 1.89; land before the big Rust WPs |
| 4 | `wp7-symbol-metadata` | — | DuckDB only |
| 5 | `wp2-portfolio-analytics` | — | largest schema change (new table, new module) |
| 6 | `wp4-report-metrics` | — | touches `scenario_comparison.rs`, `GoldmanReport.tsx`, `types.ts` exclusively |
| 7 | `wp3-staged-analyzer` | — | touches `copilot_service.rs` prompt constants and the Analyze dialog exclusively |
| 8–11 | `wp8a` … `wp8d` | each on the previous | Flow is the only WP that edits `state.rs`, `App.tsx`, `navigation.ts`, `index.css`, `webhook/server.rs` |

After step 7, agents for the remaining branches must `git merge feat/historify-desktop-backend` into their branch before declaring merge-ready, so their gates run against the integrated code.

## Conflict hotspots

| File | WPs | Mechanism |
|---|---|---|
| `src-tauri/src/lib.rs` | 1,2,3,5,6,7,8 | anchors |
| `src-tauri/src/commands/mod.rs`, `services/mod.rs`, `db/sqlite/mod.rs` | several | anchors |
| `src-tauri/src/db/sqlite/migrations.rs` | 1,2,8 | anchors + reserved numbers |
| `src-tauri/Cargo.toml` | 5,6,7,8 | anchors; `Cargo.lock` regenerated |
| `package.json` | 1,8 | different alphabetical regions; `package-lock.json` regenerated |
| `src/pages/clients/ScenarioDetail.tsx` | 2 only | — |
| `src/pages/clients/AnalyzeWithClaudeDialog.tsx` | 3 only | — |
| `src-tauri/src/services/scenario_comparison.rs`, `src/components/reports/goldman/*` | 4 only | — |
| `src-tauri/src/services/greeks.rs`, `commands/greeks.rs`, `src/pages/options/*` | 5 only | — |
| `src-tauri/src/services/indicators_service.rs`, `commands/indicators.rs`, `src/pages/fundamentals/AnalysisTab.tsx` | 6 only | — |
| `src-tauri/src/services/symbol_service.rs`, `db/duckdb/*`, `src/pages/Search.tsx` | 7 only | — |
| `src-tauri/src/state.rs`, `src/App.tsx`, `src/config/navigation.ts`, `src/index.css`, `src-tauri/src/webhook/server.rs` | 8 only | — |

## Standards notes the user has been asked to rule on

- Table-interaction spec vs. read-only analytics tables (spec, open item 6). Until ruled, WP2/WP4/WP7/WP8 tables follow the existing `src/components/ui/table` pattern.
- Health thresholds and "app default" rules (spec, thresholds table).
