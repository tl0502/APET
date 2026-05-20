<script setup lang="ts">
// ActivityBar：纵向 icon 按钮列表（VSCode-style，左侧 48px 窄列）
//
// 职责：
// - 渲染 mgr.registry.list() 里所有有 icon 的 panel（M2 阶段固定 3 项）
// - click → mgr.revealPanel(id)
// - active 高亮 = mgr.getActivePanel()，订阅 onPanelActivated 自动刷新
// - 底部加固定 ⌘P 按钮触发命令面板（Phase D 才实装；Phase C 仅占位）
//
// when DSL 过滤：list 只显示 when 求真的 panel（如 'dev.mode' 的 debug panel 只在开发期出现）

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElIcon, ElTooltip } from 'element-plus'
import { Search } from '@element-plus/icons-vue'

import { useWorkspaceManager } from '@/composables/useWorkspaceManager'

const mgr = useWorkspaceManager()

const activePanelId = ref<string | null>(mgr.getActivePanel())

const panels = computed(() =>
  mgr.registry.list().filter((p) => p.icon !== undefined && mgr.isWhenSatisfied(p.when)),
)

let unsubActivated: (() => void) | null = null
let unsubDeactivated: (() => void) | null = null

onMounted(() => {
  unsubActivated = mgr.onPanelActivated((id) => {
    activePanelId.value = id
  })
  unsubDeactivated = mgr.onPanelDeactivated(() => {
    activePanelId.value = mgr.getActivePanel()
  })
})

onBeforeUnmount(() => {
  unsubActivated?.()
  unsubDeactivated?.()
})

function handleClick(id: string) {
  try {
    mgr.revealPanel(id)
  } catch (e) {
    console.warn('[ActivityBar] revealPanel failed:', id, e)
  }
}

async function handleCommandPalette() {
  // Phase D 实装：mgr.executeCommand('workspace.togglePalette') → mgr.setContextKey('paletteVisible', true)
  // Phase C 阶段命令已注册（WorkspaceApp.vue 内 placeholder handler），调用不抛但无视觉效果。
  try {
    await mgr.executeCommand('workspace.togglePalette')
  } catch (e) {
    console.warn('[ActivityBar] togglePalette failed:', e)
  }
}
</script>

<template>
  <nav class="activity-bar" aria-label="工作台导航">
    <ul class="activity-bar__list">
      <li v-for="p in panels" :key="p.id" class="activity-bar__item">
        <ElTooltip :content="String(p.title)" placement="right" :show-after="500">
          <button
            class="activity-bar__btn"
            :class="{ 'activity-bar__btn--active': activePanelId === p.id }"
            :aria-pressed="activePanelId === p.id"
            :aria-label="String(p.title)"
            @click="handleClick(p.id)"
          >
            <ElIcon :size="20">
              <component :is="p.icon" />
            </ElIcon>
          </button>
        </ElTooltip>
      </li>
    </ul>
    <div class="activity-bar__spacer" />
    <ul class="activity-bar__list">
      <li class="activity-bar__item">
        <ElTooltip content="命令面板（Ctrl+P）" placement="right" :show-after="500">
          <button
            class="activity-bar__btn"
            aria-label="命令面板"
            @click="handleCommandPalette"
          >
            <ElIcon :size="20"><Search /></ElIcon>
          </button>
        </ElTooltip>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
.activity-bar {
  flex: 0 0 48px;
  width: 48px;
  height: 100%;
  background: var(--aipet-color-surface);
  border-right: 1px solid var(--aipet-color-border-faint);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  padding: var(--aipet-space-2) 0;
}

.activity-bar__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.activity-bar__spacer {
  flex: 1 1 auto;
}

.activity-bar__item {
  display: flex;
  justify-content: center;
}

.activity-bar__btn {
  position: relative;
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--aipet-color-text-3);
  cursor: pointer;
  border-radius: 6px;
  transition: color 120ms ease, background-color 120ms ease;
  padding: 0;
}

.activity-bar__btn:hover {
  color: var(--aipet-color-text-1);
  background: color-mix(in srgb, var(--aipet-color-text-1) 6%, transparent);
}

.activity-bar__btn:active {
  background: color-mix(in srgb, var(--aipet-color-text-1) 12%, transparent);
}

.activity-bar__btn--active {
  color: var(--aipet-color-primary);
}

/* VSCode-style：active 在 btn 左侧渲染 2px primary 竖条 */
.activity-bar__btn--active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 8px;
  bottom: 8px;
  width: 2px;
  border-radius: 2px;
  background: var(--aipet-color-primary);
}
</style>
