import { invoke } from '@tauri-apps/api/core'
import {
  ArrowLeft,
  BarChart3,
  Copy,
  GitBranch,
  HelpCircle,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { useClientScenarioStore } from '@/stores/clientScenarioStore'
import type { Client } from '@/types/clients'
import { ACCOUNT_TYPES } from '@/types/clients'
import type { ClientScenario } from '@/types/client-scenarios'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err)
    return (err as { message: string }).message
  return String(err)
}

export default function ScenariosList() {
  const { id } = useParams<{ id: string }>()
  const clientId = Number(id)
  const navigate = useNavigate()

  const {
    scenarios,
    isLoading,
    loadScenarios,
    createScenario,
    deleteScenario,
    cloneScenario,
    syncBaseline,
  } = useClientScenarioStore()

  const [client, setClient] = useState<Client | null>(null)
  const [showCreateDialog, setShowCreateDialog] = useState(false)
  const [showCloneDialog, setShowCloneDialog] = useState(false)
  const [cloneSource, setCloneSource] = useState<ClientScenario | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<ClientScenario | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isSyncing, setIsSyncing] = useState(false)
  const [createForm, setCreateForm] = useState({ name: '', description: '' })
  const [cloneForm, setCloneForm] = useState({ name: '', description: '' })
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())

  const toggleSelection = (scenarioId: number, e: React.MouseEvent) => {
    e.stopPropagation()
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(scenarioId)) {
        next.delete(scenarioId)
      } else {
        if (next.size >= 3) {
          toast.error('You can compare up to 3 scenarios')
          return prev
        }
        next.add(scenarioId)
      }
      return next
    })
  }

  const handleCompare = () => {
    if (selectedIds.size < 2) {
      toast.error('Select at least 2 scenarios to compare')
      return
    }
    const ids = [...selectedIds].join(',')
    navigate(`/clients/${clientId}/scenarios/compare?scenarios=${ids}`)
  }

  const loadClient = useCallback(async () => {
    try {
      const data = await invoke<Client>('get_client', { id: clientId })
      setClient(data)
    } catch (err) {
      toast.error(`Failed to load client: ${errMsg(err)}`)
    }
  }, [clientId])

  useEffect(() => {
    loadClient()
    loadScenarios(clientId)
  }, [clientId, loadClient, loadScenarios])

  const handleCreate = async () => {
    if (!createForm.name.trim()) {
      toast.error('Scenario name is required')
      return
    }
    setIsSubmitting(true)
    try {
      const scenario = await createScenario(
        clientId,
        createForm.name.trim(),
        createForm.description.trim() || undefined
      )
      toast.success(`Created scenario: ${scenario.name}`)
      setShowCreateDialog(false)
      setCreateForm({ name: '', description: '' })
    } catch (err) {
      toast.error(`Failed to create scenario: ${errMsg(err)}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleClone = async () => {
    if (!cloneSource?.id || !cloneForm.name.trim()) {
      toast.error('Scenario name is required')
      return
    }
    setIsSubmitting(true)
    try {
      const cloned = await cloneScenario(
        cloneSource.id,
        clientId,
        cloneForm.name.trim(),
        cloneForm.description.trim() || undefined
      )
      toast.success(`Cloned scenario: ${cloned.name}`)
      setShowCloneDialog(false)
      setCloneSource(null)
      setCloneForm({ name: '', description: '' })
    } catch (err) {
      toast.error(`Failed to clone scenario: ${errMsg(err)}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleDelete = async () => {
    if (!deleteTarget?.id) return
    try {
      await deleteScenario(deleteTarget.id)
      toast.success(`Deleted scenario: ${deleteTarget.name}`)
      setDeleteTarget(null)
    } catch (err) {
      toast.error(`Failed to delete scenario: ${errMsg(err)}`)
    }
  }

  const handleSyncBaseline = async () => {
    setIsSyncing(true)
    try {
      await syncBaseline(clientId)
      toast.success('Baseline scenarios synced from trade history')
    } catch (err) {
      toast.error(`Failed to sync baseline: ${errMsg(err)}`)
    } finally {
      setIsSyncing(false)
    }
  }

  const openCloneDialog = (scenario: ClientScenario) => {
    setCloneSource(scenario)
    setCloneForm({
      name: `${scenario.name} (Copy)`,
      description: scenario.description || '',
    })
    setShowCloneDialog(true)
  }

  if (isLoading && scenarios.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="space-y-1">
          <Link
            to={`/clients/${clientId}`}
            className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
          >
            <ArrowLeft className="h-4 w-4" /> Back to {client?.name || 'Client'}
          </Link>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold tracking-tight">
              Sandbox Scenarios
            </h1>
            <Popover>
              <PopoverTrigger asChild>
                <Button variant="ghost" size="icon" className="h-8 w-8 rounded-full">
                  <HelpCircle className="h-4 w-4 text-muted-foreground" />
                </Button>
              </PopoverTrigger>
              <PopoverContent className="w-80 text-sm space-y-3" align="start">
                <p className="font-semibold">How Sandbox Scenarios Work</p>
                <div className="space-y-2 text-muted-foreground">
                  <p>
                    <span className="font-medium text-foreground">Sync Baseline</span> — Creates one baseline scenario per account type from your imported trade history. If you have an Individual and IRA account, you'll get a separate baseline for each.
                  </p>
                  <p>
                    <span className="font-medium text-foreground">New Scenario</span> — Creates an empty scenario you can manually populate with positions.
                  </p>
                  <p>
                    <span className="font-medium text-foreground">Clone</span> — Duplicates an existing scenario (including all positions) so you can model changes without affecting the original.
                  </p>
                  <p>
                    <span className="font-medium text-foreground">Compare</span> — Select 2–3 scenarios via the checkboxes, then click Compare to view them side by side.
                  </p>
                </div>
              </PopoverContent>
            </Popover>
          </div>
          <p className="text-sm text-muted-foreground">
            What-if portfolio modeling for {client?.name}
          </p>
        </div>
        <div className="flex gap-2">
          {selectedIds.size >= 2 && (
            <Button variant="outline" size="sm" onClick={handleCompare}>
              <BarChart3 className="h-4 w-4 mr-2" />
              Compare ({selectedIds.size})
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={handleSyncBaseline}
            disabled={isSyncing}
          >
            {isSyncing ? (
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            ) : (
              <RefreshCw className="h-4 w-4 mr-2" />
            )}
            Sync Baseline
          </Button>
          <Button size="sm" onClick={() => setShowCreateDialog(true)}>
            <Plus className="h-4 w-4 mr-2" />
            New Scenario
          </Button>
        </div>
      </div>

      {/* Scenarios Grid */}
      {scenarios.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <GitBranch className="h-12 w-12 text-muted-foreground mb-4" />
            <p className="text-lg font-semibold mb-1">No scenarios yet</p>
            <p className="text-sm text-muted-foreground mb-4">
              Create a scenario or sync from trade history to get started.
            </p>
            <div className="flex gap-2">
              <Button variant="outline" onClick={handleSyncBaseline} disabled={isSyncing}>
                <RefreshCw className="h-4 w-4 mr-2" />
                Sync Baseline
              </Button>
              <Button onClick={() => setShowCreateDialog(true)}>
                <Plus className="h-4 w-4 mr-2" />
                New Scenario
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {scenarios.map((scenario) => (
            <Card
              key={scenario.id}
              className={`cursor-pointer transition-colors hover:border-primary/50 ${
                scenario.id != null && selectedIds.has(scenario.id)
                  ? 'border-primary bg-primary/5'
                  : ''
              }`}
              onClick={() =>
                navigate(`/clients/${clientId}/scenarios/${scenario.id}`)
              }
            >
              <CardHeader className="flex flex-row items-start justify-between pb-2">
                <div className="flex items-start gap-3 min-w-0 flex-1">
                  <div
                    className="pt-0.5 shrink-0"
                    onClick={(e) => {
                      e.stopPropagation()
                      if (scenario.id != null) toggleSelection(scenario.id, e)
                    }}
                  >
                    <Checkbox
                      checked={scenario.id != null && selectedIds.has(scenario.id)}
                      tabIndex={-1}
                    />
                  </div>
                  <div className="space-y-1 min-w-0 flex-1">
                    <div className="flex items-center gap-2 flex-wrap">
                      <CardTitle className="text-base truncate">
                        {scenario.name}
                      </CardTitle>
                      {scenario.is_baseline && (
                        <Badge variant="secondary" className="shrink-0">
                          Baseline
                        </Badge>
                      )}
                      {scenario.account_type && (
                        <Badge variant="outline" className="shrink-0 text-[10px]">
                          {ACCOUNT_TYPES.find((t) => t.value === scenario.account_type)?.label ?? scenario.account_type}
                        </Badge>
                      )}
                    </div>
                    {scenario.description && (
                      <p className="text-xs text-muted-foreground line-clamp-2">
                        {scenario.description}
                      </p>
                    )}
                  </div>
                </div>
              </CardHeader>
              <CardContent className="pt-0">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted-foreground">
                    {scenario.updated_at
                      ? `Updated ${new Date(scenario.updated_at + 'Z').toLocaleDateString()}`
                      : scenario.created_at
                        ? `Created ${new Date(scenario.created_at + 'Z').toLocaleDateString()}`
                        : ''}
                  </span>
                  <div className="flex gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      onClick={(e) => {
                        e.stopPropagation()
                        openCloneDialog(scenario)
                      }}
                    >
                      <Copy className="h-4 w-4" />
                    </Button>
                    {!scenario.is_baseline && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 text-destructive hover:text-destructive"
                        onClick={(e) => {
                          e.stopPropagation()
                          setDeleteTarget(scenario)
                        }}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    )}
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {/* Create Scenario Dialog */}
      <Dialog open={showCreateDialog} onOpenChange={setShowCreateDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>New Scenario</DialogTitle>
            <DialogDescription>
              Create a what-if portfolio scenario for {client?.name}.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="scenario-name">Name *</Label>
              <Input
                id="scenario-name"
                value={createForm.name}
                onChange={(e) =>
                  setCreateForm({ ...createForm, name: e.target.value })
                }
                placeholder="e.g., Conservative Rebalance"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="scenario-desc">Description</Label>
              <Input
                id="scenario-desc"
                value={createForm.description}
                onChange={(e) =>
                  setCreateForm({ ...createForm, description: e.target.value })
                }
                placeholder="Optional notes about this scenario"
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowCreateDialog(false)}
            >
              Cancel
            </Button>
            <Button onClick={handleCreate} disabled={isSubmitting}>
              {isSubmitting && (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              )}
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Clone Scenario Dialog */}
      <Dialog open={showCloneDialog} onOpenChange={setShowCloneDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Clone Scenario</DialogTitle>
            <DialogDescription>
              Create a copy of &ldquo;{cloneSource?.name}&rdquo; with all its
              positions.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="clone-name">Name *</Label>
              <Input
                id="clone-name"
                value={cloneForm.name}
                onChange={(e) =>
                  setCloneForm({ ...cloneForm, name: e.target.value })
                }
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="clone-desc">Description</Label>
              <Input
                id="clone-desc"
                value={cloneForm.description}
                onChange={(e) =>
                  setCloneForm({ ...cloneForm, description: e.target.value })
                }
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowCloneDialog(false)}
            >
              Cancel
            </Button>
            <Button onClick={handleClone} disabled={isSubmitting}>
              {isSubmitting && (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              )}
              Clone
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation */}
      <Dialog open={!!deleteTarget} onOpenChange={() => setDeleteTarget(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete Scenario</DialogTitle>
            <DialogDescription>
              This will permanently delete &ldquo;{deleteTarget?.name}&rdquo;
              and all its positions. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleDelete}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
