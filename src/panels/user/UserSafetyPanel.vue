<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { ElSwitch } from 'element-plus'

import { useToast } from '@/composables/useToast'
import { useSafetyStore } from '@/stores/safety'
import type { SafetyScope } from '@/services/safety'

const store = useSafetyStore()
const toast = useToast()

interface SafetyToggleDef {
  scope: SafetyScope
  label: string
  hint: string
}

const TOGGLES: SafetyToggleDef[] = [
  {
    scope: 'prefixInjection',
    label: '安全前缀注入',
    hint: '开启后在对话 system prompt 第一位注入内置安全前缀。',
  },
  {
    scope: 'userInput',
    label: '用户输入扫描',
    hint: '开启后在发送前扫描用户输入，命中规则时拒绝或改写。',
  },
  {
    scope: 'streamToken',
    label: '流式输出扫描',
    hint: '开启后扫描模型输出流，命中软规则时替换尾部片段。',
  },
  {
    scope: 'finalOutput',
    label: '最终输出扫描',
    hint: '开启后在回复完成时做最终扫描，并写入 safety_scan_status。',
  },
]

const enabledCount = computed(
  () => TOGGLES.filter((item) => store.scopes[item.scope]).length,
)

onMounted(async () => {
  if (!store.loaded) {
    try {
      await store.load()
    } catch (e) {
      toast.error(`加载安全设置失败：${messageOf(e)}`)
    }
  }
})

async function onToggle(scope: SafetyScope, enabled: boolean) {
  try {
    await store.setScope(scope, enabled)
  } catch (e) {
    toast.error(`保存安全设置失败：${messageOf(e)}`)
  }
}

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <section class="safety-panel">
    <div class="safety-panel__summary">
      <div>
        <div class="safety-panel__summary-label">SafetyPolicy</div>
        <div class="safety-panel__summary-value">
          {{ enabledCount }} / {{ TOGGLES.length }} 已开启
        </div>
      </div>
      <span class="safety-panel__summary-chip">
        {{ enabledCount === 0 ? '全 OFF' : '自定义' }}
      </span>
    </div>

    <div class="safety-panel__section">
      <div
        v-for="item in TOGGLES"
        :key="item.scope"
        class="safety-panel__row"
      >
        <div class="safety-panel__row-main">
          <div class="safety-panel__row-title">{{ item.label }}</div>
          <div class="safety-panel__row-hint">{{ item.hint }}</div>
        </div>
        <ElSwitch
          :model-value="store.scopes[item.scope]"
          :disabled="!store.loaded || store.savingScopes[item.scope]"
          @update:model-value="(value) => onToggle(item.scope, Boolean(value))"
        />
      </div>
    </div>
  </section>
</template>

<style scoped>
.safety-panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  max-width: 720px;
}

.safety-panel__summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-4);
  padding: var(--aipet-space-4);
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-base);
}

.safety-panel__summary-label {
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.4;
}

.safety-panel__summary-value {
  margin-top: 2px;
  font-size: 18px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
  line-height: 1.3;
}

.safety-panel__summary-chip {
  flex: 0 0 auto;
  min-width: 64px;
  padding: 4px 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--aipet-color-primary) 12%, transparent);
  color: var(--aipet-color-primary);
  font-size: 12px;
  font-weight: 600;
  text-align: center;
}

.safety-panel__section {
  border-top: 1px solid var(--aipet-color-border-faint);
}

.safety-panel__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-4);
  padding: var(--aipet-space-4) 0;
  border-bottom: 1px solid var(--aipet-color-border-faint);
}

.safety-panel__row-main {
  min-width: 0;
}

.safety-panel__row-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--aipet-color-text-1);
  line-height: 1.4;
}

.safety-panel__row-hint {
  margin-top: 3px;
  font-size: 12px;
  color: var(--aipet-color-text-3);
  line-height: 1.5;
}
</style>
