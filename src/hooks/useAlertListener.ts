import { listen } from '@tauri-apps/api/event'
import { useEffect } from 'react'
import { toast } from 'sonner'
import { useAlertStore, type AlertTriggeredPayload } from '@/stores/alertStore'

/**
 * Hook that listens for Tauri `alert_triggered` events from the Rust backend
 * and delivers visual/audio notifications.
 */
export function useAlertListener() {
  useEffect(() => {
    let unlisten: (() => void) | undefined

    listen<AlertTriggeredPayload>('alert_triggered', (event) => {
      const payload = event.payload

      // Determine toast variant and duration based on alert type
      const isRisingStar = payload.alert_type === 'rising_star'
      const isMovement = payload.alert_type === 'movement'
      const duration = isRisingStar ? 10000 : 8000

      if (isRisingStar) {
        toast.info(payload.title, {
          description: payload.message,
          duration,
          action: {
            label: 'View',
            onClick: () => {
              window.location.hash = `/fundamentals/${payload.symbol}`
            },
          },
        })
      } else if (isMovement) {
        toast.error(payload.title, {
          description: payload.message,
          duration,
        })
      } else {
        // price_above, price_below, news, and other types
        toast.warning(payload.title, {
          description: payload.message,
          duration,
        })
      }

      // Play alert sound
      new Audio('/sounds/alert.mp3').play().catch(() => {})

      // Increment unacknowledged count
      useAlertStore.getState().incrementUnacknowledged()
    }).then((fn) => {
      unlisten = fn
    })

    return () => {
      if (unlisten) {
        unlisten()
      }
    }
  }, [])
}
