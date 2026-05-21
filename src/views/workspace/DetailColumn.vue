<script setup lang="ts">
// DetailColumn (#33 phase B-redo)：最右 detail 列（flex:1 占剩余）。
//
// 路由策略：所有非 chat panel 一律 v-show 始终 mount（决策 #7/#8）：
// - SettingsPersona 保 VRM RAF DOM（切走 pauseLoop 而非 unmount）
// - TasksPomodoro 保 4 Tauri listener 在线（phase C 接入）
// - chat 类别走 ChatThreadPane（phase D 接入，phase B-redo 占位）
//
// isPanelActive 透传给需要 RAF/listener pause 的 panel；其他 panel 不感知 active。

import { computed } from 'vue'

import SettingsThemePanel from '@/panels/settings/SettingsThemePanel.vue'
import SettingsProviderPanel from '@/panels/settings/SettingsProviderPanel.vue'
import SettingsPersonaPanel from '@/panels/settings/SettingsPersonaPanel.vue'
import SettingsNicknamePanel from '@/panels/settings/SettingsNicknamePanel.vue'
import SettingsAboutPanel from '@/panels/settings/SettingsAboutPanel.vue'
import TasksReminderPanel from '@/panels/tasks/TasksReminderPanel.vue'
import TasksPomodoroPanel from '@/panels/tasks/TasksPomodoroPanel.vue'
import TasksTodoPanel from '@/panels/tasks/TasksTodoPanel.vue'

import { useWorkspaceLayoutStore } from '@/stores/workspaceLayout'

const layout = useWorkspaceLayoutStore()

// 仅 SettingsPersona 需要 isActive prop（VRM RAF pause/resume）
const isPersonaActive = computed(
  () =>
    layout.currentCategory !== 'chat' && layout.currentItem === 'SettingsPersona',
)
</script>

<template>
  <main class="detail-col" aria-label="detail 面板">
    <!-- chat 类别占位（phase D 接入 ChatThreadPane） -->
    <section v-if="layout.currentCategory === 'chat'" class="detail-col__placeholder">
      <p>chat 主床（Phase D 接入 ChatThreadPane）</p>
    </section>

    <!-- 非 chat 类别：v-show 永远 mount + 按 currentItem 切显示 -->
    <template v-else>
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
      <SettingsNicknamePanel
        v-show="layout.currentItem === 'SettingsNickname'"
        class="detail-col__panel"
      />
      <SettingsAboutPanel
        v-show="layout.currentItem === 'SettingsAbout'"
        class="detail-col__panel"
      />
      <!-- Phase C：tasks 3 panel；v-show 永远 mount 保 listener / scheduler 在线 -->
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
    </template>
  </main>
</template>

<style scoped>
.detail-col {
  flex: 1 1 auto;
  height: 100%;
  background: var(--aipet-color-bg);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  min-width: 0;
}

.detail-col__panel {
  flex: 1 1 auto;
  padding: var(--aipet-space-5) var(--aipet-space-6);
  min-height: 0;
}

.detail-col__placeholder {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}
</style>
