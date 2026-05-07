<script setup lang="ts">
// LLM Provider tab：M1 占位（issue #9）。
// 6 preset 与 architecture §0.1 对齐；输入框灰显，等 #13 ChatService MVP 启用。
import { ElForm, ElFormItem, ElInput, ElTag } from 'element-plus'

interface Preset {
  id: string
  name: string
  hint: string
}

// 顺序与 architecture §0.1 一致；自定义放最后。
const PRESETS: Preset[] = [
  { id: 'openai', name: 'OpenAI', hint: 'gpt-4o / gpt-4o-mini' },
  { id: 'deepseek', name: 'DeepSeek', hint: 'deepseek-chat / deepseek-reasoner' },
  { id: 'moonshot', name: 'Moonshot', hint: 'moonshot-v1-8k / 32k / 128k' },
  { id: 'qwen', name: 'Qwen', hint: 'qwen-plus / qwen-max（DashScope OpenAI 兼容）' },
  { id: 'ollama', name: 'Ollama', hint: '本地端点（如 llama3.1 / qwen2.5）' },
  { id: 'custom', name: '自定义', hint: '任意 OpenAI 兼容协议端点' },
]
</script>

<template>
  <section class="panel">
    <h2 class="panel__title">LLM Provider</h2>
    <p class="panel__hint">
      预设清单（OpenAI 兼容协议，6 个）。配置 API Key、Base URL、模型与连接测试将在
      <code>#13 ChatService MVP</code> 启用。
    </p>

    <ul class="preset-list">
      <li v-for="preset in PRESETS" :key="preset.id" class="preset-item">
        <ElTag>{{ preset.name }}</ElTag>
        <span class="preset-hint">{{ preset.hint }}</span>
      </li>
    </ul>

    <ElForm class="placeholder-form" label-position="top" disabled>
      <ElFormItem label="API Key">
        <ElInput placeholder="将在 #13 ChatService MVP 启用" />
      </ElFormItem>
      <ElFormItem label="Base URL">
        <ElInput placeholder="将在 #13 ChatService MVP 启用" />
      </ElFormItem>
      <ElFormItem label="模型 ID">
        <ElInput placeholder="将在 #13 ChatService MVP 启用" />
      </ElFormItem>
    </ElForm>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
}
.panel__title {
  margin: 0;
  font-size: var(--aipet-font-size-lg);
  font-weight: 600;
  color: var(--aipet-color-text-1);
}
.panel__hint {
  margin: 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}
.preset-list {
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--aipet-space-2);
  list-style: none;
}
.preset-item {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  padding: var(--aipet-space-2) var(--aipet-space-3);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
}
.preset-hint {
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-xs);
}
.placeholder-form {
  max-width: 480px;
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
