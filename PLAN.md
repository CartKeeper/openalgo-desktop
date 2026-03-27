# OpenAlgo Desktop — Recovery & Completion Plan

**Date:** 2026-03-26
**Working Directory:** `/Users/jasonborst/openalgo-desktop`
**DO NOT TOUCH:** `/Users/jasonborst/openalgo` (upstream open-source repo — not our app)

---

## What Happened

Work was incorrectly started in the upstream `openalgo` repo instead of this desktop app. Files were copied, imports were mangled, and Flask/web hybrid code was injected where it doesn't belong. None of that matters here. This is the real app. Everything lives here.

## Migration Check (2026-03-26)

Verified that no files were stranded in the wrong repo:

| File in `openalgo/frontend/src/` | Desktop equivalent | Action |
|---|---|---|
| `_desktop_pending/AlertCenter.tsx` | `src/pages/alerts/AlertCenter.tsx` | **Identical** — already in desktop. No migration needed. |
| `_desktop_pending/useAlertListener.ts` | `src/hooks/useAlertListener.ts` | **Already in desktop.** No migration needed. |
| `_desktop_pending/useAutoLogout.ts` | `src/hooks/useAutoLogout.ts` | **Already in desktop.** No migration needed. |
| `contexts/MarketDataContext.tsx` | Does not exist in desktop | **Not needed.** Nothing in the desktop app imports it. Desktop uses `SocketProvider` instead. |
| `utils/toast.ts` | Does not exist in desktop | **Not needed.** Desktop uses `sonner` directly. |

**Result:** All desktop code already lives in this repo. Nothing to migrate. The upstream repo can be ignored entirely.

---

## Current State of This App

### What's Working (committed to git)
- Tauri 2.0 shell with Rust backend
- Login / setup / session management
- India brokers: Angel, Zerodha, Fyers (OAuth + TOTP flows)
- Embedded webhook server (port 5000) for OAuth callbacks
- Order placement, modification, cancellation
- Positions, holdings, orderbook, tradebook
- Funds / margin queries
- Symbol search and master contract downloads
- Sandbox / analyzer mode
- Historify (DuckDB time-series)
- Python strategy management
- Webhook strategy management
- Chartink strategies
- Telegram bot integration
- Admin panel (freeze qty, holidays, market timings)
- Monitoring (latency, security, traffic dashboards)
- WebSocket test page
- Playground (API tester)
- Sidebar navigation with region-based filtering
- Dark/light theme
- AES-256 encrypted credential storage (file-based, replaced keychain)
- Auto-logout scheduler
- API rate limiting

### What's Built But NOT Committed (50+ untracked files)
All of these exist in the working tree but have never been committed:

#### US Broker Integrations (Rust)
- `src-tauri/src/brokers/alpaca/mod.rs` — Alpaca Markets
- `src-tauri/src/brokers/tradier/mod.rs` — Tradier
- `src-tauri/src/brokers/schwab/mod.rs` — Charles Schwab
- `src-tauri/src/brokers/ibkr/mod.rs` — Interactive Brokers

#### Data Providers (Rust)
- `src-tauri/src/providers/anthropic.rs` — Claude AI (powers copilot)
- `src-tauri/src/providers/fmp.rs` — Financial Modeling Prep (fundamentals)
- `src-tauri/src/providers/fred.rs` — Federal Reserve economic data
- `src-tauri/src/providers/yahoo.rs` — Yahoo Finance

#### New Rust Commands (IPC)
- `alerts.rs`, `clients.rs`, `copilot.rs`, `greeks.rs`
- `indicators.rs`, `portfolio.rs`, `providers.rs`
- `quant.rs`, `reports.rs`, `watchlist.rs`

#### New Rust Services
- `alert_service.rs`, `copilot_service.rs`, `greeks.rs`
- `indicators_service.rs`, `portfolio_service.rs`, `quant_service.rs`

#### New Database Tables (SQLite)
- `alerts.rs`, `clients.rs`, `portfolio.rs`
- `provider_keys.rs`, `reports.rs`, `watchlist.rs`

#### New React Pages
| Path | Page | Status |
|---|---|---|
| `/briefing` | BriefingPage | Untracked |
| `/copilot` | CopilotPage (AI Research) | Untracked |
| `/fundamentals` | FundamentalsPage + AnalysisTab | Untracked |
| `/news` | NewsPage | Untracked |
| `/calendar` | CalendarPage | Untracked |
| `/screener` | ScreenerPage | Untracked |
| `/analyst` | AnalystPage | Untracked |
| `/options` | OptionsPage | Untracked |
| `/quant` | QuantPage | Untracked |
| `/congress` | CongressPage | Untracked |
| `/alerts` | AlertCenter | Untracked |
| `/reports` | ReportsLibrary + ViewReport | Untracked |
| `/portfolio` | PortfolioManager | Untracked |
| `/clients` | ClientsIndex + ClientDetail + ImportCsvDialog | Untracked |
| `/generic-setup` | GenericSetup (data provider config) | Untracked |
| `/admin/server` | ServerSettings | Untracked |

#### New React Components
- `Sidebar.tsx` — collapsible sidebar layout
- `PlaceOrderDialog.tsx` — order entry dialog
- `WatchlistCard.tsx` — real-time watchlist
- `WatchlistAnalysisCard.tsx` — watchlist with analysis

#### New Stores
- `alertStore.ts` — price alert management
- `copilotStore.ts` — AI copilot session state
- `reportsStore.ts` — saved reports

#### New Hooks
- `useAlertListener.ts` — Tauri event listener for alerts

#### New Types
- `clients.ts` — client management types

### Database
- **Location:** `~/Library/Application Support/com.openalgo.desktop/`
- `openalgo.db` (3.5 MB) — all operational data, auth, settings, broker configs
- `historify.duckdb` — historical OHLCV data
- `secrets.dat` — AES-encrypted broker credentials

---

## What Needs To Be Done

### Phase 1: Stabilize & Commit (FIRST)

**Goal:** Get all untracked work committed so nothing else gets lost.

1. Review all 50+ untracked files for completeness
2. Check that `cargo build` compiles cleanly with all new Rust code
3. Check that `npm run build` compiles cleanly with all new React code
4. Fix any compilation errors
5. Commit everything in logical chunks:
   - US broker integrations (Alpaca, Tradier, Schwab, IBKR)
   - Data providers (Anthropic, FMP, FRED, Yahoo)
   - Research pages (copilot, fundamentals, news, screener, analyst)
   - Market data pages (congress, calendar, options, quant)
   - Portfolio & client management
   - Alert system
   - Reports system
   - Sidebar + layout changes
   - Dashboard components (watchlist cards)

### Phase 2: Verify Functionality

**Goal:** Make sure everything actually works end-to-end.

1. **Auth flow** — Login, broker select, broker login (test with at least one US broker)
2. **Sidebar navigation** — All links route correctly, region filtering works
3. **Research pages** — Copilot talks to Anthropic, Fundamentals pulls from FMP, News renders
4. **US broker trading** — Place order, view positions, check holdings (paper trading)
5. **Alert system** — Create alert, receive notification, view history
6. **Reports** — Save report, view saved reports
7. **Portfolio** — Add holdings, view analytics
8. **Clients** — Add client, import CSV, view client detail
9. **Generic mode** — Research-only mode without broker connection

### Phase 3: Missing Functionality (if any)

Based on the inventory, these Rust commands exist but may need frontend wiring:

| Command | Has Rust Code | Has React Page | Wired Up? |
|---|---|---|---|
| Watchlist | `watchlist.rs` | `WatchlistCard.tsx` | Needs verification |
| Greeks | `greeks.rs` | Used by OptionsPage | Needs verification |
| Indicators | `indicators.rs` | Used by QuantPage | Needs verification |
| Providers | `providers.rs` | `GenericSetup.tsx` | Needs verification |

### Phase 4: Dev Launcher Config

Update the dev launcher at `/Users/jasonborst/Devrun/dev-launcher/` to point here:

```json
{
  "name": "OpenAlgo Desktop",
  "projectPath": "/Users/jasonborst/openalgo-desktop",
  "commands": [
    {
      "label": "Tauri Dev",
      "command": "npx tauri dev",
      "cwd": "/Users/jasonborst/openalgo-desktop"
    },
    {
      "label": "Lean Trading API",
      "command": "/usr/local/bin/python3.11 app.py",
      "cwd": "/Users/jasonborst/lean-trading"
    }
  ],
  "devUrl": "http://localhost:1420",
  "stack": "Tauri, React, TypeScript, Rust"
}
```

Note: Flask backend is NOT needed. The Rust backend handles everything.

---

## Architecture Reference

```
openalgo-desktop/
  src/                    # React frontend
    pages/                # 78 page components
    components/           # UI components + layout
    stores/               # Zustand state (auth, theme, alerts, copilot, reports)
    api/                  # Tauri IPC wrappers (invoke → Rust)
    hooks/                # React hooks (prices, market data, sockets)
    types/                # TypeScript type definitions
    config/               # Navigation, playground endpoints
  src-tauri/              # Rust backend
    src/
      brokers/            # 7 broker adapters (3 India + 4 US)
      commands/           # 20+ IPC command modules
      services/           # Business logic
      providers/          # External data (Anthropic, FMP, FRED, Yahoo)
      db/                 # SQLite + DuckDB
      webhook/            # Embedded HTTP server (port 5000)
      websocket/          # WebSocket manager
      scheduler/          # Auto-logout, alert monitor
      security/           # AES encryption, hashing
```

**Data flow:** React → `invoke()` → Rust IPC command → Service → Broker/DB/Provider → Response → React

**No Flask. No web server. No browser. Desktop only.**
