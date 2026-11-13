/**
 * The help panel of each Flow page (the app's shared HelpButton), in this
 * app's words: US venues, whole shares, Eastern time, the Orderbook.
 * Adapted from upstream OpenAlgo Desktop's user guide, chapter 21 (WP8
 * Phase B, docs/flow/PORT.md); Phase D's docs/flow/user-guide.md is the long
 * form.
 *
 * Phase B connected Run Now, the editor and the workflow store; Phase C the
 * triggers (schedules, price alerts, the public webhook, order updates from
 * an order-book poller), Telegram alerts and the Live-mode question on
 * Activate. Times are America/New_York, 12-hour.
 */

import type { HelpItem } from '@/components/HelpButton'

export const FLOW_INDEX_HELP: HelpItem[] = [
  {
    title: 'What Flow is',
    description:
      'A drag-and-drop canvas for trading automations: a trigger wired through data and conditions to orders and alerts. Workflows place orders through the app’s API order path, so analyzer mode applies: with it on, orders go to the sandbox. In Live mode a workflow places real orders with no confirmation per order. Active workflows run only while this app is open and you are signed in.',
  },
  {
    title: 'New Workflow',
    description:
      'Asks for a name and opens the editor. Each card shows a green dot while the workflow is active, its last run (completed, failed, running) and when it was last changed. Choose a card to open it.',
  },
  {
    title: 'Import',
    description:
      'Brings in a workflow file or pasted JSON, including one exported from OpenAlgo web. It becomes a new, inactive workflow named "<name> (imported)" with its own webhook address and secret. Its schedule and time-window times are read as Eastern time.',
  },
  {
    title: 'Activate and Deactivate',
    description:
      "Activate checks the whole workflow (symbols, quantities, wiring) and names the first problem; then its trigger starts watching. In Live mode it asks first, because a workflow's orders are not confirmed one by one; in analyzer mode it does not ask, and a workflow activated there places real orders if the app is switched to Live later. Deactivate stops the trigger. A card's menu also has Edit and Delete; Delete asks first, because the run history and the webhook address go with it.",
  },
  {
    title: 'Webhook',
    description:
      "A card's Webhook entry shows the workflow's webhook address and secret. The address is this computer's own until you enter a public address as the Ngrok/Public URL on Admin → Server Settings; the dialog says so.",
  },
  {
    title: 'Schedules',
    description:
      "A Start node's schedule runs every few seconds, minutes or hours, daily, weekly or once, on America/New_York time whatever this computer's time zone: every 1 minute runs at 2 seconds past each minute (10:01:02 AM ET, then 10:02:02 AM ET). With Market hours only on, a run that falls outside the session (9:30 AM to 4:00 PM ET, or the broker's own hours when it can tell) is skipped and leaves nothing in the run history. A one-time schedule deactivates its workflow after it runs. A run that fell due while the app was closed runs only if the app opens within a minute of it.",
  },
  {
    title: 'Price alerts',
    description:
      "A Price Alert asks your broker for its symbol's quote every 5 seconds while the workflow is active (a broker must be logged in) and runs the workflow when its condition is met, for example when AAPL crosses above 190. A one-time alert runs once and then deactivates its workflow; an every-time alert runs again on each new cross, one run at a time. An alert with an expiry deactivates its workflow when it ends.",
  },
  {
    title: 'Webhook alerts',
    description:
      'With Enable Webhook on (the card\'s Webhook entry), an alert is a POST to the address shown, http://127.0.0.1:5000/flow/webhook/<token> on this computer, with JSON that carries the secret, for example {"secret": "<secret>", "symbol": "AAPL"} (or ?secret= in the address with Secret in URL). Add /AAPL to the address to set the symbol; the run reads the fields as {{webhook.symbol}} and so on. It works only while Enable API Server is on (Admin → Server Settings; a change there applies after the app restarts), the workflow is active and you are signed in. Ten alerts with a wrong secret lock the webhook for 15 minutes; Regenerate URL & Secret unlocks it. An alert while the workflow is still running is refused as already running.',
  },
  {
    title: 'Order Update triggers',
    description:
      "An Order Update trigger runs the workflow when an order it watches (an order id, or a symbol, with a status such as complete) changes. This app reads the order book every 5 seconds while such a trigger is active (the sandbox's orders in analyzer mode), so a change is seen up to 5 seconds late, and orders already in the book when the trigger starts do not fire it. A one-time trigger runs once and then deactivates its workflow.",
  },
  {
    title: 'Telegram alerts',
    description:
      'A Telegram Alert node sends its message, with the time in Eastern time, to the chat set up on the Telegram page (profile menu → Telegram Bot). Until a bot token and chat id are saved there, the node stops with "Telegram is not set up".',
  },
  {
    title: 'Test in analyzer mode first',
    description:
      "With analyzer mode on (the header switch), every Flow order goes to the sandbox, and the Orderbook shows the sandbox's orders. Build and run a workflow there before you activate it live.",
  },
]

export const FLOW_EDITOR_HELP: HelpItem[] = [
  {
    title: 'Build the workflow',
    description:
      'Drag nodes from the palette on the left (Trigger, Action, Data, Stream, Logic, Util) onto the canvas, or choose one to add it in the middle. Connect the dot at the bottom of one node to the top of the next; a condition has a true and a false output, and the + on a connection inserts a node between two.',
  },
  {
    title: 'Node settings',
    description:
      'Select a node to edit it on the right. A new node starts on the US exchange, product CNC; pick a venue for the symbol (for example AAPL on NASDAQ) and a quantity in whole shares. The Indian exchanges are still in the list. A field with a { } switch takes a value from an earlier node.',
  },
  {
    title: 'Save',
    description:
      'Save (Ctrl+S, Cmd+S on a Mac) stores the workflow. "Unsaved" shows while there are changes, and leaving the editor with unsaved changes asks first.',
  },
  {
    title: 'Run Now and the Execution Log',
    description:
      "Run Now (the ... menu) saves the workflow and runs it once, active or not, outside market hours too. The Execution Log on the right shows every node's line: what it read, what it sent, what came back. A run that waits more than 10 seconds finishes in the background; its orders go out when the wait ends and appear in the order book.",
  },
  {
    title: 'Activate',
    description:
      "Checks the whole workflow and names the first problem; then the trigger starts watching. In Live mode it asks first, because a workflow's orders are not confirmed one by one. A workflow runs one run at a time; Deactivate stops the trigger. The header shows Live or Analyze mode and switches it: in analyzer mode the orders go to the sandbox, and the Orderbook shows them.",
  },
  {
    title: 'Variables',
    description:
      'Most nodes keep their result under an Output Variable, and a later node reads it as {{name.field}}: {{quote.data.ltp}} from a Get Quote node named quote, {{order.orderid}} from a Place Order node named order. An order node with a variable that does not resolve stops before anything is sent.',
  },
  {
    title: 'Times are Eastern',
    description:
      "Schedules, time windows, Wait Until and the {{time}} and {{date}} variables use America/New_York whatever this computer's time zone, and the regular session is 9:30 AM to 4:00 PM ET. A new Schedule runs at 9:35 AM ET, held to that session, and a new Time Window covers it. A workflow imported from OpenAlgo web is read the same way.",
  },
  {
    title: 'Export and Replace from JSON',
    description:
      "Export Workflow (the ... menu) downloads the last saved version, without the webhook address or secret. Replace from JSON swaps this workflow's nodes and connections for a file's and keeps its id, webhook address and history.",
  },
  {
    title: 'Keyboard shortcuts',
    description:
      'Delete or Backspace removes the selected node or connection, Escape clears the selection, scrolling zooms and dragging the canvas pans. ? opens the full list.',
  },
]

export const FLOW_SHORTCUTS_HELP: HelpItem[] = [
  {
    title: 'Open this page from the editor',
    description: "Press ? on the canvas, or choose Keyboard Shortcuts in the editor's ... menu.",
  },
  {
    title: 'Back to the editor',
    description:
      'The arrow at the top left goes back to the Flow page, where the workflow cards open the editor.',
  },
  {
    title: 'On a Mac',
    description: 'Ctrl is Cmd: Cmd+S saves.',
  },
]
