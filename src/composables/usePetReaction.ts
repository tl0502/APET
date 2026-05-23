// #29 桌宠对 reminder:fired 事件的反应（点头）。
// #23 接 reaction_table 时改内部 mapping，外部接口不动。

import { onBeforeUnmount, onMounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { REMINDER_FIRED_EVENT, type ReminderFiredPayload } from '@/types/reminder'
import type { VRMRuntime } from '@/services/vrm'

export function usePetReaction(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): void {
  let unlistenFired: UnlistenFn | null = null

  onMounted(async () => {
    if (!isEnabled()) return
    try {
      unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, () => {
        if (!isEnabled()) return
        runtime.playAction('nod').catch((e) => {
          console.warn('[pet-reaction] playAction failed:', e)
        })
      })
    } catch (e) {
      console.warn('[pet-reaction] listen failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenFired?.()
  })
}
