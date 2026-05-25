// #29 桌宠对 reminder:fired 事件的反应（点头）。
// #23 接 reaction_table 时改内部 mapping，外部接口不动。
//
// 2026-05-24 第三轮：同 reminderId 30s 内重复 fire 不重复 playAction。原 reminder.rs::fire
// 每次都会 emit REMINDER_FIRED_EVENT（包括 snooze 后再 fire / 周期 reminder 再次到点），
// 同一 reminderId 短时间内反复触发完整动作会让用户感觉桌宠"在抽搐"。
// dedup 阈值 30s（M2 经验值，M3+ 接 reaction_table 时可改播 attention 而非 skip）。
//
// PetReminderBubble 内的同 id dedup（移到 newest + 重置 auto-dismiss）和本文件独立，
// 各自负责"气泡 UX"和"角色动作"两条线。

import { onBeforeUnmount, onMounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { REMINDER_FIRED_EVENT, type ReminderFiredPayload } from '@/types/reminder'
import type { VRMRuntime } from '@/services/vrm'

const DEDUP_WINDOW_MS = 30_000

export function usePetReaction(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): void {
  let unlistenFired: UnlistenFn | null = null
  const lastFiredAt = new Map<string, number>()

  onMounted(async () => {
    if (!isEnabled()) return
    try {
      unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, (e) => {
        if (!isEnabled()) return
        const payload = e.payload
        if (!payload?.reminderId) return
        const now = performance.now()
        const prev = lastFiredAt.get(payload.reminderId)
        if (prev != null && now - prev < DEDUP_WINDOW_MS) return
        lastFiredAt.set(payload.reminderId, now)
        runtime.playAction('nod').catch((err) => {
          console.warn('[pet-reaction] playAction failed:', err)
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
