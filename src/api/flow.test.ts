/**
 * The Flow editor's side of the command bridge.
 *
 * The pages are carried from OpenAlgo web unchanged, so they call these
 * functions exactly as the web's pages do. Upstream pinned the HTTP wire
 * (method, path under /flow/api, body, CSRF header); here the wire is Tauri's
 * `invoke`: this pins, call by call, the `flow_*` command and its arguments,
 * which part of the reply each function hands back, and how a refused
 * reply (`status` 400 and up) reads: as the server said it, the way the
 * pages' `error.message` and `failedRunLog(error)` expect.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))

import { failedRunLog } from '@/lib/desktopFlow'
import {
  activateWorkflow,
  createWorkflow,
  deactivateWorkflow,
  deleteWorkflow,
  disableWebhook,
  enableWebhook,
  executeWorkflow,
  exportWorkflow,
  FlowApiError,
  type FlowReply,
  flowQueryKeys,
  flowReply,
  getIndexSymbolsLotSizes,
  getOptionStrikes,
  getSymbolLotSizes,
  getWebhookInfo,
  getWorkflow,
  getWorkflowExecutions,
  importWorkflow,
  listWorkflows,
  regenerateWebhook,
  regenerateWebhookSecret,
  replaceWorkflow,
  updateWebhookAuthType,
  updateWorkflow,
  type WorkflowExportData,
} from './flow'

const ok = (body: unknown): FlowReply => ({ status: 200, body })
const refused = (status: number, body: unknown): FlowReply => ({ status, body })

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.invoke.mockResolvedValue(ok({}))
})

const EXPORTED: WorkflowExportData = {
  version: '1.0',
  name: 'Opening range',
  description: null,
  nodes: [{ id: 'n1', type: 'start', position: { x: 0, y: 0 }, data: {} }],
  edges: [],
  exported_at: '2026-10-30T09:15:00',
}

describe('the commands the editor invokes', () => {
  it.each([
    ['list workflows', () => listWorkflows(), 'flow_list_workflows', undefined],
    ['open a workflow', () => getWorkflow(7), 'flow_get_workflow', { id: 7 }],
    ['delete a workflow', () => deleteWorkflow(7), 'flow_delete_workflow', { id: 7 }],
    ['activate', () => activateWorkflow(7), 'flow_activate', { id: 7 }],
    ['deactivate', () => deactivateWorkflow(7), 'flow_deactivate', { id: 7 }],
    ['run now', () => executeWorkflow(7), 'flow_execute_workflow', { id: 7 }],
    [
      'execution history (default 20)',
      () => getWorkflowExecutions(7),
      'flow_list_executions',
      { id: 7, limit: 20 },
    ],
    [
      'execution history (a limit)',
      () => getWorkflowExecutions(7, 50),
      'flow_list_executions',
      { id: 7, limit: 50 },
    ],
    ['webhook settings', () => getWebhookInfo(7), 'flow_webhook_info', { id: 7 }],
    ['enable the webhook', () => enableWebhook(7), 'flow_webhook_enable', { id: 7 }],
    ['disable the webhook', () => disableWebhook(7), 'flow_webhook_disable', { id: 7 }],
    [
      'new webhook URL and secret',
      () => regenerateWebhook(7),
      'flow_webhook_regenerate',
      { id: 7 },
    ],
    [
      'new webhook secret only',
      () => regenerateWebhookSecret(7),
      'flow_webhook_regenerate_secret',
      { id: 7 },
    ],
    ['export', () => exportWorkflow(7), 'flow_export_workflow', { id: 7 }],
    ['index lot sizes', () => getIndexSymbolsLotSizes(), 'flow_index_symbols', undefined],
  ])('%s', async (_label, call, command, args) => {
    await call()
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.invoke).toHaveBeenCalledWith(command, args)
  })

  it('creates a workflow with its name, description and graph', async () => {
    await createWorkflow({ name: 'Opening range', description: 'AAPL', nodes: [], edges: [] })
    expect(mocks.invoke).toHaveBeenCalledWith('flow_create_workflow', {
      body: { name: 'Opening range', description: 'AAPL', nodes: [], edges: [] },
    })
  })

  it('saves the edited fields only, and hands back needs_reactivate', async () => {
    mocks.invoke.mockResolvedValue(ok({ id: 7, needs_reactivate: true }))
    const saved = await updateWorkflow(7, { name: 'Renamed' })
    expect(mocks.invoke).toHaveBeenCalledWith('flow_update_workflow', {
      id: 7,
      body: { name: 'Renamed' },
    })
    expect(saved.needs_reactivate).toBe(true)
  })

  it('switches the webhook auth type with an auth_type field', async () => {
    await updateWebhookAuthType(7, 'url')
    expect(mocks.invoke).toHaveBeenCalledWith('flow_webhook_set_auth_type', {
      id: 7,
      body: { auth_type: 'url' },
    })
  })

  it('imports the exported document as sent', async () => {
    mocks.invoke.mockResolvedValue(refused(201, { status: 'success', workflow_id: 42 }))
    await importWorkflow(EXPORTED)
    expect(mocks.invoke).toHaveBeenCalledWith('flow_import_workflow', { body: EXPORTED })
  })

  it('replaces a graph in place under the same id', async () => {
    await replaceWorkflow(7, EXPORTED)
    expect(mocks.invoke).toHaveBeenCalledWith('flow_replace_workflow', {
      id: 7,
      body: EXPORTED,
    })
  })

  it("asks for option strikes with the web's query keys, the unset ones left out", async () => {
    mocks.invoke.mockResolvedValue(
      ok({ status: 'success', data: { underlying: 'NIFTY', strikes: [] } })
    )
    await getOptionStrikes({ underlying: 'NIFTY', expiryType: 'current_week', optionType: 'CE' })
    expect(mocks.invoke).toHaveBeenCalledWith('flow_option_strikes', {
      params: { underlying: 'NIFTY', expiryType: 'current_week', optionType: 'CE' },
    })
  })

  it('looks lot sizes up in one batch', async () => {
    mocks.invoke.mockResolvedValue(
      ok({ status: 'success', lotSizes: { 'NFO:NIFTY28OCT25FUT': 75 } })
    )
    const refs = [
      { symbol: 'NIFTY28OCT25FUT', exchange: 'NFO' },
      { symbol: 'SBIN', exchange: 'NSE' },
    ]
    await getSymbolLotSizes(refs)
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.invoke).toHaveBeenCalledWith('flow_symbol_lotsizes', {
      body: { symbols: refs },
    })
  })
})

describe('what each function hands back', () => {
  it('an import answers with the new id and the imported name', async () => {
    mocks.invoke.mockResolvedValue(refused(201, { status: 'success', workflow_id: 42 }))
    expect(await importWorkflow(EXPORTED)).toEqual({ id: 42, name: 'Opening range' })
    expect(await importWorkflow({ ...EXPORTED, name: '' })).toEqual({
      id: 42,
      name: 'Imported Workflow',
    })
  })

  it("an import hands back the reply's notes and migrations, which the Flow page shows", async () => {
    const note = 'Schedule and time-window times are read as Eastern time in this app'
    mocks.invoke.mockResolvedValue(
      refused(201, {
        status: 'success',
        workflow_id: 42,
        notes: [note],
        migrations: ["positionCheck 'n3': operator 'gt' -> condition 'quantity_above'"],
      })
    )
    expect(await importWorkflow(EXPORTED)).toEqual({
      id: 42,
      name: 'Opening range',
      notes: [note],
      migrations: ["positionCheck 'n3': operator 'gt' -> condition 'quantity_above'"],
    })
  })

  it('index lot sizes unwrap data, and an empty reply is an empty list', async () => {
    mocks.invoke.mockResolvedValue(
      ok({ status: 'success', data: [{ value: 'NIFTY', label: 'NIFTY', lotSize: 75 }] })
    )
    expect(await getIndexSymbolsLotSizes()).toEqual([
      { value: 'NIFTY', label: 'NIFTY', lotSize: 75 },
    ])
    mocks.invoke.mockResolvedValue(ok({ status: 'success' }))
    expect(await getIndexSymbolsLotSizes()).toEqual([])
  })

  it('option strikes unwrap data', async () => {
    mocks.invoke.mockResolvedValue(
      ok({ status: 'success', data: { underlying: 'GOLDM', exchange: 'MCX', atm: 163000 } })
    )
    expect(await getOptionStrikes({ underlying: 'GOLDM' })).toEqual({
      underlying: 'GOLDM',
      exchange: 'MCX',
      atm: 163000,
    })
  })

  it('a lot size that is not a positive whole number reads as none', async () => {
    mocks.invoke.mockResolvedValue(
      ok({
        status: 'success',
        lotSizes: {
          'NFO:A': 75,
          'NFO:B': 0,
          'NFO:C': -5,
          'NFO:D': 1.5,
          'NFO:E': null,
          'NFO:F': '75',
        },
      })
    )
    expect(await getSymbolLotSizes([{ symbol: 'A', exchange: 'NFO' }])).toEqual({
      'NFO:A': 75,
      'NFO:B': null,
      'NFO:C': null,
      'NFO:D': null,
      'NFO:E': null,
      'NFO:F': null,
    })
  })

  it('an empty basket asks nothing, and a reply without lot sizes is empty', async () => {
    expect(await getSymbolLotSizes([])).toEqual({})
    expect(mocks.invoke).not.toHaveBeenCalled()
    mocks.invoke.mockResolvedValue(ok({ status: 'success' }))
    expect(await getSymbolLotSizes([{ symbol: 'A', exchange: 'NFO' }])).toEqual({})
  })

  it('the other calls hand back the reply body unchanged', async () => {
    const list = [{ id: 1, name: 'A', is_active: false }]
    mocks.invoke.mockResolvedValue(ok(list))
    expect(await listWorkflows()).toEqual(list)
    const activated = { status: 'success', message: 'Workflow activated', job_id: 'wf-1' }
    mocks.invoke.mockResolvedValue(ok(activated))
    expect(await activateWorkflow(1)).toEqual(activated)
  })

  it('hands back the whole Replace reply, migrations and reactivation included', async () => {
    const reply = {
      status: 'success',
      message: 'Workflow replaced',
      migrations: ["positionCheck 'n3': operator 'gt' -> condition 'quantity_above'"],
      needs_reactivate: true,
    }
    mocks.invoke.mockResolvedValue(ok(reply))
    expect(await replaceWorkflow(7, EXPORTED)).toEqual(reply)
  })
})

describe('query keys', () => {
  it('nest every workflow key under the workflow, so one invalidation covers it', () => {
    expect(flowQueryKeys.workflow(7)).toEqual(['flow', 'workflows', 7])
    expect(flowQueryKeys.executions(7)).toEqual(['flow', 'workflows', 7, 'executions'])
    expect(flowQueryKeys.webhook(7)).toEqual(['flow', 'workflows', 7, 'webhook'])
    expect(flowQueryKeys.indexSymbols()).toEqual(['flow', 'index-symbols'])
    expect(flowQueryKeys.optionStrikes('NIFTY', '28OCT25', 'CE')).toEqual([
      'flow',
      'option-strikes',
      'NIFTY',
      '28OCT25',
      'CE',
    ])
    expect(flowQueryKeys.symbolLotSizes(['NFO:A'])).toEqual(['flow', 'symbol-lotsizes', ['NFO:A']])
  })
})

describe('a refused reply reads as the server said it (V-150)', () => {
  it('a validator refusal rejects with the server message, which the pages toast', async () => {
    mocks.invoke.mockResolvedValue(
      refused(400, {
        status: 'error',
        error: 'Invalid workflow',
        message: 'Node "Place Order" is missing a symbol.',
      })
    )
    await expect(updateWorkflow(7, { name: 'x' })).rejects.toThrow(
      'Node "Place Order" is missing a symbol.'
    )
  })

  it('falls back to error, then detail, and keeps a plain-text body', async () => {
    mocks.invoke.mockResolvedValue(
      refused(409, { status: 'error', error: 'Workflow is already running' })
    )
    await expect(executeWorkflow(7)).rejects.toThrow('Workflow is already running')
    mocks.invoke.mockResolvedValue(refused(422, { detail: 'Only the detail says why.' }))
    await expect(listWorkflows()).rejects.toThrow('Only the detail says why.')
    mocks.invoke.mockResolvedValue(refused(500, 'The server could not finish that.'))
    await expect(listWorkflows()).rejects.toThrow('The server could not finish that.')
  })

  it('a refusal without a sentence names the status', async () => {
    mocks.invoke.mockResolvedValue(refused(500, {}))
    await expect(listWorkflows()).rejects.toThrow('Request failed with status 500')
  })

  it('carries the status and body, and the body under response.data for failedRunLog (F-156)', async () => {
    const logs = [
      { time: '2026-10-30T09:19:58', message: 'Getting quote for AAPL', level: 'info' },
      { time: '2026-10-30T09:19:59', message: 'Order rejected: margin shortfall', level: 'error' },
    ]
    mocks.invoke.mockResolvedValue(
      refused(500, { status: 'error', message: 'Workflow failed at Place Order', logs })
    )
    const error = await executeWorkflow(7).then(
      () => {
        throw new Error('resolved')
      },
      (e: unknown) => e
    )
    expect(error).toBeInstanceOf(FlowApiError)
    expect(error).toBeInstanceOf(Error)
    expect((error as FlowApiError).status).toBe(500)
    expect((error as FlowApiError).body).toMatchObject({ logs })
    const now = new Date('2026-10-30T09:20:00Z')
    expect(failedRunLog(error as Error, now)).toEqual([
      ...logs,
      { time: now.toISOString(), message: 'Workflow failed at Place Order', level: 'error' },
    ])
  })

  it('a 2xx other than 200 resolves (an import answers 201)', async () => {
    mocks.invoke.mockResolvedValue(refused(201, { status: 'success', workflow_id: 9 }))
    expect(await importWorkflow(EXPORTED)).toEqual({ id: 9, name: 'Opening range' })
  })

  it('a command that rejects (an AppError) reads as a 500 with its message', async () => {
    mocks.invoke.mockRejectedValue({ code: 'INTERNAL_ERROR', message: 'Database error: locked' })
    await expect(listWorkflows()).rejects.toThrow('Database error: locked')
    mocks.invoke.mockRejectedValue(new Error('invoke is not available'))
    const error = await flowReply(mocks.invoke('flow_list_workflows')).catch((e: unknown) => e)
    expect(error).toBeInstanceOf(FlowApiError)
    expect((error as FlowApiError).status).toBe(500)
    expect((error as FlowApiError).message).toBe('invoke is not available')
  })
})
