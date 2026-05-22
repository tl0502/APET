<script setup lang="ts">
// MasterColumn (#33 phase B-redo / phase D 接入)：中间 master 列（240px 默认，可 sash 调宽）。
//
// 路由：
// - chat 类别 → 渲染 ConversationListPane（共享 ConversationStore，与磁吸窗同源）
// - 其他类别 → 渲染 MasterList（含 task / creation / config items）
//
// header：当前类别名 + 类别 icon（视觉锚定）

import { computed } from 'vue'
import { ElIcon, ElButton } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'

import MasterList from './MasterList.vue'
import ConversationListPane from '@/components/chat/ConversationListPane.vue'
import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'
import { useConversationStore } from '@/stores/conversation'

const layout = useWorkspaceLayoutStore()
const convStore = useConversationStore()

const currentCategoryMeta = computed(() =>
  layout.brandBarItems.find((c) => c.id === layout.currentCategory),
)

function onSelect(itemId: string) {
  layout.setItem(itemId)
}
</script>

<template>
  <aside class="master-col" aria-label="master 列">
    <header class="master-col__header">
      <ElIcon :size="16" class="master-col__header-icon">
        <component :is="currentCategoryMeta.icon" v-if="currentCategoryMeta" />
      </ElIcon>
      <span class="master-col__header-title">{{ currentCategoryMeta?.title ?? '' }}</span>
    </header>

    <div class="master-col__body">
      <!-- chat 类别：用 ConversationListPane（共享 store）；其余用 MasterList -->
      <template v-if="layout.currentCategory === 'chat'">
        <div v-if="convStore.conversations.length === 0" class="conv-empty">
          <img src="/avatar/momo-avatar.svg" alt="" class="conv-empty__illustration" />
          <p class="conv-empty__title">还没开始对话</p>
          <p class="conv-empty__hint">说一句“你好”就行</p>
          <ElButton type="primary" @click="convStore.create()">
            <ElIcon><Plus /></ElIcon>
            <span style="margin-left: 6px;">新对话</span>
          </ElButton>
        </div>
        <ConversationListPane v-else :collapsed="false" />
      </template>
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
  /* 宽度由父 grid 列 var(--workspace-master-width) 控制；高度由 grid 行 1fr 控制（#37 P3） */
  background: var(--aipet-color-surface);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.master-col__header {
  flex: 0 0 48px;
  height: 48px;
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: 0 var(--aipet-space-4);
  border-bottom: 1px solid var(--aipet-color-border-faint);
  user-select: none;
  /* 与 panel__title 同语言：sticky 浮玻璃 */
  position: sticky;
  top: 0;
  z-index: 2;
  background: var(--aipet-color-surface-blur);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
}

.master-col__header-icon {
  flex: 0 0 auto;
  color: var(--aipet-color-text-2);
}

.master-col__header-title {
  font-size: 15px;
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

.conv-empty {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--aipet-space-6);
  gap: var(--aipet-space-3);
  text-align: center;
}

.conv-empty__illustration {
  width: 64px;
  height: 64px;
  opacity: 0.6;
  filter: grayscale(20%);
}

.conv-empty__title {
  font-size: 15px;
  font-weight: 500;
  color: var(--aipet-color-text-1);
  margin: 0;
}

.conv-empty__hint {
  font-size: 13px;
  color: var(--aipet-color-text-3);
  margin: 0;
}
</style>
