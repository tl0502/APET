<script setup lang="ts">
// ChatBody (#33 phase D 简化)：磁吸窗专用的"双 pane 组装器"。
//
// 历史：phase A 时 ChatBody 含完整 sidebar + content + composer + chrome + persona + ESC。
// phase D 拆出：
// - ConversationListPane.vue（sidebar + 删除二次确认）
// - ChatThreadPane.vue（content-header + messages + composer + persona + ESC）
//
// 本组件现职责：
// 1) sidebarCollapsed 本地 state（仅磁吸窗用，workspace 不消费）
// 2) 双 pane 组装 + chrome props 透传给 ChatThreadPane
// 3) emit close 上抛（磁吸窗 ChatApp 接 → hideChat）
//
// workspace 不用本组件 — DetailColumn 直接挂 ChatThreadPane，MasterColumn 直接挂 ConversationListPane。

import { ref } from 'vue'
import ConversationListPane from '@/components/chat/ConversationListPane.vue'
import ChatThreadPane from '@/components/chat/ChatThreadPane.vue'

const props = withDefaults(
  defineProps<{
    /** 父容器是否激活（磁吸窗永远 true） */
    panelActive?: boolean
    /** 是否渲染 content-header 右侧 ✕（磁吸窗 true） */
    showCloseButton?: boolean
    /** 是否渲染 content-header 中央胶囊拖动块（磁吸窗 true） */
    showTitlebarDrag?: boolean
  }>(),
  {
    panelActive: true,
    showCloseButton: true,
    showTitlebarDrag: true,
  },
)

const emit = defineEmits<{
  close: []
}>()

// 磁吸窗内的折叠状态（每实例独立）
const sidebarCollapsed = ref(false)

function onToggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}
</script>

<template>
  <div class="app-body">
    <ConversationListPane :collapsed="sidebarCollapsed" />
    <ChatThreadPane
      :panel-active="props.panelActive"
      :show-close-button="props.showCloseButton"
      :show-titlebar-drag="props.showTitlebarDrag"
      :show-sidebar-toggle="true"
      :sidebar-collapsed="sidebarCollapsed"
      @close="emit('close')"
      @toggle-sidebar="onToggleSidebar"
    />
  </div>
</template>

<style scoped>
.app-body {
  width: 100%;
  height: 100%;
  display: flex;
  min-height: 0;
}
</style>
