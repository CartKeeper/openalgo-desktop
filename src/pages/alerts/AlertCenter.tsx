import {
  ArrowLeft,
  Bell,
  BellOff,
  Check,
  CheckCheck,
  Clock,
  DollarSign,
  Edit2,
  Loader2,
  Newspaper,
  Plus,
  Settings2,
  Star,
  Trash2,
  TrendingDown,
  TrendingUp,
  Volume2,
  VolumeX,
  X,
  Zap,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/utils'
import {
  useAlertStore,
  type AlertConfig,
  type AlertGlobalSettings,
  type AlertHistoryEntry,
} from '@/stores/alertStore'

// ---------- Helpers ----------

const ALERT_TYPE_LABELS: Record<string, string> = {
  price_above: 'Price Above',
  price_below: 'Price Below',
  movement: 'Movement',
  news: 'News',
  rising_star: 'Rising Star',
}

function alertTypeLabel(type: string): string {
  return ALERT_TYPE_LABELS[type] ?? type
}

function alertTypeIcon(type: string) {
  switch (type) {
    case 'price_above':
      return <TrendingUp className="h-4 w-4 text-green-500" />
    case 'price_below':
      return <TrendingDown className="h-4 w-4 text-red-500" />
    case 'movement':
      return <Zap className="h-4 w-4 text-amber-500" />
    case 'news':
      return <Newspaper className="h-4 w-4 text-blue-500" />
    case 'rising_star':
      return <Star className="h-4 w-4 text-yellow-500" />
    default:
      return <Bell className="h-4 w-4 text-muted-foreground" />
  }
}

function formatRelativeTime(dateStr: string): string {
  const now = Date.now()
  const d = new Date(dateStr + (dateStr.endsWith('Z') ? '' : 'Z'))
  const diff = now - d.getTime()
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return 'just now'
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days}d ago`
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

// ---------- Sub-Components ----------

function HistoryItem({
  entry,
  onAcknowledge,
}: {
  entry: AlertHistoryEntry
  onAcknowledge: (id: number) => void
}) {
  return (
    <div
      className={cn(
        'flex items-start gap-3 rounded-lg border px-4 py-3 transition-colors duration-150',
        !entry.acknowledged && 'bg-accent/5 border-accent/20'
      )}
    >
      <div className="mt-0.5 shrink-0">{alertTypeIcon(entry.alert_type)}</div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {entry.symbol}
          </span>
          <Badge variant="outline" className="text-[10px] px-1.5 py-0">
            {alertTypeLabel(entry.alert_type)}
          </Badge>
          {!entry.acknowledged && (
            <Badge variant="default" className="text-[10px] px-1.5 py-0">
              New
            </Badge>
          )}
        </div>
        <p className="mt-0.5 text-sm font-medium leading-snug">{entry.title}</p>
        <p className="mt-0.5 text-xs text-muted-foreground line-clamp-2">{entry.message}</p>
        <p className="mt-1 text-[10px] text-muted-foreground tabular-nums">
          {formatRelativeTime(entry.created_at)}
        </p>
      </div>
      {!entry.acknowledged && (
        <Button
          variant="ghost"
          size="sm"
          className="h-8 w-8 p-0 shrink-0"
          onClick={() => onAcknowledge(entry.id)}
          title="Mark as read"
        >
          <Check className="h-3.5 w-3.5" />
        </Button>
      )}
    </div>
  )
}

function AlertConfigRow({
  alert,
  onToggle,
  onEdit,
  onDelete,
}: {
  alert: AlertConfig
  onToggle: (id: number, enabled: boolean) => void
  onEdit: (alert: AlertConfig) => void
  onDelete: (id: number) => void
}) {
  return (
    <div className="flex items-center gap-3 rounded-lg border px-4 py-3">
      <div className="shrink-0">{alertTypeIcon(alert.alert_type)}</div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold">{alert.symbol}</span>
          <Badge variant="outline" className="text-[10px] px-1.5 py-0">
            {alertTypeLabel(alert.alert_type)}
          </Badge>
        </div>
        <div className="flex items-center gap-3 mt-0.5">
          {alert.threshold_value !== null && (
            <span className="text-xs text-muted-foreground tabular-nums">
              <DollarSign className="inline h-3 w-3" />
              {alert.threshold_value}
            </span>
          )}
          <span className="text-xs text-muted-foreground">
            <Clock className="inline h-3 w-3 mr-0.5" />
            {alert.cooldown_minutes}m cooldown
          </span>
        </div>
      </div>
      <Switch
        checked={alert.enabled}
        onCheckedChange={(checked) => {
          if (alert.id !== null) onToggle(alert.id, checked)
        }}
      />
      <Button
        variant="ghost"
        size="sm"
        className="h-8 w-8 p-0"
        onClick={() => onEdit(alert)}
        title="Edit alert"
      >
        <Edit2 className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="h-8 w-8 p-0 text-destructive hover:text-destructive"
        onClick={() => {
          if (alert.id !== null) onDelete(alert.id)
        }}
        title="Delete alert"
      >
        <Trash2 className="h-3.5 w-3.5" />
      </Button>
    </div>
  )
}

// ---------- Create/Edit Form ----------

interface AlertFormData {
  symbol: string
  alert_type: string
  threshold_value: string
  threshold_period_minutes: string
  cooldown_minutes: string
}

const EMPTY_FORM: AlertFormData = {
  symbol: '',
  alert_type: 'price_above',
  threshold_value: '',
  threshold_period_minutes: '',
  cooldown_minutes: '15',
}

function AlertForm({
  initial,
  editingId,
  onSubmit,
  onCancel,
}: {
  initial: AlertFormData
  editingId: number | null
  onSubmit: (data: AlertFormData) => void
  onCancel: () => void
}) {
  const [form, setForm] = useState<AlertFormData>(initial)

  useEffect(() => {
    setForm(initial)
  }, [initial])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!form.symbol.trim()) {
      toast.error('Symbol is required')
      return
    }
    onSubmit(form)
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="space-y-1.5">
        <Label htmlFor="alert-symbol" className="text-xs font-semibold">
          Symbol
        </Label>
        <Input
          id="alert-symbol"
          placeholder="AAPL"
          className="h-10 uppercase"
          value={form.symbol}
          onChange={(e) => setForm((f) => ({ ...f, symbol: e.target.value.toUpperCase() }))}
        />
      </div>

      <div className="space-y-1.5">
        <Label htmlFor="alert-type" className="text-xs font-semibold">
          Alert Type
        </Label>
        <Select
          value={form.alert_type}
          onValueChange={(val) => setForm((f) => ({ ...f, alert_type: val }))}
        >
          <SelectTrigger className="h-10">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="price_above">Price Above</SelectItem>
            <SelectItem value="price_below">Price Below</SelectItem>
            <SelectItem value="movement">Movement</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="space-y-1.5">
        <Label htmlFor="alert-threshold" className="text-xs font-semibold">
          Threshold Value
        </Label>
        <Input
          id="alert-threshold"
          type="number"
          step="any"
          placeholder={form.alert_type === 'movement' ? 'e.g. 5 (%)' : 'e.g. 170.00'}
          className="h-10 tabular-nums"
          value={form.threshold_value}
          onChange={(e) => setForm((f) => ({ ...f, threshold_value: e.target.value }))}
        />
      </div>

      {form.alert_type === 'movement' && (
        <div className="space-y-1.5">
          <Label htmlFor="alert-period" className="text-xs font-semibold">
            Period (minutes)
          </Label>
          <Input
            id="alert-period"
            type="number"
            placeholder="e.g. 60"
            className="h-10 tabular-nums"
            value={form.threshold_period_minutes}
            onChange={(e) => setForm((f) => ({ ...f, threshold_period_minutes: e.target.value }))}
          />
        </div>
      )}

      <div className="space-y-1.5">
        <Label htmlFor="alert-cooldown" className="text-xs font-semibold">
          Cooldown (minutes)
        </Label>
        <Input
          id="alert-cooldown"
          type="number"
          placeholder="15"
          className="h-10 tabular-nums"
          value={form.cooldown_minutes}
          onChange={(e) => setForm((f) => ({ ...f, cooldown_minutes: e.target.value }))}
        />
      </div>

      <div className="flex gap-2 pt-2">
        <Button type="submit" size="sm" className="h-10 flex-1">
          {editingId !== null ? 'Update Alert' : 'Create Alert'}
        </Button>
        <Button type="button" variant="outline" size="sm" className="h-10" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  )
}

// ---------- Settings Section ----------

function SettingsSection({
  settings,
  onSave,
}: {
  settings: AlertGlobalSettings
  onSave: (s: AlertGlobalSettings) => void
}) {
  const [local, setLocal] = useState<AlertGlobalSettings>(settings)
  const [dirty, setDirty] = useState(false)

  useEffect(() => {
    setLocal(settings)
    setDirty(false)
  }, [settings])

  const update = <K extends keyof AlertGlobalSettings>(key: K, value: AlertGlobalSettings[K]) => {
    setLocal((prev) => ({ ...prev, [key]: value }))
    setDirty(true)
  }

  const handleSave = () => {
    onSave(local)
    setDirty(false)
  }

  return (
    <div className="space-y-6">
      {/* Master Controls */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-semibold">Alerts Enabled</p>
            <p className="text-xs text-muted-foreground">Master switch for all alert processing</p>
          </div>
          <Switch
            checked={local.alerts_enabled}
            onCheckedChange={(val) => update('alerts_enabled', val)}
          />
        </div>

        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-semibold">Sound</p>
            <p className="text-xs text-muted-foreground">Play audio when alerts trigger</p>
          </div>
          <Switch
            checked={local.sound_enabled}
            onCheckedChange={(val) => update('sound_enabled', val)}
          />
        </div>

        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-semibold">Native Notifications</p>
            <p className="text-xs text-muted-foreground">Show OS-level notifications</p>
          </div>
          <Switch
            checked={local.native_notifications_enabled}
            onCheckedChange={(val) => update('native_notifications_enabled', val)}
          />
        </div>
      </div>

      {/* Rising Star Scanner */}
      <div className="space-y-4 border-t pt-4">
        <h4 className="text-sm font-semibold flex items-center gap-2">
          <Star className="h-4 w-4 text-yellow-500" />
          Rising Star Scanner
        </h4>

        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium">Enable Scanner</p>
            <p className="text-xs text-muted-foreground">
              Auto-detect stocks with unusual momentum
            </p>
          </div>
          <Switch
            checked={local.rising_star_enabled}
            onCheckedChange={(val) => update('rising_star_enabled', val)}
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <Label className="text-xs font-semibold">Scan Interval (hours)</Label>
            <Input
              type="number"
              step="0.5"
              className="h-10 tabular-nums"
              value={String(local.rising_star_interval_seconds / 3600)}
              onChange={(e) => {
                const hours = parseFloat(e.target.value) || 1
                update('rising_star_interval_seconds', Math.round(hours * 3600))
              }}
            />
          </div>
          <div className="space-y-1.5">
            <Label className="text-xs font-semibold">Min Change %</Label>
            <Input
              type="number"
              step="0.5"
              className="h-10 tabular-nums"
              value={String(local.rising_star_min_change_pct)}
              onChange={(e) =>
                update('rising_star_min_change_pct', parseFloat(e.target.value) || 0)
              }
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <Label className="text-xs font-semibold">Min Volume Ratio</Label>
          <Input
            type="number"
            step="0.1"
            className="h-10 tabular-nums"
            value={String(local.rising_star_min_volume_ratio)}
            onChange={(e) =>
              update('rising_star_min_volume_ratio', parseFloat(e.target.value) || 0)
            }
          />
        </div>
      </div>

      {/* Movement Defaults */}
      <div className="space-y-4 border-t pt-4">
        <h4 className="text-sm font-semibold flex items-center gap-2">
          <Zap className="h-4 w-4 text-amber-500" />
          Movement Defaults
        </h4>

        <div className="space-y-1.5">
          <Label className="text-xs font-semibold">Default Threshold %</Label>
          <Input
            type="number"
            step="0.5"
            className="h-10 tabular-nums"
            value={String(local.movement_default_threshold)}
            onChange={(e) =>
              update('movement_default_threshold', parseFloat(e.target.value) || 0)
            }
          />
        </div>
      </div>

      {/* Save Button */}
      {dirty && (
        <div className="pt-2">
          <Button onClick={handleSave} className="h-10 w-full">
            Save Settings
          </Button>
        </div>
      )}
    </div>
  )
}

// ---------- Main Page ----------

export default function AlertCenter() {
  const {
    alerts,
    history,
    unacknowledgedCount,
    globalSettings,
    fetchAlerts,
    createAlert,
    updateAlert,
    deleteAlert,
    toggleAlert,
    fetchHistory,
    acknowledgeAlert,
    acknowledgeAll,
    fetchUnacknowledgedCount,
    fetchSettings,
    updateSettings,
  } = useAlertStore()

  const [historyFilter, setHistoryFilter] = useState<string>('all')
  const [showForm, setShowForm] = useState(false)
  const [editingAlert, setEditingAlert] = useState<AlertConfig | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    fetchAlerts()
    fetchHistory()
    fetchUnacknowledgedCount()
    fetchSettings()
  }, [fetchAlerts, fetchHistory, fetchUnacknowledgedCount, fetchSettings])

  // ---------- Handlers ----------

  const handleCreateOrUpdate = async (data: AlertFormData) => {
    setIsSubmitting(true)
    try {
      const params = {
        alert_type: data.alert_type,
        symbol: data.symbol.trim().toUpperCase(),
        threshold_value: data.threshold_value ? parseFloat(data.threshold_value) : null,
        threshold_period_minutes: data.threshold_period_minutes
          ? parseInt(data.threshold_period_minutes, 10)
          : null,
        cooldown_minutes: parseInt(data.cooldown_minutes, 10) || 15,
      }

      if (editingAlert?.id !== null && editingAlert?.id !== undefined) {
        await updateAlert(editingAlert.id, params)
        toast.success('Alert updated')
      } else {
        await createAlert(params)
        toast.success('Alert created')
      }
      setShowForm(false)
      setEditingAlert(null)
    } catch {
      toast.error('Failed to save alert')
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleDelete = async (id: number) => {
    try {
      await deleteAlert(id)
      toast.success('Alert deleted')
    } catch {
      toast.error('Failed to delete alert')
    }
  }

  const handleToggle = async (id: number, enabled: boolean) => {
    try {
      await toggleAlert(id, enabled)
    } catch {
      toast.error('Failed to toggle alert')
    }
  }

  const handleAcknowledge = async (id: number) => {
    try {
      await acknowledgeAlert(id)
    } catch {
      toast.error('Failed to acknowledge alert')
    }
  }

  const handleAcknowledgeAll = async () => {
    try {
      await acknowledgeAll()
      toast.success('All alerts marked as read')
    } catch {
      toast.error('Failed to mark all as read')
    }
  }

  const handleSaveSettings = async (settings: AlertGlobalSettings) => {
    try {
      await updateSettings(settings)
      toast.success('Settings saved')
    } catch {
      toast.error('Failed to save settings')
    }
  }

  const handleEdit = (alert: AlertConfig) => {
    setEditingAlert(alert)
    setShowForm(true)
  }

  const handleCancelForm = () => {
    setShowForm(false)
    setEditingAlert(null)
  }

  // ---------- Derived ----------

  const activeAlertCount = alerts.filter((a) => a.enabled).length
  const risingStarStatus = globalSettings?.rising_star_enabled ? 'Active' : 'Off'

  const filteredHistory =
    historyFilter === 'all'
      ? history
      : history.filter((h) => h.alert_type === historyFilter)

  const formInitial: AlertFormData = editingAlert
    ? {
        symbol: editingAlert.symbol,
        alert_type: editingAlert.alert_type,
        threshold_value: editingAlert.threshold_value !== null ? String(editingAlert.threshold_value) : '',
        threshold_period_minutes:
          editingAlert.threshold_period_minutes !== null
            ? String(editingAlert.threshold_period_minutes)
            : '',
        cooldown_minutes: String(editingAlert.cooldown_minutes),
      }
    : EMPTY_FORM

  // ---------- Render ----------

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <Link to="/dashboard" className="text-muted-foreground hover:text-foreground">
            <ArrowLeft className="h-4 w-4" />
          </Link>
          <h1 className="text-2xl font-bold">Alert Center</h1>
        </div>
        <p className="text-sm text-muted-foreground">
          Configure price alerts, movement detection, and the Rising Star scanner.
        </p>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-3 gap-4">
        <Card className="py-4">
          <CardContent className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-accent/10">
              <Bell className="h-5 w-5 text-accent-foreground" />
            </div>
            <div>
              <p className="text-2xl font-bold tabular-nums">{activeAlertCount}</p>
              <p className="text-xs text-muted-foreground font-semibold">Active Alerts</p>
            </div>
          </CardContent>
        </Card>

        <Card className="py-4">
          <CardContent className="flex items-center gap-3">
            <div
              className={cn(
                'flex h-10 w-10 items-center justify-center rounded-lg',
                unacknowledgedCount > 0 ? 'bg-red-500/10' : 'bg-accent/10'
              )}
            >
              {unacknowledgedCount > 0 ? (
                <BellOff className="h-5 w-5 text-red-500" />
              ) : (
                <CheckCheck className="h-5 w-5 text-accent-foreground" />
              )}
            </div>
            <div>
              <p className="text-2xl font-bold tabular-nums">{unacknowledgedCount}</p>
              <p className="text-xs text-muted-foreground font-semibold">Unread</p>
            </div>
          </CardContent>
        </Card>

        <Card className="py-4">
          <CardContent className="flex items-center gap-3">
            <div
              className={cn(
                'flex h-10 w-10 items-center justify-center rounded-lg',
                globalSettings?.rising_star_enabled ? 'bg-yellow-500/10' : 'bg-accent/10'
              )}
            >
              <Star
                className={cn(
                  'h-5 w-5',
                  globalSettings?.rising_star_enabled
                    ? 'text-yellow-500'
                    : 'text-muted-foreground'
                )}
              />
            </div>
            <div>
              <p className="text-sm font-bold">{risingStarStatus}</p>
              <p className="text-xs text-muted-foreground font-semibold">Rising Star</p>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Two-Column Layout: History + My Alerts */}
      <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
        {/* Alert History — wider column */}
        <Card className="lg:col-span-3 py-4">
          <CardHeader className="pb-0">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">Alert History</CardTitle>
              <div className="flex items-center gap-2">
                <Select value={historyFilter} onValueChange={setHistoryFilter}>
                  <SelectTrigger className="h-8 w-[140px] text-xs">
                    <SelectValue placeholder="Filter" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All Types</SelectItem>
                    <SelectItem value="price_above">Price Above</SelectItem>
                    <SelectItem value="price_below">Price Below</SelectItem>
                    <SelectItem value="movement">Movement</SelectItem>
                    <SelectItem value="news">News</SelectItem>
                    <SelectItem value="rising_star">Rising Star</SelectItem>
                  </SelectContent>
                </Select>
                {unacknowledgedCount > 0 && (
                  <Button variant="outline" size="sm" className="h-8 text-xs" onClick={handleAcknowledgeAll}>
                    <CheckCheck className="h-3.5 w-3.5 mr-1" />
                    Mark All Read
                  </Button>
                )}
              </div>
            </div>
          </CardHeader>
          <CardContent>
            {filteredHistory.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <Bell className="h-12 w-12 text-muted-foreground mb-3" />
                <p className="text-sm font-medium">No alerts yet</p>
                <p className="text-xs text-muted-foreground mt-1">
                  Triggered alerts will appear here.
                </p>
              </div>
            ) : (
              <div className="space-y-2 max-h-[500px] overflow-y-auto">
                {filteredHistory.map((entry) => (
                  <HistoryItem
                    key={entry.id}
                    entry={entry}
                    onAcknowledge={handleAcknowledge}
                  />
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {/* My Alerts — narrower column */}
        <Card className="lg:col-span-2 py-4">
          <CardHeader className="pb-0">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">My Alerts</CardTitle>
              {!showForm && (
                <Button
                  size="sm"
                  className="h-8 text-xs"
                  onClick={() => {
                    setEditingAlert(null)
                    setShowForm(true)
                  }}
                >
                  <Plus className="h-3.5 w-3.5 mr-1" />
                  Create Alert
                </Button>
              )}
            </div>
          </CardHeader>
          <CardContent>
            {showForm && (
              <div className="mb-4 rounded-lg border p-4">
                <div className="flex items-center justify-between mb-3">
                  <h4 className="text-sm font-semibold">
                    {editingAlert ? 'Edit Alert' : 'New Alert'}
                  </h4>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-8 w-8 p-0"
                    onClick={handleCancelForm}
                  >
                    <X className="h-3.5 w-3.5" />
                  </Button>
                </div>
                <AlertForm
                  initial={formInitial}
                  editingId={editingAlert?.id ?? null}
                  onSubmit={handleCreateOrUpdate}
                  onCancel={handleCancelForm}
                />
                {isSubmitting && (
                  <div className="flex items-center justify-center py-2">
                    <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                  </div>
                )}
              </div>
            )}

            {alerts.length === 0 && !showForm ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <BellOff className="h-12 w-12 text-muted-foreground mb-3" />
                <p className="text-sm font-medium">No alerts configured</p>
                <p className="text-xs text-muted-foreground mt-1">
                  Create your first alert to get started.
                </p>
                <Button
                  size="sm"
                  className="mt-4 h-10"
                  onClick={() => {
                    setEditingAlert(null)
                    setShowForm(true)
                  }}
                >
                  <Plus className="h-4 w-4 mr-1" />
                  Create Alert
                </Button>
              </div>
            ) : (
              <div className="space-y-2 max-h-[500px] overflow-y-auto">
                {alerts.map((alert) => (
                  <AlertConfigRow
                    key={alert.id ?? alert.symbol + alert.alert_type}
                    alert={alert}
                    onToggle={handleToggle}
                    onEdit={handleEdit}
                    onDelete={handleDelete}
                  />
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Settings Section */}
      <Card className="py-4">
        <CardHeader className="pb-0">
          <CardTitle className="text-base flex items-center gap-2">
            <Settings2 className="h-4 w-4" />
            Alert Settings
          </CardTitle>
        </CardHeader>
        <CardContent>
          {globalSettings ? (
            <SettingsSection settings={globalSettings} onSave={handleSaveSettings} />
          ) : (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
