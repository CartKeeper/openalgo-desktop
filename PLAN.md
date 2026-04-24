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

---

## Phase 5: TOS-Style Account Switcher & Trading Features

**Date Added:** 2026-03-27
**Goal:** Build thinkorswim-style multi-account management, analysis tools, and charting — both in the main app hub and in the client sandbox area.

### Context

User has Schwab/thinkorswim accounts with multiple sub-accounts (Individual, IRA, 401k). Current CSV import merges all positions into a single pile. TOS allows switching between accounts or viewing all combined. We need this same UX.

This applies to:
- **Main app** — the user's own brokerage accounts
- **Client sandbox** — each client may have multiple accounts

### Research Sources

| Source | What It Provides |
|--------|-----------------|
| **Schwab Official API** (`developer.schwab.com`) | Accounts, positions, balances, quotes, options chains, price history, orders, streaming, transactions |
| **schwab-py** (205+ stars) | Best-documented Python wrapper for Schwab API |
| **Schwabdev** (724 stars) | Most popular Python wrapper, auto token management, streaming |
| **schwab-client-js** | Node/TS wrapper — 3 client classes (Market, Trading, Streaming) |
| **tos-wsjson-client** | Reverse-engineered TOS WebSocket (richer than official API — alerts, watchlists, market depth) |
| **open-stocks-mcp** | MCP Server: 104 tools (80 Robinhood + 24 Schwab) for AI agents |

**No open-source TOS UI replacement exists.** Every existing project is a focused tool (options screener, GEX chart, trading bot). What we're building is novel.

### What's NOT Available via Any API
- Historical options pricing data
- Paper trading
- ThinkScript execution outside TOS
- Full Level 2 / Time & Sales (limited market depth only)

---

### 5A: Must-Have — Account Switcher & Core

| # | Feature | Description | Where It Appears |
|---|---------|-------------|-----------------|
| 1 | **Account Switcher Dropdown** | Dropdown in header/toolbar: Individual, IRA, 401k, All Accounts. Persists across page navigation. | Main app + Client sandbox |
| 2 | **Per-Account Positions/Holdings** | Filter positions and holdings table by selected account | Positions page, Holdings page, Client detail |
| 3 | **Combined "All Accounts" View** | Aggregate positions across all accounts with account tag on each row | Same pages as above |
| 4 | **Per-Account Balance Cards** | Account value, buying power, day P&L, total P&L — scoped to selected account or combined | Dashboard, Client detail |
| 5 | **Account-Scoped Order History** | Order book filters by selected account | Order Book page, Client detail |

#### Implementation Notes — Account Switcher

**Data model changes needed:**
- Import batches need `account_type` (already added) and `account_identifier` (e.g., last 4 digits: *621, *229)
- Positions/holdings queries need to filter by account
- Need a global store (`accountStore.ts`) to hold the selected account, shared across all pages
- The switcher dropdown should show: account nickname + type + last 4 digits
- "All Accounts" is the default view — shows everything with an account column

**Client sandbox:**
- Each client can have multiple accounts (imported via separate CSVs)
- The same switcher UX applies within a client's detail page
- Scenarios are per-account (a rebalance scenario for the 401k is independent of the Individual account)

---

### 5B: Nice-to-Have — Analysis & Charting

| # | Feature | Description | Data Source | New Provider? |
|---|---------|-------------|------------|---------------|
| 6 | **Stock Screener/Scanner** | Custom filters: market cap, P/E, volume, sector, price range, technical signals | FMP + Yahoo | No — already have both |
| 7 | **Gamma Exposure (GEX) Visualization** | Chart showing dealer gamma by strike price for a given symbol | Schwab options chain + calculation | Schwab provider needed |
| 8 | **Volatility Models** | Parkinson, Garman-Klass, Yang-Zhang historical vol estimators alongside standard deviation | Price history (Yahoo/Schwab) | No — calculation on existing data |
| 9 | **Probability Cones on Charts** | Statistical projection of price range at future dates based on vol model | Calculated from vol models | No — calculation |
| 10 | **Unusual Options Activity Detection** | Flag options trades with volume >> open interest, large notional, or unusual spreads | Schwab options chain snapshots | Schwab provider needed |
| 11 | **Multi-Calendar System** | Unified calendar with tabs/filters by type | Multiple sources (see below) | Partial — crypto needs new provider |
| 12 | **Full Charting with Technical Indicators** | Candlestick charts with VWAP, RSI, moving averages, Bollinger, MACD, etc. | Price history (Yahoo/Schwab) | No — charting library needed |
| 13 | **Multi-Leg Options Strategy Builder** | Visual builder for spreads, straddles, strangles, condors, butterflies with P&L graph | Schwab options chain + Greeks | Schwab provider needed |

#### Calendar Detail — Multiple Asset Class Calendars

| Calendar Type | Data Source | Provider Status | Events |
|---------------|------------|-----------------|--------|
| **Earnings** | Yahoo Finance, FMP | **Have both** | Earnings dates, EPS estimates, revenue estimates, surprise history |
| **Economic** | FRED, FMP | **Have both** | Fed meetings, GDP, CPI, jobs, ISM, retail sales |
| **Real Estate** | FRED | **Have it** | Housing Starts, Building Permits, Case-Shiller Index, Existing Home Sales, New Home Sales, Mortgage Rate changes, REIT earnings |
| **Crypto** | CoinMarketCal API, CoinGecko | **Need new provider** | Hard forks, halvings, exchange listings, protocol upgrades, token unlocks, partnerships |
| **IPO** | FMP | **Have it** | IPO dates, expected price range, sector |
| **Dividend** | Yahoo Finance, FMP | **Have both** | Ex-dividend dates, payment dates, yield, amount |
| **Options Expiration** | Calculated + CBOE | **Need to build** | Monthly/weekly/quarterly expiration dates, triple/quadruple witching |
| **Forex / Central Bank** | FRED, Finnhub | **Partial** | Rate decisions, central bank minutes, currency intervention events |

The calendar page would be a single unified view with filter chips/tabs for each type. Color-coded dots on each date. Click a date to see all events. Supports "my watchlist only" filter to show only events for symbols the user holds or watches.

#### Charting Library Options

| Library | License | Features | Notes |
|---------|---------|----------|-------|
| **TradingView Lightweight Charts** | Apache 2.0 | Candlestick, line, area, bar, histogram; crosshair; price scales; time scales | Free, open source, TradingView quality. No built-in indicators — we compute them ourselves. |
| **TradingView Widget** (embedded) | Free tier available | Full TradingView charts with indicators built-in | Requires internet, their branding, limited customization |
| **Recharts** | MIT | General charting | Not built for financial data — poor fit |
| **D3.js** | BSD | Unlimited customization | Massive effort to build financial charts from scratch |

**Recommendation for user decision:** TradingView Lightweight Charts (open source, free, no branding, we own the rendering) with custom indicator calculations in Rust.

---

### 5C: Skipped (Not Building)

| Feature | Reason |
|---------|--------|
| ThinkScript interpreter | Platform-locked to TOS, massive effort, no value outside TOS |
| RTD/DDE bridge | Windows-only, requires TOS desktop running |
| Market depth / Level 2 | Complex UI, limited data availability, niche use |
| Paper trading simulation engine | Schwab API has no paper trading, would need full order simulation |

---

### Implementation Order (Proposed)

| Phase | What | Why First |
|-------|------|-----------|
| **5A-1** | Account data model + switcher dropdown + store | Foundation — everything else depends on account scoping |
| **5A-2** | Per-account filtering on Positions, Holdings, Orders | Immediate user pain point (merged accounts) |
| **5A-3** | Per-account balance cards on Dashboard | Visual confirmation the switcher works |
| **5A-4** | Client sandbox account switcher | Mirror the same UX for client management |
| **5B-1** | Charting library integration + basic candlestick + indicators | High visual impact, enables probability cones and GEX later |
| **5B-2** | Stock screener/scanner | Uses existing providers (FMP + Yahoo) |
| **5B-3** | Multi-calendar system | Uses existing providers (Yahoo, FMP, FRED) + one new (crypto) |
| **5B-4** | Volatility models + probability cones | Calculation layer on top of price history + charts |
| **5B-5** | GEX visualization | Needs Schwab options chain data |
| **5B-6** | Unusual options activity | Needs Schwab options chain data |
| **5B-7** | Multi-leg options strategy builder | Most complex — needs options chain + Greeks + P&L graphing |

---

### Decisions (Resolved 2026-03-27)

1. **Charting library** — **TradingView Lightweight Charts** (Apache 2.0, open source, no branding). Indicators computed in Rust, sent to chart. `npm: lightweight-charts`
2. **Schwab API** — **Skip for now.** Stay with CSV import + Yahoo/FMP data. No live Schwab connection. Can add later.
3. **Crypto calendar** — **CoinMarketCal** (free tier: 20 req/hour, structured event data with significance scores)
4. **Account switcher position** — **Top bar, left side** (TOS-style). Always visible, dropdown shows all accounts + "All Accounts" option.
