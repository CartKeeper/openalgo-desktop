# Flow — the visual workflow builder

Adapted for this app from upstream OpenAlgo Desktop's user guide, chapter 21
("Flow Visual Builder"): US examples, Eastern time, no built-in tunnel,
Telegram as the only alert channel. The same how-tos, briefly, are in the
app itself: the **?** (Help) button on the Flow page, the editor and the
keyboard-shortcuts page.

## What Flow is

**Flow** is a drag-and-drop canvas for trading automations. You wire nodes
from a trigger (a schedule, a webhook alert, a price alert or an order
update) through data, indicators and conditions to orders and alerts, then
activate the workflow and it runs on its own while the app is open and you
are signed in.

```
trigger            data and conditions                  actions
Schedule  ---->  Get Quote, Indicator, Funds  ---->  Place Order, Smart Order,
Webhook          Price Check, Time Window,           Close Positions, Telegram,
Price Alert      AND / OR / NOT gates                HTTP Request ...
Order Update
```

Flow runs inside the app; a workflow is the same JSON as in OpenAlgo web, so a
workflow exported from OpenAlgo web imports here, and back. Flow places its
orders through the app's own API order path, so **analyzer mode** applies: with
it on, every Flow order goes to the sandbox. In **Live mode** a workflow places
real orders on the live account with no confirmation per order; that is why
Activate and Run Now ask first in Live mode. Build and test every workflow in
analyzer mode first.

Every time Flow reads or shows is **Eastern time** (America/New_York), whatever
your computer's time zone: schedules, time windows, Wait Until, the `{{time}}`
and `{{date}}` variables, the run history and the Execution Log. The regular
session is 9:30 AM to 4:00 PM ET.

## What you need

- The app open and signed in. A trigger that fires while you are signed out
  does not run the workflow; the run history records why.
- Your broker logged in, for live orders and market data (quotes, price
  alerts). With no broker logged in, a workflow still runs: in analyzer mode
  its orders go to the sandbox, and in Live mode each node that needs the
  broker fails with a message that no broker is logged in.
- No API key: a workflow uses the app's own credential.
- For TradingView, Chartink and other alerts from the internet: a public
  address in front of this computer (for example an ngrok tunnel you run),
  entered as the **Ngrok/Public URL** on **Admin → Server Settings**, and
  **Enable API Server** on there (a change applies after the app restarts).
- For the Telegram Alert node: a bot token and chat id saved on the
  **Telegram** page (profile menu → **Telegram Bot**).

## The Flow page

Open **Flow** in the sidebar (Strategy). The page lists your workflows as
cards:

- a green dot and a green edge when the workflow is active, grey when not;
- **Last:** and the last run's result (`completed`, `failed`, `running`), or
  **No executions**;
- when it was last changed, in Eastern time (`Nov 12, 2026, 2:35 PM ET`).

A click on a card **selects** it. Double-click a card, or use its **Open**
button, to open it in the editor (Enter opens the card in focus). The card's
**...** menu has **Edit**, **Webhook** (the webhook settings), **Run history**,
**Activate** or **Deactivate**, and **Delete**, which asks first.

Selecting several works as in any list of this app: **Select all** (it shows a
dash when some are selected), Shift+click for a range, Cmd+click (Ctrl+click)
to add or take away one, Cmd+A, Shift+arrows, Space, Escape to clear. With a
selection, the bar above the cards offers **Copy** (the export JSON of the
chosen workflows on the clipboard, several as one array), **Move** (shown
disabled: workflows have no folders) and **Delete** (asks first; an active
workflow is stopped before it goes, so no schedule, alert, watch or webhook
outlives it). The right-click menu adds **Open** (one workflow) and
**Duplicate** ("<name> (copy)", inactive, with its own webhook address,
switched off).

**New Workflow** asks for a name and opens the editor. **Import** brings in a
workflow file (below).

## The editor

- **The node palette** on the left has six tabs: Trigger, Action, Data,
  Stream, Logic and Util. Drag a node onto the canvas, or choose it to add it
  in the middle.
- **Connect** two nodes from the dot at the bottom of one to the top of the
  next. A condition has a true and a false output. The **+** on a connection
  inserts a node between the two.
- **Select a node** to edit it on the right. A new node starts on the `US`
  exchange with product `CNC` (whole shares on US venues); the Indian exchanges
  are still in the list. A field with a `{ }` switch takes a value from an
  earlier node, such as `{{webhook.symbol}}`.
- **Save** (Ctrl+S, Cmd+S on a Mac). **Unsaved** shows while there are changes,
  and leaving the editor with unsaved changes asks first.
- The header shows **Live** or **Analyze** mode and switches it.

The **...** menu has **Run Now**, **Show Logs** / **Hide Logs**, **Run
history**, **Export Workflow**, **Replace from JSON** and **Keyboard
Shortcuts**.

## The nodes

There are 61 node types, one palette tab per family.

- **Triggers**: Schedule, Webhook, Price Alert, Order Update. A workflow has
  exactly one.
- **Actions**: Place Order, Smart Order, Options Order, Multi-Leg, Basket
  Order, Split Order, Modify Order, Cancel Order, Cancel All, Close
  Positions. Quantities on US venues are whole shares.
- **Conditions**: Time Condition, Position Check, Fund Check, Price Check,
  Time Window, Var Condition, and the AND, OR and NOT gates. A condition that
  cannot get a trustworthy answer takes neither branch and the run is marked
  failed.
- **Data**: Get Quote, Multi Quotes, Get Depth, History, Indicator, Prior
  Period OHLC, Bar Offset, Open Position, Order Status, the order, trade and
  position books, Holdings, Funds, Symbol Info, Calendar and more.
- **Streams**: Subscribe LTP, Quote, Depth and Unsubscribe. This app has no
  live stream for Flow: each subscribe reads one quote through your broker.
- **Utilities**: Variable, Math, Log, Telegram, Delay (up to 5 minutes), Wait
  Until (an Eastern time, at most 30 minutes ahead), HTTP Request (public
  addresses only), Holidays, Timings, Group.

Not available in this app, and answered as a clear node error rather than a
crash: **WhatsApp** alerts, **Strategy P&L**, and the lookups that need an
Indian broker. With a US broker logged in, a **Margin** node says "Margin
needs an Indian broker's margin calculator: log in to Zerodha, Angel One or
Fyers...", and an option node on an index such as NIFTY says "NIFTY on NFO
needs an Indian broker: ...".

Put a Telegram Alert **after** the orders it reports: a failed alert stops the
branch below it.

## Variables

Most nodes keep their result under an **Output Variable**; a later node reads
it as `{{name.field}}`:

| You write | You get |
| --- | --- |
| `{{quote.data.ltp}}` | The last price from a Get Quote node named `quote` |
| `{{rsi.latest.value}}` | The latest RSI from an Indicator node named `rsi` |
| `{{order.orderid}}` | The order id from a Place Order node named `order` |
| `{{webhook.symbol}}` | The `symbol` field of the alert that started the run |
| `{{time}}`, `{{date}}` | The time and date in Eastern time |

A variable that does not resolve on an order node stops the node before
anything is sent; on other nodes it stays in the text as written, which shows
in the log.

## Triggers

### Schedule

Once, daily (on the days you tick), weekly, or every N seconds, minutes or
hours, on Eastern time. Every 1 minute runs at 2 seconds past each minute
(10:01:02 AM ET, then 10:02:02 AM ET). A new Schedule runs at 9:35 AM ET with
**Market hours only** on: a run outside the session (9:30 AM to 4:00 PM ET, or
the broker's own hours when it can tell) is skipped and leaves nothing in the
run history. A one-time schedule deactivates its workflow after it runs. A run
that fell due while the app was closed runs only if the app opens within a
minute of it.

### Webhook (TradingView, Chartink and your own programs)

Each workflow has its own webhook address and secret. Open the card's
**...** menu, **Webhook**:

1. Turn on **Enable Webhook**.
2. Choose **Secret in Payload** (a JSON `secret` field, for TradingView and
   your scripts) or **Secret in URL** (`?secret=...`, for services whose
   message you cannot write).
3. Copy the address into the alert service. On this computer it is
   `http://127.0.0.1:5000/flow/webhook/<token>`, which services on the
   internet cannot reach; with a public address entered as the Ngrok/Public
   URL, the dialog shows the public address instead. Add `/AAPL` to the
   address to set the symbol.
4. Activate the workflow.

A TradingView message for a workflow that reads `{{webhook.symbol}}`:

```json
{ "secret": "<your webhook secret>", "symbol": "{{ticker}}", "action": "BUY", "quantity": 1 }
```

To test from this computer:

```bash
curl -X POST "http://127.0.0.1:5000/flow/webhook/<token>" \
  -H "Content-Type: application/json" \
  -d '{"secret":"<secret>","symbol":"AAPL"}'
```

Things to know:

- The webhook must be on ("Webhook is disabled") and the workflow active
  ("Workflow is not active"), and you must be signed in.
- Ten wrong secrets within 10 minutes lock that workflow's webhook for 15
  minutes; **Regenerate URL & Secret** (or switching the webhook off and on)
  unlocks it.
- Limits, as upstream's: 100 alerts a minute from one sender's address, 100 a
  minute on one webhook, and 10 failed attempts (an unknown address or a wrong
  secret) a minute from one sender. Over a limit the answer is HTTP 429, "Rate
  limit exceeded. Please slow down your requests.", with Retry-After 60. Once a
  sender has spent its 10 failures, its wrong secrets are answered 429 too
  (and still count toward the lock), but an alert with the right secret goes
  on. Everything that comes through a tunnel on this computer arrives from
  127.0.0.1, so through a tunnel the 100-a-minute address limit is shared by
  every sender.
- An alert that arrives while the workflow is still running is answered
  "Workflow is already running".

### Price Alert

Choose the symbol, the condition (greater than, less than, crosses above,
crosses below, at the level, inside or outside a channel, moving up or down)
and the price. Your broker's quote is read every 5 seconds while the workflow
is active (a broker must be logged in). **Only Once** runs the workflow on the
first match and then deactivates it. **Every Time** keeps watching: a crossing
runs it on each new cross; a condition that can stay true, such as "greater
than 190", runs it when it **becomes** true and again only after a check where
it was false, never every 5 seconds while the price stays above.
**Expiration** stops the watch and deactivates the workflow.

### Order Update

Name an order id or a symbol, and the status to wait for (complete, rejected,
cancelled, open, trigger pending or any). This app reads the order book every
5 seconds while such a trigger is active (the sandbox's in analyzer mode), so
a change is seen up to 5 seconds late, and orders already in the book when the
trigger starts do not fire it. A watch never fires on an order its **own**
workflow placed, so a watch on AAPL whose run buys AAPL does not run itself
again on its own fill. The US venues count as one market: a watch on NASDAQ
hears an order your broker reports on `US`.

## Run Now, Activate and the run history

- **Run Now** (the editor's **...** menu) saves the workflow and runs it once,
  active or not, outside market hours too. In Live mode it asks first: "Run in
  Live mode? This run will place real orders on the live account without
  asking for each order." In analyzer mode it runs at once. The **Execution
  Log** on the right shows each node's line, times in Eastern 12-hour time.
- **Activate** checks the whole workflow and names the first problem, then the
  trigger starts watching. In Live mode it asks first. A workflow activated in
  analyzer mode places real orders if the app is switched to Live later.
- **Deactivate** stops the trigger.
- **Run history** (the card's **...** menu, or the editor's) lists the
  workflow's runs: when each started (`Nov 12, 2:35:02 PM ET`), its status, how
  long it took and its error. Double-click a run, press Enter, or use its
  **Open** button to read its log. The list refreshes every 10 seconds while it
  is open and when a Run Now ends. Select runs to **Copy** them as text or
  **Delete** them (asked first; permanent; a run still going cannot be
  deleted). The app keeps the newest 500 runs of each workflow, up to 30 days,
  and the screen shows the newest 200.

A workflow runs one run at a time. A run that waits (Delay, Wait Until) more
than 10 seconds is answered at once and finishes in the background; its orders
go out when the wait ends.

## Telegram alerts

Save the bot token and chat id on the **Telegram** page (profile menu →
**Telegram Bot**) and use **Send test message** there. A Telegram Alert node
then sends its message, with the time in Eastern 12-hour time, to that chat.
Until the settings are saved the node stops with "Telegram is not set up: add
the bot token and chat id on the Telegram page".

## Import and export

- **Export Workflow** (the editor's **...** menu) saves the last saved version
  as a `.json` file in Downloads, without the webhook address or secret.
- **Import** (the Flow page) takes a file or pasted JSON, checks it as
  Activate does, and makes a new, inactive workflow "<name> (imported)" with
  its own webhook address. A workflow from OpenAlgo web imports unchanged; its
  schedule and time-window times are read as Eastern time, and the import says
  so.
- **Replace from JSON** swaps this workflow's nodes and connections and keeps
  its id, webhook address, active state and history.

## Keyboard shortcuts

| Keys | Does |
| --- | --- |
| Ctrl+S (Cmd+S) | Save |
| Delete or Backspace | Delete the selected node or connection |
| Escape | Clear the selection |
| ? | Open the shortcuts page |
| Scroll | Zoom |
| Drag the canvas | Pan |

## Three starting points

**A TradingView alert to an order.** Webhook, then Place Order with Symbol
`{{webhook.symbol}}`, Exchange `US`, Action `{{webhook.action}}`, Quantity
`{{webhook.quantity}}`, output variable `order`, then Telegram "Placed
{{webhook.action}} {{webhook.symbol}}: {{order.orderid}}".

**A morning buy on a schedule.** Schedule (daily, 9:35 AM ET, weekdays, Market
hours only), Get Quote (AAPL, `US`, output `quote`), Price Check (AAPL, LTP
above 100), true branch to Place Order (AAPL, 1 share, MARKET, CNC), then
Telegram "Bought 1 AAPL share at about {{quote.data.ltp}}". The repository's
round-trip fixture `tests/fixtures/flow/us-schedule-quote-order.json` is this
workflow, ready to import.

**Square-off before the close.** Schedule (daily, 3:50 PM ET, weekdays), Close
Positions (no symbol: everything), Telegram "Squared off at {{time}}".

## Troubleshooting

| Problem | Check |
| --- | --- |
| Activate says a field is missing | The message names the node and field |
| The workflow never runs on schedule | Active? Market hours only outside 9:30 AM to 4:00 PM ET? Signed in? The app open at that time? |
| Nodes fail saying no broker is logged in | Log in to your broker |
| TradingView alerts do nothing | Enable API Server on, a public address entered and running, webhook on, workflow active, `secret` in the message |
| "Invalid webhook secret" | The alert's secret differs from the dialog's; ten lock the webhook for 15 minutes |
| HTTP 429 from the webhook | Over 100 alerts a minute, or 10 failed attempts a minute from that sender: wait a minute |
| A price alert runs only once although it is Every Time | A level condition runs again only after a check where it was false |
| An order went to the sandbox | Analyzer mode is on: the editor's header shows it |
| The Telegram node failed | Save the bot token and chat id on the Telegram page |
| A Margin or NIFTY options node fails | Those need an Indian broker logged in |
