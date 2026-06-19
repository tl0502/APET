<script setup lang="ts">
import { computed } from 'vue'
import { ElButton, ElTag } from 'element-plus'
import PersonaEditorTabs from './PersonaEditorTabs.vue'
import type {
  PersonaDiagnostic,
  PersonaSourceDraft,
  PersonaWorkshopMode,
} from '@/features/persona-workshop/types'

const props = defineProps<{
  open: boolean
  draft: PersonaSourceDraft | null
  mode: PersonaWorkshopMode
  personaName: string
  diagnostics: PersonaDiagnostic[]
  tokenEstimate: number
}>()

const emit = defineEmits<{
  close: []
  'update:mode': [mode: PersonaWorkshopMode]
  'update:draft': [draft: PersonaSourceDraft]
}>()

const hasDiagnostics = computed(() => props.diagnostics.length > 0)
</script>

<template>
  <aside v-if="props.open" class="persona-inspector" aria-label="人格编辑抽屉">
    <header class="persona-inspector__header">
      <div class="persona-inspector__identity">
        <p class="persona-inspector__eyebrow">Inspector</p>
        <h3 class="persona-inspector__title">{{ props.personaName }}</h3>
      </div>
      <ElButton size="small" @click="emit('close')">关闭</ElButton>
    </header>

    <div class="persona-inspector__editor">
      <PersonaEditorTabs
        v-if="props.draft"
        :draft="props.draft"
        :mode="props.mode"
        @update:mode="emit('update:mode', $event)"
        @update:draft="emit('update:draft', $event)"
      />
      <p v-else class="persona-inspector__empty">选择一张角色卡开始编辑</p>
    </div>

    <section class="persona-inspector__diagnostics" aria-label="编译诊断">
      <div class="persona-inspector__row">
        <span>Token 估算</span>
        <ElTag size="small">{{ props.tokenEstimate }}</ElTag>
      </div>

      <div class="persona-inspector__diagnostic-header">
        <h4 class="persona-inspector__section-title">编译诊断</h4>
        <ElTag v-if="hasDiagnostics" size="small" type="warning">
          {{ props.diagnostics.length }}
        </ElTag>
      </div>

      <p v-if="!hasDiagnostics" class="persona-inspector__ok">没有阻塞问题</p>
      <ul v-else class="persona-inspector__diagnostic-list">
        <li
          v-for="diagnostic in props.diagnostics"
          :key="diagnostic.code"
          :class="`persona-inspector__diagnostic persona-inspector__diagnostic--${diagnostic.severity}`"
        >
          {{ diagnostic.message }}
        </li>
      </ul>

      <div class="persona-inspector__actions">
        <ElButton disabled>验证</ElButton>
        <ElButton disabled>试聊</ElButton>
        <ElButton type="primary" disabled>保存快照</ElButton>
      </div>
    </section>
  </aside>
</template>

<style scoped>
.persona-inspector {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  min-width: 0;
  min-height: 0;
  height: 100%;
  padding-left: var(--aipet-space-4);
  border-left: 1px solid var(--aipet-color-border-faint);
  background: var(--aipet-color-surface);
  animation: persona-inspector-enter 160ms ease-out;
}

.persona-inspector__header {
  display: flex;
  flex: 0 0 auto;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--aipet-space-3);
}

.persona-inspector__identity {
  min-width: 0;
}

.persona-inspector__eyebrow {
  margin: 0;
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-inspector__title {
  margin: 0;
  overflow: hidden;
  font-size: var(--aipet-font-size-lg);
  color: var(--aipet-color-text-1);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.persona-inspector__editor {
  min-height: 0;
  overflow: auto;
}

.persona-inspector__empty {
  display: grid;
  min-height: 220px;
  margin: 0;
  place-items: center;
  color: var(--aipet-color-text-3);
}

.persona-inspector__diagnostics {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  gap: var(--aipet-space-3);
  padding-top: var(--aipet-space-4);
  border-top: 1px solid var(--aipet-color-border-faint);
}

.persona-inspector__row,
.persona-inspector__diagnostic-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-2);
  color: var(--aipet-color-text-2);
}

.persona-inspector__section-title {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}

.persona-inspector__ok {
  margin: 0;
  color: var(--aipet-color-text-3);
}

.persona-inspector__diagnostic-list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.persona-inspector__diagnostic {
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}

.persona-inspector__diagnostic--error {
  color: var(--aipet-color-danger);
}

.persona-inspector__diagnostic--warning {
  color: var(--aipet-color-warning);
}

.persona-inspector__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-2);
}

@keyframes persona-inspector-enter {
  from {
    opacity: 0;
    transform: translateX(18px);
  }

  to {
    opacity: 1;
    transform: translateX(0);
  }
}
</style>
