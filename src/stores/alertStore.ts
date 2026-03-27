import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'

// ---------- Types ----------

export interface AlertConfig {
  id: number | null
  alert_type: string // "price_above", "price_below", "movement", "news"
  symbol: string
  enabled: boolean
  threshold_value: number | null
  threshold_period_minutes: number | null
  cooldown_minutes: number
  last_triggered_at: string | null
  created_at: string | null
  updated_at: string | null
}

export interface AlertHistoryEntry {
  id: number
  alert_config_id: number | null
  alert_type: string
  symbol: string
  title: string
  message: string
  metadata_json: string | null
  acknowledged: boolean
  created_at: string
}

export interface AlertGlobalSettings {
  alerts_enabled: boolean
  sound_enabled: boolean
  native_notifications_enabled: boolean
  movement_default_threshold: number
  movement_check_interval_seconds: number
  news_check_interval_seconds: number
  rising_star_enabled: boolean
  rising_star_interval_seconds: number
  rising_star_min_change_pct: number
  rising_star_min_volume_ratio: number
}

export interface AlertTriggeredPayload {
  id: number | null
  alert_type: string
  symbol: string
  title: string
  message: string
  metadata_json: string | null
  timestamp: string
}

// ---------- Store Interface ----------

interface AlertStore {
  alerts: AlertConfig[]
  history: AlertHistoryEntry[]
  unacknowledgedCount: number
  globalSettings: AlertGlobalSettings | null

  fetchAlerts: () => Promise<void>
  createAlert: (params: {
    alert_type: string
    symbol: string
    threshold_value: number | null
    threshold_period_minutes: number | null
    cooldown_minutes: number
  }) => Promise<void>
  updateAlert: (
    id: number,
    params: {
      alert_type: string
      symbol: string
      threshold_value: number | null
      threshold_period_minutes: number | null
      cooldown_minutes: number
    }
  ) => Promise<void>
  deleteAlert: (id: number) => Promise<void>
  toggleAlert: (id: number, enabled: boolean) => Promise<void>
  fetchHistory: (limit?: number, offset?: number) => Promise<void>
  acknowledgeAlert: (id: number) => Promise<void>
  acknowledgeAll: () => Promise<void>
  fetchUnacknowledgedCount: () => Promise<void>
  fetchSettings: () => Promise<void>
  updateSettings: (settings: AlertGlobalSettings) => Promise<void>
  incrementUnacknowledged: () => void
}

// ---------- Store ----------

export const useAlertStore = create<AlertStore>((set, get) => ({
  alerts: [],
  history: [],
  unacknowledgedCount: 0,
  globalSettings: null,

  fetchAlerts: async () => {
    try {
      const alerts = await invoke<AlertConfig[]>('get_alerts')
      set({ alerts })
    } catch (err) {
      console.error('Failed to fetch alerts:', err)
    }
  },

  createAlert: async (params) => {
    try {
      await invoke('create_alert', { params })
      await get().fetchAlerts()
    } catch (err) {
      console.error('Failed to create alert:', err)
      throw err
    }
  },

  updateAlert: async (id, params) => {
    try {
      await invoke('update_alert', { id, params })
      await get().fetchAlerts()
    } catch (err) {
      console.error('Failed to update alert:', err)
      throw err
    }
  },

  deleteAlert: async (id) => {
    try {
      await invoke('delete_alert', { id })
      set((s) => ({
        alerts: s.alerts.filter((a) => a.id !== id),
      }))
    } catch (err) {
      console.error('Failed to delete alert:', err)
      throw err
    }
  },

  toggleAlert: async (id, enabled) => {
    try {
      await invoke('toggle_alert', { id, enabled })
      set((s) => ({
        alerts: s.alerts.map((a) => (a.id === id ? { ...a, enabled } : a)),
      }))
    } catch (err) {
      console.error('Failed to toggle alert:', err)
      throw err
    }
  },

  fetchHistory: async (limit = 50, offset = 0) => {
    try {
      const history = await invoke<AlertHistoryEntry[]>('get_alert_history', { limit, offset })
      set({ history })
    } catch (err) {
      console.error('Failed to fetch alert history:', err)
    }
  },

  acknowledgeAlert: async (id) => {
    try {
      await invoke('acknowledge_alert', { id })
      set((s) => ({
        history: s.history.map((h) => (h.id === id ? { ...h, acknowledged: true } : h)),
        unacknowledgedCount: Math.max(0, s.unacknowledgedCount - 1),
      }))
    } catch (err) {
      console.error('Failed to acknowledge alert:', err)
      throw err
    }
  },

  acknowledgeAll: async () => {
    try {
      await invoke('acknowledge_all_alerts')
      set((s) => ({
        history: s.history.map((h) => ({ ...h, acknowledged: true })),
        unacknowledgedCount: 0,
      }))
    } catch (err) {
      console.error('Failed to acknowledge all alerts:', err)
      throw err
    }
  },

  fetchUnacknowledgedCount: async () => {
    try {
      const count = await invoke<number>('get_unacknowledged_count')
      set({ unacknowledgedCount: count })
    } catch (err) {
      console.error('Failed to fetch unacknowledged count:', err)
    }
  },

  fetchSettings: async () => {
    try {
      const settings = await invoke<AlertGlobalSettings>('get_alert_settings')
      set({ globalSettings: settings })
    } catch (err) {
      console.error('Failed to fetch alert settings:', err)
    }
  },

  updateSettings: async (settings) => {
    try {
      await invoke('update_alert_settings', { settings })
      set({ globalSettings: settings })
    } catch (err) {
      console.error('Failed to update alert settings:', err)
      throw err
    }
  },

  incrementUnacknowledged: () => {
    set((s) => ({ unacknowledgedCount: s.unacknowledgedCount + 1 }))
  },
}))
