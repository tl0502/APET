<script setup lang="ts">
// UserAboutPanel（#37 2026-05-21 重设计）— 关于（搬自 SettingsAboutPanel，套 panel--form 规范）。
//
// 与原版差异：
// - 套 panel--form 修饰类 + 包 .panel__content
// - 内容不变（应用名 + 版本 + 仓库 + 数据策略）

import { onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'

const APP_NAME = 'AI 桌宠'
const REPO_URL = 'https://github.com/tl0502/APET'
const DATA_POLICY_HINT = 'assets/legal/data_policy_v1.md（将在 #16 灵魂宣誓页随首次入库）'

const version = ref<string>('—')
const versionError = ref<string | null>(null)

onMounted(async () => {
  try {
    version.value = await getVersion()
  } catch (e) {
    versionError.value = e instanceof Error ? e.message : String(e)
  }
})
</script>

<template>
  <section class="panel panel--form">
    <h2 class="panel__title">关于</h2>
    <div class="panel__content">
      <dl class="about-grid">
        <dt>应用</dt>
        <dd>{{ APP_NAME }}</dd>

        <dt>版本</dt>
        <dd>
          <code v-if="!versionError">{{ version }}</code>
          <span v-else class="panel__error">{{ versionError }}</span>
        </dd>

        <dt>仓库</dt>
        <dd>
          <a :href="REPO_URL" target="_blank" rel="noopener">{{ REPO_URL }}</a>
        </dd>

        <dt>数据策略</dt>
        <dd class="panel__hint">{{ DATA_POLICY_HINT }}</dd>
      </dl>
    </div>
  </section>
</template>

<style scoped>
.about-grid {
  display: grid;
  grid-template-columns: 96px 1fr;
  gap: var(--aipet-space-2) var(--aipet-space-4);
  margin: 0;
}
.about-grid dt {
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}
.about-grid dd {
  margin: 0;
  color: var(--aipet-color-text-1);
  font-size: var(--aipet-font-size-base);
}
.about-grid a {
  color: var(--aipet-color-primary);
  text-decoration: none;
}
.about-grid a:hover {
  text-decoration: underline;
}
code {
  padding: 0 var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface-raised);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-2);
}
</style>
