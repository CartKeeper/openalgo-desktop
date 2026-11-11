import { invoke } from '@tauri-apps/api/core'
import { Loader2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { Client } from '@/types/clients'
import type { ClientScenario } from '@/types/client-scenarios'

interface ScenarioPickerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Called with the chosen scenario. The dialog stays open until it resolves. */
  onPick: (scenarioId: number) => Promise<void>
}

/**
 * Ask which client scenario a saved report analyzes. Used by reports saved
 * before Save as Report recorded the scenario.
 */
export function ScenarioPickerDialog({ open, onOpenChange, onPick }: ScenarioPickerDialogProps) {
  const [clients, setClients] = useState<Client[] | null>(null)
  const [clientId, setClientId] = useState('')
  const [scenarios, setScenarios] = useState<ClientScenario[] | null>(null)
  const [scenarioId, setScenarioId] = useState('')
  const [isSaving, setIsSaving] = useState(false)

  useEffect(() => {
    if (!open) return
    setClients(null)
    setClientId('')
    setScenarios(null)
    setScenarioId('')
    invoke<Client[]>('get_clients')
      .then(setClients)
      .catch((err) => {
        setClients([])
        toast.error(`Couldn't load clients: ${err instanceof Error ? err.message : String(err)}`)
      })
  }, [open])

  useEffect(() => {
    if (!clientId) return
    let cancelled = false
    setScenarios(null)
    setScenarioId('')
    invoke<ClientScenario[]>('get_client_scenarios', { clientId: Number(clientId) })
      .then((list) => {
        if (!cancelled) setScenarios(list)
      })
      .catch((err) => {
        if (cancelled) return
        setScenarios([])
        toast.error(`Couldn't load scenarios: ${err instanceof Error ? err.message : String(err)}`)
      })
    return () => {
      cancelled = true
    }
  }, [clientId])

  const handleUse = async () => {
    if (!scenarioId || isSaving) return
    setIsSaving(true)
    try {
      await onPick(Number(scenarioId))
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Which Client Scenario?</DialogTitle>
          <DialogDescription>
            This report was saved before reports remembered their client. Choose the scenario it
            analyzes. The trades apply to that sandbox, never to your live account, and the report
            remembers your choice.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="pick-client">Client</Label>
            <Select value={clientId} onValueChange={setClientId} disabled={!clients}>
              <SelectTrigger id="pick-client">
                <SelectValue placeholder={clients ? 'Choose a client' : 'Loading clients…'} />
              </SelectTrigger>
              <SelectContent>
                {(clients ?? [])
                  .filter((c) => c.id != null)
                  .map((c) => (
                    <SelectItem key={c.id} value={String(c.id)}>
                      {c.name}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="pick-scenario">Scenario</Label>
            <Select
              value={scenarioId}
              onValueChange={setScenarioId}
              disabled={!clientId || !scenarios || scenarios.length === 0}
            >
              <SelectTrigger id="pick-scenario">
                <SelectValue
                  placeholder={
                    !clientId
                      ? 'Choose a client first'
                      : !scenarios
                        ? 'Loading scenarios…'
                        : scenarios.length === 0
                          ? 'This client has no scenarios'
                          : 'Choose a scenario'
                  }
                />
              </SelectTrigger>
              <SelectContent>
                {(scenarios ?? [])
                  .filter((s) => s.id != null)
                  .map((s) => (
                    <SelectItem key={s.id} value={String(s.id)}>
                      {s.name}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleUse} disabled={!scenarioId || isSaving}>
            {isSaving && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Use This Scenario
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
