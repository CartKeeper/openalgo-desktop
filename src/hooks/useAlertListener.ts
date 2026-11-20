import { listen } from '@tauri-apps/api/event'
import { useEffect, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { useAlertStore, type AlertTriggeredPayload } from '@/stores/alertStore'

/**
 * Hook that listens for Tauri `alert_triggered` events from the Rust backend
 * and delivers visual/audio notifications.
 */
export function useAlertListener() {
  // Keep a live ref to the router navigate fn so the long-lived Tauri event
  // listener (registered once) always calls the current navigator without
  // needing to re-subscribe.
  const navigate = useNavigate()
  const navigateRef = useRef(navigate)
  navigateRef.current = navigate

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
              navigateRef.current(`/fundamentals/${payload.symbol}`)
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
