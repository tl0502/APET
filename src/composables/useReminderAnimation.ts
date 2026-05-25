// useReminderAnimation：PetReminderBubble 动画状态机（P2 职责拆分，2026-05-25）。
//
// 职责：
// - TransitionGroup 切换语义（reason → class 前缀）：fired / badge-bump / collapse-merge /
//   page-next / single-restore
// - badge-pop：collapsedCount 增长时 badge 数字短暂 scale 1.25 后回 1
// - 250ms 后自动 reset reason 回 'fired'（避免下次 enter/leave 用错套 class）
//
// 与 useReminderQueue 解耦：queue 通过 onPushReason/onRemoveReason 回调通知，
// 本 composable 不持有队列引用。

import { computed, onBeforeUnmount, ref } from 'vue'

export type TransitionReason =
  | 'fired'
  | 'badge-bump'
  | 'collapse-merge'
  | 'page-next'
  | 'single-restore'

const REASON_RESET_MS = 250
const BADGE_POP_MS = 180

export function useReminderAnimation() {
  const currentReason = ref<TransitionReason>('fired')
  const badgePopActive = ref(false)

  let reasonResetTimer: number | null = null
  let badgePopTimer: number | null = null

  function setReason(r: TransitionReason) {
    if (reasonResetTimer !== null) {
      window.clearTimeout(reasonResetTimer)
      reasonResetTimer = null
    }
    currentReason.value = r
    reasonResetTimer = window.setTimeout(() => {
      currentReason.value = 'fired'
      reasonResetTimer = null
    }, REASON_RESET_MS) as unknown as number
  }

  function triggerBadgePop() {
    if (badgePopTimer !== null) {
      window.clearTimeout(badgePopTimer)
      badgePopTimer = null
    }
    badgePopActive.value = true
    badgePopTimer = window.setTimeout(() => {
      badgePopActive.value = false
      badgePopTimer = null
    }, BADGE_POP_MS) as unknown as number
  }

  const transitionName = computed(() => `bubble-${currentReason.value}`)

  onBeforeUnmount(() => {
    if (reasonResetTimer !== null) window.clearTimeout(reasonResetTimer)
    if (badgePopTimer !== null) window.clearTimeout(badgePopTimer)
  })

  return {
    currentReason,
    transitionName,
    badgePopActive,
    setReason,
    triggerBadgePop,
  }
}
