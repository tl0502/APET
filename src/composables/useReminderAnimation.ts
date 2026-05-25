// useReminderAnimation：PetReminderBubble badge pop 动画。
//
// 2026-05-26 简化（spec 2026-05-25-pet-reminder-card-stack §4.1，plan Task 4）：
// - 移除 TransitionGroup reason 状态机（fired / badge-bump / collapse-merge / page-next /
//   single-restore），原因：新模型只有单卡/叠卡两种形态，count > 1 时叠卡顶层不重入场，
//   无需切换 transition class
// - 保留 badge pop：count 增加时短暂 scale 1→1.3→1，给用户视觉感知"又新增一条"
//
// 触发方式：组件层 watch reminders.length，count 从 N → N+1（且 N+1 > 1）时调 triggerBadgePop。

import { onBeforeUnmount, ref } from 'vue'

/** 与 spec §4.1 badge pop 时长一致：200ms。 */
const BADGE_POP_MS = 200

export function useReminderAnimation() {
  const badgePopActive = ref(false)
  let badgePopTimer: number | null = null

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

  onBeforeUnmount(() => {
    if (badgePopTimer !== null) window.clearTimeout(badgePopTimer)
  })

  return {
    badgePopActive,
    triggerBadgePop,
  }
}
