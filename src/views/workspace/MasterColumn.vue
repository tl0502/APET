<script setup lang="ts">
// MasterColumn (#33 phase B-redo / phase D 接入)：中间 master 列（240px 默认，可 sash 调宽）。
//
// 路由：
// - chat 类别 → 渲染 ConversationListPane（共享 ConversationStore，与磁吸窗同源）
// - 其他类别 → 渲染 MasterList（含 task / creation / config items）
//
// header：当前类别名 + 类别 icon（视觉锚定）

import { computed } from 'vue'
import { ElIcon } from 'element-plus'

import MasterList from './MasterList.vue'
import ConversationListPane from '@/components/chat/ConversationListPane.vue'
import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'

const layout = useWorkspaceLayoutStore()

const currentCategoryMeta = computed(() =>
  layout.brandBarItems.find((c) => c.id === layout.currentCategory),
)

function onSelect(itemId: string) {
  layout.setItem(itemId)
}
</script>

<template>
  <aside class="master-col" :style="{ width: layout.masterWidth + 'px' }" aria-label="master 列">
    <header class="master-col__header">
      <ElIcon :size="16" class="master-col__header-icon">
        <component :is="currentCategoryMeta.icon" v-if="currentCategoryMeta" />
      </ElIcon>
      <span class="master-col__header-title">{{ currentCategoryMeta?.title ?? '' }}</span>
    </header>

    <div class="master-col__body">
      <!-- chat 类别：用 ConversationListPane（共享 store）；其余用 MasterList -->
      <ConversationListPane
        v-if="layout.currentCategory === 'chat'"
        :collapsed="false"
      />
      <MasterList
        v-else
        :items="layout.currentMasterItems"
        :active-item-id="layout.currentItem"
        @select="onSelect"
      />
    </div>
  </aside>
</template>

<style scoped>
.master-col {
  flex: 0 0 auto;
  height: 100%;
  background: var(--aipet-color-surface);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.master-col__header {
  flex: 0 0 44px;
  height: 44px;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: 0 var(--aipet-space-3);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
}

.master-col__header-icon {
  flex: 0 0 auto;
  color: var(--aipet-color-text-2);
}

.master-col__header-title {
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  letter-spacing: 0.01em;
}

.master-col__body {
  flex: 1 1 auto;
  overflow-y: auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
</style>
