<script setup lang="ts">
import { ElButton, ElButtonGroup, ElTooltip, ElMessage, ElMessageBox } from 'element-plus'
import type { TodoPriority } from '@/types/todo'

interface Props {
  count: number
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'completeAll'): void
  (e: 'cancelAll'): void
  (e: 'setPriority', priority: TodoPriority): void
  (e: 'clearSelection'): void
}>()

async function onCancelAll() {
  try {
    await ElMessageBox.confirm(
      `确认取消 ${props.count} 个待办？`,
      '批量取消',
      { confirmButtonText: '取消选中', cancelButtonText: '返回', type: 'warning' },
    )
    emit('cancelAll')
  } catch {
    // 用户点返回
  }
}
</script>

<template>
  <div v-if="props.count > 0" class="todo-batch-bar">
    <span class="todo-batch-bar__count">已选 {{ props.count }} 项</span>
    <ElButtonGroup>
      <ElButton size="small" @click="emit('completeAll')">批量完成</ElButton>
      <ElButton size="small" type="warning" @click="onCancelAll">批量取消</ElButton>
      <ElButton size="small" @click="emit('setPriority', 'high')">设为重要</ElButton>
      <ElButton size="small" @click="emit('setPriority', 'normal')">设为普通</ElButton>
      <ElButton size="small" @click="emit('setPriority', 'low')">设为低</ElButton>
    </ElButtonGroup>
    <ElButton size="small" link @click="emit('clearSelection')">清除选择</ElButton>
  </div>
</template>

<style scoped>
.todo-batch-bar {
  position: sticky;
  top: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--aipet-surface-2);
  border-bottom: 1px solid var(--aipet-color-border);
  border-radius: 6px 6px 0 0;
}
.todo-batch-bar__count {
  font-size: 13px;
  color: var(--aipet-color-text-1);
}
</style>
