<script setup lang="ts">
// 占位面板：番茄（#28）/ 待办（#29）tab 灰显时显示。
// 简单告知用户该功能未上线 + 跳转 issue 链接（10 天展示路径里只演示提醒）。
import { ElEmpty } from 'element-plus'

defineProps<{
  title: string
  issue: string
}>()
</script>

<template>
  <div class="panel placeholder-panel">
    <ElEmpty description="" :image-size="80">
      <template #image>
        <span class="placeholder-panel__emoji">🚧</span>
      </template>
      <div class="placeholder-panel__title">{{ title }}即将上线</div>
      <div class="placeholder-panel__hint">
        作为 M2 第二刀 / 第三刀实现，跟踪进度见
        <code class="placeholder-panel__code">{{ issue }}</code>
      </div>
    </ElEmpty>
  </div>
</template>

<style scoped>
.placeholder-panel {
  align-items: center;
  justify-content: center;
  min-height: 320px;
  padding: var(--aipet-space-8);
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

.placeholder-panel__code {
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--aipet-color-surface);
  color: var(--aipet-color-primary);
  font-family: var(--aipet-font-family-mono, ui-monospace, monospace);
  font-size: 12px;
}
</style>
