<script setup lang="ts">
// PetReminderBubble：桌宠头顶气泡 overlay（issue #22）。
//
// 设计要点：
// - listen `reminder:fired` event → 推一条气泡到 stack（最多 3 条，超过推掉最旧的）
// - 气泡位置：absolute top:6px 居中；不溢出 pet 窗 320×320 边界
// - 自动消失：8 秒淡出（除非 hover 或展开稍后子菜单）
// - 按钮区 `[data-no-drag]` 隔离拖动（PetCanvas 整窗 startDragging）
// - snooze_count >= MAX_SNOOZE_COUNT 时隐藏「稍后」按钮（与 ReminderList 同款屏蔽）
//
// 与 PetCanvas 的协作：
// - 挂在 App.vue 内 PetCanvas 同级 overlay；transparent 窗体内画在 VRM 头顶上方
// - 不与 PetCanvas pointerdown(startDragging) 冲突：data-no-drag 让按钮事件不冒泡到拖动
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { completeReminder, snoozeReminder } from '@/services/reminder'
import {
  MAX_SNOOZE_COUNT,
  REMINDER_FIRED_EVENT,
  SNOOZE_OPTIONS,
  type ReminderFiredPayload,
  type SnoozeMinutes,
} from '@/types/reminder'

const MAX_BUBBLES = 3
const AUTO_DISMISS_MS = 8000

interface BubbleState {
  /** local 唯一 id（reminderId + 触发时戳）— 防多次触发同 reminder 时 v-for key 冲突 */
  key: string
  payload: ReminderFiredPayload
  snoozeOpen: boolean
  busy: boolean
  hover: boolean
  timer: number | null
  /** 本地 snoozeCount 复制（snooze action 成功后会更新；payload 来自首次 fire 时刻） */
  snoozeCount: number
}

const bubbles = ref<BubbleState[]>([])
let unlistenFired: UnlistenFn | null = null

function createBubble(payload: ReminderFiredPayload): BubbleState {
  return {
    key: `${payload.reminderId}:${Date.now()}`,
    payload,
    snoozeOpen: false,
    busy: false,
    hover: false,
    timer: null,
    snoozeCount: payload.snoozeCount,
  }
}

function pushBubble(payload: ReminderFiredPayload) {
  // 同 reminderId 已在 stack → 更新而非追加（避免重复堆叠）
  const existing = bubbles.value.find((b) => b.payload.reminderId === payload.reminderId)
  if (existing) {
    existing.payload = payload
    existing.snoozeCount = payload.snoozeCount
    startAutoDismiss(existing)
    return
  }
  // 容量 cap 3：超过推掉最旧的
  if (bubbles.value.length >= MAX_BUBBLES) {
    const oldest = bubbles.value[0]
    if (oldest) removeBubble(oldest)
  }
  const b = createBubble(payload)
  bubbles.value.push(b)
  startAutoDismiss(b)
}

function startAutoDismiss(b: BubbleState) {
  if (b.timer !== null) {
    window.clearTimeout(b.timer)
  }
  b.timer = window.setTimeout(() => {
    if (!b.hover && !b.snoozeOpen) {
      removeBubble(b)
    }
  }, AUTO_DISMISS_MS) as unknown as number
}

function removeBubble(b: BubbleState) {
  if (b.timer !== null) {
    window.clearTimeout(b.timer)
    b.timer = null
  }
  const idx = bubbles.value.indexOf(b)
  if (idx >= 0) bubbles.value.splice(idx, 1)
}

function onMouseEnter(b: BubbleState) {
  b.hover = true
}

function onMouseLeave(b: BubbleState) {
  b.hover = false
  // 离开后重置 auto-dismiss 倒计时（避免离开瞬间立刻消失）
  startAutoDismiss(b)
}

async function onComplete(b: BubbleState) {
  b.busy = true
  try {
    await completeReminder(b.payload.reminderId)
    removeBubble(b)
  } catch (e) {
    console.error('[reminder-bubble] complete failed:', e)
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
    console.error('[reminder-bubble] snooze failed:', e)
  } finally {
    b.busy = false
  }
}

function canSnooze(b: BubbleState): boolean {
  return b.snoozeCount < MAX_SNOOZE_COUNT
}

function iconOf(b: BubbleState): string {
  // hard 用 🔔（强提醒），soft 用 💭（温和）；可后续做主题切换
  return b.payload.priority === 'hard' ? '🔔' : '💭'
}

onMounted(async () => {
  try {
    unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, (e) => {
      if (e.payload) pushBubble(e.payload)
    })
  } catch (e) {
    console.warn('[reminder-bubble] listen failed:', e)
  }
})

onBeforeUnmount(() => {
  unlistenFired?.()
  bubbles.value.forEach((b) => {
    if (b.timer !== null) window.clearTimeout(b.timer)
  })
})
</script>

<template>
  <TransitionGroup name="bubble" tag="div" class="reminder-bubble-stack">
    <div
      v-for="b in bubbles"
      :key="b.key"
      class="reminder-bubble"
      :class="{
        'reminder-bubble--hard': b.payload.priority === 'hard',
      }"
      @mouseenter="onMouseEnter(b)"
      @mouseleave="onMouseLeave(b)"
    >
      <div class="reminder-bubble__body">
        <span class="reminder-bubble__icon">{{ iconOf(b) }}</span>
        <div class="reminder-bubble__text">
          <span class="reminder-bubble__title">{{ b.payload.title }}</span>
          <span v-if="b.snoozeCount > 0" class="reminder-bubble__sub">
            已稍后 {{ b.snoozeCount }}/{{ MAX_SNOOZE_COUNT }}
          </span>
        </div>
      </div>

      <div class="reminder-bubble__actions" data-no-drag>
        <template v-if="!b.snoozeOpen">
          <button
            type="button"
            class="reminder-bubble__btn reminder-bubble__btn--primary"
            :disabled="b.busy"
            @click="onComplete(b)"
          >
            完成
          </button>
          <button
            v-if="canSnooze(b)"
            type="button"
            class="reminder-bubble__btn"
            :disabled="b.busy"
            @click="b.snoozeOpen = true"
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
            :disabled="b.busy"
            @click="onSnooze(b, m)"
          >
            {{ m }}
          </button>
        </template>
        <button
          type="button"
          class="reminder-bubble__btn reminder-bubble__btn--ghost"
          :disabled="b.busy"
          aria-label="关闭"
          @click="removeBubble(b)"
        >
          ✕
        </button>
      </div>
    </div>
  </TransitionGroup>
</template>

<style scoped>
.reminder-bubble-stack {
  /* fixed 相对 webview 视口；pet 窗 320×320 透明，气泡浮在顶部居中。
     用 absolute 需要给 ancestor 加 position:relative；fixed 更简洁。 */
  position: fixed;
  top: 6px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  gap: 6px;
  pointer-events: none;
  /* 大于 PetCanvas 默认 cursor 视觉层级 */
  z-index: 5;
}

.reminder-bubble {
  pointer-events: auto;
  min-width: 220px;
  max-width: 296px;
  padding: 8px 10px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: 1px solid var(--aipet-color-border-strong, var(--aipet-color-border));
  border-radius: 14px;
  box-shadow: 0 8px 24px -8px rgba(0, 0, 0, 0.18), 0 2px 6px -2px rgba(0, 0, 0, 0.08);
  backdrop-filter: blur(8px);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.reminder-bubble--hard {
  border-color: color-mix(in srgb, var(--aipet-color-primary) 60%, var(--aipet-color-border));
  box-shadow: 0 8px 24px -8px color-mix(in srgb, var(--aipet-color-primary) 30%, transparent),
    0 2px 6px -2px rgba(0, 0, 0, 0.08);
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
  font-size: 11px;
  font-weight: 500;
  padding: 3px 8px;
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

/* TransitionGroup 淡入淡出 + 上滑 */
.bubble-enter-active,
.bubble-leave-active {
  transition: opacity 220ms var(--aipet-ease-standard),
    transform 220ms var(--aipet-ease-standard);
}

.bubble-enter-from,
.bubble-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

.bubble-move {
  transition: transform 220ms var(--aipet-ease-standard);
}
</style>
