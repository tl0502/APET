// useReminderQueue：PetReminderBubble 队列管理（P2 职责拆分，2026-05-25）。
//
// 职责：
// - reminder 队列（newest-first）的增删去重
// - IPC listen（REMINDER_FIRED_EVENT → pushBubble）
// - auto-dismiss timer（8s + hover/snoozeOpen 暂停 + collapsed 模式下不可见 timer 暂停）
// - hover / snooze / complete 用户交互处理（调 Rust IPC）
// - displayMode（expanded / collapsed）状态维护
// - isCollapsed 计算（依赖 displayMode + trayOpen getter）
// - 向外暴露 bubbleCount（供 PetReminderOverlayApp 监听 active/idle 信号）
//
// 与 useReminderAnimation 解耦：通过 onPushReason / onBadgePop 回调通知动画层，
// 不持有 transitionName / badgePopActive 的引用。

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
import type { TransitionReason } from '@/composables/useReminderAnimation'

const MAX_EXPANDED = 2
const AUTO_DISMISS_MS = 8000

export interface BubbleState {
  /** 稳定 key = reminderId（P3：不含 Date.now()，collapsed 模式内容切换不重播 enter 动画）。 */
  key: string
  payload: ReminderFiredPayload
  snoozeOpen: boolean
  busy: boolean
  hover: boolean
  timer: number | null
  snoozeCount: number
}

type DisplayMode = 'expanded' | 'collapsed'

export interface UseReminderQueueOptions {
  /** 获取当前 trayOpen 状态（component prop getter）。 */
  trayOpen: () => boolean
  /** push 成功后回调，传递动画语义（collapsed-merge / badge-bump / fired）。 */
  onPushReason: (r: TransitionReason) => void
  /** badge-bump 时需要额外触发 badge pop 动画。 */
  onBadgePop: () => void
  /** removeBubble 成功后回调，传递动画语义（page-next / single-restore / fired）。 */
  onRemoveReason: (r: TransitionReason) => void
}

export function useReminderQueue(opts: UseReminderQueueOptions) {
  const reminders = ref<BubbleState[]>([])
  const displayMode = ref<DisplayMode>('expanded')
  let unlistenFired: UnlistenFn | null = null

  const isCollapsed = computed(() => {
    if (opts.trayOpen() && reminders.value.length >= 1) return true
    return (
      reminders.value.length > MAX_EXPANDED ||
      (displayMode.value === 'collapsed' && reminders.value.length > 1)
    )
  })

  const bubbleCount = computed(() => reminders.value.length)

  function createBubble(payload: ReminderFiredPayload): BubbleState {
    return {
      key: payload.reminderId,
      payload,
      snoozeOpen: false,
      busy: false,
      hover: false,
      timer: null,
      snoozeCount: payload.snoozeCount,
    }
  }

  /** collapsed 模式下清掉所有非 [0] 气泡的 timer（P4：不可见 timer 不应异步减少队列计数）。 */
  function pauseNonVisibleTimers() {
    if (!isCollapsed.value) return
    for (let i = 1; i < reminders.value.length; i++) {
      const b = reminders.value[i]
      if (b.timer !== null) {
        window.clearTimeout(b.timer)
        b.timer = null
      }
    }
  }

  function startAutoDismiss(b: BubbleState) {
    if (b.timer !== null) {
      window.clearTimeout(b.timer)
      b.timer = null
    }
    // collapsed mode 下只有 reminders[0] 可见；不可见的不启动 timer。
    const willBeVisible = !isCollapsed.value || b === reminders.value[0]
    if (!willBeVisible) return
    b.timer = window.setTimeout(() => {
      if (!b.hover && !b.snoozeOpen) removeBubble(b)
    }, AUTO_DISMISS_MS) as unknown as number
  }

  function removeBubble(b: BubbleState) {
    if (b.timer !== null) {
      window.clearTimeout(b.timer)
      b.timer = null
    }
    const wasCollapsed = isCollapsed.value
    const idx = reminders.value.indexOf(b)
    if (idx < 0) return
    reminders.value.splice(idx, 1)
    // count ≤ 1 → 重置 expanded（单卡用普通气泡形态）
    if (reminders.value.length <= 1) displayMode.value = 'expanded'
    // collapsed 翻页：新 [0] 变可见，启动 auto-dismiss
    if (reminders.value.length > 0 && isCollapsed.value) {
      startAutoDismiss(reminders.value[0])
    }
    // 动画 reason
    let reason: TransitionReason = 'fired'
    if (wasCollapsed && reminders.value.length === 1) {
      reason = 'single-restore'
    } else if (wasCollapsed && reminders.value.length > 1) {
      reason = 'page-next'
    }
    opts.onRemoveReason(reason)
  }

  function pushBubble(payload: ReminderFiredPayload) {
    const existingIdx = reminders.value.findIndex(
      (b) => b.payload.reminderId === payload.reminderId,
    )
    if (existingIdx >= 0) {
      // 去重 + 移到 newest 头部 + 更新 payload
      // 同 id 重 fire：key 不变，TransitionGroup 不触发 enter → 不需要 setReason
      const [existing] = reminders.value.splice(existingIdx, 1)
      existing.payload = payload
      existing.snoozeCount = payload.snoozeCount
      reminders.value.unshift(existing)
      startAutoDismiss(existing)
      return
    }
    const lenBefore = reminders.value.length
    const b = createBubble(payload)
    reminders.value.unshift(b)
    startAutoDismiss(b)
    if (reminders.value.length > MAX_EXPANDED) displayMode.value = 'collapsed'
    // P4：清掉不可见气泡的 timer
    pauseNonVisibleTimers()
    // 动画 reason
    if (lenBefore === MAX_EXPANDED) {
      opts.onPushReason('collapse-merge')
    } else if (lenBefore > MAX_EXPANDED) {
      opts.onPushReason('badge-bump')
      opts.onBadgePop()
    } else {
      opts.onPushReason('fired')
    }
  }

  function onMouseEnter(b: BubbleState) {
    b.hover = true
  }

  function onMouseLeave(b: BubbleState) {
    b.hover = false
    startAutoDismiss(b)
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

  // trayOpen 变化：可能切换 isCollapsed → 重算 timer 起停
  // 注意：由于 trayOpen 是 getter，无法直接 watch；由调用方在 watch(trayOpen) 时调此函数
  function reconcileTimers() {
    for (const b of reminders.value) {
      startAutoDismiss(b)
    }
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
    reminders.value.forEach((b) => {
      if (b.timer !== null) window.clearTimeout(b.timer)
    })
  })

  return {
    reminders,
    isCollapsed,
    bubbleCount,
    removeBubble,
    onMouseEnter,
    onMouseLeave,
    onComplete,
    onSnooze,
    canSnooze,
    iconOf,
    reconcileTimers,
    SNOOZE_OPTIONS,
    MAX_SNOOZE_COUNT,
  }
}
