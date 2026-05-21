<script setup lang="ts">
// Persona panel（#33 phase B-redo 简化）：
// - 删 PanelContext / useWorkspaceManagerOptional / subscribeContextKeys
// - 直接接 props.isActive，由父级 DetailColumn 透传（workspaceLayout.currentItem === 'SettingsPersona'）
// - settings 独立窗 mount 时 props 缺失 → 默认 true（独立窗内此 panel 唯一可见）

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { ElButton, ElDescriptions, ElDescriptionsItem, ElTag } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getActivePersona } from '@/services/persona'
import type { PersonaSummary } from '@/types/persona'
import VrmAvatarExporter from '@/components/settings/VrmAvatarExporter.vue'

const props = withDefaults(defineProps<{ isActive?: boolean }>(), { isActive: true })

const persona = ref<PersonaSummary | null>(null)
const errorMsg = ref<string | null>(null)
const loading = ref(true)
let unlistenActivated: UnlistenFn | null = null

async function refresh() {
  try {
    persona.value = await getActivePersona()
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(async () => {
  await refresh()
  loading.value = false
  try {
    unlistenActivated = await listen('persona:activated', () => {
      void refresh()
    })
  } catch (e) {
    console.warn('[SettingsPersonaPanel] listen persona:activated failed:', e)
  }
})

onBeforeUnmount(() => {
  unlistenActivated?.()
  unlistenActivated = null
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

    <!-- #26 VRM 头像导出：is-active 由 workspaceLayout.currentItem === 'SettingsPersona' 驱动 -->
    <VrmAvatarExporter :persona-id="persona?.id ?? null" :is-active="props.isActive" />
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
