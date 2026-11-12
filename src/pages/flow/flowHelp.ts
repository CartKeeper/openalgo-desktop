/**
 * The help panel of each Flow page (the app's shared HelpButton), in this
 * app's words: US venues, whole shares, Eastern time, the Orderbook.
 * Adapted from upstream OpenAlgo Desktop's user guide, chapter 21 (WP8
 * Phase B, docs/flow/PORT.md); Phase D's docs/flow/user-guide.md is the long
 * form.
 *
 * Phase B connects Run Now, the editor and the workflow store. The public
 * webhook route, the order-update poller and Telegram arrive with Phase C,
 * which updates the "Not connected yet" item below when it lands.
 */

import type { HelpItem } from '@/components/HelpButton'

export const FLOW_INDEX_HELP: HelpItem[] = [
  {
    title: 'What Flow is',
    description:
      'A drag-and-drop canvas for trading automations: a trigger wired through data and conditions to orders and alerts. Workflows place orders through the same path as every other order in the app, so analyzer mode and the order checks apply. Active workflows run only while this app is open and you are signed in.',
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
      "Activate checks the whole workflow (symbols, quantities, wiring) and names the first problem; then its trigger starts watching. Deactivate stops the trigger. A card's menu also has Edit and Delete; Delete asks first, because the run history and the webhook address go with it.",
  },
  {
    title: 'Webhook',
    description:
      "A card's Webhook entry shows the workflow's webhook address and secret. The address is this computer's own until you enter a public address as the Ngrok/Public URL on Admin → Server Settings; the dialog says so.",
  },
  {
    title: 'Not connected yet',
    description:
      'Webhook alerts, Order Update triggers and Telegram alerts are not connected in this version: a webhook address does not receive alerts yet, an Order Update trigger does not fire, and a Telegram Alert node stops with "Telegram is not set up".',
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
      'Select a node to edit it on the right. US stocks take the symbol and the exchange (for example AAPL on NASDAQ) and a quantity in whole shares; the default product on a US exchange is CNC. A field with a { } switch takes a value from an earlier node.',
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
      'Checks the whole workflow and names the first problem; then the trigger starts watching. A workflow runs one run at a time; Deactivate stops the trigger. The header shows Live or Analyze mode and switches it: in analyzer mode the orders go to the sandbox, and the Orderbook shows them.',
  },
  {
    title: 'Variables',
    description:
      'Most nodes keep their result under an Output Variable, and a later node reads it as {{name.field}}: {{quote.data.ltp}} from a Get Quote node named quote, {{order.orderid}} from a Place Order node named order. An order node with a variable that does not resolve stops before anything is sent.',
  },
  {
    title: 'Times are Eastern',
    description:
      "Schedules, time windows, Wait Until and the {{time}} and {{date}} variables use America/New_York whatever this computer's time zone, and the regular session is 9:30 AM to 4:00 PM ET. A workflow imported from OpenAlgo web is read the same way.",
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
