// useReminderQueue：PetReminderBubble 队列管理。
//
// 2026-05-26 简化（spec 2026-05-25-pet-reminder-card-stack §3-4，plan Task 4）：
// - 移除 auto-dismiss（用户手动 complete/snooze；无 timer/hover/reconcile 逻辑）
// - 移除 expanded/collapsed displayMode 状态机（count 直接决定单卡/叠卡，由组件层判定）
// - 移除 onPushReason/onRemoveReason/onBadgePop 回调机制
//   （badge pop 由组件层 watch reminders.length 触发，详见 useReminderAnimation）
//
// 剩余职责：
// - reminder 队列（newest-first）的增删去重
// - IPC listen（REMINDER_FIRED_EVENT → pushBubble）
// - 用户交互（complete / snooze；调 Rust IPC）
// - 同 reminderId 重 fire 时移到顶 + 刷新 payload（不重播入场动画）

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { completeReminder, snoozeReminder } from '@/services/reminder'
import {
  MAX_SNOOZE_COUNT,
  REMINDER_FIRED_EVENT,
  SNOOZE_OPTIONS,
  type ReminderFiredPayload,
  type SnoozeMinutes,
} from '@/types/reminder'

export interface BubbleState {
  /** 稳定 key = reminderId（同 id 重 fire 不重播 enter 动画）。 */
  key: string
  payload: ReminderFiredPayload
  snoozeOpen: boolean
  busy: boolean
  snoozeCount: number
}

export function useReminderQueue() {
  const reminders = ref<BubbleState[]>([])
  let unlistenFired: UnlistenFn | null = null

  const bubbleCount = computed(() => reminders.value.length)

  function createBubble(payload: ReminderFiredPayload): BubbleState {
    return {
      key: payload.reminderId,
      payload,
      snoozeOpen: false,
      busy: false,
      snoozeCount: payload.snoozeCount,
    }
  }

  function removeBubble(b: BubbleState) {
    const idx = reminders.value.indexOf(b)
    if (idx < 0) return
    reminders.value.splice(idx, 1)
  }

  function pushBubble(payload: ReminderFiredPayload) {
    const existingIdx = reminders.value.findIndex(
      (b) => b.payload.reminderId === payload.reminderId,
    )
    if (existingIdx >= 0) {
      // 同 id 去重 + 移到顶（newest-first）+ 刷新 payload
      const [existing] = reminders.value.splice(existingIdx, 1)
      existing.payload = payload
      existing.snoozeCount = payload.snoozeCount
      reminders.value.unshift(existing)
      return
    }
    reminders.value.unshift(createBubble(payload))
  }

  async function onComplete(b: BubbleState) {
    b.busy = true
    try {
      await completeReminder(b.payload.reminderId)
      removeBubble(b)
    } catch (e) {
      console.error('[reminder-queue] complete failed:', e)
    } finally {
      b.busy = false
    }
  }

  async function onSnooze(b: BubbleState, minutes: SnoozeMinutes) {
    b.busy = true
    try {
      await snoozeReminder(b.payload.reminderId, minutes)
      removeBubble(b)
    } catch (e) {
      console.error('[reminder-queue] snooze failed:', e)
    } finally {
      b.busy = false
    }
  }

  function canSnooze(b: BubbleState): boolean {
    return b.snoozeCount < MAX_SNOOZE_COUNT
  }

  function iconOf(b: BubbleState): string {
    return b.payload.priority === 'hard' ? '🔔' : '💭'
  }

  onMounted(async () => {
    try {
      unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, (e) => {
        if (e.payload) pushBubble(e.payload)
      })
    } catch (e) {
      console.warn('[reminder-queue] listen failed:', e)
    }
  })

  onBeforeUnmount(() => {
    unlistenFired?.()
  })

  return {
    reminders,
    bubbleCount,
    removeBubble,
    onComplete,
    onSnooze,
    canSnooze,
    iconOf,
    SNOOZE_OPTIONS,
    MAX_SNOOZE_COUNT,
  }
}
