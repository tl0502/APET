<script setup lang="ts">
// PetReminderBubble：桌宠头顶气泡 overlay（issue #22；2026-05-24 UI 重构）。
//
// 行为模型变更（与原 cap=3 截断方案差异）：
// - 数据结构：reminders newest-first，旧条目永不因容量被丢弃
// - 展示规则：
//   - count = 0：不显示
//   - count = 1：单气泡（普通形态，无 badge）
//   - count = 2 且未进入 collapsed：两气泡（template reverse 让旧上新下）
//   - count > 2：进入 collapsed 翻页模式，只显示 reminders[0]（最新）+ 左上 count badge
// - 一旦 collapsed，count 回落到 2 仍保持 collapsed；count 回落到 1 → 重置 expanded
// - 同 reminderId 去重：更新内容 + 移到 newest 头部 + 重置 auto-dismiss
//
// 与 PetCommandTray 的避让：
// - props.trayOpen=true 时强制 isCollapsed=true + 整体 opacity 40%
// - tray 关闭后恢复（不强制保持 collapsed）
//
// auto-dismiss：保留 8s + hover/snoozeOpen 暂停 + 只移除当前
// collapsed mode 下不可见 bubble 暂停 timer（防 reminders[0] 被换走后又 fire 一次）

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { completeReminder, snoozeReminder } from '@/services/reminder'
import {
  MAX_SNOOZE_COUNT,
  REMINDER_FIRED_EVENT,
  SNOOZE_OPTIONS,
  type ReminderFiredPayload,
  type SnoozeMinutes,
} from '@/types/reminder'

const MAX_EXPANDED = 2
const AUTO_DISMISS_MS = 8000

interface Props {
  /** App.vue 透传：command tray 打开时强制 collapsed + 整体降透明度 */
  trayOpen?: boolean
}

const props = withDefaults(defineProps<Props>(), { trayOpen: false })

type DisplayMode = 'expanded' | 'collapsed'

interface BubbleState {
  key: string
  payload: ReminderFiredPayload
  snoozeOpen: boolean
  busy: boolean
  hover: boolean
  timer: number | null
  snoozeCount: number
}

const reminders = ref<BubbleState[]>([])
const displayMode = ref<DisplayMode>('expanded')
let unlistenFired: UnlistenFn | null = null

const isCollapsed = computed(() => {
  if (props.trayOpen && reminders.value.length >= 1) return true
  return (
    reminders.value.length > MAX_EXPANDED ||
    (displayMode.value === 'collapsed' && reminders.value.length > 1)
  )
})

const visibleReminders = computed<BubbleState[]>(() => {
  if (reminders.value.length === 0) return []
  if (isCollapsed.value) return reminders.value.slice(0, 1)
  // expanded：旧上新下 = reverse(newest-first.slice(0,2))
  return reminders.value.slice(0, 2).slice().reverse()
})

const collapsedCount = computed(() => reminders.value.length)

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
  const existingIdx = reminders.value.findIndex(
    (b) => b.payload.reminderId === payload.reminderId,
  )
  if (existingIdx >= 0) {
    // 去重 + 移到 newest 头部 + 更新 payload
    const [existing] = reminders.value.splice(existingIdx, 1)
    existing.payload = payload
    existing.snoozeCount = payload.snoozeCount
    reminders.value.unshift(existing)
    startAutoDismiss(existing)
  } else {
    const b = createBubble(payload)
    reminders.value.unshift(b)
    startAutoDismiss(b)
  }
  if (reminders.value.length > MAX_EXPANDED) {
    displayMode.value = 'collapsed'
  }
}

function startAutoDismiss(b: BubbleState) {
  if (b.timer !== null) {
    window.clearTimeout(b.timer)
    b.timer = null
  }
  // collapsed mode 下只有 reminders[0] 可见；不可见的暂停 timer 防止被换走后才到期。
  // trayOpen=true 也强制 collapsed，同款逻辑生效。
  const willBeVisible = !isCollapsed.value || b === reminders.value[0]
  if (!willBeVisible) return
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
  const idx = reminders.value.indexOf(b)
  if (idx >= 0) reminders.value.splice(idx, 1)
  // count 回落到 ≤1 → 重置 expanded（让单卡用普通气泡形态）。
  // count 仍 ≥ 2 时不动 displayMode（一旦 collapsed 不自动展开）。
  if (reminders.value.length <= 1) {
    displayMode.value = 'expanded'
  }
  // collapsed 翻页：新的 reminders[0] 现在变可见，启动它的 auto-dismiss。
  if (reminders.value.length > 0 && isCollapsed.value) {
    startAutoDismiss(reminders.value[0])
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
  return b.payload.priority === 'hard' ? '🔔' : '💭'
}

// trayOpen 变化：可能切换 isCollapsed → 重算 timer 起停
watch(
  () => props.trayOpen,
  () => {
    for (const b of reminders.value) {
      startAutoDismiss(b)
    }
  },
)

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
  reminders.value.forEach((b) => {
    if (b.timer !== null) window.clearTimeout(b.timer)
  })
})

// 跨窗协作（pet-reminder overlay 迁移）：让父级 overlay App watch bubbleCount → emit
// Tauri 全局事件，由 Rust services/pet_overlay.rs 控 overlay window 的 show/hide。
defineExpose({
  bubbleCount: computed(() => reminders.value.length),
})
</script>

<template>
  <TransitionGroup
    name="bubble"
    tag="div"
    class="reminder-bubble-stack"
    :class="{ 'reminder-bubble-stack--dimmed': trayOpen }"
  >
    <div
      v-for="b in visibleReminders"
      :key="b.key"
      class="reminder-bubble"
      :class="{
        'reminder-bubble--hard': b.payload.priority === 'hard',
        'reminder-bubble--collapsed': isCollapsed,
      }"
      @mouseenter="onMouseEnter(b)"
      @mouseleave="onMouseLeave(b)"
    >
      <!-- collapsed mode 左上 count badge：仅最新一张显示（visibleReminders.length=1） -->
      <div v-if="isCollapsed" class="reminder-bubble__count-badge" aria-label="未处理提醒数">
        {{ collapsedCount }}
      </div>

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
  position: fixed;
  top: 6px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  gap: 6px;
  pointer-events: none;
  z-index: 5;
  transition: opacity 140ms var(--aipet-ease-standard);
}

.reminder-bubble-stack--dimmed {
  opacity: 0.4;
}

.reminder-bubble {
  pointer-events: auto;
  position: relative;
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

.reminder-bubble--collapsed .reminder-bubble__body {
  padding-left: 18px; /* 留位给 badge */
}

.reminder-bubble__count-badge {
  position: absolute;
  top: -6px;
  left: -6px;
  min-width: 22px;
  height: 22px;
  padding: 0 7px;
  background: var(--aipet-color-primary);
  color: #fff;
  border-radius: 11px;
  font-size: 11px;
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
