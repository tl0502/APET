<script setup lang="ts">
// Theme tab：M1 完整可用区（issue #9）。
// 三选一 radio 直接驱动 useThemeStore.setMode；store 内 storage event listener 让 pet 窗同步。
// dev 模式下露出 token 预览页地址（pet 窗 ?view=tokens 路由），便于设计核对。
import { ElRadio, ElRadioGroup } from 'element-plus'
import { useThemeStore } from '@/stores/theme'
import type { ThemeMode } from '@/stores/theme'

const theme = useThemeStore()
const isDev = import.meta.env.DEV

function onChange(value: ThemeMode | string | number | boolean | undefined) {
  if (value === 'auto' || value === 'light' || value === 'dark') {
    theme.setMode(value)
  }
}
</script>

<template>
  <section class="panel">
    <h2 class="panel__title">外观主题</h2>
    <p class="panel__hint">
      切换会同步到桌宠窗口（与未来的 onboarding / hub 窗口）；选择持久化到本地。
    </p>
    <ElRadioGroup :model-value="theme.mode" @change="onChange">
      <ElRadio value="auto">跟随系统（当前：{{ theme.systemDark ? '暗色' : '亮色' }}）</ElRadio>
      <ElRadio value="light">亮色</ElRadio>
      <ElRadio value="dark">暗色</ElRadio>
    </ElRadioGroup>

    <div v-if="isDev" class="panel__dev">
      <h3 class="panel__subtitle">开发工具</h3>
      <p class="panel__hint">
        在浏览器或新 webview 中访问
        <code>http://localhost:1420/?view=tokens</code>
        可查看 token 视觉对照页（仅 dev 模式可用）。
      </p>
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
}
.panel__title {
  margin: 0;
  font-size: var(--aipet-font-size-lg);
  font-weight: 600;
  color: var(--aipet-color-text-1);
}
.panel__subtitle {
  margin: 0;
  font-size: var(--aipet-font-size-base);
  font-weight: 600;
  color: var(--aipet-color-text-2);
}
.panel__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}
.panel__dev {
  margin-top: var(--aipet-space-4);
  padding: var(--aipet-space-3) var(--aipet-space-4);
  border: 1px dashed var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}
code {
  padding: 0 var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface-raised);
  color: var(--aipet-color-text-2);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
}
</style>
