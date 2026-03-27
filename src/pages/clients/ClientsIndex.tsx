import { invoke } from '@tauri-apps/api/core'
import { Loader2, Plus, Trash2, Users } from 'lucide-react'
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
import type { Client } from '@/types/clients'

function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'object' && err !== null && 'message' in err) return (err as { message: string }).message
  return String(err)
}

export default function ClientsIndex() {
  const navigate = useNavigate()
  const [clients, setClients] = useState<Client[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [showAddDialog, setShowAddDialog] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<Client | null>(null)
  const [form, setForm] = useState({
    name: '',
    email: '',
    phone: '',
    broker: '',
    account_id: '',
    notes: '',
  })

  const loadClients = useCallback(async () => {
    try {
      const data = await invoke<Client[]>('get_clients')
      setClients(data)
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to load clients: ${msg}`)
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    loadClients()
  }, [loadClients])

  const resetForm = () => {
    setForm({ name: '', email: '', phone: '', broker: '', account_id: '', notes: '' })
  }

  const handleAddClient = async () => {
    if (!form.name.trim()) {
      toast.error('Client name is required')
      return
    }

    setIsSubmitting(true)
    try {
      await invoke('create_client', {
        name: form.name.trim(),
        email: form.email.trim() || null,
        phone: form.phone.trim() || null,
        broker: form.broker.trim() || null,
        accountId: form.account_id.trim() || null,
        notes: form.notes.trim() || null,
      })
      toast.success(`Added client: ${form.name}`)
      resetForm()
      setShowAddDialog(false)
      await loadClients()
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to add client: ${msg}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleDelete = async () => {
    if (!deleteTarget?.id) return
    try {
      await invoke('delete_client', { id: deleteTarget.id })
      toast.success(`Deleted client: ${deleteTarget.name}`)
      setDeleteTarget(null)
      await loadClients()
    } catch (err) {
      const msg = errMsg(err)
      toast.error(`Failed to delete client: ${msg}`)
    }
  }

  if (isLoading) {
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
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Clients</h1>
          <p className="text-sm text-muted-foreground">Manage client profiles and trade data</p>
        </div>
        <Button onClick={() => setShowAddDialog(true)}>
          <Plus className="h-4 w-4 mr-2" />
          Add Client
        </Button>
      </div>

      {/* Client List */}
      {clients.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-16">
            <Users className="h-12 w-12 text-muted-foreground mb-4" />
            <p className="text-lg font-medium text-muted-foreground">No clients yet</p>
            <p className="text-sm text-muted-foreground mb-4">Add your first client to get started</p>
            <Button onClick={() => setShowAddDialog(true)}>
              <Plus className="h-4 w-4 mr-2" />
              Add Client
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">
              {clients.length} Client{clients.length !== 1 ? 's' : ''}
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Name</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Broker</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Account ID</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Email</th>
                    <th className="h-10 px-4 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Created</th>
                    <th className="h-10 px-4 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {clients.map((client) => (
                    <tr
                      key={client.id}
                      className="border-b cursor-pointer hover:bg-muted/50 transition-colors"
                      onClick={() => navigate(`/clients/${client.id}`)}
                    >
                      <td className="h-12 px-4 font-medium">{client.name}</td>
                      <td className="h-12 px-4 text-muted-foreground">{client.broker || '—'}</td>
                      <td className="h-12 px-4 text-muted-foreground font-mono text-sm">{client.account_id || '—'}</td>
                      <td className="h-12 px-4 text-muted-foreground">{client.email || '—'}</td>
                      <td className="h-12 px-4 text-muted-foreground text-sm">
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

      {/* Add Client Dialog */}
      <Dialog open={showAddDialog} onOpenChange={setShowAddDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Add Client</DialogTitle>
            <DialogDescription>Create a new client profile</DialogDescription>
          </DialogHeader>
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
                  placeholder="e.g. Schwab"
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
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowAddDialog(false)}>
              Cancel
            </Button>
            <Button onClick={handleAddClient} disabled={isSubmitting}>
              {isSubmitting && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
              Add Client
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation Dialog */}
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
