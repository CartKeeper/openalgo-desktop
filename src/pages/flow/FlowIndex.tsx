// pages/flow/FlowIndex.tsx
// Flow workflow list page

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Clock,
  Copy,
  Eye,
  EyeOff,
  History,
  Loader2,
  MoreVertical,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Trash2,
  Upload,
  Webhook,
  XCircle,
} from 'lucide-react'
import { type MouseEvent, useCallback, useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import {
  activateWorkflow,
  createWorkflow,
  deactivateWorkflow,
  deleteWorkflow,
  disableWebhook,
  enableWebhook,
  executeWorkflow,
  exportWorkflow,
  flowQueryKeys,
  getWorkflow,
  importWorkflow,
  listWorkflows,
  regenerateWebhook,
  updateWebhookAuthType,
  type WorkflowExportData,
  type WorkflowListItem,
} from '@/api/flow'
// Desktop: webhook URLs on the tunnel's address, and the note when they are local.
import { getWebhookInfoForTrader } from '@/api/flowDesktop'
import { HelpButton } from '@/components/HelpButton'
import {
  DeleteWorkflowDialog,
  RegenerateWebhookDialog,
} from '@/components/desktop/FlowConfirmDialogs'
import { useLiveActivation, useLiveRun } from '@/components/desktop/FlowLiveActivation'
import { FlowRunHistory } from '@/components/desktop/FlowRunHistory'
import {
  BUTTON_32,
  counted,
  DeleteManyDialog,
  type MenuEntry,
  SelectionActionBar,
  SelectionMenu,
} from '@/components/desktop/FlowSelectionParts'
import { FlowWebhookReachability } from '@/components/desktop/FlowWebhookReachability'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { useListSelection } from '@/hooks/useListSelection'
import { copyLater } from '@/lib/flow/clipboard'
import { listDate } from '@/lib/flow/listDate'
import { cn } from '@/lib/utils'
import { showToast } from '@/utils/toast'
import { FLOW_INDEX_HELP } from './flowHelp'

/** Desktop (WP8 Phase D): the selection's word for a workflow. */
const WORKFLOW_NOUN: [string, string] = ['workflow', 'workflows']
const MOVE_REASON = 'Workflows have no folders'

function StatusIcon({ status }: { status: string | null }) {
  switch (status) {
    case 'completed':
      return <CheckCircle2 className="h-4 w-4 text-green-500" />
    case 'failed':
      return <XCircle className="h-4 w-4 text-red-500" />
    case 'running':
      return <Loader2 className="h-4 w-4 animate-spin text-primary" />
    case 'pending':
      return <Clock className="h-4 w-4 text-muted-foreground" />
    default:
      return <AlertCircle className="h-4 w-4 text-muted-foreground" />
  }
}

/**
 * Desktop (WP8 Phase D, decision 6b): what a card needs from the page's
 * selection (CLAUDE.md's TABLE INTERACTION DESIGN SPEC). A click selects;
 * double-click and the Open button open the editor; the card's own buttons
 * stop their clicks, so they never select.
 */
interface CardSelection {
  selected: boolean
  focused: boolean
  onSelect: (e: MouseEvent) => void
  onCheck: (e: MouseEvent) => void
  onOpen: () => void
  onMenu: (e: MouseEvent) => void
}

function WorkflowCard({
  workflow,
  selection,
}: {
  workflow: WorkflowListItem
  selection: CardSelection
}) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [isWebhookOpen, setIsWebhookOpen] = useState(false)
  const [showSecret, setShowSecret] = useState(false)
  // Desktop: Delete and Regenerate are confirmed first (FlowConfirmDialogs).
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [confirmRegenerate, setConfirmRegenerate] = useState(false)
  // Desktop (WP8 Phase D): the workflow's run history, from the card's menu.
  const [historyOpen, setHistoryOpen] = useState(false)

  const activateMutation = useMutation({
    mutationFn: () => activateWorkflow(workflow.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows() })
      showToast.success('Workflow activated', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  // Desktop: in Live mode, activation asks first (FlowLiveActivation).
  const liveActivation = useLiveActivation(() => activateMutation.mutate())

  // Desktop (WP8 Phase D): Run Now from the run history's empty state, which
  // asks first in Live mode (FlowLiveActivation).
  const runMutation = useMutation({
    mutationFn: () => executeWorkflow(workflow.id),
    onSuccess: (data) => {
      if (data.status === 'success') {
        showToast.success(data.message || 'Execution completed', 'flow')
      } else if (data.status === 'accepted') {
        showToast.info(data.message, 'flow')
      } else {
        showToast.error(data.message || 'Execution failed', 'flow')
      }
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.executions(workflow.id) })
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows(), exact: true })
    },
  })
  const liveRun = useLiveRun(() => runMutation.mutate())

  const deactivateMutation = useMutation({
    mutationFn: () => deactivateWorkflow(workflow.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows() })
      showToast.success('Workflow deactivated', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: () => deleteWorkflow(workflow.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows() })
      showToast.success('Workflow deleted', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  // Webhook queries and mutations
  const webhookQuery = useQuery({
    queryKey: flowQueryKeys.webhook(workflow.id),
    // Desktop: on the tunnel's public address while the built-in tunnel is online.
    queryFn: () => getWebhookInfoForTrader(workflow.id),
    enabled: isWebhookOpen,
  })

  const enableWebhookMutation = useMutation({
    mutationFn: () => enableWebhook(workflow.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.webhook(workflow.id) })
      showToast.success('Webhook enabled', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const disableWebhookMutation = useMutation({
    mutationFn: () => disableWebhook(workflow.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.webhook(workflow.id) })
      showToast.success('Webhook disabled', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const regenerateWebhookMutation = useMutation({
    mutationFn: () => regenerateWebhook(workflow.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.webhook(workflow.id) })
      showToast.success('Webhook URL and secret regenerated', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const updateAuthTypeMutation = useMutation({
    mutationFn: (authType: 'payload' | 'url') => updateWebhookAuthType(workflow.id, authType),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.webhook(workflow.id) })
      showToast.success('Authentication type updated', 'flow')
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const copyToClipboard = (text: string, label: string) => {
    navigator.clipboard.writeText(text)
    showToast.success(`${label} copied to clipboard`, 'clipboard')
  }

  return (
    <>
      {/* Desktop (WP8 Phase D, decision 6b): a card is a selectable row of the
          page's grid. A click selects it (the table spec bans a click that
          opens); double-click, Enter on the list, or the Open button opens the
          editor. */}
      <Card
        id={`flow-card-${workflow.id}`}
        role="row"
        aria-label={`Workflow ${workflow.name}`}
        aria-selected={selection.selected}
        data-selectable="true"
        data-selected={selection.selected}
        data-item-id={String(workflow.id)}
        className={cn(
          'group cursor-pointer transition-colors duration-150',
          selection.selected
            ? 'bg-primary/8 shadow-[inset_3px_0_0_var(--color-primary)]'
            : 'hover:bg-primary/4',
          selection.focused && 'ring-2 ring-ring',
          workflow.is_active && 'border-green-500/30'
        )}
        onClick={(e) => {
          // The second click of a double-click is the open, not a toggle.
          if (e.detail > 1) return
          selection.onSelect(e)
        }}
        onDoubleClick={selection.onOpen}
        onContextMenu={selection.onMenu}
      >
        {/* The row's one cell: the card's content, laid out as the card lays out its parts. */}
        {/* biome-ignore lint/a11y/useSemanticElements: a cell of the cards' grid, not a table cell */}
        <div role="gridcell" tabIndex={-1} className="flex flex-col gap-6 outline-none">
          <CardHeader className="pb-3">
            <div className="flex items-start justify-between">
              <div className="flex min-w-0 items-center gap-3">
                <input
                  type="checkbox"
                  aria-label={`Select workflow ${workflow.name}`}
                  className="size-4 shrink-0 cursor-pointer accent-primary"
                  checked={selection.selected}
                  onChange={() => {}}
                  onClick={(e) => {
                    e.stopPropagation()
                    selection.onCheck(e)
                  }}
                  onDoubleClick={(e) => e.stopPropagation()}
                />
                <div
                  className={cn(
                    'h-2 w-2 rounded-full',
                    workflow.is_active ? 'bg-green-500' : 'bg-muted-foreground'
                  )}
                />
                <CardTitle className="truncate text-base">{workflow.name}</CardTitle>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  className={BUTTON_32}
                  aria-label={`Open workflow ${workflow.name}`}
                  onClick={(e) => {
                    e.stopPropagation()
                    selection.onOpen()
                  }}
                  onDoubleClick={(e) => e.stopPropagation()}
                >
                  Open
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger
                    asChild
                    onClick={(e) => e.stopPropagation()}
                    onDoubleClick={(e) => e.stopPropagation()}
                  >
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      aria-label="Workflow actions"
                    >
                      <MoreVertical className="h-4 w-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation()
                        navigate(`/flow/editor/${workflow.id}`)
                      }}
                    >
                      Edit
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation()
                        setIsWebhookOpen(true)
                      }}
                    >
                      <Webhook className="mr-2 h-4 w-4" />
                      Webhook
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation()
                        setHistoryOpen(true)
                      }}
                    >
                      <History className="mr-2 h-4 w-4" />
                      Run history
                    </DropdownMenuItem>
                    {workflow.is_active ? (
                      <DropdownMenuItem
                        onClick={(e) => {
                          e.stopPropagation()
                          deactivateMutation.mutate()
                        }}
                      >
                        <Pause className="mr-2 h-4 w-4" />
                        Deactivate
                      </DropdownMenuItem>
                    ) : (
                      <DropdownMenuItem
                        onClick={(e) => {
                          e.stopPropagation()
                          liveActivation.request()
                        }}
                      >
                        <Play className="mr-2 h-4 w-4" />
                        Activate
                      </DropdownMenuItem>
                    )}
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      className="text-destructive"
                      onClick={(e) => {
                        e.stopPropagation()
                        setConfirmDelete(true)
                      }}
                    >
                      <Trash2 className="mr-2 h-4 w-4" />
                      Delete
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </div>
            {workflow.description && (
              <CardDescription className="line-clamp-2">{workflow.description}</CardDescription>
            )}
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-muted-foreground">
                <StatusIcon status={workflow.last_execution_status} />
                <span>
                  {workflow.last_execution_status
                    ? `Last: ${workflow.last_execution_status}`
                    : 'No executions'}
                </span>
              </div>
              <span className="text-xs text-muted-foreground">{listDate(workflow.updated_at)}</span>
            </div>
          </CardContent>
        </div>
      </Card>

      {/* Webhook Settings Dialog */}
      <Dialog open={isWebhookOpen} onOpenChange={setIsWebhookOpen}>
        <DialogContent className="max-w-2xl max-h-[90vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Webhook className="h-5 w-5" />
              Webhook Settings
            </DialogTitle>
            <DialogDescription>Configure webhook for "{workflow.name}"</DialogDescription>
          </DialogHeader>

          {webhookQuery.isLoading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin" />
            </div>
          ) : webhookQuery.data ? (
            <div className="space-y-6 py-4 overflow-y-auto flex-1">
              {/* Enable/Disable Toggle */}
              <div className="flex items-center justify-between rounded-lg border p-4">
                <div>
                  <Label className="text-base">Enable Webhook</Label>
                  <p className="text-sm text-muted-foreground">
                    Allow external systems to trigger this workflow
                  </p>
                </div>
                <Switch
                  checked={webhookQuery.data.webhook_enabled}
                  onCheckedChange={(checked) => {
                    if (checked) {
                      enableWebhookMutation.mutate()
                    } else {
                      disableWebhookMutation.mutate()
                    }
                  }}
                  disabled={enableWebhookMutation.isPending || disableWebhookMutation.isPending}
                />
              </div>

              {/* Authentication Type */}
              <div className="space-y-3">
                <Label>Authentication Method</Label>
                <div className="grid grid-cols-2 gap-3">
                  <button
                    type="button"
                    className={cn(
                      'rounded-lg border p-4 text-left transition-all',
                      webhookQuery.data.webhook_auth_type === 'payload'
                        ? 'border-primary bg-primary/5'
                        : 'border-border hover:border-primary/50'
                    )}
                    onClick={() => updateAuthTypeMutation.mutate('payload')}
                    disabled={updateAuthTypeMutation.isPending}
                  >
                    <div className="font-medium">Secret in Payload</div>
                    <p className="text-xs text-muted-foreground mt-1">
                      For TradingView, custom scripts
                    </p>
                  </button>
                  <button
                    type="button"
                    className={cn(
                      'rounded-lg border p-4 text-left transition-all',
                      webhookQuery.data.webhook_auth_type === 'url'
                        ? 'border-primary bg-primary/5'
                        : 'border-border hover:border-primary/50'
                    )}
                    onClick={() => updateAuthTypeMutation.mutate('url')}
                    disabled={updateAuthTypeMutation.isPending}
                  >
                    <div className="font-medium">Secret in URL</div>
                    <p className="text-xs text-muted-foreground mt-1">
                      For Chartink, fixed-format services
                    </p>
                  </button>
                </div>
              </div>

              {/* Desktop: a URL on this computer's own address cannot receive alerts. */}
              <FlowWebhookReachability webhookUrl={webhookQuery.data.webhook_url} />

              {/* Webhook URL */}
              <div className="space-y-2">
                <Label>Webhook URL</Label>
                <div className="flex gap-2">
                  <Input
                    readOnly
                    value={webhookQuery.data.webhook_url}
                    className="font-mono text-sm"
                  />
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() => copyToClipboard(webhookQuery.data.webhook_url, 'URL')}
                    aria-label="Copy webhook URL"
                  >
                    <Copy className="h-4 w-4" />
                  </Button>
                </div>
              </div>

              {/* Webhook URL with Symbol */}
              <div className="space-y-2">
                <Label>Webhook URL (with symbol)</Label>
                <div className="flex gap-2">
                  <Input
                    readOnly
                    value={webhookQuery.data.webhook_url_with_symbol}
                    className="font-mono text-sm"
                  />
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() =>
                      copyToClipboard(webhookQuery.data.webhook_url_with_symbol, 'URL')
                    }
                    aria-label="Copy webhook URL with symbol"
                  >
                    <Copy className="h-4 w-4" />
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  Replace {'{symbol}'} with actual symbol like RELIANCE, INFY, etc.
                </p>
              </div>

              {/* Webhook Secret */}
              <div className="space-y-2">
                <Label>Webhook Secret</Label>
                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <Input
                      readOnly
                      type={showSecret ? 'text' : 'password'}
                      value={webhookQuery.data.webhook_secret}
                      className="font-mono text-sm pr-10"
                    />
                    <Button
                      variant="ghost"
                      size="icon"
                      className="absolute right-0 top-0 h-full"
                      onClick={() => setShowSecret(!showSecret)}
                      aria-label={showSecret ? 'Hide webhook secret' : 'Show webhook secret'}
                    >
                      {showSecret ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                    </Button>
                  </div>
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() => copyToClipboard(webhookQuery.data.webhook_secret, 'Secret')}
                    aria-label="Copy webhook secret"
                  >
                    <Copy className="h-4 w-4" />
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  {webhookQuery.data.webhook_auth_type === 'url'
                    ? 'Append this secret to URL as ?secret=...'
                    : 'Include this secret in your webhook payload as "secret" field'}
                </p>
              </div>

              {/* URL with Secret (for URL auth type) */}
              {webhookQuery.data.webhook_auth_type === 'url' &&
                webhookQuery.data.webhook_url_with_secret && (
                  <div className="space-y-2">
                    <Label>Webhook URL (with secret)</Label>
                    <div className="flex gap-2">
                      <Input
                        readOnly
                        value={
                          showSecret
                            ? webhookQuery.data.webhook_url_with_secret
                            : `${webhookQuery.data.webhook_url}?secret=........`
                        }
                        className="font-mono text-sm"
                      />
                      <Button
                        variant="outline"
                        size="icon"
                        onClick={() =>
                          copyToClipboard(
                            webhookQuery.data.webhook_url_with_secret!,
                            'URL with secret'
                          )
                        }
                        aria-label="Copy webhook URL with secret"
                      >
                        <Copy className="h-4 w-4" />
                      </Button>
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Use this URL for Chartink and other services with fixed payloads
                    </p>
                  </div>
                )}

              {/* Example Usage */}
              <div className="space-y-2">
                <Label>Example Usage</Label>
                <div className="rounded-lg bg-muted p-4">
                  {webhookQuery.data.webhook_auth_type === 'url' ? (
                    <>
                      <p className="text-xs text-muted-foreground mb-2">
                        For Chartink (secret in URL):
                      </p>
                      <pre className="text-xs font-mono overflow-x-auto">
                        {`POST ${webhookQuery.data.webhook_url}?secret=${showSecret ? webhookQuery.data.webhook_secret : '........'}

Chartink payload (sent as-is):
{
  "stocks": "RELIANCE,INFY,TCS",
  "trigger_prices": "2500.50,1800.25,3500.00",
  "triggered_at": "9:30 am",
  "scan_name": "My Scanner",
  "alert_name": "Buy Alert"
}`}
                      </pre>
                    </>
                  ) : (
                    <>
                      <p className="text-xs text-muted-foreground mb-2">
                        For TradingView (secret in payload):
                      </p>
                      <pre className="text-xs font-mono overflow-x-auto">
                        {`POST ${webhookQuery.data.webhook_url}

{
  "secret": "${showSecret ? webhookQuery.data.webhook_secret : '................'}",
  "symbol": "RELIANCE",
  "action": "BUY",
  "quantity": 10,
  "price": 2500.50
}`}
                      </pre>
                    </>
                  )}
                </div>
              </div>

              {/* Regenerate Button */}
              <div className="flex justify-end pt-2">
                <Button
                  variant="outline"
                  onClick={() => setConfirmRegenerate(true)}
                  disabled={regenerateWebhookMutation.isPending}
                >
                  {regenerateWebhookMutation.isPending ? (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <RefreshCw className="mr-2 h-4 w-4" />
                  )}
                  Regenerate URL & Secret
                </Button>
              </div>
            </div>
          ) : null}
        </DialogContent>
      </Dialog>

      {/* Desktop: confirmations the web does not ask for. */}
      {liveActivation.dialog}
      {liveRun.dialog}
      <FlowRunHistory
        workflowId={workflow.id}
        workflowName={workflow.name}
        open={historyOpen}
        onOpenChange={setHistoryOpen}
        onRunNow={() => liveRun.request()}
      />
      <DeleteWorkflowDialog
        name={workflow.name}
        isActive={workflow.is_active}
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        onConfirm={() => deleteMutation.mutate()}
      />
      <RegenerateWebhookDialog
        open={confirmRegenerate}
        onOpenChange={setConfirmRegenerate}
        onConfirm={() => regenerateWebhookMutation.mutate()}
      />
    </>
  )
}

export default function FlowIndex() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [isImportOpen, setIsImportOpen] = useState(false)
  const [newWorkflowName, setNewWorkflowName] = useState('')
  const [importJson, setImportJson] = useState('')
  const [importError, setImportError] = useState<string | null>(null)

  // Every way out of the import dialog leaves it empty for the next import.
  const handleImportOpenChange = (open: boolean) => {
    setIsImportOpen(open)
    if (!open) {
      setImportJson('')
      setImportError(null)
    }
  }

  const {
    data: workflows,
    isLoading,
    isError,
    error: listError,
    refetch,
  } = useQuery({
    queryKey: flowQueryKeys.workflows(),
    queryFn: listWorkflows,
  })

  // Desktop (WP8 Phase D, decision 6b): the cards carry CLAUDE.md's table
  // selection system (src/hooks/useListSelection.ts). Copy, Duplicate and
  // Delete act on the chosen workflows one call each (no command takes
  // several), with one toast for all.
  const list = workflows ?? []
  const ids = useMemo(() => (workflows ?? []).map((w) => String(w.id)), [workflows])
  const openEditor = useCallback((id: string) => navigate(`/flow/editor/${id}`), [navigate])
  const selection = useListSelection(ids, { noun: WORKFLOW_NOUN, onOpen: openEditor })
  const chosen = list.filter((w) => selection.isSelected(String(w.id)))
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const [askingDelete, setAskingDelete] = useState(false)
  const [busy, setBusy] = useState(false)

  const copyChosen = async () => {
    try {
      const picked = [...chosen]
      // The clipboard write is made now, inside the click, with the text to
      // come (lib/flow/clipboard): WebKit refuses one made after the awaits.
      await copyLater(
        (async () => {
          const exports = []
          for (const w of picked) exports.push(await exportWorkflow(w.id))
          return JSON.stringify(exports.length === 1 ? exports[0] : exports, null, 2)
        })()
      )
      showToast.success(`${counted(picked.length, WORKFLOW_NOUN)} copied`, 'clipboard')
    } catch (error) {
      showToast.error(
        error instanceof Error ? error.message : 'The workflows could not be copied',
        'clipboard'
      )
    }
  }

  const duplicateChosen = async () => {
    const made: string[] = []
    const failures: string[] = []
    for (const w of chosen) {
      try {
        const source = await getWorkflow(w.id)
        const copy = await createWorkflow({
          name: `${source.name} (copy)`,
          description: source.description ?? undefined,
          nodes: source.nodes,
          edges: source.edges,
        })
        made.push(copy.name)
      } catch (error) {
        failures.push(`${w.name}: ${error instanceof Error ? error.message : String(error)}`)
      }
    }
    queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows(), exact: true })
    if (failures.length > 0) {
      showToast.error(
        `${counted(made.length, WORKFLOW_NOUN)} duplicated; ${failures.length} could not be: ${failures.join('; ')}`,
        'flow'
      )
    } else if (made.length === 1) {
      showToast.success(`Workflow duplicated: ${made[0]}`, 'flow')
    } else {
      showToast.success(`${counted(made.length, WORKFLOW_NOUN)} duplicated`, 'flow')
    }
  }

  const deleteChosen = async () => {
    setBusy(true)
    let deleted = 0
    const failures: string[] = []
    for (const w of chosen) {
      try {
        await deleteWorkflow(w.id)
        deleted++
      } catch (error) {
        failures.push(`${w.name}: ${error instanceof Error ? error.message : String(error)}`)
      }
    }
    setBusy(false)
    setAskingDelete(false)
    selection.clear()
    queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows() })
    if (failures.length > 0) {
      showToast.error(
        `${counted(deleted, WORKFLOW_NOUN)} deleted; ${failures.length} could not be: ${failures.join('; ')}`,
        'flow'
      )
    } else {
      showToast.success(`${counted(deleted, WORKFLOW_NOUN)} deleted`, 'flow')
    }
  }

  const single = selection.count === 1 ? selection.selectedIds[0] : null
  const menuEntries: MenuEntry[] = [
    { label: 'Copy', onSelect: copyChosen },
    { label: 'Move to…', disabledReason: MOVE_REASON },
    { label: 'Delete', onSelect: () => setAskingDelete(true), danger: true },
    { label: 'Select All', onSelect: selection.selectAll, separatorBefore: true },
    { label: 'Deselect All', onSelect: selection.clear },
    single === null
      ? { label: 'Open', disabledReason: 'Choose one workflow to open', separatorBefore: true }
      : { label: 'Open', onSelect: () => openEditor(single), separatorBefore: true },
    { label: 'Duplicate', onSelect: duplicateChosen },
  ]

  const createMutation = useMutation({
    mutationFn: createWorkflow,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows() })
      setIsCreateOpen(false)
      setNewWorkflowName('')
      navigate(`/flow/editor/${data.id}`)
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const importMutation = useMutation({
    mutationFn: importWorkflow,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: flowQueryKeys.workflows() })
      setIsImportOpen(false)
      setImportJson('')
      setImportError(null)
      // This app (WP8): the server's notes on the import (times read as Eastern
      // time) are a warning the trader reads; upgraded fields are counted as
      // the editor's Replace from JSON counts them.
      const upgraded = data.migrations?.length
        ? ` ${data.migrations.length} legacy field(s) upgraded.`
        : ''
      if (data.notes?.length) {
        showToast.warning(
          `Workflow "${data.name}" imported. ${data.notes.join(' ')}${upgraded}`,
          'flow'
        )
      } else if (upgraded) {
        showToast.success(`Workflow "${data.name}" imported.${upgraded}`, 'flow')
      } else {
        showToast.success(`Workflow "${data.name}" imported`, 'flow')
      }
      navigate(`/flow/editor/${data.id}`)
    },
    onError: (error: Error) => {
      showToast.error(error.message, 'flow')
    },
  })

  const handleCreate = () => {
    if (!newWorkflowName.trim()) return
    createMutation.mutate({ name: newWorkflowName.trim() })
  }

  const handleImport = () => {
    setImportError(null)
    try {
      const parsed = JSON.parse(importJson) as WorkflowExportData
      // Validate basic structure
      if (!parsed.name || !Array.isArray(parsed.nodes) || !Array.isArray(parsed.edges)) {
        setImportError('Invalid workflow format. Must have name, nodes, and edges.')
        return
      }
      importMutation.mutate(parsed)
    } catch {
      setImportError('Invalid JSON format. Please check the workflow data.')
    }
  }

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) return

    const reader = new FileReader()
    reader.onload = (e) => {
      const content = e.target?.result as string
      setImportJson(content)
      setImportError(null)
    }
    reader.onerror = () => {
      setImportError('Failed to read file')
    }
    reader.readAsText(file)
  }

  if (isLoading) {
    return (
      <div className="flex h-[calc(100vh-3.5rem)] items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-6xl px-6 py-8">
      <HelpButton items={FLOW_INDEX_HELP} pageTitle="Flow" />
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Flow Editor</h1>
          <p className="text-muted-foreground">Create and manage your trading automations</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => setIsImportOpen(true)}>
            <Upload className="mr-2 h-4 w-4" />
            Import
          </Button>
          <Button onClick={() => setIsCreateOpen(true)}>
            <Plus className="mr-2 h-4 w-4" />
            New Workflow
          </Button>
        </div>
      </div>

      {isError ? (
        // Without this branch a failed fetch left workflows undefined and
        // isLoading false, so an expired session or a 500 rendered the cheerful
        // "No workflows yet" empty state - telling the user their workflows were
        // gone. Retries are capped at 1, so this is two requests away.
        <Card className="flex flex-col items-center justify-center py-16">
          <div className="mb-4 rounded-full bg-destructive/10 p-4">
            <AlertTriangle className="h-8 w-8 text-destructive" />
          </div>
          <h2 className="mb-2 text-lg font-medium">Could not load workflows</h2>
          <p className="mb-6 max-w-md text-center text-sm text-muted-foreground">
            {listError instanceof Error
              ? listError.message
              : 'Something went wrong while loading your workflows.'}
          </p>
          <Button onClick={() => refetch()}>Retry</Button>
        </Card>
      ) : workflows && workflows.length > 0 ? (
        <div className="space-y-4">
          <div className="flex min-h-12 items-center gap-4">
            <label className="flex h-8 shrink-0 cursor-pointer items-center gap-2 text-sm">
              <input
                type="checkbox"
                data-select-all="true"
                aria-label="Select all workflows"
                className="size-4 cursor-pointer accent-primary"
                checked={selection.allSelected}
                ref={(el) => {
                  if (el) el.indeterminate = selection.someSelected
                }}
                onChange={selection.toggleAll}
              />
              Select all
            </label>
            {selection.count > 0 && (
              <SelectionActionBar
                className="min-w-0 flex-1"
                count={selection.count}
                noun={WORKFLOW_NOUN}
                onClear={selection.clear}
                onCopy={copyChosen}
                moveDisabledReason={MOVE_REASON}
                onDelete={() => setAskingDelete(true)}
              />
            )}
          </div>
          <span aria-live="polite" aria-atomic="true" className="sr-only">
            {selection.announcement}
          </span>
          {/* biome-ignore lint/a11y/useSemanticElements: a grid of selectable cards (CLAUDE.md's table spec), not a data table */}
          <div
            role="grid"
            tabIndex={0}
            aria-label="Workflows"
            aria-multiselectable="true"
            aria-activedescendant={
              selection.focusedId === null ? undefined : `flow-card-${selection.focusedId}`
            }
            // Keys meant for the list only: never a key typed in a card's
            // controls or in a dialog a card opens (useListSelection).
            onKeyDown={selection.onKeyDown}
            className="grid gap-4 rounded-[12px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring sm:grid-cols-2 lg:grid-cols-3"
          >
            {workflows.map((workflow) => {
              const id = String(workflow.id)
              return (
                <WorkflowCard
                  key={workflow.id}
                  workflow={workflow}
                  selection={{
                    selected: selection.isSelected(id),
                    focused: selection.focusedId === id,
                    onSelect: (e) => selection.select(id, e),
                    onCheck: (e) => selection.check(id, e),
                    onOpen: () => openEditor(id),
                    onMenu: (e) => {
                      e.preventDefault()
                      selection.contextSelect(id)
                      setMenu({ x: e.clientX, y: e.clientY })
                    },
                  }}
                />
              )
            })}
          </div>
          {menu && (
            <SelectionMenu
              at={menu}
              label="Selected workflows"
              entries={menuEntries}
              onClose={() => setMenu(null)}
            />
          )}
          <DeleteManyDialog
            open={askingDelete}
            onOpenChange={setAskingDelete}
            count={selection.count}
            noun={WORKFLOW_NOUN}
            detail="Each goes with its schedule, its webhook address and its run history; an active one is stopped first."
            busy={busy}
            onConfirm={deleteChosen}
          />
        </div>
      ) : (
        <Card className="flex flex-col items-center justify-center py-16">
          <div className="mb-4 rounded-full bg-muted p-4">
            <Plus className="h-8 w-8 text-muted-foreground" />
          </div>
          <h2 className="mb-2 text-lg font-medium">No workflows yet</h2>
          <p className="mb-6 text-center text-sm text-muted-foreground">
            Create your first workflow to automate your trading
          </p>
          <Button onClick={() => setIsCreateOpen(true)}>
            <Plus className="mr-2 h-4 w-4" />
            Create Workflow
          </Button>
        </Card>
      )}

      <Dialog open={isCreateOpen} onOpenChange={setIsCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create New Workflow</DialogTitle>
            <DialogDescription>Give your workflow a name to get started</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="name">Workflow Name</Label>
              <Input
                id="name"
                placeholder="e.g., Morning Order Automation"
                value={newWorkflowName}
                onChange={(e) => setNewWorkflowName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
                autoFocus
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setIsCreateOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={!newWorkflowName.trim() || createMutation.isPending}
            >
              {createMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={isImportOpen} onOpenChange={handleImportOpenChange}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Import Workflow</DialogTitle>
            <DialogDescription>
              Import a workflow from a JSON file or paste the JSON data
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="file">Upload File</Label>
              <Input
                id="file"
                type="file"
                accept=".json"
                onChange={handleFileUpload}
                className="cursor-pointer"
              />
            </div>
            <div className="relative">
              <div className="absolute inset-0 flex items-center">
                <span className="w-full border-t" />
              </div>
              <div className="relative flex justify-center text-xs uppercase">
                <span className="bg-background px-2 text-muted-foreground">or paste JSON</span>
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="json">Workflow JSON</Label>
              <Textarea
                id="json"
                placeholder='{"name": "My Workflow", "nodes": [...], "edges": [...]}'
                value={importJson}
                onChange={(e) => {
                  setImportJson(e.target.value)
                  setImportError(null)
                }}
                className="h-40 font-mono text-sm"
              />
              {importError && <p className="text-sm text-destructive">{importError}</p>}
            </div>
          </div>
          <DialogFooter>
            {/* Desktop: Cancel clears the pasted text as Escape does (parity F-123). */}
            <Button variant="outline" onClick={() => handleImportOpenChange(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleImport}
              disabled={!importJson.trim() || importMutation.isPending}
            >
              {importMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Import
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
