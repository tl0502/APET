<script setup lang="ts">
// Persona panel（#33 phase B 从 src/views/settings/panels/PersonaPanel.vue 迁入）。
//
// 改动 vs 原 PersonaPanel.vue：
// - 接 `PanelContext` props（dockview-vue 6.x 嵌套 props 模式；MVP 不消费 params）
// - 新增 isPanelActive ref + `subscribeContextKeys(['activePanel'])` 监听 workspace activePanel
//   → 传给 `<VrmAvatarExporter :is-active />` 让 VRM RAF 在切走时 pause（替代 inject('settings-active-tab')）
// - settings 独立窗 fallback：useWorkspaceManagerOptional 返 null 时 isPanelActive 永远 true
//   （独立窗内此 panel 即 active；Phase E 删独立窗后可改 useWorkspaceManager 严格版）
//
// 业务逻辑（getActivePersona + listen persona:activated）零改。

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { ElButton, ElDescriptions, ElDescriptionsItem, ElTag } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getActivePersona } from '@/services/persona'
import type { PersonaSummary } from '@/types/persona'
import VrmAvatarExporter from '@/components/settings/VrmAvatarExporter.vue'
import { useWorkspaceManagerOptional } from '@/composables/useWorkspaceManager'
import type { PanelContext } from '@/lib/workspace/types'

// dockview 嵌套 props：PanelContext<MyParams>（本 panel 无业务 params）
// settings 独立窗 mount 时 params 不存在；用 ? 守护
defineProps<{ params?: PanelContext }>()

const mgr = useWorkspaceManagerOptional()

const persona = ref<PersonaSummary | null>(null)
const errorMsg = ref<string | null>(null)
const loading = ref(true)
let unlistenActivated: UnlistenFn | null = null

// === panel active 监控 ===
// mgr 为 null = settings 独立窗内，永远视为 active（panel 唯一可见）
const isPanelActive = ref(mgr === null ? true : mgr.getActivePanel() === 'SettingsPersona')
let unsubActive: (() => void) | null = null

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

  if (mgr) {
    unsubActive = mgr.subscribeContextKeys(['activePanel'], () => {
      isPanelActive.value = mgr.getActivePanel() === 'SettingsPersona'
    })
  }
})

onBeforeUnmount(() => {
  unlistenActivated?.()
  unlistenActivated = null
  unsubActive?.()
  unsubActive = null
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

    <!-- #26 VRM 头像导出：is-active 由 workspace activePanel contextKey 驱动（替代 inject 链） -->
    <VrmAvatarExporter :persona-id="persona?.id ?? null" :is-active="isPanelActive" />
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
