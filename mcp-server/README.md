# OpenAlgo MCP Server

Exposes the OpenAlgo Desktop **local REST API** to Claude Desktop (or any MCP
client) as tools — so you can ask Claude to read your account and place/manage
orders in plain language.

It's a thin proxy: each tool POSTs to `http://127.0.0.1:5000/api/v1/*` with your
API key. The desktop app must be **running** (its API server listens on port
5000 by default).

---

## Read-only by default

By default this server exposes **observation tools only** — funds, holdings,
positions, quotes, order/trade book, and analyzer status. The AI can *see* your
account and the market; it **cannot place, modify, cancel, or close** anything.
The order-placement tools are **not even registered** unless you deliberately
turn them on.

This is on purpose: an AI placing live trades on your real money is a capability
the long-term plan is better off without — the returns that compound come from
allocation and savings rate, not from an AI's trades. Keep the eyes; skip the
hands.

### Enabling live trading (opt-in)

If you have a specific reason — e.g. a small, ring-fenced, **expendable**
speculation sleeve you've consciously chosen to run — set
`OPENALGO_MCP_ALLOW_LIVE_TRADING=true` in the MCP env. Only then are the
order tools (`place_order`, `place_smart_order`, `place_basket_order`,
`modify_order`, `cancel_order`, `cancel_all_orders`, `close_position`, and
`set_analyzer_mode`) registered.

**When enabled and the desktop app is on a LIVE broker, these tools move REAL
money.** Note: the MCP path does **not** carry the in-app order confirmations
(Gate A/B). Put the app in **Analyze (paper) mode** first if you want a sandbox.
The startup log line states which mode the server booted in.

---

## Setup

1. **Install dependencies**

   ```bash
   cd mcp-server
   npm install
   ```

   Requires Node.js 18+ (uses the built-in `fetch`).

2. **Get your API key** — in the desktop app, open the **API Key** page
   (Settings → API Key) and copy the key.

3. **Add it to Claude Desktop** — edit
   `~/Library/Application Support/Claude/claude_desktop_config.json`
   (macOS) and add:

   ```json
   {
     "mcpServers": {
       "openalgo": {
         "command": "node",
         "args": ["/Users/jasonborst/openalgo-desktop/mcp-server/index.mjs"],
         "env": {
           "OPENALGO_API_KEY": "PASTE_YOUR_API_KEY_HERE",
           "OPENALGO_BASE_URL": "http://127.0.0.1:5000",
           "OPENALGO_STRATEGY": "Claude"
         }
       }
     }
   }
   ```

4. **Restart Claude Desktop.** You should see the `openalgo` tools available.
   Make sure the desktop app is running first.

---

## Tools

**Read (always available, safe):**
`get_funds`, `get_holdings`, `get_positions`, `get_orderbook`, `get_tradebook`,
`get_quote`, `get_depth`, `get_order_status`, `get_open_position`,
`get_analyzer_status`

**Trade + mode control (⚠️ real money; registered ONLY when
`OPENALGO_MCP_ALLOW_LIVE_TRADING=true`):**
`place_order`, `place_smart_order`, `place_basket_order`, `modify_order`,
`cancel_order`, `cancel_all_orders`, `close_position`, `set_analyzer_mode`

With live trading disabled (the default), the tools above are not exposed at
all — the AI has no way to invoke them.

---

## Config (env vars)

| Var | Required | Default | Meaning |
|-----|----------|---------|---------|
| `OPENALGO_API_KEY` | yes | — | The app's API key |
| `OPENALGO_BASE_URL` | no | `http://127.0.0.1:5000` | Local API base URL |
| `OPENALGO_STRATEGY` | no | `Claude` | Strategy tag applied to orders |
| `OPENALGO_MCP_ALLOW_LIVE_TRADING` | no | `false` | `true` registers the order-placement tools. Default: read-only. |

---

## Notes / limitations

- The REST API takes **whole-share** integer quantities. Fractional / dollar
  (notional) / bracket orders are available in the **desktop UI**, not through
  this REST path — so orders placed via Claude are whole-share.
- `exchange` defaults to `NSE`; for US (Alpaca) symbols pass the right exchange
  (e.g. `NASDAQ`, `NYSE`) explicitly.
- The app must be running and its API server enabled for any tool to work.

## Test it without Claude

```bash
OPENALGO_API_KEY=YOUR_KEY node index.mjs
# (it waits on stdio; Ctrl-C to exit. A successful start prints a "connected" line to stderr.)
```
