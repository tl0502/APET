<script setup lang="ts">
// Persona tab：M1 占位（issue #9） + #21 接入「显示当前 active 而非硬编码 momo」+
// 监听 persona:activated 事件跨窗口刷新（onboarding 窗 / 设置面板自身切换都会触发）。
// 工坊按钮灰显，等 M2 启用（H.2/H.3 列表 / 编辑 / 切换 UI）。
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { ElButton, ElDescriptions, ElDescriptionsItem, ElTag } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getActivePersona } from '@/services/persona'
import type { PersonaSummary } from '@/types/persona'

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
  // 跨窗口监听：onboarding 窗 / 后续工坊 UI 切人格后,本面板自动刷新。
  // ElTabPane 是 v-show（不销毁），首次 mount 后用户切到其他 tab 再回来不会重 onMounted；
  // 没有 listener 的话 active 显示会停在首次拉取的值。
  try {
    unlistenActivated = await listen('persona:activated', () => {
      void refresh()
    })
  } catch (e) {
    // dev 浏览器模式下 listen 抛错；不阻断面板基本渲染。
    console.warn('[PersonaPanel] listen persona:activated failed:', e)
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
