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
      <!-- Phase C 接入：TasksReminder / TasksPomodoro / TasksTodo -->
      <div
        v-show="
          layout.currentCategory === 'task' && layout.currentItem !== null
        "
        class="detail-col__placeholder"
      >
        <p>任务面板（Phase C 接入：{{ layout.currentItem }}）</p>
      </div>
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
