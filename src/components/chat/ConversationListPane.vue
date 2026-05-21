<script setup lang="ts">
// ConversationListPane (#33 phase D)：chat 业务 sidebar 单独 pane。
//
// 共享给：
// - chat 磁吸窗 ChatBody.vue（与 ChatThreadPane 双 pane 组装）
// - workspace MasterColumn.vue（chat 类别下 v-if 渲染）
//
// 设计：
// - 业务状态全部走 ConversationStore（Pinia singleton，磁吸 + workspace 共享）
// - ElMessageBox 二次确认（删除对话）留组件层（UI 阻塞交互不适合 store）
// - props.collapsed 由父级控制（磁吸窗 ChatBody 内管理；workspace 永远 false）

import { ElMessageBox } from 'element-plus'

import ConversationSidebar from '@/components/chat/ConversationSidebar.vue'
import { useConversationStore } from '@/stores/conversation'

withDefaults(
  defineProps<{
    /** 是否折叠（仅磁吸窗用；workspace 内永远 false 让 sidebar 全宽） */
    collapsed?: boolean
  }>(),
  { collapsed: false },
)

const store = useConversationStore()

async function onDeleteConversation(id: string) {
  const target = store.conversations.find((c) => c.id === id)
  const label = target?.title?.trim() || '此对话'
  try {
    await ElMessageBox.confirm(
      `删除「${label}」及其所有消息？此操作不可撤销。`,
      '确认删除',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return // 用户取消 / ESC
  }
  await store.remove(id)
}
</script>

<template>
  <ConversationSidebar
    :conversations="store.conversations"
    :active-id="store.activeId"
    :locked-ids="store.streamingConvIds"
    :collapsed="collapsed"
    @select="store.switchTo"
    @create="store.create"
    @rename="store.rename"
    @archive="store.archive"
    @delete="onDeleteConversation"
  />
</template>
