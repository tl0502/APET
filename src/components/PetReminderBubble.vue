<script setup lang="ts">
// PetReminderBubble：桌宠头顶气泡 overlay（issue #22）。
//
// 2026-05-25 P2 职责拆分：
// - useReminderQueue：队列增删去重 + IPC listen + complete/snooze（auto-dismiss 已于 2026-05-26 移除）
// - useReminderAnimation：badge pop（transition reason 状态机已于 2026-05-26 移除）
// - 本 SFC 只负责：展示状态 + DOM 渲染
//
// 当前展示规则（plan Task 4 过渡态；Task 5 将整段重写为单卡/叠卡新模型）：
// - count = 0：不显示
// - count = 1：单气泡（无 badge）
// - count > 1：collapsed 翻页模式，只显示 reminders[0]（最新）+ 左上 count badge

import { computed, ref, watch } from 'vue'
import { useReminderQueue } from '@/composables/useReminderQueue'
import { useReminderAnimation } from '@/composables/useReminderAnimation'

interface Props {
  /** pet-command overlay 打开时整体降透明度（dim only，不再强制 collapsed —— spec §2 #6） */
  trayOpen?: boolean
}

const props = withDefaults(defineProps<Props>(), { trayOpen: false })

const anim = useReminderAnimation()

const queue = useReminderQueue()

const { reminders, bubbleCount, removeBubble, onComplete, onSnooze, canSnooze, iconOf,
  SNOOZE_OPTIONS, MAX_SNOOZE_COUNT } = queue

/** 叠卡形态（count > 1）：当前过渡态仍走旧 collapsed slot 视觉；Task 5 改为新的单卡/叠卡分层。 */
const isStacked = computed(() => reminders.value.length > 1)

const collapsedCount = computed(() => reminders.value.length)

/** count 由 N → N+1 且 N+1 > 1 时触发 badge pop（spec §4.1）。 */
watch(
  () => reminders.value.length,
  (newLen, oldLen) => {
    if (newLen > oldLen && newLen > 1) anim.triggerBadgePop()
  },
)

/** 展示项列表，key 保证稳定：
 *  - 叠卡形态：固定 key 'stacked-slot'，内容指向 reminders[0]（同 id 重 fire 不重播 enter）
 *  - 单卡形态：key = reminderId
 */
const displayItems = computed(() => {
  if (reminders.value.length === 0) return []
  if (isStacked.value) {
    return [{ key: 'stacked-slot', bubble: reminders.value[0], collapsed: true }]
  }
  return reminders.value.map((b) => ({
    key: b.payload.reminderId,
    bubble: b,
    collapsed: false,
  }))
})

// stackEl 暴露给父级 PetReminderOverlayApp 供 ResizeObserver 观测。
// 注：<script setup> + defineExpose 下 template ref 的 $el 不会自动暴露，需显式 expose。
// TransitionGroup（tag="div"）是内置组件，其 ref.value.$el 即 .reminder-bubble-stack div。
const tgRef = ref<{ $el: HTMLElement } | null>(null)

// 供父级 PetReminderOverlayApp watch → emit active/idle Tauri 事件
defineExpose({ bubbleCount, stackEl: computed(() => tgRef.value?.$el ?? null) })
</script>

<template>
  <TransitionGroup
    ref="tgRef"
    name="bubble"
    tag="div"
    class="reminder-bubble-stack"
    :class="{ 'reminder-bubble-stack--dimmed': trayOpen, 'reminder-bubble-stack--collapsed': isStacked }"
  >
    <!-- 叠卡形态 key 固定为 'stacked-slot'，不触发 TransitionGroup enter/leave -->
    <div
      v-for="item in displayItems"
      :key="item.key"
      class="reminder-bubble"
      :class="{
        'reminder-bubble--hard': item.bubble.payload.priority === 'hard',
        'reminder-bubble--collapsed': item.collapsed,
      }"
    >
      <!-- 叠卡模式左上 count badge -->
      <div
        v-if="item.collapsed"
        class="reminder-bubble__count-badge"
        :class="{ 'badge-pop': anim.badgePopActive.value }"
        aria-label="未处理提醒数"
      >
        {{ collapsedCount }}
      </div>

      <div class="reminder-bubble__body">
        <span class="reminder-bubble__icon">{{ iconOf(item.bubble) }}</span>
        <div class="reminder-bubble__text">
          <span class="reminder-bubble__title">{{ item.bubble.payload.title }}</span>
          <span v-if="item.bubble.snoozeCount > 0" class="reminder-bubble__sub">
            已稍后 {{ item.bubble.snoozeCount }}/{{ MAX_SNOOZE_COUNT }}
          </span>
        </div>
      </div>

      <div class="reminder-bubble__actions" data-no-drag>
        <template v-if="!item.bubble.snoozeOpen">
          <button
            type="button"
            class="reminder-bubble__btn reminder-bubble__btn--primary"
            :disabled="item.bubble.busy"
            @click="onComplete(item.bubble)"
          >
            完成
          </button>
          <button
            v-if="canSnooze(item.bubble)"
            type="button"
            class="reminder-bubble__btn"
            :disabled="item.bubble.busy"
            @click="item.bubble.snoozeOpen = true"
          >
            稍后
          </button>
        </template>
        <template v-else>
          <button
            v-for="m in SNOOZE_OPTIONS"
            :key="m"
            type="button"
            class="reminder-bubble__btn"
            :disabled="item.bubble.busy"
            @click="onSnooze(item.bubble, m)"
          >
            {{ m }}
          </button>
        </template>
        <button
          type="button"
          class="reminder-bubble__btn reminder-bubble__btn--ghost"
          :disabled="item.bubble.busy"
          aria-label="关闭"
          @click="removeBubble(item.bubble)"
        >
          ✕
        </button>
      </div>
    </div>
  </TransitionGroup>
</template>

<style scoped>
.reminder-bubble-stack {
  /* 2026-05-25 精修：absolute + top:0 + left:50% + translateX(-50%) 让气泡在 320px
     overlay 窗内水平居中。Rust anchor 已把窗口中心对齐 pet 中心（target_x = pet_center - w/2），
     因此气泡与 pet 中轴线对齐。stack 自身 pointer-events: none，透明区不拦下层操作；
     .reminder-bubble 自己 pointer-events: auto 接 hover/click。 */
  position: absolute;
  top: 0;
  left: 50%;
  transform: translateX(-50%);
  width: fit-content;
  max-width: 280px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  pointer-events: none;
  z-index: 5;
  transition: opacity 140ms var(--aipet-ease-standard);
}

/* collapsed 模式：给 badge（top:-5px; left:-5px）预留顶部安全空间，
   并微调水平位置补偿 badge 左悬挂造成的视觉重心偏移（~2.5px）。
   transform 覆盖基础 translateX(-50%) —— 同选择器权重，顺序靠后生效。
   expanded 状态不受影响。 */
.reminder-bubble-stack--collapsed {
  padding-top: 8px;
  transform: translateX(calc(-50% + 2.5px));
}

.reminder-bubble-stack--dimmed {
  opacity: 0.4;
}

.reminder-bubble {
  pointer-events: auto;
  position: relative;
  min-width: 200px;
  max-width: 280px;
  padding: 7px 9px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: 1px solid var(--aipet-color-border-strong, var(--aipet-color-border));
  border-radius: 16px;
  box-shadow: 0 8px 24px -8px rgba(0, 0, 0, 0.18), 0 2px 6px -2px rgba(0, 0, 0, 0.08);
  backdrop-filter: blur(8px);
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.reminder-bubble--hard {
  border-color: color-mix(in srgb, var(--aipet-color-primary) 60%, var(--aipet-color-border));
  box-shadow: 0 8px 24px -8px color-mix(in srgb, var(--aipet-color-primary) 30%, transparent),
    0 2px 6px -2px rgba(0, 0, 0, 0.08);
}

.reminder-bubble--collapsed .reminder-bubble__body {
  padding-left: 18px; /* 留位给 badge */
}

.reminder-bubble__count-badge {
  position: absolute;
  top: -5px;
  left: -5px;
  min-width: 20px;
  height: 20px;
  padding: 0 6px;
  background: var(--aipet-color-primary);
  color: #fff;
  border-radius: 10px;
  font-size: 10px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.2);
  z-index: 1;
}

.reminder-bubble__body {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.reminder-bubble__icon {
  flex: 0 0 auto;
  font-size: 18px;
  line-height: 1.2;
}

.reminder-bubble__text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.reminder-bubble__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.35;
  word-break: break-word;
}

.reminder-bubble__sub {
  font-size: 11px;
  color: var(--aipet-color-text-3);
}

.reminder-bubble__actions {
  display: flex;
  align-items: center;
  gap: 4px;
  justify-content: flex-end;
  flex-wrap: wrap;
}

.reminder-bubble__btn {
  appearance: none;
  -webkit-appearance: none;
  border: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-bg);
  color: var(--aipet-color-text-2);
  font: inherit;
  font-size: 10px;
  font-weight: 500;
  padding: 2px 7px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    color var(--aipet-duration-fast) var(--aipet-ease-standard),
    border-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.reminder-bubble__btn:hover:not(:disabled) {
  border-color: var(--aipet-color-border-strong, var(--aipet-color-border));
  color: var(--aipet-color-text-1);
  background: var(--aipet-color-surface);
}

.reminder-bubble__btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.reminder-bubble__btn--primary {
  background: var(--aipet-color-primary);
  border-color: var(--aipet-color-primary);
  color: #fff;
}

.reminder-bubble__btn--primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--aipet-color-primary) 88%, #000);
  color: #fff;
  border-color: color-mix(in srgb, var(--aipet-color-primary) 88%, #000);
}

.reminder-bubble__btn--ghost {
  border-color: transparent;
  background: transparent;
  padding: 3px 6px;
  font-size: 12px;
  line-height: 1;
}

.reminder-bubble__btn--ghost:hover:not(:disabled) {
  background: var(--aipet-color-surface);
  border-color: var(--aipet-color-border);
}

.bubble-fired-enter-active,
.bubble-fired-leave-active {
  transition: opacity 220ms var(--aipet-ease-standard),
    transform 220ms var(--aipet-ease-standard);
}

.bubble-fired-enter-from,
.bubble-fired-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

.bubble-fired-move {
  transition: transform 220ms var(--aipet-ease-standard);
}

/* collapse-merge：从 expanded 双卡进入 collapsed 单卡的过渡。 */
.bubble-collapse-merge-enter-active {
  transition: opacity 220ms var(--aipet-ease-standard),
    transform 220ms var(--aipet-ease-standard);
}
.bubble-collapse-merge-enter-from {
  opacity: 0;
  transform: scale(0.92);
}
.bubble-collapse-merge-leave-active {
  transition: opacity 180ms var(--aipet-ease-standard);
}
.bubble-collapse-merge-leave-to {
  opacity: 0;
}
.bubble-collapse-merge-move {
  transition: transform 200ms var(--aipet-ease-standard);
}

/* page-next：collapsed 内"翻页"动画。旧 bubble 向右滑出，新 reminders[0] 从左滑入。 */
.bubble-page-next-leave-active {
  transition: opacity 200ms, transform 200ms var(--aipet-ease-standard);
}
.bubble-page-next-leave-to {
  opacity: 0;
  transform: translateX(12px);
}
.bubble-page-next-enter-active {
  transition: opacity 200ms, transform 200ms var(--aipet-ease-standard);
}
.bubble-page-next-enter-from {
  opacity: 0;
  transform: translateX(-12px);
}
.bubble-page-next-move {
  transition: transform 200ms var(--aipet-ease-standard);
}

/* single-restore：collapsed 回到单卡。 */
.bubble-single-restore-enter-active {
  transition: opacity 220ms var(--aipet-ease-standard),
    transform 220ms var(--aipet-ease-standard);
}
.bubble-single-restore-enter-from {
  opacity: 0;
  transform: scale(1.05);
}
.bubble-single-restore-leave-active {
  transition: opacity 180ms var(--aipet-ease-standard);
}
.bubble-single-restore-leave-to {
  opacity: 0;
}

/* badge-bump：bubble 整张 NOT re-enter（empty enter class → instant render）。
   仅靠 .badge-pop class 让 count 数字 scale 1.25。 */
.bubble-badge-bump-enter-active,
.bubble-badge-bump-leave-active {
  transition: none;
}
.bubble-badge-bump-enter-from,
.bubble-badge-bump-leave-to {
  opacity: 1;
}
.bubble-badge-bump-move {
  transition: transform 0ms;
}

/* count badge 数字 pop 动画 —— 与 transition reason 独立。 */
.reminder-bubble__count-badge.badge-pop {
  animation: badge-pop 180ms var(--aipet-ease-standard);
}
@keyframes badge-pop {
  0% {
    transform: scale(1);
  }
  40% {
    transform: scale(1.25);
  }
  100% {
    transform: scale(1);
  }
}
</style>
