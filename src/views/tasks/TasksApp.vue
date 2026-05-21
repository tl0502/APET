<script setup lang="ts">
// TasksApp：任务三件套独立窗口的 root（issue #22）。
//
// - AppShell variant='standalone' 提供 [data-tauri-drag-region] header
// - ElTabs tab-position='left' 横排左侧栏（与 SettingsApp 同款）
// - 3 个 tab：提醒（#22 完整）/ 番茄（#28 完整）/ 待办（#29 占位 → disabled）
// - 关闭路径走 lib.rs on_window_event 拦截改 hide；activeTab 是组件 ref，hide 后 webview
//   不销毁，下次唤起自动保留位置
import { ref } from 'vue'
import { ElIcon, ElTabPane, ElTabs } from 'element-plus'
import { AlarmClock, Calendar, Timer } from '@element-plus/icons-vue'
import AppShell from '@/components/layouts/AppShell.vue'
import { hideTasks } from '@/services/window'
// #33 phase C：panel 已迁入 @/panels/tasks/（与 workspace 共用源）；独立窗 TasksApp 继续承载到 Phase E 删除
import ReminderPanel from '@/panels/tasks/TasksReminderPanel.vue'
import PomodoroPanel from '@/panels/tasks/TasksPomodoroPanel.vue'
import PlaceholderPanel from '@/panels/tasks/TasksTodoPanel.vue'

type TaskTab = 'reminder' | 'pomodoro' | 'todo'
const activeTab = ref<TaskTab>('reminder')

/** 删除 OS 原生标题栏后，自绘 header 的 ✕ 调 hideTasks IPC（与后端 on_window_event "关 = hide" 同源）。 */
async function onClose() {
  try {
    await hideTasks()
  } catch (e) {
    console.warn('[TasksApp] hideTasks failed:', e)
  }
}
</script>

<template>
  <AppShell variant="standalone">
    <template #header>
      <span class="aipet-shell__title" data-tauri-drag-region>任务</span>
      <span class="aipet-shell__header-spacer" data-tauri-drag-region />
      <button
        class="aipet-shell__close"
        title="关闭"
        aria-label="关闭"
        data-tauri-drag-region="false"
        @click="onClose"
      >✕</button>
    </template>
    <ElTabs v-model="activeTab" tab-position="left" class="tasks-tabs">
      <ElTabPane name="reminder">
        <template #label>
          <span class="tasks-tab__label">
            <ElIcon class="tasks-tab__icon"><AlarmClock /></ElIcon>
            提醒
          </span>
        </template>
        <ReminderPanel />
      </ElTabPane>
      <ElTabPane name="pomodoro">
        <template #label>
          <span class="tasks-tab__label">
            <ElIcon class="tasks-tab__icon"><Timer /></ElIcon>
            番茄
          </span>
        </template>
        <PomodoroPanel />
      </ElTabPane>
      <ElTabPane name="todo" disabled>
        <template #label>
          <span class="tasks-tab__label tasks-tab__label--placeholder">
            <ElIcon class="tasks-tab__icon"><Calendar /></ElIcon>
            待办
            <span class="tasks-tab__chip">#29</span>
          </span>
        </template>
        <PlaceholderPanel title="待办" issue="#29" />
      </ElTabPane>
    </ElTabs>
  </AppShell>
</template>

<style scoped>
.tasks-tabs {
  flex: 1 1 auto;
  min-height: 0;
}

/* 左排 tab：让 content 区独立滚动；header（即 nav）由 EP 自身固定 */
.tasks-tabs :deep(.el-tabs__content) {
  height: 100%;
  overflow-y: auto;
}

.tasks-tabs :deep(.el-tab-pane) {
  padding: 0 var(--aipet-space-2);
}

/* === 同 SettingsApp 的 Vercel/Apple-Bear 风左 tab 改造 ===
 * EP 默认 active-bar 隐藏，伪元素 3px 紫色左条；inactive: text-2 / hover: text-1 + surface 浅底
 * active: primary 紫字 + 600 weight + 左条。
 */
.tasks-tabs :deep(.el-tabs__active-bar) {
  display: none;
}

.tasks-tabs :deep(.el-tabs__item.is-left) {
  position: relative;
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-base);
  padding: var(--aipet-space-3) var(--aipet-space-4);
  transition: color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.tasks-tabs :deep(.el-tabs__item.is-left:hover) {
  color: var(--aipet-color-text-1);
  background-color: var(--aipet-color-surface);
}

.tasks-tabs :deep(.el-tabs__item.is-left.is-active) {
  color: var(--aipet-color-primary);
  font-weight: 600;
}

.tasks-tabs :deep(.el-tabs__item.is-left.is-active)::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 3px;
  border-radius: 3px;
  background: var(--aipet-color-primary);
}

.tasks-tabs :deep(.el-tabs__item.is-disabled) {
  cursor: not-allowed;
  opacity: 0.6;
}

/* tab label slot: icon + 文字 + 占位 chip */
.tasks-tab__label {
  display: inline-flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.tasks-tab__label--placeholder {
  color: var(--aipet-color-text-3);
}

.tasks-tab__icon {
  font-size: 16px;
  color: inherit;
}

.tasks-tab__chip {
  display: inline-flex;
  align-items: center;
  padding: 0 6px;
  height: 18px;
  border-radius: 9px;
  font-size: 11px;
  font-weight: 500;
  background: color-mix(in srgb, var(--aipet-color-text-3) 18%, transparent);
  color: var(--aipet-color-text-3);
}
</style>
