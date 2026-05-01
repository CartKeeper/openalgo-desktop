import { invoke } from '@tauri-apps/api/core'
import { ChevronLeft, HelpCircle, Loader2, Plus, ShieldCheck, Trash2, Users } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
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
import type { Client, ImportReport } from '@/types/clients'
import ImportDocumentsDialog from './ImportCsvDialog'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err) return (err as { message: string }).message
  return String(err)
}

type WizardStep = 'profile' | 'documents'

export default function ClientsIndex() {
  const navigate = useNavigate()
  const [clients, setClients] = useState<Client[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [showAddDialog, setShowAddDialog] = useState(false)
  const [isCreating, setIsCreating] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<Client | null>(null)
  const [showHelp, setShowHelp] = useState(false)
  const [confirmCancel, setConfirmCancel] = useState(false)

  const [step, setStep] = useState<WizardStep>('profile')
  const [pendingClientId, setPendingClientId] = useState<number | null>(null)
  const [showDocumentsDialog, setShowDocumentsDialog] = useState(false)

  const [form, setForm] = useState({
    name: '',
    email: '',
    phone: '',
    broker: 'Schwab',
    account_id: '',
    notes: '',
  })

  const loadClients = useCallback(async () => {
    try {
      const data = await invoke<Client[]>('get_clients')
      setClients(data)
    } catch (err) {
      toast.error(`Failed to load clients: ${errMsg(err)}`)
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    loadClients()
  }, [loadClients])

  const resetWizard = () => {
    setForm({ name: '', email: '', phone: '', broker: 'Schwab', account_id: '', notes: '' })
    setStep('profile')
    setPendingClientId(null)
    setShowDocumentsDialog(false)
    setShowHelp(false)
    setConfirmCancel(false)
  }

  /** Step 1 → Step 2: create the client record so the importer has an ID to attach to. */
  const handleProfileNext = async () => {
    if (!form.name.trim()) {
      toast.error('Client name is required')
      return
    }
    setIsCreating(true)
    try {
      const created = await invoke<Client>('create_client', {
        name: form.name.trim(),
        email: form.email.trim() || null,
        phone: form.phone.trim() || null,
        broker: form.broker.trim() || null,
        accountId: form.account_id.trim() || null,
        accountType: '401k', // strict 401k rules apply per project spec
        notes: form.notes.trim() || null,
      })
      if (!created.id) throw new Error('Created client returned no id')
      setPendingClientId(created.id)
      setStep('documents')
      setShowDocumentsDialog(true)
    } catch (err) {
      toast.error(`Failed to create client: ${errMsg(err)}`)
    } finally {
      setIsCreating(false)
    }
  }

  /** Documents successfully imported → wizard is complete */
  const handleDocumentsSuccess = async (report: ImportReport) => {
    const clientId = report.client_id
    setShowDocumentsDialog(false)
    setShowAddDialog(false)
    resetWizard()
    await loadClients()
    toast.success('Client created and documents imported.')
    navigate(`/clients/${clientId}`)
  }

  /** Cancel during Step 2 → roll back the client record so we never leave an empty profile */
  const cancelWizard = async () => {
    if (pendingClientId != null) {
      try {
        await invoke('delete_client', { id: pendingClientId })
      } catch (err) {
        // Non-fatal — surface but don't block close
        console.warn('Rollback delete failed:', errMsg(err))
      }
    }
    resetWizard()
    setShowAddDialog(false)
    await loadClients()
  }

  const handleAddDialogChange = (open: boolean) => {
    if (!open) {
      // If we're in step 2 with a pending client, require confirmation before close
      if (step === 'documents' && pendingClientId != null) {
        setConfirmCancel(true)
        return
      }
      resetWizard()
    }
    setShowAddDialog(open)
  }

  const handleDelete = async () => {
    if (!deleteTarget?.id) return
    try {
      await invoke('delete_client', { id: deleteTarget.id })
      toast.success(`Deleted client: ${deleteTarget.name}`)
      setDeleteTarget(null)
      await loadClients()
    } catch (err) {
      toast.error(`Failed to delete client: ${errMsg(err)}`)
    }
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            <Users className="h-6 w-6" />
            Clients
          </h1>
          <p className="text-sm text-muted-foreground">
            Strict 401(k) rules are enforced on new client setup.
          </p>
        </div>
        <Button onClick={() => setShowAddDialog(true)}>
          <Plus className="h-4 w-4 mr-2" />
          Add Client
        </Button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center h-64">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      ) : clients.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Users className="h-12 w-12 text-muted-foreground mb-3" strokeWidth={1.25} />
            <p className="text-base font-semibold">No clients yet</p>
            <p className="text-sm text-muted-foreground mt-1 mb-4">
              Add your first client and upload their Schwab documents.
            </p>
            <Button onClick={() => setShowAddDialog(true)}>
              <Plus className="h-4 w-4 mr-2" />
              Add Client
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">All Clients</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="rounded-lg border overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Name</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Broker</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Account</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Email</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Created</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground"></th>
                  </tr>
                </thead>
                <tbody>
                  {clients.map((client) => (
                    <tr
                      key={client.id}
                      className="border-b last:border-b-0 hover:bg-muted/30 cursor-pointer"
                      onClick={() => client.id && navigate(`/clients/${client.id}`)}
                    >
                      <td className="h-12 px-4 font-semibold">{client.name}</td>
                      <td className="h-12 px-4 text-muted-foreground">{client.broker || '—'}</td>
                      <td className="h-12 px-4 text-muted-foreground font-mono text-xs">{client.account_id || '—'}</td>
                      <td className="h-12 px-4 text-muted-foreground">{client.email || '—'}</td>
                      <td className="h-12 px-4 text-muted-foreground text-xs">
                        {client.created_at ? new Date(client.created_at + 'Z').toLocaleDateString() : '—'}
                      </td>
                      <td className="h-12 px-4 text-right">
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-8 w-8 text-destructive hover:text-destructive"
                          onClick={(e) => {
                            e.stopPropagation()
                            setDeleteTarget(client)
                          }}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* New-client wizard — Step 1: Profile */}
      <Dialog open={showAddDialog && step === 'profile'} onOpenChange={handleAddDialogChange}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <div className="flex items-start justify-between gap-4">
              <div className="space-y-1.5">
                <DialogTitle>New Client — Step 1 of 2</DialogTitle>
                <DialogDescription>
                  Enter the client profile. Strict 401(k) rules will apply to imported data.
                </DialogDescription>
              </div>
              <button
                type="button"
                onClick={() => setShowHelp((v) => !v)}
                className="text-muted-foreground hover:text-foreground"
                aria-label="Help"
              >
                <HelpCircle className="h-5 w-5" strokeWidth={1.5} />
              </button>
            </div>
          </DialogHeader>
          {showHelp && (
            <div className="rounded-lg border bg-muted/40 p-3 text-xs space-y-1">
              <p className="font-semibold text-sm">What happens next</p>
              <p>Step 2 requires uploading the client's Schwab Transactions file (JSON or CSV) and optionally the Order Status CSV. Strict 401(k) rules block the import if any violation is found.</p>
              <p className="text-muted-foreground pt-1">Cancel during Step 2 will roll back this client record.</p>
            </div>
          )}
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="name">Name *</Label>
              <Input
                id="name"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder="Client name"
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="broker">Broker</Label>
                <Input
                  id="broker"
                  value={form.broker}
                  onChange={(e) => setForm({ ...form, broker: e.target.value })}
                  placeholder="Schwab"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="account_id">Account ID</Label>
                <Input
                  id="account_id"
                  value={form.account_id}
                  onChange={(e) => setForm({ ...form, account_id: e.target.value })}
                  placeholder="Account number"
                />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  type="email"
                  value={form.email}
                  onChange={(e) => setForm({ ...form, email: e.target.value })}
                  placeholder="email@example.com"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="phone">Phone</Label>
                <Input
                  id="phone"
                  value={form.phone}
                  onChange={(e) => setForm({ ...form, phone: e.target.value })}
                  placeholder="Phone number"
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="notes">Notes</Label>
              <Input
                id="notes"
                value={form.notes}
                onChange={(e) => setForm({ ...form, notes: e.target.value })}
                placeholder="Optional notes"
              />
            </div>
            <div className="flex items-center gap-2 text-xs text-muted-foreground rounded-md border bg-muted/30 p-2">
              <ShieldCheck className="h-3.5 w-3.5 text-emerald-600" />
              Account type will be set to 401(k). Documents are required to complete setup.
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => handleAddDialogChange(false)} disabled={isCreating}>
              Cancel
            </Button>
            <Button onClick={handleProfileNext} disabled={isCreating}>
              {isCreating && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
              Next: Add Documents
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* New-client wizard — Step 2: Documents (delegates to ImportDocumentsDialog) */}
      {pendingClientId != null && (
        <ImportDocumentsDialog
          open={showDocumentsDialog}
          onOpenChange={(next) => {
            if (!next) {
              setConfirmCancel(true)
            } else {
              setShowDocumentsDialog(next)
            }
          }}
          clientId={pendingClientId}
          onSuccess={handleDocumentsSuccess}
          embeddedWizard
        />
      )}

      {/* Cancel-during-Step-2 confirmation */}
      <Dialog open={confirmCancel} onOpenChange={setConfirmCancel}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Cancel new client setup?</DialogTitle>
            <DialogDescription>
              The client record was created at Step 1 and will be deleted if you cancel now. You'll need to start over.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmCancel(false)}>
              Keep editing
            </Button>
            <Button
              variant="destructive"
              onClick={async () => {
                setConfirmCancel(false)
                await cancelWizard()
              }}
            >
              <ChevronLeft className="h-4 w-4 mr-2" />
              Cancel & roll back
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete confirmation */}
      <Dialog open={!!deleteTarget} onOpenChange={() => setDeleteTarget(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete Client</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete <strong>{deleteTarget?.name}</strong>? This will also delete all their trades and import history. This action cannot be undone.
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
