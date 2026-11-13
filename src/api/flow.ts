// api/flow.ts
// Flow Workflow API module
//
// Carried from OpenAlgo web through upstream OpenAlgo Desktop (52580437) and
// adapted in WP8 Phase B: the web's `/flow/api/*` requests are this app's
// `flow_*` Tauri commands, which answer the status and JSON body the web's
// routes answered (`FlowReply`). The types, the function names and
// signatures, and the query keys are the web's, so the pages carried with
// them run unchanged. docs/flow/PORT.md records the change.

import { invoke } from '@tauri-apps/api/core'
import type { Edge, Node } from '@xyflow/react'

// =============================================================================
// Types
// =============================================================================

export interface Workflow {
  id: number
  name: string
  description: string | null
  nodes: Node[]
  edges: Edge[]
  is_active: boolean
  schedule_job_id: string | null
  webhook_token: string | null
  webhook_secret: string | null
  webhook_enabled: boolean
  webhook_auth_type: 'payload' | 'url'
  created_at: string
  updated_at: string
}

export interface WorkflowListItem {
  id: number
  name: string
  description: string | null
  is_active: boolean
  created_at: string
  updated_at: string
  last_execution_status: string | null
}

export interface WorkflowExecution {
  id: number
  workflow_id: number
  status: string
  started_at: string | null
  completed_at: string | null
  logs: ExecutionLog[]
  error: string | null
}

export interface ExecutionLog {
  time: string
  message: string
  level: string
}

export interface WebhookInfo {
  webhook_token: string
  webhook_secret: string
  webhook_enabled: boolean
  webhook_auth_type: 'payload' | 'url'
  webhook_url: string
  webhook_url_with_symbol: string
  webhook_url_with_secret: string | null
}

export interface WorkflowExportData {
  version: string
  name: string
  description: string | null
  nodes: Node[]
  edges: Edge[]
  exported_at: string
}

// =============================================================================
// The command bridge
// =============================================================================

/** What every `flow_*` command answers: the web's HTTP status and JSON body. */
export interface FlowReply {
  status: number
  body: unknown
}

/**
 * A refused request (status 400 and up), shaped as the pages read upstream's
 * axios error: `message` is the server's sentence, and `response.data` is the
 * body, where `failedRunLog` (lib/desktopFlow.ts) finds a failed run's log.
 */
export class FlowApiError extends Error {
  readonly status: number
  readonly body: unknown
  readonly response: { status: number; data: unknown }

  constructor(status: number, body: unknown) {
    super(messageOf(status, body))
    this.name = 'FlowApiError'
    this.status = status
    this.body = body
    this.response = { status, data: body }
  }
}

/**
 * The sentence a refusal carries, as upstream's web client read it (V-150):
 * `message`, then `error`, then `detail`, then a plain-text body.
 */
function messageOf(status: number, body: unknown): string {
  if (typeof body === 'string' && body.trim()) return body
  if (body && typeof body === 'object') {
    const fields = body as Record<string, unknown>
    for (const key of ['message', 'error', 'detail']) {
      const value = fields[key]
      if (typeof value === 'string' && value.trim()) return value
    }
  }
  return `Request failed with status ${status}`
}

/** What a rejected invoke said: an AppError `{ code, message }`, an Error, or text. */
function rejectionText(reason: unknown): string {
  if (reason instanceof Error) return reason.message
  if (typeof reason === 'string') return reason
  if (reason && typeof reason === 'object') {
    const message = (reason as { message?: unknown }).message
    if (typeof message === 'string') return message
  }
  return String(reason)
}

/**
 * The body of a command's reply when its status is below 400, a
 * `FlowApiError` otherwise. A command that rejects outright (an AppError from
 * the Rust side) is a 500 with its message.
 */
export async function flowReply<T>(call: Promise<FlowReply>): Promise<T> {
  let reply: FlowReply
  try {
    reply = await call
  } catch (reason) {
    throw new FlowApiError(500, { message: rejectionText(reason) })
  }
  if (reply.status >= 400) throw new FlowApiError(reply.status, reply.body)
  return reply.body as T
}

const flow = (command: string, args?: Record<string, unknown>) =>
  invoke<FlowReply>(command, args)

// =============================================================================
// API Functions
// =============================================================================

/**
 * List all workflows
 */
export async function listWorkflows(): Promise<WorkflowListItem[]> {
  return flowReply(flow('flow_list_workflows'))
}

/**
 * Get a single workflow by ID
 */
export async function getWorkflow(id: number): Promise<Workflow> {
  return flowReply(flow('flow_get_workflow', { id }))
}

/**
 * Create a new workflow
 */
export async function createWorkflow(data: {
  name: string
  description?: string
  nodes?: Node[]
  edges?: Edge[]
}): Promise<Workflow> {
  return flowReply(flow('flow_create_workflow', { body: data }))
}

/**
 * Update an existing workflow
 */
export async function updateWorkflow(
  id: number,
  data: {
    name?: string
    description?: string
    nodes?: Node[]
    edges?: Edge[]
  }
): Promise<Workflow & { needs_reactivate?: boolean }> {
  // needs_reactivate is set when the saved graph changed the trigger config of a
  // workflow that is currently active: the scheduler and monitors registered the
  // old configuration at activation time and will keep using it until the
  // workflow is deactivated and reactivated.
  return flowReply(flow('flow_update_workflow', { id, body: data }))
}

/**
 * Delete a workflow
 */
export async function deleteWorkflow(id: number): Promise<{ status: string; message: string }> {
  return flowReply(flow('flow_delete_workflow', { id }))
}

/**
 * Activate a workflow
 */
export async function activateWorkflow(id: number): Promise<{
  status: string
  message: string
  job_id?: string
  next_run?: string
}> {
  return flowReply(flow('flow_activate', { id }))
}

/**
 * Deactivate a workflow
 */
export async function deactivateWorkflow(id: number): Promise<{ status: string; message: string }> {
  return flowReply(flow('flow_deactivate', { id }))
}

/**
 * Execute a workflow manually
 */
export async function executeWorkflow(id: number): Promise<{
  status: string
  message: string
  execution_id?: number
  logs?: ExecutionLog[]
}> {
  return flowReply(flow('flow_execute_workflow', { id }))
}

/**
 * Get workflow execution history
 */
export async function getWorkflowExecutions(id: number, limit = 20): Promise<WorkflowExecution[]> {
  return flowReply(flow('flow_list_executions', { id, limit }))
}

/**
 * Get webhook configuration for a workflow
 */
export async function getWebhookInfo(id: number): Promise<WebhookInfo> {
  return flowReply(flow('flow_webhook_info', { id }))
}

/**
 * Enable webhook for a workflow
 */
export async function enableWebhook(
  id: number
): Promise<WebhookInfo & { status: string; message: string }> {
  return flowReply(flow('flow_webhook_enable', { id }))
}

/**
 * Disable webhook for a workflow
 */
export async function disableWebhook(id: number): Promise<{ status: string; message: string }> {
  return flowReply(flow('flow_webhook_disable', { id }))
}

/**
 * Regenerate webhook token
 */
export async function regenerateWebhook(id: number): Promise<{
  status: string
  message: string
  webhook_token: string
  webhook_secret: string
  webhook_url: string
  webhook_url_with_symbol: string
}> {
  return flowReply(flow('flow_webhook_regenerate', { id }))
}

/**
 * Regenerate webhook secret only
 */
export async function regenerateWebhookSecret(id: number): Promise<{
  status: string
  message: string
  webhook_secret: string
}> {
  return flowReply(flow('flow_webhook_regenerate_secret', { id }))
}

/**
 * Update webhook authentication type
 */
export async function updateWebhookAuthType(
  id: number,
  authType: 'payload' | 'url'
): Promise<{
  status: string
  message: string
  webhook_auth_type: 'payload' | 'url'
  webhook_url: string
  webhook_url_with_secret: string | null
}> {
  return flowReply(flow('flow_webhook_set_auth_type', { id, body: { auth_type: authType } }))
}

/**
 * Export workflow for sharing
 */
export async function exportWorkflow(id: number): Promise<WorkflowExportData> {
  return flowReply(flow('flow_export_workflow', { id }))
}

/** What an import hands back: the new workflow, and what the server said about it. */
export interface ImportedWorkflow {
  id: number
  name: string
  /** This app: sentences the trader must read, e.g. that times are read as Eastern time. */
  notes?: string[]
  /** Legacy fields that were upgraded on the way in, if any. */
  migrations?: string[]
}

/**
 * Import workflow from JSON
 * Backend returns { status, workflow_id } so we transform it to { id, name }
 * (and, in this app, the reply's notes and migrations when it has any).
 */
export async function importWorkflow(data: WorkflowExportData): Promise<ImportedWorkflow> {
  const response = await flowReply<{
    status: string
    workflow_id: number
    notes?: string[]
    migrations?: string[]
  }>(flow('flow_import_workflow', { body: data }))
  return {
    id: response.workflow_id,
    name: data.name || 'Imported Workflow',
    ...(response.notes?.length ? { notes: response.notes } : {}),
    ...(response.migrations?.length ? { migrations: response.migrations } : {}),
  }
}

/**
 * Replace an existing workflow's graph from JSON, in place.
 *
 * Import always creates a new workflow, which leaves a trail of copies and a
 * new webhook URL each time you iterate on a strategy as JSON. This keeps the
 * workflow's id, webhook token and active state and swaps only the graph.
 */
export interface ReplaceWorkflowResult {
  status: string
  workflow_id: number
  /** Legacy fields that were upgraded on the way in, if any. */
  migrations?: string[]
  /** True when the trigger changed on an active workflow, which needs a reactivate. */
  needs_reactivate?: boolean
  message?: string
}

export async function replaceWorkflow(
  id: number,
  data: WorkflowExportData
): Promise<ReplaceWorkflowResult> {
  return flowReply(flow('flow_replace_workflow', { id, body: data }))
}

// =============================================================================
// Index Symbols Types & API
// =============================================================================

export interface IndexSymbolInfo {
  value: string
  label: string
  exchange: string
  lotSize: number
}

/**
 * Get lot sizes for index symbols from master contract database
 * Returns dynamic lot sizes instead of hardcoded values
 */
export async function getIndexSymbolsLotSizes(): Promise<IndexSymbolInfo[]> {
  const response = await flowReply<{ data?: IndexSymbolInfo[] }>(flow('flow_index_symbols'))
  return response.data || []
}

export interface SymbolRef {
  symbol: string
  exchange: string
}

/** Keyed `EXCHANGE:SYMBOL` -> lot size, null where the contract has none. */
export type LotSizeMap = Record<string, number | null>

interface SymbolLotSizesResponse {
  status: 'success'
  lotSizes: LotSizeMap
}

/**
 * Lot sizes for a bounded set of exact contracts, so derivative quantities can
 * be entered in lots. Batched because a margin basket holds up to 50 legs and
 * one request per leg is pure overhead.
 *
 * A pair resolves to null - not a rejection - when the master contract has no
 * usable lot size, letting the caller fall back to units. A rejected promise
 * means the lookup itself failed, which is a different state the caller must
 * not present as "no lot size".
 */
export async function getSymbolLotSizes(symbols: SymbolRef[]): Promise<LotSizeMap> {
  if (!symbols.length) return {}
  const response = await flowReply<SymbolLotSizesResponse>(
    flow('flow_symbol_lotsizes', { body: { symbols } })
  )
  const raw = response?.lotSizes
  if (!raw || typeof raw !== 'object') return {}
  // Guard the values rather than trusting the payload: a non-positive or
  // non-integer lot size would silently multiply a basket by a wrong factor.
  const clean: LotSizeMap = {}
  for (const [key, value] of Object.entries(raw)) {
    clean[key] = Number.isInteger(value) && (value as number) > 0 ? (value as number) : null
  }
  return clean
}

export interface ListedStrike {
  strike: number
  /** The contract this strike resolves to, e.g. GOLDM28AUG26163000CE. */
  symbol: string | null
  /** ATM / ITMn / OTMn. Differs per side at the same strike. */
  label: string | null
}

export interface OptionStrikesResponse {
  underlying: string
  exchange: string
  /** The expiry the strikes belong to, in DDMMMYY. */
  expiry: string
  /** Every listed expiry for this underlying, nearest first. */
  expiries: string[]
  /** What each relative expiry type resolves to right now, e.g.
   * `{ current_week: '28AUG26' }`. Resolved server-side with the selector the
   * executor uses, so the panel can name the contract a leg will trade. */
  resolved: Record<string, string | null>
  optionType: 'CE' | 'PE'
  strikes: ListedStrike[]
  /** Nearest listed strike to the underlying LTP, or null when the quote failed. */
  atm: number | null
  underlyingLtp: number | null
  /** What the ATM was priced against - the index, or the near-month future on
   * an exchange with no spot. */
  underlyingSymbol: string | null
}

/**
 * Listed expiries and strikes for one underlying, for the manual leg builder.
 *
 * A leg that names an absolute strike and its own expiry should pick contracts
 * the exchange actually lists, so the builder offers the master contract's own
 * list rather than a free number and a typed date. `atm` is a convenience
 * marker and may be null - a strike list is still usable when the underlying
 * quote fails.
 */
export async function getOptionStrikes(params: {
  underlying: string
  expiry?: string
  expiryType?: string
  optionType?: 'CE' | 'PE'
}): Promise<OptionStrikesResponse> {
  // The web sent these as query parameters, the unset ones left out; the
  // command reads the same keys (flow::lookups::StrikeQuery::from_params).
  const query: Record<string, string> = {}
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) query[key] = String(value)
  }
  const response = await flowReply<{ data: OptionStrikesResponse }>(
    flow('flow_option_strikes', { params: query })
  )
  return response.data
}

// =============================================================================
// React Query Keys
// =============================================================================

export const flowQueryKeys = {
  all: ['flow'] as const,
  workflows: () => [...flowQueryKeys.all, 'workflows'] as const,
  workflow: (id: number) => [...flowQueryKeys.workflows(), id] as const,
  executions: (id: number) => [...flowQueryKeys.workflow(id), 'executions'] as const,
  webhook: (id: number) => [...flowQueryKeys.workflow(id), 'webhook'] as const,
  indexSymbols: () => [...flowQueryKeys.all, 'index-symbols'] as const,
  // Keyed on the exact triple the response describes, so switching expiry or
  // option type refetches instead of showing another expiry's strikes.
  optionStrikes: (underlying: string, expiry: string, optionType: string) =>
    [...flowQueryKeys.all, 'option-strikes', underlying, expiry, optionType] as const,
  // Keyed on the sorted pair list, so reopening an unchanged basket reuses its
  // entry. This key cannot give incremental reuse on its own - any change to
  // the set is a different key, and would re-request the whole basket - so the
  // caller keeps its own per-contract map and asks only for what is missing.
  symbolLotSizes: (keys: string[]) => [...flowQueryKeys.all, 'symbol-lotsizes', keys] as const,
}
