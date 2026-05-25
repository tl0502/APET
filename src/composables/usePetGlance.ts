// Pet 窗口对 reminder 弹窗"首次出现"的 head glance 反应（看 2 秒，抬头 / 低头）。
//
// 2026-05-26 第二轮修订（用户 e2e 反馈）：
// - 触发节流：原版每条 REMINDER_FIRED_EVENT 都触发 glance（+ 30s 同 id dedup），
//   实测体感像"抽搐"；用户要求"只在弹窗出现时看，叠加不再重复"。
//   新触发：监听 pet-reminder:active（仅当 count 0→1 emit）+ 配合 pet-reminder:placement
//   决定方向。同 id 重 fire / 新条目叠卡 → 不再触发 glance。
// - 持续时长：vrm.playGlance 已升级为 240 + 1600 + 240 ms 三段式（240 ease-in 抬到 peak，
//   1600 hold "看着"，240 ease-out 回正），共 ~2 秒。
//
// 同时监听 pet-reminder:idle 防止 active 触发与 placement 到达之间窗口已关闭导致 stale glance。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  PET_REMINDER_PLACEMENT_EVENT,
  type PetReminderPlacement,
  type PetReminderPlacementPayload,
} from '@/types/reminder'
import type { VRMRuntime } from '@/services/vrm'

/** PetReminderOverlayApp 在 count 0→1 时 emit；Rust pet_overlay 收到后 reposition 并 emit placement。 */
const PET_REMINDER_ACTIVE_EVENT = 'pet-reminder:active'
/** count 1→0 时 emit。 */
const PET_REMINDER_IDLE_EVENT = 'pet-reminder:idle'

export function usePetGlance(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): void {
  let unlistenActive: UnlistenFn | null = null
  let unlistenIdle: UnlistenFn | null = null
  let unlistenPlacement: UnlistenFn | null = null
  // 默认 'above' — placement 事件 race 未到达时按上方处理
  const placement = ref<PetReminderPlacement>('above')
  // active 事件已收到、等待 placement 到达以触发 glance 的临时门控
  let pendingActive = false

  function triggerGlance() {
    if (!isEnabled()) return
    const actionId = placement.value === 'below' ? 'glance_down' : 'glance_up'
    runtime.playAction(actionId).catch((err) => {
      console.warn('[pet-glance] playAction failed:', err)
    })
  }

  onMounted(async () => {
    try {
      unlistenPlacement = await listen<PetReminderPlacementPayload>(
        PET_REMINDER_PLACEMENT_EVENT,
        (e) => {
          const direction = e.payload?.direction
          if (direction === 'above' || direction === 'below') {
            placement.value = direction
          }
          // 仅当 active 在等待中（count 0→1 触发的本轮 reposition）时才放 glance；
          // pet settle 触发的 placement 不放（避免移动 pet 后重复看）。
          if (pendingActive) {
            pendingActive = false
            triggerGlance()
          }
        },
      )
    } catch (e) {
      console.warn('[pet-glance] listen placement failed:', e)
    }
    try {
      unlistenActive = await listen(PET_REMINDER_ACTIVE_EVENT, () => {
        pendingActive = true
      })
    } catch (e) {
      console.warn('[pet-glance] listen active failed:', e)
    }
    try {
      unlistenIdle = await listen(PET_REMINDER_IDLE_EVENT, () => {
        pendingActive = false
      })
    } catch (e) {
      console.warn('[pet-glance] listen idle failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenActive?.()
    unlistenIdle?.()
    unlistenPlacement?.()
  })
}
