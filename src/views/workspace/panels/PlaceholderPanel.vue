<script setup lang="ts">
// PlaceholderPanel：dockview 主区里的占位 panel（#35 Phase C，MVP 演示）
//
// 用途：
// - 验证 spike 4 实操坑全部落地 — 尤其是坑 3 嵌套 props（PanelContext<T>）
// - 给 Phase D 命令面板 / Phase E 入口三件套提供"有东西可切"的视觉反馈
// - P2 #33 实业务 panel 迁入前的占位（同一 SFC 走 3 个不同 component 名注册）
//
// spike #32 坑 3 落地：dockview-vue 6.x 给 panel SFC 的 props 是嵌套结构
// `{ params: { params: userParams, api, containerApi, tabLocation } }`。本 SFC
// 用 PanelContext<MyParams> 抽象，让业务 panel 写法干净。

import { computed } from 'vue'
import { ElEmpty } from 'element-plus'

import type { PanelContext } from '@/lib/workspace/types'

interface MyParams {
  /** 三个占位 panel 区分用：'chat' | 'library' | 'settings' */
  tone?: 'chat' | 'library' | 'settings'
  /** 自定义副标题（命令面板 / ActivityBar 触发时可传） */
  subtitle?: string
}

const props = defineProps<{ params: PanelContext<MyParams> }>()

const tone = computed(() => props.params.params.tone ?? 'chat')
const subtitle = computed(() => props.params.params.subtitle ?? '')

const toneMeta = computed(() => {
  switch (tone.value) {
    case 'library':
      return { emoji: '📚', title: '资源库占位', hint: 'P2 #33 迁入：装扮商城 / 人格工坊 / 声音库' }
    case 'settings':
      return { emoji: '⚙️', title: '设置占位', hint: 'P2 #33 迁入：主题 / 模型 / 偏好' }
    default:
      return { emoji: '💬', title: '对话占位', hint: 'P2 #33 迁入：ChatHub（多对话 + 历史）' }
  }
})
</script>

<template>
  <div class="placeholder-panel">
    <ElEmpty :image-size="80">
      <template #image>
        <span class="placeholder-panel__emoji">{{ toneMeta.emoji }}</span>
      </template>
      <div class="placeholder-panel__title">{{ toneMeta.title }}</div>
      <div class="placeholder-panel__hint">{{ toneMeta.hint }}</div>
      <div v-if="subtitle" class="placeholder-panel__subtitle">{{ subtitle }}</div>
      <div class="placeholder-panel__meta">
        <code>tabLocation = {{ params.tabLocation }}</code>
      </div>
    </ElEmpty>
  </div>
</template>

<style scoped>
.placeholder-panel {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  padding: var(--aipet-space-8);
  background: var(--aipet-color-bg);
}

.placeholder-panel__emoji {
  font-size: 56px;
  line-height: 1;
}

.placeholder-panel__title {
  margin-top: var(--aipet-space-3);
  font-size: 16px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.placeholder-panel__hint {
  margin-top: var(--aipet-space-2);
  font-size: 13px;
  color: var(--aipet-color-text-2);
}

.placeholder-panel__subtitle {
  margin-top: var(--aipet-space-2);
  font-size: 12px;
  color: var(--aipet-color-text-3);
  font-style: italic;
}

.placeholder-panel__meta {
  margin-top: var(--aipet-space-4);
  font-size: 11px;
  color: var(--aipet-color-text-3);
}

.placeholder-panel__meta code {
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--aipet-color-surface);
  font-family: var(--aipet-font-family-mono, ui-monospace, monospace);
}
</style>
