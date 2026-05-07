<script setup lang="ts">
// Persona tab：M1 占位（issue #9）。
// 接 persona_load('momo') 仅展示当前激活人格；工坊按钮灰显，等 M2 启用。
// 当前 IPC 仅 persona_load + persona_activate（#5），无 persona_list；M1 W1 只 seed momo
// 一个内置人格，直接 load 'momo' 即可；H.2/H.3 工坊上线后再加列表。
import { onMounted, ref } from 'vue'
import { ElButton, ElDescriptions, ElDescriptionsItem, ElTag } from 'element-plus'
import { loadPersona } from '@/services/persona'
import type { PersonaSummary } from '@/types/persona'

const persona = ref<PersonaSummary | null>(null)
const errorMsg = ref<string | null>(null)
const loading = ref(true)

onMounted(async () => {
  try {
    persona.value = await loadPersona('momo')
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <section class="panel">
    <h2 class="panel__title">人格</h2>
    <p class="panel__hint">
      当前激活人格信息。<code>.soul.md</code> 编辑、人格列表、内置/用户切换将在
      <code>人格工坊（M2 W4）</code> 上线。
    </p>

    <p v-if="loading" class="panel__hint">加载中...</p>
    <p v-else-if="errorMsg" class="panel__error">读取失败：{{ errorMsg }}</p>
    <ElDescriptions v-else-if="persona" :column="1" border>
      <ElDescriptionsItem label="ID">
        <code>{{ persona.id }}</code>
      </ElDescriptionsItem>
      <ElDescriptionsItem label="名称">{{ persona.name }}</ElDescriptionsItem>
      <ElDescriptionsItem label="版本">
        <ElTag size="small">{{ persona.version }}</ElTag>
      </ElDescriptionsItem>
      <ElDescriptionsItem label="来源">
        <ElTag size="small" :type="persona.source === 'builtin' ? 'info' : 'success'">
          {{ persona.source }}
        </ElTag>
      </ElDescriptionsItem>
    </ElDescriptions>

    <div class="panel__actions">
      <ElButton disabled>打开人格工坊</ElButton>
      <span class="panel__hint">工坊将在 M2 上线</span>
    </div>
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
.panel__error {
  margin: 0;
  color: var(--aipet-color-danger);
  font-size: var(--aipet-font-size-sm);
}
.panel__actions {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
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
