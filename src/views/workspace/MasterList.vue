<script setup lang="ts">
// MasterList (#33 phase B-redo)：通用 master 列（task / creation / config 类别用）。
//
// chat 类别走 MasterColumn 内 v-if → 渲染 ConversationListPane（Phase D 接入）。
// 本组件仅渲染静态 items：icon + label，selected 加 primary 左竖条 + bg。

import { ElIcon } from 'element-plus'
import type { MasterItem } from '@/stores/workspaceLayout'

defineProps<{
  items: MasterItem[]
  activeItemId: string | null
}>()

const emit = defineEmits<{
  select: [itemId: string]
}>()

function onClick(id: string) {
  emit('select', id)
}
</script>

<template>
  <ul class="master-list" role="listbox" aria-label="master 列表">
    <li v-for="item in items" :key="item.id" class="master-list__item">
      <button
        class="master-list__btn"
        :class="{ 'master-list__btn--active': activeItemId === item.id }"
        :aria-current="activeItemId === item.id ? 'true' : undefined"
        role="option"
        :aria-selected="activeItemId === item.id"
        @click="onClick(item.id)"
      >
        <ElIcon :size="18" class="master-list__icon">
          <component :is="item.icon" />
        </ElIcon>
        <span class="master-list__label">{{ item.title }}</span>
      </button>
    </li>
  </ul>
</template>

<style scoped>
.master-list {
  list-style: none;
  margin: 0;
  padding: var(--aipet-space-2);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.master-list__btn {
  position: relative;
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: var(--aipet-space-2) var(--aipet-space-3);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  color: var(--aipet-color-text-2);
  font-size: 14px;
  line-height: 1.4;
  min-height: 36px;
  text-align: left;
  transition: background-color 100ms ease, color 100ms ease;
}

.master-list__btn:hover {
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
  color: var(--aipet-color-text-1);
}

.master-list__btn:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 10%, transparent);
}

.master-list__btn--active {
  background: color-mix(in srgb, var(--aipet-color-primary) 12%, transparent);
  color: var(--aipet-color-primary);
  font-weight: 500;
}

.master-list__btn--active::before {
  content: '';
  position: absolute;
  left: -2px;
  top: 8px;
  bottom: 8px;
  width: 3px;
  border-radius: 3px;
  background: var(--aipet-color-primary);
}

.master-list__icon {
  flex: 0 0 auto;
  color: currentColor;
}

.master-list__label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
