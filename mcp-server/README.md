# OpenAlgo MCP Server

Exposes the OpenAlgo Desktop **local REST API** to Claude Desktop (or any MCP
client) as tools — so you can ask Claude to read your account and place/manage
orders in plain language.

It's a thin proxy: each tool POSTs to `http://127.0.0.1:5000/api/v1/*` with your
API key. The desktop app must be **running** (its API server listens on port
5000 by default).

---

## ⚠️ Read this first — FULL ACCESS / real money

This server is configured for **full access**, including **live trading tools**
(`place_order`, `place_smart_order`, `place_basket_order`, `modify_order`,
`cancel_order`, `cancel_all_orders`, `close_position`).

**When the desktop app is connected to a LIVE broker, these tools move REAL
money.** Claude Desktop will be able to place and cancel real orders on your live
account. Treat the MCP connection like handing the keys over.

Safer practice: call `set_analyzer_mode` with `mode: true` to put the app in
**Analyze (paper) mode** first — then trading tools hit the fake-money sandbox,
not your live account. `get_analyzer_status` tells you which mode you're in.

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

**Read (safe):**
`get_funds`, `get_holdings`, `get_positions`, `get_orderbook`, `get_tradebook`,
`get_quote`, `get_depth`, `get_order_status`, `get_open_position`,
`get_analyzer_status`

**Trade (⚠️ real money on a live broker):**
`place_order`, `place_smart_order`, `place_basket_order`, `modify_order`,
`cancel_order`, `cancel_all_orders`, `close_position`

**Mode:**
`set_analyzer_mode` (switch to paper/live)

---

## Config (env vars)

| Var | Required | Default | Meaning |
|-----|----------|---------|---------|
| `OPENALGO_API_KEY` | yes | — | The app's API key |
| `OPENALGO_BASE_URL` | no | `http://127.0.0.1:5000` | Local API base URL |
| `OPENALGO_STRATEGY` | no | `Claude` | Strategy tag applied to orders |

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
