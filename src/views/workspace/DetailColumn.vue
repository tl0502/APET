<script setup lang="ts">
// DetailColumn (#33 phase B-redo / phase C/D 接入)：最右 detail 列（flex:1 占剩余）。
//
// 路由策略：v-show 永远 mount（决策 #7/#8）：
// - ChatThreadPane：chat 类别下显（保 streaming + scroll position + textarea state）
// - SettingsPersona：保 VRM RAF DOM（isActive 透传控制 pause/resume）
// - TasksPomodoro / TasksReminder：保 4 Tauri listener 在线
//
// 互斥：chat 类别下 currentItem 恒为 null（workspaceLayout 内置约束），
//      其他 panel 的 v-show layout.currentItem === 'XXX' 自动 false → 不显示。

import { computed } from 'vue'

import SettingsThemePanel from '@/panels/settings/SettingsThemePanel.vue'
import SettingsProviderPanel from '@/panels/settings/SettingsProviderPanel.vue'
import SettingsPersonaPanel from '@/panels/settings/SettingsPersonaPanel.vue'
import TasksReminderPanel from '@/panels/tasks/TasksReminderPanel.vue'
import TasksPomodoroPanel from '@/panels/tasks/TasksPomodoroPanel.vue'
import TasksTodoPanel from '@/panels/tasks/TasksTodoPanel.vue'
import ChatThreadPane from '@/components/chat/ChatThreadPane.vue'

import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'

const layout = useWorkspaceLayoutStore()

const isChat = computed(() => layout.currentCategory === 'chat')

// SettingsPersona 需要 isActive prop（VRM RAF pause/resume）
const isPersonaActive = computed(
  () => !isChat.value && layout.currentItem === 'SettingsPersona',
)
</script>

<template>
  <main class="detail-col" aria-label="detail 面板">
    <!-- chat 主床：永久 mount，v-show 切显（保 streaming + scroll + textarea state） -->
    <ChatThreadPane
      v-show="isChat"
      class="detail-col__chat-pane"
      :panel-active="isChat"
      :show-close-button="false"
      :show-titlebar-drag="false"
      :show-sidebar-toggle="false"
    />

    <!-- settings 3 panel：v-show 永远 mount + 按 currentItem 切显示 -->
    <SettingsThemePanel
      v-show="layout.currentItem === 'SettingsTheme'"
      class="detail-col__panel"
    />
    <SettingsProviderPanel
      v-show="layout.currentItem === 'SettingsProvider'"
      class="detail-col__panel"
    />
    <SettingsPersonaPanel
      v-show="layout.currentItem === 'SettingsPersona'"
      class="detail-col__panel"
      :is-active="isPersonaActive"
    />

    <!-- tasks 3 panel：v-show 永远 mount 保 listener / scheduler 在线 -->
    <TasksReminderPanel
      v-show="layout.currentItem === 'TasksReminder'"
      class="detail-col__panel"
    />
    <TasksPomodoroPanel
      v-show="layout.currentItem === 'TasksPomodoro'"
      class="detail-col__panel"
    />
    <TasksTodoPanel
      v-show="layout.currentItem === 'TasksTodo'"
      class="detail-col__panel"
      title="待办"
      issue="#29"
    />
  </main>
</template>

<style scoped>
.detail-col {
  /* 宽度由父 grid 列 1fr 控制；高度由 grid 行 1fr 控制（#37 P3） */
  background: var(--aipet-color-bg);
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

/* 普通 panel：内边距 + 可滚动 */
.detail-col__panel {
  flex: 1 1 auto;
  padding: var(--aipet-space-5) var(--aipet-space-6);
  min-height: 0;
  overflow-y: auto;
}

/* chat pane：ChatThreadPane 自含 layout（content-header + scroll + composer），
   detail-col 内沿 flex 主轴撑满即可，不加 padding（pane 内部 floating-composer 自管 padding） */
.detail-col__chat-pane {
  flex: 1 1 auto;
  min-height: 0;
}
</style>
