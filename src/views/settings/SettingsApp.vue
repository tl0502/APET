<script setup lang="ts">
// SettingsApp：设置面板独立窗口的 root（issue #9）。
// - AppShell variant='standalone' 提供 [data-tauri-drag-region] header
// - ElTabs tab-position='left' 横排左侧栏（PRD §7.7 设置面板形态）
// - 5 个 tab：theme(完整) / provider(占位) / persona(占位) / nickname(占位) / about(完整)
// - 关闭路径走 lib.rs on_window_event 拦截改 hide；activeTab 是组件 ref，hide 后 webview
//   不销毁，下次唤起自动保留位置
import { provide, ref } from 'vue'
import { ElIcon, ElTabPane, ElTabs } from 'element-plus'
import { Brush, Connection, EditPen, InfoFilled, User } from '@element-plus/icons-vue'
import AppShell from '@/components/layouts/AppShell.vue'
import ThemePanel from './panels/ThemePanel.vue'
import ProviderPanel from './panels/ProviderPanel.vue'
import PersonaPanel from './panels/PersonaPanel.vue'
import NicknamePanel from './panels/NicknamePanel.vue'
import AboutPanel from './panels/AboutPanel.vue'

const activeTab = ref<'theme' | 'provider' | 'persona' | 'nickname' | 'about'>('theme')

// #26 修复 B1：子面板（VrmAvatarExporter）需感知 tab 切换才能 pause/resume RAF + WebGL。
// 用 provide 暴露当前 active tab 名；ElTabPane v-show 模式下子面板 mount 后不会自动 unmount。
provide('settings-active-tab', activeTab)
</script>

<template>
  <AppShell variant="standalone" title="设置">
    <ElTabs v-model="activeTab" tab-position="left" class="settings-tabs">
      <ElTabPane name="theme">
        <template #label>
          <span class="settings-tab__label">
            <ElIcon class="settings-tab__icon"><Brush /></ElIcon>
            外观
          </span>
        </template>
        <ThemePanel />
      </ElTabPane>
      <ElTabPane name="provider">
        <template #label>
          <span class="settings-tab__label">
            <ElIcon class="settings-tab__icon"><Connection /></ElIcon>
            LLM Provider
          </span>
        </template>
        <ProviderPanel />
      </ElTabPane>
      <ElTabPane name="persona">
        <template #label>
          <span class="settings-tab__label">
            <ElIcon class="settings-tab__icon"><User /></ElIcon>
            人格
          </span>
        </template>
        <PersonaPanel />
      </ElTabPane>
      <ElTabPane name="nickname">
        <template #label>
          <span class="settings-tab__label">
            <ElIcon class="settings-tab__icon"><EditPen /></ElIcon>
            昵称
          </span>
        </template>
        <NicknamePanel />
      </ElTabPane>
      <ElTabPane name="about">
        <template #label>
          <span class="settings-tab__label">
            <ElIcon class="settings-tab__icon"><InfoFilled /></ElIcon>
            关于
          </span>
        </template>
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

/* === Vercel/Apple-Bear 风左 tab 改造 ===
 * EP 默认 active-bar 是细蓝条;隐藏掉,改用伪元素 3px 紫色左条(同 Chat 会话栏 active)。
 * inactive: text-2 / hover: text-1 + surface 浅底 / active: primary 紫字 + 600 weight + 左条。
 */
.settings-tabs :deep(.el-tabs__active-bar) {
  display: none;
}

.settings-tabs :deep(.el-tabs__item.is-left) {
  position: relative;
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-base);
  padding: var(--aipet-space-3) var(--aipet-space-4);
  transition: color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.settings-tabs :deep(.el-tabs__item.is-left:hover) {
  color: var(--aipet-color-text-1);
  background-color: var(--aipet-color-surface);
}

.settings-tabs :deep(.el-tabs__item.is-left.is-active) {
  color: var(--aipet-color-primary);
  font-weight: 600;
}

.settings-tabs :deep(.el-tabs__item.is-left.is-active)::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 3px;
  border-radius: 3px;
  background: var(--aipet-color-primary);
}

/* tab label slot:icon + 文字水平居中 */
.settings-tab__label {
  display: inline-flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.settings-tab__icon {
  font-size: 16px;
  /* 跟随 tab item color(active = primary / hover = text-1 / 默认 = text-2) */
  color: inherit;
}
</style>
