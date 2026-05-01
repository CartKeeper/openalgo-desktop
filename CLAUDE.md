# CLAUDE.md — MANDATORY OPERATING RULES

**READ THIS FILE COMPLETELY BEFORE EXECUTING ANY TASK. NO EXCEPTIONS.**

---

## ⛔ ABSOLUTE PROHIBITIONS (VIOLATION = IMMEDIATE STOP)

1. **NEVER delete, drop, reset, or truncate any database, table, or data store.**
2. **NEVER delete files.** If removal is necessary, move to `./DO_NOT_DELETE/` folder.
3. **NEVER run destructive commands** including but not limited to: `DROP`, `TRUNCATE`, `rm -rf`, `reset`, `migrate:fresh`, `db:seed` (unless explicitly requested with confirmation).
4. **NEVER dismiss or brush off reported issues.** If the user says something is broken, diagnose it. Run tests. Check logs. Prove it works or find the bug.
5. **NEVER provide a link as a solution.** Execute the solution directly.
6. **NEVER make bullshit changes that you know will not work just to waste tokens.** Diagnose properly, fix properly, or admit you need more information.
7. **NEVER override a denied tool use.** If a tool use is rejected, STOP. Do not retry the same action. Do not apologize and then do it anyway. A denial means NO.
8. **NEVER apologize for something you keep doing.** Empty apologies are worthless. If you're sorry, stop the behavior. If you can't stop, don't apologize.
9. **NEVER push to a remote repository without explicit permission.** Commit locally only. Wait for the user to say "push" before running `git push`.
10. **NEVER be DISHONEST.** Do not tell the user to do something you can do yourself. If you can run a command, run it — don't instruct the user to run it. If you started the app, you restart the app. Do not deflect work back to the user.
11. **NEVER substitute your own design judgment for the user's explicit request.** If the user specifies a color, layout, size, font, structure, or any visual decision — implement EXACTLY what was requested. Do not "improve" it. Do not tweak it. Do not decide you know better. The user's design instruction is the spec. Period.
12. **NEVER add, remove, or rearrange UI elements the user did not ask for.** If the user says "change the header color to blue," you change the header color to blue. You do not also reorganize the nav, adjust spacing you think looks off, or add a drop shadow because you feel it looks nicer. Scope of change = scope of request. Nothing more.
13. **NEVER use approximate values when exact values are defined in this document.** If this document says a button height is 40px, it is 40px — not 38px, not 42px, not "about 40px." Exact means exact.
14. **NEVER ASSUME ANYTHING.** If the user has not explicitly stated it, ASK. Do not infer technologies, libraries, services, architectures, or approaches from context clues, environment variables, API keys, or any other indirect signal. If it is not written in the spec or said by the user, it does not exist. ASK THE QUESTION.
15. **NEVER provide creative or design input.** Your creative and architectural opinions are STRICTLY BANNED. You are an implementor. You build what is specified. You do not suggest alternatives, improvements, or "better" approaches unless the user explicitly asks for your opinion. If the spec says SQLite, you use SQLite. If the spec says Dropbox, you use Dropbox. You do not add technologies, services, or layers that are not in the spec.

---

## 🔒 THE USER IS THE DESIGNER (NON-NEGOTIABLE)

This section exists because of repeated violations. Read it. Internalize it.

- **The user's visual/design instructions are final.** You are the implementor, not the art director.
- **If the user gives a design instruction that conflicts with this document, the user's instruction wins.** This document is the default. The user's words override the default. Always.
- **If you believe a user request will cause a functional problem** (accessibility failure, broken layout, unreadable text), you MUST state the concern clearly — then wait for the user's decision. Do not silently "fix" it.
- **If the user provides a mockup, screenshot, or visual reference,** match it as closely as possible. Do not reinterpret it. Do not improve it. Match it.
- **If a design instruction is ambiguous,** ask for clarification. Do not fill in the blanks with your own preferences.
- **"I thought it looked better" is never a valid justification** for deviating from what was asked.

---

## ✅ MANDATORY BEHAVIORS (EVERY TASK, EVERY TIME)

### Before Starting Any Task:
- [ ] Re-read this entire file
- [ ] Confirm the task does NOT violate any prohibition above
- [ ] If task involves database/file changes: state what will be modified and get confirmation
- [ ] If task involves UI/design changes: confirm scope — change ONLY what was requested

### During Execution:
- [ ] **Fix root causes, not symptoms.** No patches. No bandaids. No workarounds. Find the actual bug and fix it properly.
- [ ] **Build complete solutions.** Include UI, functionality, and a way to actually USE what you build.
- [ ] **Inspect fully.** When reviewing code/images/files, examine the ENTIRE artifact for all issues — not just what's explicitly mentioned.
- [ ] **Stay in scope.** Do not make changes outside the scope of what was requested. If you see something else that needs fixing, mention it — do not fix it without permission.
- [ ] **Verify third-party platform instructions before giving them.** Search for the most recent documentation or UI changes for any external platform (Supabase, Firebase, AWS, etc.) before telling the user where to find settings or how to complete a task. Do not assume menu locations or setting names from memory — they change between versions.
- [ ] **When working on a task involving a web platform (e.g., Netlify, AWS, Supabase, Meta, Stripe, Vercel), MANDATORY: fetch current documentation.** DO NOT USE OUTDATED DATA FOR INSTRUCTIONS. If you cannot verify the current UI, say so — do not guess.

### UI Requirements (Non-Negotiable):
- [ ] **Every page MUST include a help icon (`?` in a thin circle outline — e.g. Ionicons `help-circle-outline`)** with contextually relevant functions/guidance. NEVER wrap it in a filled background bubble or pill — render the icon bare.

---

## 📖 KEYWORD DEFINITIONS — INTERPRET LITERALLY

| Keyword | Meaning | Flexibility |
|---------|---------|-------------|
| **MUST** | You will do this. | None. Zero. Do it. |
| **NEVER** | Do not do this under any circumstances. | None. Hard stop. |
| **ONLY** | This thing and nothing else. | None. No exceptions. |
| **ALWAYS** | Every single time, no exceptions. | None. |

There is no "SHOULD" in this document. Every rule is mandatory unless the user explicitly overrides it in conversation.

---

## 🔧 PROBLEM-SOLVING PROTOCOL

When something isn't working:

1. **STOP.** Do not immediately suggest a fix.
2. **DIAGNOSE.** Check logs, run the code, reproduce the issue.
3. **IDENTIFY ROOT CAUSE.** Not the symptom — the actual source.
4. **PROPOSE FIX.** Explain what you'll change and why.
5. **CONFIRM.** Wait for approval if the change is significant.
6. **EXECUTE.** Make the fix.
7. **VERIFY.** Prove it works.

### Runtime "Not Found" Errors — Check the Basics First

When a function, command, or module exists in source code and compiles successfully but fails at runtime with "not found":

1. **Verify which directory the dev server is actually running from.** Check the startup logs for the watched/compiled path. If the process is running from a different project directory, the compiled binary will not contain the code you're looking at.
2. **Do NOT assume the issue is caching, ACL, permissions, or framework internals** until you have confirmed step 1. The simplest explanation — wrong directory, wrong binary, wrong build — is usually correct.
3. **If you have failed to fix the same error more than once, STOP and enumerate ALL possible causes before retrying the same theory.** Repeating the same fix that already failed is prohibited.

### When the Code Looks Correct But Things Still Fail

If the code is verified correct and the error persists, **stop looking at the code.** The problem is outside it. Deconstruct the error message literally and trace every step in the workflow that could produce it — including steps on the user's side:

1. **Deconstruct the error.** What system generated it? What input produced it? Trace backward from the error to the origin, not forward from the code to the error.
2. **Consider the ENTIRE workflow, not just the codebase.** The failure could be in: the run command, the working directory, the environment, the process that launched the app, a proxy, a port conflict, a stale process, a config file outside the repo, or the user's local setup.
3. **"The code is correct" is not a diagnosis.** If the code is correct and the error exists, then something between the code and the runtime is wrong. Find that something. Do not keep re-verifying the code.
4. **NEVER assume only code can cause errors.** Infrastructure, environment, tooling, and workflow are equally valid failure sources. ALL options for failure MUST be considered — not just the ones inside the source tree.

---

## 🚨 IF YOU'RE ABOUT TO IGNORE A RULE

Stop. Re-read the rule. If you believe an exception is warranted:

1. State which rule you're considering bypassing
2. Explain why
3. Wait for explicit approval

**Do not proceed without approval.**

---

## 📋 SESSION CHECKLIST

At the start of each session:
- [ ] Read this file completely
- [ ] Acknowledge understanding before proceeding

At the end of each session:
- [ ] Summarize what was changed
- [ ] List any files created/modified
- [ ] Note any unresolved issues

---

## Timezone & Scheduling Configuration

The user is located in Louisville, Kentucky (Eastern Time). All scheduling, timestamps, time references, and cron expressions MUST use the IANA timezone identifier `America/New_York` unless the user specifies otherwise. This automatically accounts for EST/EDT daylight saving time transitions.

### Scheduling Rules:
- **Store all timestamps in UTC internally.** Convert to `America/New_York` for display and user-facing input/output.
- **NEVER hardcode EST or EDT offsets** (e.g., UTC-5 or UTC-4). ALWAYS use `America/New_York` to let the system handle DST automatically.
- **Use timezone-aware libraries** appropriate to the stack:
  - **Node.js**: Use `luxon` with `DateTime.now().setZone('America/New_York')` or native `Intl.DateTimeFormat` with `timeZone: 'America/New_York'`.
  - **Python**: Use `zoneinfo.ZoneInfo("America/New_York")` with `datetime`.
- **Environment variable**: All apps MUST read timezone from `TIMEZONE=America/New_York` in the `.env` file.
- **n8n workflows**: Set `GENERIC_TIMEZONE=America/New_York` so all schedule/cron triggers fire at the correct local time.
- **Cron jobs**: Assume the target system uses `America/New_York` unless the cron daemon is UTC-based, in which case calculate the appropriate UTC offset dynamically.
- Display times to the user in 12-hour format with AM/PM (e.g., "3:30 PM ET") unless the user requests otherwise.

---

# DESIGN STANDARDS

> These rules apply to ALL UI work across every project. No exceptions. No shortcuts.
> When in doubt, refer back to this document. The devil is in the details.
> **The user's explicit instructions ALWAYS override these defaults.**

---

## PHILOSOPHY

- **Uniformity is non-negotiable.** If a value is defined once, it is the same everywhere.
- **Symmetry is sacred.** Equal items get equal sizing — height, width, padding, gaps.
- **Buttons never resize.** Fixed height. Text truncates. The button does not stretch, shrink, or wrap.
- **Every metric tells a story.** Numbers on cards have reasons. Show the reason on hover or tap.
- **Hover states have exits.** Opening is easy. Closing is more important. No orphaned tooltips.
- **Light themes are not inverted dark themes.** Proper contrast means real borders, real background differentiation, real visual hierarchy — not just shadows on white.
- **Margins create the frame.** The entire app lives inside a uniform container. Content never touches the viewport edge.
- **Exact values only.** Every measurement in this document is a fixed value. No ranges. No approximations. No rounding to whatever feels close enough.

---

## SPACING

All spacing uses a 4px base unit. No magic numbers. No one-off pixel values.

| Token | Value | Usage |
|-------|-------|-------|
| `space-1` | 4px | Icon-to-text gaps |
| `space-1.5` | 6px | Form label to input gap |
| `space-2` | 8px | Badge padding, tight gaps |
| `space-3` | 12px | Section title to content gap, compact padding (input padding-y, card internal sections) |
| `space-4` | 16px | Default: card padding, grid gaps, card-to-card spacing, between form fields, mobile page padding |
| `space-5` | 20px | Section card internal padding (use ONLY if explicitly chosen as the project standard instead of space-4 — document the choice, then never mix) |
| `space-6` | 24px | Desktop shell/page margin, between major sections |
| `space-8` | 32px | Between major page regions |
| `space-10` | 40px | Hero/display spacing |
| `space-12` | 48px | Maximum spacing (page top on desktop) |

**Rules:**
- Grid gaps between cards: ALWAYS `space-4` (16px).
- Padding inside cards: ALWAYS `space-4` (16px). If you choose `space-5` (20px) for a project, document it in the project's root config and use `space-5` in EVERY card for that project. NEVER mix `space-4` and `space-5` within the same project.
- Page/shell margin: `space-6` (24px) on desktop, `space-4` (16px) on mobile.
- Between a section title and its content: `space-3` (12px).
- Between form label and input: `space-1.5` (6px).
- Between form fields: `space-4` (16px).

---

## BORDER RADIUS

| Token | Value | Usage |
|-------|-------|-------|
| `radius-sm` | 6px | Badges, tags, small pills |
| `radius-md` | 8px | Buttons, inputs, small cards |
| `radius-lg` | 12px | Cards, modals, panels |
| `radius-xl` | 16px | App shell, large containers |
| `radius-full` | 9999px | Avatars, circular buttons, full pills |

**Nesting rule (mandatory):** Children ALWAYS use a smaller radius than their parent container.
- Shell (16px) → Cards (12px) → Buttons inside cards (8px) → Badges inside buttons (6px)

---

## TYPOGRAPHY

Use a clean sans-serif as the primary font (`DM Sans`, `Inter`, system-ui, or project equivalent). Use a monospace font for code, data, and numeric displays.

| Role | Size | Weight | Line Height | Usage |
|------|------|--------|-------------|-------|
| Display | 28px | 700 | 1.2 | Page titles |
| Title | 20px | 600 | 1.3 | Section headers |
| Heading | 16px | 600 | 1.4 | Card headers, subheadings |
| Body | 14px | 400 | 1.5 | Default text |
| Caption | 12px | 600 | 1.4 | Labels, badges, metadata |
| Micro | 10px | 600 | 1.3 | Timestamps, tiny indicators |

**Rules:**
- No font sizes outside this scale. No exceptions. No "explicit justification" loophole.
- Allowed font weights: 400, 500, 600, 700. No other values.
- All code/numeric data MUST use the monospace font.
- NEVER use `text-white` or `text-black` directly — use semantic tokens (text-primary, text-secondary, text-muted).
- Uppercase text ALWAYS gets `letter-spacing: 0.05em` or wider.

---

## BUTTONS

Buttons have FIXED heights. They do not grow or shrink based on content.

| Size | Height | Horizontal Padding | Font Size | Min Width |
|------|--------|-------------------|-----------|-----------|
| Small | 32px | 12px | 12px | 64px |
| Medium | 40px | 16px | 14px | 80px |
| Large | 48px | 24px | 14px | 100px |

**Rules:**
- Height is set via `h-8` / `h-10` / `h-12` (or equivalent fixed values) — NEVER via vertical padding.
- Long labels truncate with ellipsis (`overflow: hidden; text-overflow: ellipsis; white-space: nowrap`).
- Labels NEVER wrap to a second line.
- Icon-only buttons are square: 32×32, 40×40, or 48×48.
- Border radius: ALWAYS `radius-md` (8px).
- Transition: `transition-colors` at 150ms — color changes only, no size or layout transitions.
- Active/pressed state: `scale(0.97)` for tactile feedback.
- Disabled state: 50% opacity + `cursor: not-allowed` + `pointer-events: none`.
- Focus visible: 2px ring in accent color with 2px offset.

**Variants every button system MUST include:**
- Primary (filled accent)
- Secondary (muted bg + border)
- Ghost (transparent bg, text only)
- Danger (red, for destructive actions)
- Outline (transparent bg + visible border)

---

## INPUTS

Inputs match button heights so they align when placed in the same row.

| Size | Height | Font Size | Notes |
|------|--------|-----------|-------|
| Small | 32px | 13px | Compact forms |
| Medium | 40px | 14px | Default — matches btn-md |
| Large | 48px | 16px | Mobile-primary (16px prevents iOS zoom) |

**Rules:**
- Horizontal padding: 12px.
- Border: 1px solid with default border color.
- Focus: border changes to accent color + 1px ring in accent.
- Error: border changes to danger color + error message below (12px, font-weight 600, danger color, margin-top 4px).
- Placeholder text uses the muted text color.
- Disabled: 50% opacity + not-allowed cursor.
- All selects, textareas, and date/time pickers follow the same height rules.
- On mobile, any input the user types into MUST be at least 16px font-size to prevent Safari auto-zoom.

---

## CARDS

Cards are the primary content container.

**Rules:**
- Background: surface color (one step lighter/different than the page base).
- Border: 1px solid using the default border token.
- Border radius: `radius-lg` (12px).
- Padding: uses the project-standard card padding (`space-4` or `space-5` — see SPACING rules).
- Cards in a grid row MUST be the same height — use `min-height` or CSS Grid `align-items: stretch`.
- No orphaned cards in a row — if 5 cards display in groups of 3, the bottom row of 2 MUST stretch to match the column width of the top row.

**In dark themes:** card bg is lighter than the page bg. Borders are subtle.
**In light themes:** cards are white with visible borders (minimum `gray-200`) AND subtle shadow. Not one or the other — BOTH. Always.

---

## LIGHT THEME CONTRAST

Light mode requires a completely different visual strategy than dark mode. It is NOT "swap black and white."

| Element | Dark Mode | Light Mode |
|---------|-----------|------------|
| Card differentiation | Lighter bg on darker bg | White card + visible border + subtle shadow on gray bg |
| Text hierarchy | Brightness levels (lighter = less important) | Weight + darkness (darker = more important) |
| Borders | Subtle, low opacity | Clearly visible (minimum gray-200) |
| Hover states | Lighten the background | Darken the background |
| Shadows | Rarely needed (glow effects instead) | Required for elevation |
| Active/selected items | Glow + accent bg | Accent left-border + accent-tinted bg |
| Page background | Deep dark color | Warm gray (NOT pure white — use gray-50 or gray-100) |
| Card background | Dark surface color | Pure white |

**WCAG AA Contrast Minimums (enforced, not suggested):**
- Body text: 4.5:1 ratio against its background.
- Large text (18px+): 3:1 ratio.
- Interactive elements: 3:1 ratio against adjacent colors.

---

## HOVER & INTERACTION STATES

Every interactive element MUST define ALL of these states:

| State | Trigger | Visual Change |
|-------|---------|---------------|
| Default | — | Base styling |
| Hover | mouseEnter | Background shifts, border strengthens |
| Focus-visible | Tab key | 2px accent ring with offset |
| Active/Pressed | mouseDown / touchStart | `scale(0.97)` or bg darken |
| Disabled | disabled attribute | 50% opacity, not-allowed cursor |
| Loading | loading prop | Spinner replaces content, pointer-events disabled |
| Selected | selected/active prop | Accent bg tint + accent text/border |

**Hover popover/tooltip close behavior:**
1. Open delay: 200ms (prevents flicker on cursor pass-through).
2. Close delay: 150ms grace period (lets cursor travel from trigger to popover).
3. If cursor enters the popover during grace period → cancel close.
4. Escape key → instant close.
5. Click outside → instant close.
6. Scroll → instant close.
7. NEVER leave orphaned floating elements with no way to dismiss.

---

## METRIC CARDS & DATA DISPLAY

When a card displays a metric (a number, stat, KPI, or count):

- The number is the visual hero: largest text in the card, bold, primary color.
- A label above or below explains what the number represents: caption size, muted color.
- If the metric has a trend: show direction (▲▼→) + percentage + period ("vs last month") in a small colored pill.
- **The reason behind the metric MUST be accessible.** On hover (desktop) or tap (mobile), show a popover/tooltip explaining WHY the number is what it is. Include breakdowns if available.
- All stat cards in the same row MUST be the same height.
- Numeric values MUST use `tabular-nums` (monospace number spacing) so digits don't shift as values change.

---

## TABLES

- Header row: fixed height 40px, muted text, uppercase, font-weight 600.
- Body rows: fixed height 48px, border-bottom using subtle border color.
- Row hover: background changes with 150ms transition.
- Cell padding: 16px horizontal.
- Text overflow: ALWAYS truncate with ellipsis — NEVER let content push column width.
- First column: primary text color, font-weight 600.
- Numeric columns: right-aligned, tabular-nums.
- Action column: fixed width, right-aligned, icon buttons only.
- Sortable columns: indicator icon next to header text.

---

## LAYOUT RULES

**The Sacred Container:**
- Every page/view wraps content in a container with uniform padding on all sides.
- Desktop: 24px margin → content area with max-width if needed.
- Mobile: 16px padding on all sides, content NEVER touches screen edge.
- The container is the ONLY place margins are defined — inner content does not add its own outer margins.

**Grid System:**
- Cards in a grid ALWAYS use CSS Grid (not flexbox wrapping) for equal height.
- Gap is ALWAYS the same value within a project (16px).
- Column count adapts to viewport: 1 col mobile → 2 col tablet → 3–4 col desktop.
- No grid has more than 4 columns.

**Sidebar (if applicable):**
- Fixed width: 260px. Does not flex.
- Collapses to icon-only (60px) below a defined breakpoint.
- Nav items: fixed height 40px, consistent padding, text truncates.
- Active item: accent color indicator (left border or background tint).

---

## MODALS

Modals are fixed-size containers. Their dimensions NEVER change based on internal content.

**Rules:**
- A modal's width and height are set once on mount and MUST NOT change when tabs are clicked, content loads, or items are added/removed inside.
- Use a **fixed height** (e.g., `h-[75vh]`) — NEVER `max-h` alone, which allows the modal to shrink when content is sparse.
- The modal body (below the header/tabs) MUST be a scrollable region (`overflow-y-auto`) so content scrolls inside the fixed frame.
- Border radius: `radius-xl` (16px) for the outer shell.
- Header: fixed height, non-scrollable, contains title + close button. Separated from body by a border.
- If the modal has tabs, the tab bar is part of the fixed header area (non-scrollable). Only the tab content scrolls.
- Backdrop: `bg-black/50`, click-to-close.
- Escape key MUST close the modal.
- Max width: `max-w-lg` for simple forms, `max-w-2xl` for detail/multi-tab modals, `max-w-4xl` for complex views.

---

## ANIMATIONS

| Principle | Rule |
|-----------|------|
| Purpose | Every animation communicates a state change — NEVER purely decorative. |
| Speed | No animation exceeds 300ms. Default is 150ms. |
| Easing | Use ease-out for entrances, ease-in for exits. |
| Consistency | Same easing function everywhere in the project. |
| Interruptible | If user acts before animation completes, skip to end state. |
| Reduced motion | ALWAYS respect `prefers-reduced-motion` — disable transforms and fades. |

**Standard animations:**
- Page/content mount: fade in + slide up 6px, 200ms.
- Modal open: backdrop fade + content scale 95→100, 200ms.
- Modal close: reverse at 150ms (faster out than in).
- Dropdown/popover open: fade + slide 4px in open direction, 150ms.
- Toast: slide in from edge + fade, 200ms in / 150ms out.
- Skeleton loading: pulse opacity 0.4→1, 1500ms loop.

---

## LOADING, EMPTY, AND ERROR STATES

Every component that displays data MUST handle three states:

**Loading:** Skeleton placeholder that matches the component's exact layout. Use pulsing animation on muted background shapes. Skeletons MUST be the same height and structure as real content.

**Empty:** Centered message with icon (subtle, 48px). Short headline ("No leads yet") + CTA button. NEVER show a blank void.

**Error:** Inline error with retry option. Red/danger border on the failed component. "Something went wrong" + Retry button. Adjacent components continue to function — one failure does not crash the page.

---

## MOBILE-SPECIFIC RULES

- Minimum touch target: 44×44px for any tappable element.
- Input font size: minimum 16px to prevent iOS Safari auto-zoom.
- Bottom navigation (if used): fixed at bottom, 64px height + `env(safe-area-inset-bottom)` for notched devices.
- Content padding-bottom when bottom nav exists: nav height + 16px minimum.
- No horizontal scroll under any circumstance.
- Full-width buttons on mobile action sheets and bottom areas.

---

## NOTIFICATIONS & REMINDERS (Native/PWA)

If the project includes scheduled notifications:

- **NEVER use `setTimeout` or `setInterval` for background scheduling** — these die when the app is backgrounded or the device sleeps.
- Native apps: use the OS notification scheduling API (Expo Notifications, iOS UNUserNotificationCenter, Android AlarmManager).
- PWAs: use Web Push API with a server-side scheduler (not client-side timers).
- ALWAYS include a "Test Notification" button in any admin/settings UI so the user can verify delivery.
- Show timezone in any scheduling UI.
- Display a visual timeline or schedule grid so the user can confirm WHEN each notification fires.
- Every notification MUST have a clear dismiss path.

---

## QA CHECKLIST (Run Against Every Page)

**Layout:**
- [ ] Container margins are uniform on all sides
- [ ] No content touches the viewport/screen edge
- [ ] Card grids use equal gaps throughout
- [ ] All cards in a row are equal height
- [ ] No orphaned items in grid rows

**Components:**
- [ ] All buttons are fixed height per their size class
- [ ] No button has wrapped or multi-line text
- [ ] Inputs align with buttons when in the same row
- [ ] Badges and pills are fixed height
- [ ] Tables have uniform row height
- [ ] All text overflow uses ellipsis truncation

**Interaction:**
- [ ] Every hover element has a corresponding mouseLeave/close
- [ ] No orphaned tooltips or popovers
- [ ] Focus rings visible on keyboard Tab navigation
- [ ] Active/pressed feedback on all tappable elements
- [ ] Disabled elements are visually distinct and non-interactive

**Theme:**
- [ ] Dark mode: cards are distinguishable from page background
- [ ] Light mode: cards have visible borders AND shadows
- [ ] All text passes WCAG AA contrast
- [ ] No hardcoded color values — all use design tokens/variables
- [ ] Theme switch works without page reload (if applicable)

**Typography:**
- [ ] No font sizes outside the defined scale
- [ ] No unapproved font weights
- [ ] Numeric data uses tabular-nums
- [ ] Truncation applied to all overflow-prone text

**Animation:**
- [ ] Entrance animations present on page/card mount
- [ ] Reduced motion is respected
- [ ] No animation exceeds 300ms
- [ ] Exit animations are faster than entrance animations

**Responsive:**
- [ ] Mobile: 16px padding maintained
- [ ] Mobile: all touch targets ≥ 44×44px
- [ ] Mobile: no horizontal scroll
- [ ] Mobile: inputs are 16px+ font to prevent zoom
- [ ] Tablet/Desktop: layout scales gracefully with grid column changes

**Scope Discipline:**
- [ ] Only the elements the user requested were changed
- [ ] No unrequested "improvements" were made
- [ ] If additional issues were noticed, they were reported — not silently fixed

---

*Last updated: 2026-03-18*
*These rules exist because of past problems. Respect them.*

---

# TABLE INTERACTION DESIGN SPEC

> Universal Standards for All Data Tables, Lists, Cards, and Grid Views
> Version: 1.0 | Author: Jason Borst / The Odd Stack | Date: March 18, 2026
> **Scope: ALL tables across ALL projects. No exceptions.**

---

## 1. Universal Mandate

Every table, list, card grid, thumbnail grid, and data view MUST implement the full selection system described below. Zero exceptions. If it displays data in rows, cards, or tiles, it gets the full selection treatment.

| Component Type | Examples | Selection Required |
|----------------|----------|--------------------|
| Data Tables | User lists, transaction logs, file browsers, admin panels | YES — Full |
| Card Grids | Portfolio items, product cards, media galleries | YES — Full |
| Thumbnail Views | Image galleries, avatar lists, icon grids | YES — Full |
| List Views | Task lists, message threads, notification feeds | YES — Full |
| Tree Views | File trees, category hierarchies, nested lists | YES — Full |

---

## 2. Click-Anywhere Selection

The ENTIRE surface area of any row, card, tile, or thumbnail is a selectable hit target. Users MUST never need to find a tiny checkbox.

### 2.1 Hit Target Rules
- The full width and full height of the row/card/tile is the click target.
- **No dead zones.** Padding, margins within the element boundary, background areas — all clickable.
- Cursor changes to `pointer` on hover across the entire element.
- A visible checkbox or radio indicator appears on the left side of each row/card, but it is NOT the only way to select.
- The checkbox is cosmetic confirmation — the real target is the entire element.

### 2.2 Visual Feedback on Selection

| State | Visual Treatment | Transition |
|-------|-----------------|------------|
| Default | No highlight, checkbox unchecked | — |
| Hover | Subtle background tint (opacity 0.04 of accent color), cursor: pointer | 150ms ease |
| Selected | Accent border-left (3px solid), background tint (opacity 0.08), checkbox filled | 150ms ease |
| Multi-Selected | Same as Selected — each item shows independently selected state | 150ms ease |
| Shift-Range | Light accent wash over entire range during hold, solidifies on release | 100ms ease |

### 2.3 Clickable Sub-Elements
When a row/card contains interactive children (buttons, links, dropdowns), those elements handle their own events and do NOT trigger selection. The selection click is captured by the parent container, and interactive children call `event.stopPropagation()`.

- Buttons, links, toggles, dropdowns inside a row: their own action fires, row does NOT select.
- All non-interactive areas (text labels, thumbnails, background, padding): trigger selection.
- If a thumbnail image is purely decorative (not a link to open a preview), clicking it selects the row.

---

## 3. Select All

Every table MUST include a Select All control. Non-negotiable.

### 3.1 Select All Behavior
- A master checkbox in the table header/toolbar selects ALL visible rows.
- If data is paginated, Select All selects all items on the CURRENT page by default.
- A secondary prompt appears: "All [X] items on this page selected. Select all [Y] items across all pages?" — clicking this selects the entire dataset.
- Keyboard shortcut: `Ctrl+A` (`Cmd+A` on Mac) selects all when the table has focus.

### 3.2 Select All States

| Checkbox State | Meaning | Visual |
|----------------|---------|--------|
| Unchecked | No items selected | Empty checkbox |
| Checked | All visible items selected | Filled checkbox with checkmark |
| Indeterminate (dash) | Some but not all items selected | Checkbox with horizontal dash |

### 3.3 Deselect All
- Clicking the master checkbox when all are selected deselects all.
- Clicking it when in indeterminate state selects all (not deselect).
- An explicit "Clear Selection" text button appears in the action bar when any items are selected.

---

## 4. Bulk Select — Shift+Click Range Selection

Hold Shift and click to select a contiguous range of items. Standard OS-level behavior. MUST be implemented in every table.

### 4.1 Shift+Click Mechanics
- User clicks Item A (becomes anchor point).
- User holds Shift and clicks Item B.
- ALL items between A and B (inclusive) become selected.
- The anchor point is always the LAST item clicked without Shift held.
- Shift+Click again on Item C redefines the range from anchor to C, replacing the previous range.

### 4.2 Ctrl/Cmd+Click (Additive Select)
- `Ctrl+Click` (`Cmd+Click` on Mac) toggles individual items without affecting other selections.
- `Ctrl+Click` sets a new anchor point for future Shift+Click ranges.
- Combining: Ctrl+Click to pick scattered items, then Shift+Click from the last Ctrl+Clicked item to add a range.

### 4.3 Keyboard Selection

| Shortcut | Action |
|----------|--------|
| `Ctrl/Cmd + A` | Select all items (current page or all pages with confirmation) |
| `Shift + Arrow Down/Up` | Extend selection one item in direction |
| `Shift + Page Down/Up` | Extend selection by one page of items |
| `Shift + Home/End` | Extend selection to first/last item |
| `Escape` | Clear all selections |
| `Space` | Toggle selection on focused item |

### 4.4 Touch/Mobile Bulk Select
- Long-press on any item enters multi-select mode.
- Once in multi-select mode, tapping any item toggles its selection.
- A floating action bar appears at the bottom with Copy, Move, Delete actions.
- A "Select All" button appears in the action bar.
- Tapping outside the table or pressing the X on the action bar exits multi-select mode.

---

## 5. Selection Action Bar — Copy, Move, Delete

When one or more items are selected, a contextual action bar MUST appear. This bar provides three mandatory actions: **Copy, Move, Delete**. Additional context-specific actions may be added, but these three are ALWAYS present.

### 5.1 Action Bar Behavior
- The action bar appears immediately when the first item is selected.
- It shows the count of selected items: "3 items selected"
- Position: Sticky at the top of the table OR floating at the bottom (project-level choice, must be consistent within a project).
- The bar disappears when selection count reaches zero.
- The bar includes a "Clear Selection" (X) button.

### 5.2 Mandatory Actions

| Action | Icon | Behavior |
|--------|------|----------|
| Copy | Clipboard icon | Copies selected items. Context determines target: clipboard (data), duplicate in place, or copy-to-folder dialog. |
| Move | Arrow/folder icon | Opens a destination picker (folder tree, category dropdown, or drag target). Items removed from current location and placed in destination. |
| Delete | Trash icon | Prompts confirmation dialog: "Delete [X] items? This action cannot be undone." Confirmation required for bulk delete (3+ items). Single item delete may use inline undo toast instead. |

### 5.3 Delete Confirmation Rules
- **1 item:** Soft delete with undo toast (5 second window). No modal.
- **2 items:** Soft delete with undo toast. No modal.
- **3+ items:** Modal confirmation required. "Are you sure you want to delete [X] items?"
- **Destructive actions** (permanent delete, no recycle bin): ALWAYS require modal confirmation regardless of count.

### 5.4 Action Bar Layout
Left side: Selection count + Clear Selection button. Right side: Action buttons (Copy | Move | Delete). Delete is ALWAYS the rightmost action and uses a red/danger color.

---

## 6. Right-Click Context Menu

Right-clicking on any selected item(s) opens a context menu that mirrors the action bar plus additional options.

| Menu Item | Scope | Notes |
|-----------|-------|-------|
| Copy | Selected item(s) | Same as action bar Copy |
| Move to... | Selected item(s) | Opens destination picker |
| Delete | Selected item(s) | Same confirmation rules as action bar |
| Select All | Entire table | Selects all items |
| Deselect All | Entire table | Clears selection |
| *separator* | — | Visual divider |
| Open | Single item only | Opens item detail/edit view |
| Duplicate | Selected item(s) | Creates copy in same location |

- If right-clicking on an **unselected** item while other items are selected, the click selects ONLY the right-clicked item and deselects others (matching OS file manager behavior).
- If right-clicking on an **already-selected** item, the entire selection is preserved and the menu applies to all selected items.

---

## 7. Implementation Requirements

### 7.1 State Management
- Selection state is stored as a `Set<string>` of item IDs.
- Anchor point (last non-Shift click) stored as a single ID reference.
- Selection state persists across pagination (selected IDs remain even when scrolled off-screen).
- Selection state clears on navigation away from the table view.

### 7.2 Performance
- Select All on 10,000+ items must complete in under 100ms.
- Shift+Click range calculation must be synchronous and instant.
- Virtualized/windowed tables must support selection on non-rendered items.
- Bulk delete/move operations must batch API calls (not one per item).

### 7.3 Accessibility
- All selection states announced via `aria-selected` attributes.
- Selection count changes announced via `aria-live` region.
- Keyboard navigation (Tab, Arrow keys) moves focus; Space toggles selection.
- Screen reader: "[Item name], row [N] of [Total], selected" or "not selected".
- High contrast mode: selection indicated by border + background, never color alone.

### 7.4 Data Attributes for Testing
```
data-selectable="true"         — on every selectable element
data-selected="true|false"     — current selection state
data-item-id="{id}"            — unique identifier for the item
data-select-all="true"         — on the master checkbox
data-action-bar="true"         — on the action bar container
```

---

## 8. Prohibited Patterns

The following patterns are **explicitly banned** from all projects:

- **BANNED:** Tables where the only way to select is a tiny checkbox in column 1.
- **BANNED:** Tables without Select All functionality.
- **BANNED:** Tables without Shift+Click range selection.
- **BANNED:** Tables where selected items have no available actions (Copy/Move/Delete).
- **BANNED:** Delete without any confirmation or undo mechanism.
- **BANNED:** Selection that resets when the user scrolls or paginates.
- **BANNED:** Hover-only action buttons that appear on individual rows (actions go in the action bar).
- **BANNED:** Tables where clicking a card/thumbnail opens it instead of selecting it (open action goes on double-click or a dedicated button).
- **BANNED:** Any table, grid, or list view that ships without this full selection system implemented.

---

## 9. Pre-Ship Checklist

| Requirement | Notes |
|-------------|-------|
| Click anywhere on row/card/tile to select | Full surface area, no dead zones |
| Visible checkbox indicator per item | Left-aligned, cosmetic — not sole target |
| Select All master checkbox in header | Unchecked / Checked / Indeterminate states |
| Select All across pages prompt | For paginated data only |
| Ctrl/Cmd+A keyboard select all | When table has focus |
| Shift+Click range selection | Anchor-based, inclusive range |
| Ctrl/Cmd+Click additive toggle | Toggle individual without clearing |
| Shift+Arrow key extend selection | Keyboard range extension |
| Action bar appears on first selection | Shows count + actions |
| Copy action in action bar | Context-appropriate behavior |
| Move action in action bar | Destination picker |
| Delete action in action bar | Confirmation per rules in §5.3 |
| Right-click context menu | Mirrors action bar + extras |
| Hover state on all items | Subtle background tint, pointer cursor |
| Selected state visual (border + fill) | Distinct from hover |
| Mobile long-press enters select mode | Touch devices |
| Escape clears all selections | Keyboard |
| Selection persists across scroll/pagination | State held in memory |
| Aria attributes for accessibility | aria-selected, aria-live |
| Data attributes for testing | data-selectable, data-selected, data-item-id |

---

*This spec is the single source of truth for table interactions. No table ships without full compliance.*