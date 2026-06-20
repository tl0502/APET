<script setup lang="ts">
// PetReminderBubble：桌宠头顶提醒卡片 overlay（spec 2026-05-25-pet-reminder-card-stack）。
//
// 2026-05-26 重写为单卡/叠卡模型，废弃原 expanded/collapsed/single-bubble 状态机：
// - count === 1：单张 ReminderCard（无 badge、无 ghost layer）
// - count > 1：top card 完整渲染 + ghost 细条（count===2 → 1 层，count>=3 → 2 层）+
//   右上角 overhang badge
// - 仅 top card 可交互；底层 ghost layers 不渲染文本/按钮
// - 顶卡退出：slide right + fade out 200ms；下一卡极轻 fade-in
//
// 2026-05-26 第二轮修订（用户 e2e 反馈结构错位 + 截断）：
// - 拆成 .reminder-bubble-stack（outer，positioning + 防 crop padding）和
//   .reminder-card-stack（inner，ghost 锚点 + 固定 240px 宽度）两层 wrap
// - Ghost 的 bottom: -8 / -14 现在相对 .reminder-card-stack（card 边）算，不再被 outer padding 干扰
// - ResizeObserver 改用 borderBoxSize 包含 padding，让 Rust 拿到真实窗口高度（之前少 26px 导致截断）
//
// 视觉源头：.superpowers/brainstorm/31012-1779717504/content/card-model-overview.html
// 所有 CSS 数值（card 240 / ghost 228×16,216×16 -8/-14 / α 0.7/0.5 / badge top-7 right-7 #5b7cf6）
// 严格对齐该 HTML 文件。

import { computed, ref, watch } from 'vue'
import { useReminderQueue } from '@/composables/useReminderQueue'
import { useReminderAnimation } from '@/composables/useReminderAnimation'

interface Props {
  /** pet-command overlay 打开时整体降透明度（spec §2 #6 dim 40%）。 */
  trayOpen?: boolean
}

withDefaults(defineProps<Props>(), { trayOpen: false })

const anim = useReminderAnimation()

const queue = useReminderQueue()

const {
  reminders,
  bubbleCount,
  onComplete,
  onSnooze,
  canSnooze,
  iconOf,
  SNOOZE_OPTIONS,
  MAX_SNOOZE_COUNT,
} = queue

const isStacked = computed(() => reminders.value.length > 1)

/** ghost 层数：count===2 → 1 层；count>=3 → 2 层（spec §4.1 用户决策选项 B）。 */
const ghostCount = computed(() => {
  const n = reminders.value.length
  if (n <= 1) return 0
  if (n === 2) return 1
  return 2
})

/** top card 列表（始终 0 或 1 张）：用 reminderId 作 key 让 TransitionGroup 在
 *  top 切换时对前者播 leave、对新者播极轻 enter。 */
const topCardList = computed(() =>
  reminders.value.length > 0 ? [reminders.value[0]] : [],
)

/** count 由 N → N+1 且 N+1 > 1 时触发 badge pop（spec §4.1 keyframe 200ms）。 */
watch(
  () => reminders.value.length,
  (newLen, oldLen) => {
    if (newLen > oldLen && newLen > 1) anim.triggerBadgePop()
  },
)

/** 暴露给父级 PetReminderOverlayApp：outer 元素用于 ResizeObserver。 */
const stackRef = ref<HTMLElement | null>(null)
defineExpose({ bubbleCount, stackEl: stackRef })
</script>

<template>
  <div
    ref="stackRef"
    class="reminder-bubble-stack"
    :class="{
      'reminder-bubble-stack--dimmed': trayOpen,
      'reminder-bubble-stack--stacked': isStacked,
    }"
  >
    <!-- inner wrap：固定 240px 宽，position: relative 给 ghost 做锚点；ghost 的 bottom 偏移
         相对本元素的下边，不被 outer padding 干扰 -->
    <div class="reminder-card-stack">
      <!-- Ghost 细条层（仅 count > 1；绝对定位在 .reminder-card-stack 下边外侧） -->
      <div
        v-for="i in ghostCount"
        :key="`ghost-${i}`"
        class="reminder-card-ghost"
        :class="`reminder-card-ghost--${i}`"
        aria-hidden="true"
      />

      <!-- Top card：TransitionGroup 在 top 切换时播 leave/enter -->
      <TransitionGroup name="reminder-card" tag="div" class="reminder-card-slot">
        <div
          v-for="bubble in topCardList"
          :key="bubble.payload.reminderId"
          class="reminder-card"
          :class="{ 'reminder-card--hard': bubble.payload.priority === 'hard' }"
        >
          <!-- 右上角 overhang badge（仅叠卡形态） -->
          <div
            v-if="isStacked"
            class="reminder-card__badge"
            :class="{ 'reminder-card__badge--pop': anim.badgePopActive.value }"
            aria-label="未处理提醒数"
          >
            {{ reminders.length }}
          </div>

          <div class="reminder-card__header">
            <span class="reminder-card__icon">{{ iconOf(bubble) }}</span>
            <div class="reminder-card__text">
              <div class="reminder-card__title">{{ bubble.payload.title }}</div>
              <div v-if="bubble.snoozeCount > 0" class="reminder-card__sub">
                已稍后 {{ bubble.snoozeCount }}/{{ MAX_SNOOZE_COUNT }}
              </div>
            </div>
          </div>

          <div class="reminder-card__actions" data-no-drag>
            <template v-if="!bubble.snoozeOpen">
              <button
                v-if="canSnooze(bubble)"
                type="button"
                class="reminder-card__btn"
                :disabled="bubble.busy"
                @click="bubble.snoozeOpen = true"
              >
                稍后
              </button>
              <button
                type="button"
                class="reminder-card__btn reminder-card__btn--primary"
                :disabled="bubble.busy"
                @click="onComplete(bubble)"
              >
                完成
              </button>
            </template>
            <template v-else>
              <button
                v-for="m in SNOOZE_OPTIONS"
                :key="m"
                type="button"
                class="reminder-card__btn"
                :disabled="bubble.busy"
                @click="onSnooze(bubble, m)"
              >
                {{ m }}
              </button>
            </template>
          </div>
        </div>
      </TransitionGroup>
    </div>
  </div>
</template>

<style scoped>
/* 视觉源头：.superpowers/brainstorm/31012-1779717504/content/card-model-overview.html
   桌面 overlay 始终走暗色风格（透明窗 + 任意背景上需保证可读性）；
   不接入 aipet 主题 token —— 这是设计意图，不是疏漏。 */

/* Outer：positioning + 防 crop padding。stack 自身 pointer-events: none 让透明区不拦下层；
   .reminder-card 自接 pointer-events: auto。 */
.reminder-bubble-stack {
  position: absolute;
  top: 0;
  left: 50%;
  transform: translateX(-50%);
  pointer-events: none;
  z-index: 5;
  transition: opacity 140ms ease;
}

/* 叠卡形态 padding 防 badge / ghost 被 crop；视觉重心右偏 ~3.5px 反向补偿（badge right: -7px）。
   padding 是 outer 的，inner 的 ghost 偏移不受影响。 */
.reminder-bubble-stack--stacked {
  padding: 8px 8px 18px 0;
  transform: translateX(calc(-50% + 3.5px));
}

.reminder-bubble-stack--dimmed {
  opacity: 0.4;
}

/* Inner wrap：固定 240px 宽，position: relative 让 ghost 的 bottom 偏移相对 card 底边算。 */
.reminder-card-stack {
  position: relative;
  width: 240px;
}

.reminder-card-slot {
  position: relative;
  z-index: 2;
}

.reminder-card {
  pointer-events: auto;
  position: relative;
  width: 240px;
  padding: 10px 12px;
  background: rgba(40, 40, 48, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 14px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: #e8e8ea;
  box-sizing: border-box;
}

.reminder-card--hard {
  border-color: color-mix(in srgb, #5b7cf6 60%, rgba(255, 255, 255, 0.1));
  box-shadow: 0 8px 24px rgba(91, 124, 246, 0.3), 0 2px 6px rgba(0, 0, 0, 0.3);
}

/* Ghost 细条：position absolute 相对 .reminder-card-stack；
   width / α / bottom 严格按 brainstorm HTML（240→228→216；α 0.7→0.5；bottom -8 / -14）。 */
.reminder-card-ghost {
  position: absolute;
  left: 50%;
  height: 16px;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  pointer-events: none;
}

.reminder-card-ghost--1 {
  width: 228px;
  bottom: -8px;
  transform: translateX(-50%);
  background: rgba(38, 38, 50, 0.7);
  z-index: 1;
}

.reminder-card-ghost--2 {
  width: 216px;
  bottom: -14px;
  transform: translateX(-50%);
  background: rgba(32, 32, 44, 0.5);
  z-index: 0;
}

/* 右上角 overhang badge（spec §4.1：top:-7 right:-7 #5b7cf6 box-shadow） */
.reminder-card__badge {
  position: absolute;
  top: -7px;
  right: -7px;
  min-width: 20px;
  height: 20px;
  padding: 0 6px;
  background: #5b7cf6;
  color: #fff;
  border-radius: 10px;
  font-size: 10px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(91, 124, 246, 0.5);
  z-index: 10;
  box-sizing: border-box;
}

.reminder-card__badge--pop {
  animation: badge-pop 200ms ease;
}

@keyframes badge-pop {
  0% { transform: scale(1); }
  50% { transform: scale(1.3); }
  100% { transform: scale(1); }
}

.reminder-card__header {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.reminder-card__icon {
  font-size: 16px;
  flex-shrink: 0;
  margin-top: 1px;
  line-height: 1.2;
}

.reminder-card__text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.reminder-card__title {
  font-size: 13px;
  font-weight: 600;
  color: #eee;
  line-height: 1.35;
  word-break: break-word;
}

.reminder-card__sub {
  font-size: 11px;
  color: #777;
}

.reminder-card__actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 2px;
}

.reminder-card__btn {
  appearance: none;
  -webkit-appearance: none;
  font: inherit;
  font-size: 10px;
  font-weight: 500;
  padding: 3px 9px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  background: rgba(255, 255, 255, 0.06);
  color: #ccc;
  cursor: pointer;
  transition: background-color 120ms ease, color 120ms ease, border-color 120ms ease;
}

.reminder-card__btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  border-color: rgba(255, 255, 255, 0.25);
}

.reminder-card__btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.reminder-card__btn--primary {
  background: #5b7cf6;
  border-color: #5b7cf6;
  color: #fff;
}

.reminder-card__btn--primary:hover:not(:disabled) {
  background: color-mix(in srgb, #5b7cf6 88%, #000);
  border-color: color-mix(in srgb, #5b7cf6 88%, #000);
  color: #fff;
}

/* Top card 退场：slide right + fade out 200ms；新卡 enter：极轻 fade-in 100ms。
   leave 时 absolute 防止占位影响 ghost layer 排版。 */
.reminder-card-leave-active {
  transition: opacity 200ms ease-out, transform 200ms ease-out;
  position: absolute !important;
  top: 0;
  left: 0;
  right: 0;
}

.reminder-card-leave-to {
  opacity: 0;
  transform: translateX(40px);
}

.reminder-card-enter-active {
  transition: opacity 100ms ease-out;
}

.reminder-card-enter-from {
  opacity: 0;
}
</style>
