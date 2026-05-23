<script setup lang="ts">
import { computed } from 'vue'
import { ElCheckbox, ElButton, ElTooltip, ElIcon, ElMessage } from 'element-plus'
import { Check, Edit, Close, MagicStick } from '@element-plus/icons-vue'
import type { Todo } from '@/types/todo'

interface Props {
  todos: Todo[]
  selectedIds: Set<string>
  searchQuery: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'complete', id: string): void
  (e: 'cancel', id: string): void
  (e: 'edit', todo: Todo): void
  (e: 'toggleSelect', id: string, checked: boolean): void
}>()

function priorityClass(p: string): string {
  return `todo-row__bar--${p}`
}

function formatDue(due: string | null): string {
  if (!due) return ''
  const d = new Date(due)
  return d.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
}

function onSelectChange(id: string, val: boolean | string | number) {
  emit('toggleSelect', id, val === true)
}
</script>

<template>
  <ul class="todo-list">
    <li
      v-for="todo in props.todos"
      :key="todo.id"
      class="todo-row"
      :class="{ 'todo-row--done': todo.status === 'done', 'todo-row--cancelled': todo.status === 'cancelled' }"
    >
      <div class="todo-row__bar" :class="priorityClass(todo.priority)" />
      <ElCheckbox
        :model-value="props.selectedIds.has(todo.id)"
        @update:model-value="(v: boolean | string | number) => onSelectChange(todo.id, v)"
      />
      <div class="todo-row__body">
        <div class="todo-row__title">{{ todo.title }}</div>
        <div v-if="todo.dueAt" class="todo-row__due">⏰ {{ formatDue(todo.dueAt) }}</div>
      </div>
      <ElTooltip v-if="todo.reminderId" content="已关联提醒">
        <ElIcon><MagicStick /></ElIcon>
      </ElTooltip>
      <div class="todo-row__actions">
        <ElTooltip content="完成">
          <ElButton
            link
            :disabled="todo.status !== 'open'"
            @click="emit('complete', todo.id)"
          >
            <ElIcon><Check /></ElIcon>
          </ElButton>
        </ElTooltip>
        <ElTooltip content="编辑">
          <ElButton
            link
            :disabled="todo.status !== 'open'"
            @click="emit('edit', todo)"
          >
            <ElIcon><Edit /></ElIcon>
          </ElButton>
        </ElTooltip>
        <ElTooltip content="M3 上线后可用 — AI 帮你把大目标拆成小步骤">
          <ElButton link disabled>
            <ElIcon>✨</ElIcon>
          </ElButton>
        </ElTooltip>
        <ElTooltip content="取消">
          <ElButton
            link
            :disabled="todo.status !== 'open'"
            @click="emit('cancel', todo.id)"
          >
            <ElIcon><Close /></ElIcon>
          </ElButton>
        </ElTooltip>
      </div>
    </li>
    <li v-if="props.todos.length === 0" class="todo-list__empty">
      {{ props.searchQuery ? '没有匹配的待办' : '还没有待办，点右上角新建' }}
    </li>
  </ul>
</template>

<style scoped>
.todo-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.todo-row {
  display: grid;
  grid-template-columns: 4px auto 1fr auto auto;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--aipet-surface-1);
  border-radius: 6px;
}
.todo-row__bar {
  width: 4px;
  height: 100%;
  border-radius: 2px;
  align-self: stretch;
}
.todo-row__bar--high {
  background: var(--aipet-color-warning, #d97706);
}
.todo-row__bar--low {
  background: var(--aipet-color-text-3, #a3a3a3);
}
.todo-row__bar--normal {
  background: transparent;
}
.todo-row__body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.todo-row__title {
  font-size: 14px;
  color: var(--aipet-color-text-1);
}
.todo-row--done .todo-row__title {
  text-decoration: line-through;
  color: var(--aipet-color-text-3);
}
.todo-row__due {
  font-size: 12px;
  color: var(--aipet-color-text-2);
}
.todo-row__actions {
  display: flex;
  gap: 4px;
}
.todo-list__empty {
  text-align: center;
  padding: 24px;
  color: var(--aipet-color-text-3);
}
</style>
