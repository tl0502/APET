<script setup lang="ts">
// SettingsApp：设置面板独立窗口的 root（issue #9）。
// - AppShell variant='standalone' 提供 [data-tauri-drag-region] header
// - ElTabs tab-position='left' 横排左侧栏（PRD §7.7 设置面板形态）
// - 5 个 tab：theme(完整) / provider(占位) / persona(占位) / nickname(占位) / about(完整)
// - 关闭路径走 lib.rs on_window_event 拦截改 hide；activeTab 是组件 ref，hide 后 webview
//   不销毁，下次唤起自动保留位置
import { ref } from 'vue'
import { ElTabPane, ElTabs } from 'element-plus'
import AppShell from '@/components/layouts/AppShell.vue'
import ThemePanel from './panels/ThemePanel.vue'
import ProviderPanel from './panels/ProviderPanel.vue'
import PersonaPanel from './panels/PersonaPanel.vue'
import NicknamePanel from './panels/NicknamePanel.vue'
import AboutPanel from './panels/AboutPanel.vue'

const activeTab = ref<'theme' | 'provider' | 'persona' | 'nickname' | 'about'>('theme')
</script>

<template>
  <AppShell variant="standalone" title="设置">
    <ElTabs v-model="activeTab" tab-position="left" class="settings-tabs">
      <ElTabPane label="主题" name="theme">
        <ThemePanel />
      </ElTabPane>
      <ElTabPane label="LLM Provider" name="provider">
        <ProviderPanel />
      </ElTabPane>
      <ElTabPane label="人格" name="persona">
        <PersonaPanel />
      </ElTabPane>
      <ElTabPane label="昵称" name="nickname">
        <NicknamePanel />
      </ElTabPane>
      <ElTabPane label="关于" name="about">
        <AboutPanel />
      </ElTabPane>
    </ElTabs>
  </AppShell>
</template>

<style scoped>
.settings-tabs {
  flex: 1 1 auto;
  min-height: 0;
}

/* 左排 tab：让 content 区独立滚动；header（即 nav）由 EP 自身固定 */
.settings-tabs :deep(.el-tabs__content) {
  height: 100%;
  overflow-y: auto;
}

.settings-tabs :deep(.el-tab-pane) {
  padding: 0 var(--aipet-space-2);
}
</style>
