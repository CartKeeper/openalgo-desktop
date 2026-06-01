#!/usr/bin/env node
/**
 * OpenAlgo MCP server
 *
 * Exposes the OpenAlgo Desktop local REST API (http://127.0.0.1:5000/api/v1/*)
 * as MCP tools so Claude Desktop (or any MCP client) can read account state and
 * place/manage orders.
 *
 * ⚠️ FULL ACCESS: this server includes LIVE trading tools. When the desktop app
 * is connected to a live broker, the place_/modify_/cancel_/close_ tools move
 * REAL money. See README.md.
 *
 * Config (env):
 *   OPENALGO_API_KEY   (required) — the app's API key (Settings → API Key page)
 *   OPENALGO_BASE_URL  (optional) — default http://127.0.0.1:5000
 *   OPENALGO_STRATEGY  (optional) — strategy tag on orders, default "Claude"
 */

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import { z } from 'zod'

const BASE_URL = (process.env.OPENALGO_BASE_URL ?? 'http://127.0.0.1:5000').replace(/\/$/, '')
const API_KEY = process.env.OPENALGO_API_KEY
const STRATEGY = process.env.OPENALGO_STRATEGY ?? 'Claude'

if (!API_KEY) {
  console.error('[openalgo-mcp] OPENALGO_API_KEY is required. Set it in your MCP client config.')
  process.exit(1)
}

/** POST {apikey, ...body} to /api/v1/<path> and return parsed JSON (or text). */
async function callApi(path, body = {}) {
  let res
  try {
    res = await fetch(`${BASE_URL}/api/v1/${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ apikey: API_KEY, ...body }),
    })
  } catch (e) {
    throw new Error(
      `Could not reach OpenAlgo at ${BASE_URL}. Is the desktop app running with the API server enabled? (${e.message})`
    )
  }
  const text = await res.text()
  let data
  try {
    data = JSON.parse(text)
  } catch {
    data = text
  }
  if (!res.ok) {
    const msg = typeof data === 'string' ? data : JSON.stringify(data)
    throw new Error(`${path} failed (HTTP ${res.status}): ${msg}`)
  }
  return data
}

function ok(data) {
  return {
    content: [{ type: 'text', text: typeof data === 'string' ? data : JSON.stringify(data, null, 2) }],
  }
}
function fail(e) {
  return { content: [{ type: 'text', text: `Error: ${e?.message ?? e}` }], isError: true }
}

const server = new McpServer({ name: 'openalgo', version: '1.0.0' })

// Common enums / shapes
const action = z.enum(['BUY', 'SELL']).describe('Order side')
const pricetype = z
  .enum(['MARKET', 'LIMIT', 'SL', 'SL-M'])
  .default('MARKET')
  .describe('Order type')
const product = z.enum(['CNC', 'MIS', 'NRML']).default('CNC').describe('Product type')

// ---------------------------------------------------------------------------
// READ tools
// ---------------------------------------------------------------------------

server.tool('get_funds', 'Get account funds, cash, and buying power.', {}, async () => {
  try {
    return ok(await callApi('funds'))
  } catch (e) {
    return fail(e)
  }
})

server.tool('get_holdings', 'Get long-term holdings with P&L.', {}, async () => {
  try {
    return ok(await callApi('holdings'))
  } catch (e) {
    return fail(e)
  }
})

server.tool('get_positions', 'Get the current open positions (position book).', {}, async () => {
  try {
    return ok(await callApi('positionbook'))
  } catch (e) {
    return fail(e)
  }
})

server.tool('get_orderbook', 'Get all orders (order book).', {}, async () => {
  try {
    return ok(await callApi('orderbook'))
  } catch (e) {
    return fail(e)
  }
})

server.tool('get_tradebook', 'Get executed trades (trade book).', {}, async () => {
  try {
    return ok(await callApi('tradebook'))
  } catch (e) {
    return fail(e)
  }
})

server.tool(
  'get_quote',
  'Get a real-time quote for a symbol.',
  { symbol: z.string(), exchange: z.string().default('NSE') },
  async ({ symbol, exchange }) => {
    try {
      return ok(await callApi('quotes', { symbol, exchange }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'get_depth',
  'Get the market depth (bid/ask ladder) for a symbol.',
  { symbol: z.string(), exchange: z.string().default('NSE') },
  async ({ symbol, exchange }) => {
    try {
      return ok(await callApi('depth', { symbol, exchange }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'get_order_status',
  'Get the status of a specific order by id.',
  { orderid: z.string() },
  async ({ orderid }) => {
    try {
      return ok(await callApi('orderstatus', { strategy: STRATEGY, orderid }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'get_open_position',
  'Get the open position for a specific symbol.',
  { symbol: z.string(), exchange: z.string().default('NSE'), product: product },
  async ({ symbol, exchange, product }) => {
    try {
      return ok(await callApi('openposition', { strategy: STRATEGY, symbol, exchange, product }))
    } catch (e) {
      return fail(e)
    }
  }
)

// ---------------------------------------------------------------------------
// TRADE tools  ⚠️ live money when the app is on a live broker
// ---------------------------------------------------------------------------

server.tool(
  'place_order',
  '⚠️ Place an order. REAL money if the app is connected to a live broker.',
  {
    symbol: z.string(),
    exchange: z.string().default('NSE'),
    action,
    quantity: z.number().int().positive(),
    pricetype,
    product,
    price: z.number().default(0).describe('Limit price (for LIMIT/SL)'),
    trigger_price: z.number().default(0).describe('Trigger price (for SL/SL-M)'),
    disclosed_quantity: z.number().int().default(0),
  },
  async (a) => {
    try {
      return ok(await callApi('placeorder', { strategy: STRATEGY, ...a }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'place_smart_order',
  '⚠️ Place a smart order that reconciles to a target position_size (0 = flat). REAL money on a live broker.',
  {
    symbol: z.string(),
    exchange: z.string().default('NSE'),
    action,
    quantity: z.number().int(),
    position_size: z.number().int().describe('Desired net position after the order'),
    pricetype,
    product,
    price: z.number().default(0),
    trigger_price: z.number().default(0),
    disclosed_quantity: z.number().int().default(0),
  },
  async (a) => {
    try {
      return ok(await callApi('placesmartorder', { strategy: STRATEGY, ...a }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'place_basket_order',
  '⚠️ Place multiple orders at once. REAL money on a live broker.',
  {
    orders: z
      .array(
        z.object({
          symbol: z.string(),
          exchange: z.string().default('NSE'),
          action,
          quantity: z.number().int().positive(),
          pricetype,
          product,
          price: z.number().default(0),
          trigger_price: z.number().default(0),
        })
      )
      .describe('List of orders to place'),
  },
  async ({ orders }) => {
    try {
      return ok(await callApi('basketorder', { strategy: STRATEGY, orders }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'modify_order',
  '⚠️ Modify an existing open order. REAL money on a live broker.',
  {
    orderid: z.string(),
    symbol: z.string(),
    exchange: z.string().default('NSE'),
    action,
    quantity: z.number().int().positive(),
    pricetype,
    product,
    price: z.number().default(0),
    trigger_price: z.number().default(0),
    disclosed_quantity: z.number().int().default(0),
  },
  async (a) => {
    try {
      return ok(await callApi('modifyorder', { strategy: STRATEGY, ...a }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'cancel_order',
  '⚠️ Cancel a specific open order by id. REAL money on a live broker.',
  { orderid: z.string() },
  async ({ orderid }) => {
    try {
      return ok(await callApi('cancelorder', { strategy: STRATEGY, orderid }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'cancel_all_orders',
  '⚠️ Cancel ALL open orders. REAL money on a live broker.',
  {},
  async () => {
    try {
      return ok(await callApi('cancelallorder', { strategy: STRATEGY }))
    } catch (e) {
      return fail(e)
    }
  }
)

server.tool(
  'close_position',
  '⚠️ Close ALL open positions. REAL money on a live broker.',
  {},
  async () => {
    try {
      return ok(await callApi('closeposition', { strategy: STRATEGY }))
    } catch (e) {
      return fail(e)
    }
  }
)

// ---------------------------------------------------------------------------
// Analyzer (paper) mode toggle — useful so Claude can switch to safe practice
// ---------------------------------------------------------------------------

server.tool('get_analyzer_status', 'Check whether the app is in Analyze (paper) mode or live.', {}, async () => {
  try {
    return ok(await callApi('analyzer'))
  } catch (e) {
    return fail(e)
  }
})

server.tool(
  'set_analyzer_mode',
  'Switch the app between Analyze (paper, mode=true) and live (mode=false). Use mode=true to make trading tools safe (sandbox).',
  { mode: z.boolean().describe('true = Analyze/paper, false = live') },
  async ({ mode }) => {
    try {
      return ok(await callApi('analyzer/toggle', { mode }))
    } catch (e) {
      return fail(e)
    }
  }
)

const transport = new StdioServerTransport()
await server.connect(transport)
console.error('[openalgo-mcp] connected over stdio, proxying ' + BASE_URL)
