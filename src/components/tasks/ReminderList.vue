<script setup lang="ts">
// ReminderList：提醒条目列表 dumb 组件。
// 接收 reminders + 转发用户操作（edit/snooze/complete/delete）给父 ReminderPanel。
//
// 显示三段：emoji+title / 触发摘要+下次触发倒计时 / 操作按钮区。
// snoozeCount >= MAX_SNOOZE_COUNT 时隐藏"稍后"按钮（后端会拒，UI 主动屏蔽）。
import { ElButton, ElDropdown, ElDropdownItem, ElDropdownMenu, ElTag } from 'element-plus'
import {
  MAX_SNOOZE_COUNT,
  SNOOZE_OPTIONS,
  type Reminder,
  type SnoozeMinutes,
} from '@/types/reminder'

const props = defineProps<{
  reminders: Reminder[]
  busy?: boolean
}>()

const emit = defineEmits<{
  edit: [reminder: Reminder]
  snooze: [payload: { id: string; minutes: SnoozeMinutes }]
  complete: [id: string]
  delete: [id: string]
  toggleEnabled: [reminder: Reminder]
}>()

function formatTrigger(r: Reminder): string {
  if (r.triggerType === 'once') {
    try {
      const d = new Date(r.triggerSpec)
      return `单次 · ${d.toLocaleString()}`
    } catch {
      return `单次 · ${r.triggerSpec}`
    }
  }
  if (r.triggerSpec.startsWith('*/')) {
    const n = r.triggerSpec.slice(2).split(' ')[0] ?? '?'
    return `每 ${n} 分钟`
  }
  return `每天 ${r.triggerSpec}（UTC）`
}

function formatCountdown(nextFireAt: string | null): string {
  if (!nextFireAt) return '已暂停'
  const dt = new Date(nextFireAt)
  const delta = dt.getTime() - Date.now()
  if (delta <= 0) return '已到点'
  const mins = Math.round(delta / 60_000)
  if (mins < 1) return `< 1 分钟`
  if (mins < 60) return `${mins} 分钟后`
  const hrs = Math.floor(mins / 60)
  const rem = mins % 60
  if (hrs < 24) return rem ? `${hrs}小时${rem}分钟后` : `${hrs}小时后`
  const days = Math.floor(hrs / 24)
  return `${days}天后`
}

function emojiOf(r: Reminder): string {
  // 简单 keyword 映射，不存就用默认 🔔
  const t = r.title
  if (t.includes('水')) return '💧'
  if (t.includes('坐') || t.includes('起') || t.includes('动')) return '🪑'
  if (t.includes('学习') || t.includes('专注') || t.includes('看书')) return '📚'
  if (t.includes('伸') || t.includes('展') || t.includes('瑜伽')) return '🧘'
  if (t.includes('睡') || t.includes('休息')) return '🌙'
  return '🔔'
}
</script>

<template>
  <ul class="reminder-list">
    <li
      v-for="r in props.reminders"
      :key="r.id"
      class="reminder-item"
      :class="{ 'reminder-item--disabled': !r.enabled }"
    >
      <div class="reminder-item__emoji">{{ emojiOf(r) }}</div>

      <div class="reminder-item__body">
        <div class="reminder-item__title-row">
          <span class="reminder-item__title">{{ r.title }}</span>
          <ElTag
            v-if="r.priority === 'hard'"
            type="warning"
            size="small"
            effect="plain"
            round
          >
            强提醒
          </ElTag>
          <ElTag v-if="r.snoozeCount > 0" type="info" size="small" effect="plain" round>
            已稍后 {{ r.snoozeCount }}/{{ MAX_SNOOZE_COUNT }}
          </ElTag>
          <ElTag v-if="!r.enabled" type="info" size="small" effect="plain" round>
            暂停
          </ElTag>
        </div>
        <div class="reminder-item__meta">
          <span>{{ formatTrigger(r) }}</span>
          <span class="reminder-item__dot">·</span>
          <span>{{ formatCountdown(r.nextFireAt) }}</span>
        </div>
      </div>

      <div class="reminder-item__actions" data-no-drag>
        <ElDropdown
          v-if="r.snoozeCount < MAX_SNOOZE_COUNT && r.enabled"
          trigger="click"
          @command="(m) => emit('snooze', { id: r.id, minutes: m as SnoozeMinutes })"
        >
          <ElButton :disabled="busy" text>稍后</ElButton>
          <template #dropdown>
            <ElDropdownMenu>
              <ElDropdownItem v-for="m in SNOOZE_OPTIONS" :key="m" :command="m">
                {{ m }} 分钟
              </ElDropdownItem>
            </ElDropdownMenu>
          </template>
        </ElDropdown>
        <ElButton :disabled="busy" text @click="emit('toggleEnabled', r)">
          {{ r.enabled ? '暂停' : '启用' }}
        </ElButton>
        <ElButton :disabled="busy" text @click="emit('edit', r)">编辑</ElButton>
        <ElButton :disabled="busy" text type="success" @click="emit('complete', r.id)">
          完成
        </ElButton>
        <ElButton :disabled="busy" text type="danger" @click="emit('delete', r.id)">
          删除
        </ElButton>
      </div>
    </li>
  </ul>
</template>

<style scoped>
.reminder-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.reminder-item {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-3) var(--aipet-space-4);
  background: var(--aipet-color-surface);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-lg);
  transition: border-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.reminder-item:hover {
  border-color: var(--aipet-color-border-strong);
}

.reminder-item--disabled {
  opacity: 0.68;
}

.reminder-item__emoji {
  flex: 0 0 auto;
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 22px;
  background: var(--aipet-color-bg);
  border: 1px solid var(--aipet-color-border);
  border-radius: 50%;
}

.reminder-item__body {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.reminder-item__title-row {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  flex-wrap: wrap;
}

.reminder-item__title {
  font-size: 14px;
  font-weight: 500;
  color: var(--aipet-color-text-1);
}

.reminder-item__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--aipet-color-text-3);
}

.reminder-item__dot {
  color: var(--aipet-color-text-3);
  opacity: 0.6;
}

.reminder-item__actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 0;
}

.reminder-item__actions :deep(.el-button) {
  padding: 0 8px;
  height: 28px;
  font-size: 12px;
}
</style>
