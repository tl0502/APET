<script setup lang="ts">
// UserHelpPanel（#37 2026-05-21 重设计）— 帮助（链接 + 快捷键速查）。
//
// 静态内容：
// - GitHub 仓库链接
// - 项目文档链接（README + STATUS）
// - 全局快捷键速查表

const REPO_URL = 'https://github.com/tl0502/APET'
const DOCS_URL = 'https://github.com/tl0502/APET/blob/main/docs/README.md'

interface ShortcutDef {
  keys: string
  desc: string
}

const SHORTCUTS: ShortcutDef[] = [
  { keys: 'Ctrl + Alt + W', desc: '打开 / 切换工作区' },
  { keys: 'Esc', desc: '关闭工作区 / 关闭用户 popup' },
  { keys: 'Enter', desc: '对话发送（chat 输入框内）' },
  { keys: 'Shift + Enter', desc: '对话换行（chat 输入框内）' },
]
</script>

<template>
  <section class="panel panel--form">
    <div class="panel__content">
      <div class="panel__section">
        <h3 class="panel__subtitle">链接</h3>
        <ul class="help-links">
          <li>
            <a :href="REPO_URL" target="_blank" rel="noopener">GitHub 仓库</a>
            <span class="panel__hint">提 issue、查 release</span>
          </li>
          <li>
            <a :href="DOCS_URL" target="_blank" rel="noopener">项目文档</a>
            <span class="panel__hint">架构、决策、roadmap</span>
          </li>
        </ul>
      </div>

      <div class="panel__section">
        <h3 class="panel__subtitle">快捷键</h3>
        <dl class="shortcut-grid">
          <template v-for="s in SHORTCUTS" :key="s.keys">
            <dt>
              <kbd>{{ s.keys }}</kbd>
            </dt>
            <dd>{{ s.desc }}</dd>
          </template>
        </dl>
      </div>
    </div>
  </section>
</template>

<style scoped>
.help-links {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}
.help-links li {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.help-links a {
  color: var(--aipet-color-primary);
  text-decoration: none;
  font-size: var(--aipet-font-size-base);
}
.help-links a:hover {
  text-decoration: underline;
}

.shortcut-grid {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: var(--aipet-space-2) var(--aipet-space-4);
  margin: 0;
}
.shortcut-grid dt {
  margin: 0;
}
.shortcut-grid dd {
  margin: 0;
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-base);
}
kbd {
  padding: 2px var(--aipet-space-2);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface);
  border: 1px solid var(--aipet-color-border);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-1);
}
</style>
