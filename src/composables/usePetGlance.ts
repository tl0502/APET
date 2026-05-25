// Pet 窗口对 reminder:fired 的 head glance 反应（抬头看上 / 低头看下）。
// 替代 usePetReaction.ts（#29 仅 'nod'）；spec 2026-05-25-pet-reminder-card-stack §5。
//
// Placement 源：Rust pet_overlay 模块在 reminder overlay reposition 成功后 emit
// PET_REMINDER_PLACEMENT_EVENT，本 composable 维护当前 placement state，下一次
// REMINDER_FIRED_EVENT 时按方向选 playAction id：
//   - placement === 'above' → playAction('glance_up')   抬头（兼容旧 'nod' 行为）
//   - placement === 'below' → playAction('glance_down') 低头
// 默认值 'above' 保证 placement 事件 race 未到达时行为与旧版一致。
//
// dedup 阈值与历史一致（30s）：同 reminderId 在 30s 内重复 fire 不重复 playAction。
// 这条与 PetReminderBubble 的同 id dedup（移到 newest + 刷新 payload）独立，
// 各自负责"气泡 UX"和"角色动作"两条线。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  PET_REMINDER_PLACEMENT_EVENT,
  REMINDER_FIRED_EVENT,
  type PetReminderPlacement,
  type PetReminderPlacementPayload,
  type ReminderFiredPayload,
} from '@/types/reminder'
import type { VRMRuntime } from '@/services/vrm'

const DEDUP_WINDOW_MS = 30_000

export function usePetGlance(
  runtime: VRMRuntime,
  isEnabled: () => boolean = () => true,
): void {
  let unlistenFired: UnlistenFn | null = null
  let unlistenPlacement: UnlistenFn | null = null
  const lastFiredAt = new Map<string, number>()
  // 默认 'above' — 兼容 placement 事件 race 未到达 / Rust 模块尚未 emit 过的初始态
  const placement = ref<PetReminderPlacement>('above')

  onMounted(async () => {
    if (!isEnabled()) return
    try {
      unlistenPlacement = await listen<PetReminderPlacementPayload>(
        PET_REMINDER_PLACEMENT_EVENT,
        (e) => {
          const direction = e.payload?.direction
          if (direction === 'above' || direction === 'below') {
            placement.value = direction
          }
        },
      )
    } catch (e) {
      console.warn('[pet-glance] listen placement failed:', e)
    }
    try {
      unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, (e) => {
        if (!isEnabled()) return
        const payload = e.payload
        if (!payload?.reminderId) return
        const now = performance.now()
        const prev = lastFiredAt.get(payload.reminderId)
        if (prev != null && now - prev < DEDUP_WINDOW_MS) return
        lastFiredAt.set(payload.reminderId, now)
        const actionId = placement.value === 'below' ? 'glance_down' : 'glance_up'
        runtime.playAction(actionId).catch((err) => {
          console.warn('[pet-glance] playAction failed:', err)
        })
      })
    } catch (e) {
      console.warn('[pet-glance] listen fired failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenFired?.()
    unlistenPlacement?.()
  })
}
