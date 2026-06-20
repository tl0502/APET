<script setup lang="ts">
import { computed } from 'vue'
import { ElButton, ElIcon, ElTag } from 'element-plus'
import {
  Check,
  CircleCheckFilled,
  Close,
  WarningFilled,
} from '@element-plus/icons-vue'
import PersonaEditorTabs from './PersonaEditorTabs.vue'
import type {
  PersonaDiagnostic,
  PersonaSourceDraft,
  PersonaWorkshopMode,
} from '@/features/persona-workshop/types'
import type { PersonaSaveResult } from '@/types/persona'

const props = defineProps<{
  open: boolean
  draft: PersonaSourceDraft | null
  mode: PersonaWorkshopMode
  personaName: string
  diagnostics: PersonaDiagnostic[]
  tokenEstimate: number
  validating: boolean
  saving: boolean
  saveResult: PersonaSaveResult | null
  draftStateLabel: string
}>()

const emit = defineEmits<{
  close: []
  validate: []
  save: []
  'save-and-activate': []
  'update:mode': [mode: PersonaWorkshopMode]
  'update:draft': [draft: PersonaSourceDraft]
}>()

const hasDiagnostics = computed(() => props.diagnostics.length > 0)
const hasBlockingDiagnostics = computed(() =>
  props.diagnostics.some((diagnostic) => diagnostic.severity === 'error'),
)
const errorCount = computed(
  () => props.diagnostics.filter((diagnostic) => diagnostic.severity === 'error').length,
)
const warningCount = computed(
  () => props.diagnostics.filter((diagnostic) => diagnostic.severity === 'warning').length,
)
const diagnosticStateLabel = computed(() => {
  if (errorCount.value > 0) return `需要修正 ${errorCount.value} 项`
  if (warningCount.value > 0) return `${warningCount.value} 条建议，不影响保存`
  return '可保存'
})
const saveDisabled = computed(() => !props.draft || hasBlockingDiagnostics.value || props.saving)
const titleId = 'persona-inspector-title'
</script>

<template>
  <Transition name="persona-inspector">
    <div
      v-if="props.open"
      class="persona-inspector"
      role="dialog"
      aria-modal="true"
      aria-label="人格编辑卡片"
      :aria-labelledby="titleId"
    >
      <div
        class="persona-inspector__backdrop"
        aria-hidden="true"
        @click="emit('close')"
      />

      <aside class="persona-inspector__shell">
        <header class="persona-inspector__header">
          <div class="persona-inspector__identity">
            <p class="persona-inspector__eyebrow">Persona Card</p>
            <h3 :id="titleId" class="persona-inspector__title">{{ props.personaName }}</h3>
            <div class="persona-inspector__meta">
              <ElTag size="small" effect="plain">{{ props.mode }}</ElTag>
              <ElTag size="small" effect="plain">{{ props.draftStateLabel }}</ElTag>
              <ElTag size="small" effect="plain">~{{ props.tokenEstimate }} tokens</ElTag>
            </div>
          </div>
          <ElButton
            class="persona-inspector__close"
            :icon="Close"
            circle
            text
            aria-label="关闭"
            @click="emit('close')"
          />
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

        <footer class="persona-inspector__footer">
          <section class="persona-inspector__diagnostics" aria-label="编译诊断">
            <div class="persona-inspector__diagnostic-header">
              <div>
                <h4 class="persona-inspector__section-title">编译诊断</h4>
                <p class="persona-inspector__section-note">{{ diagnosticStateLabel }}</p>
              </div>
              <div class="persona-inspector__summary">
                <ElTag v-if="errorCount" size="small" type="danger">{{ errorCount }} error</ElTag>
                <ElTag v-if="warningCount" size="small" type="warning">
                  {{ warningCount }} warn
                </ElTag>
                <ElTag v-if="!hasDiagnostics" size="small" type="success">clean</ElTag>
              </div>
            </div>

            <p v-if="!hasDiagnostics" class="persona-inspector__ok">
              <ElIcon><CircleCheckFilled /></ElIcon>
              没有阻塞问题
            </p>
            <ul v-else class="persona-inspector__diagnostic-list">
              <li
                v-for="diagnostic in props.diagnostics"
                :key="diagnostic.code"
                :class="`persona-inspector__diagnostic persona-inspector__diagnostic--${diagnostic.severity}`"
              >
                <ElIcon class="persona-inspector__diagnostic-icon">
                  <WarningFilled v-if="diagnostic.severity === 'warning'" />
                  <Close v-else-if="diagnostic.severity === 'error'" />
                  <CircleCheckFilled v-else />
                </ElIcon>
                <span>{{ diagnostic.message }}</span>
              </li>
            </ul>

            <p v-if="props.saveResult" class="persona-inspector__save-state">
              <ElIcon><CircleCheckFilled /></ElIcon>
              已保存 v{{ props.saveResult.version }} · snapshot #{{ props.saveResult.snapshot_id }}
            </p>
          </section>

          <div class="persona-inspector__actions">
            <ElButton
              :disabled="!props.draft || props.validating || props.saving"
              :loading="props.validating"
              :icon="Check"
              @click="emit('validate')"
            >
              验证
            </ElButton>
            <ElButton
              :disabled="saveDisabled"
              :loading="props.saving"
              @click="emit('save')"
            >
              保存快照
            </ElButton>
            <ElButton
              type="primary"
              :disabled="saveDisabled"
              :loading="props.saving"
              @click="emit('save-and-activate')"
            >
              保存并激活
            </ElButton>
          </div>
        </footer>
      </aside>
    </div>
  </Transition>
</template>

<style scoped>
.persona-inspector {
  position: absolute;
  inset: 0;
  z-index: var(--aipet-z-dialog);
  display: grid;
  align-items: center;
  justify-items: center;
  min-width: 0;
  min-height: 0;
  padding: var(--aipet-space-6);
  color: var(--aipet-color-text-1);
}

.persona-inspector__backdrop {
  position: absolute;
  inset: 0;
  border: 0;
  padding: 0;
  background: color-mix(in srgb, var(--aipet-color-overlay) 34%, transparent);
  backdrop-filter: blur(12px) saturate(1.08);
  cursor: default;
}

.persona-inspector__shell {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  width: min(720px, 100%);
  height: min(760px, calc(100% - var(--aipet-space-8)));
  min-width: 0;
  min-height: min(460px, calc(100% - var(--aipet-space-8)));
  max-height: calc(100% - var(--aipet-space-8));
  padding: 0;
  border: 1px solid
    color-mix(in srgb, var(--aipet-color-border-faint) 72%, var(--aipet-color-border-strong) 28%);
  border-radius: var(--aipet-radius-lg);
  background:
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--aipet-color-surface-blur) 88%, var(--aipet-color-surface-raised)),
      color-mix(in srgb, var(--aipet-color-surface-raised) 86%, transparent)
    );
  box-shadow:
    0 22px 70px rgb(0 0 0 / 18%),
    0 0 0 1px rgb(255 255 255 / 18%) inset;
  backdrop-filter: blur(24px) saturate(1.16);
  overflow: hidden;
}

.persona-inspector__header {
  display: flex;
  flex: 0 0 auto;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--aipet-space-4);
  min-height: 86px;
  padding: var(--aipet-space-5) var(--aipet-space-5) var(--aipet-space-3);
  border-bottom: 1px solid var(--aipet-color-border-faint);
}

.persona-inspector__identity {
  flex: 1 1 auto;
  min-width: 0;
}

.persona-inspector__eyebrow {
  margin: 0;
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-inspector__title {
  margin: var(--aipet-space-1) 0 0;
  overflow: hidden;
  font-size: var(--aipet-font-size-xl);
  color: var(--aipet-color-text-1);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.persona-inspector__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--aipet-space-2);
  margin-top: var(--aipet-space-2);
}

.persona-inspector__close {
  flex: 0 0 auto;
  align-self: flex-start;
}

.persona-inspector__editor {
  flex: 1 1 auto;
  min-height: 0;
  padding: var(--aipet-space-4) var(--aipet-space-5);
  overflow: auto;
}

.persona-inspector__empty {
  display: grid;
  min-height: 220px;
  margin: 0;
  place-items: center;
  color: var(--aipet-color-text-3);
}

.persona-inspector__footer {
  display: flex;
  flex: 0 0 auto;
  align-items: end;
  justify-content: space-between;
  gap: var(--aipet-space-4);
  padding: var(--aipet-space-3) var(--aipet-space-5) var(--aipet-space-5);
  border-top: 1px solid var(--aipet-color-border-faint);
  background: color-mix(in srgb, var(--aipet-color-surface-raised) 72%, transparent);
}

.persona-inspector__diagnostics {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  gap: var(--aipet-space-2);
  min-width: 0;
}

.persona-inspector__diagnostic-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--aipet-space-3);
  color: var(--aipet-color-text-2);
}

.persona-inspector__section-title {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}

.persona-inspector__section-note {
  margin: var(--aipet-space-1) 0 0;
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-inspector__summary {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--aipet-space-1);
}

.persona-inspector__ok {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}

.persona-inspector__save-state {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-success);
}

.persona-inspector__diagnostic-list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
  max-height: 108px;
  margin: 0;
  padding: 0;
  list-style: none;
  overflow: auto;
}

.persona-inspector__diagnostic {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr);
  gap: var(--aipet-space-2);
  align-items: start;
  padding: var(--aipet-space-2);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-base);
  background: color-mix(in srgb, var(--aipet-color-surface-raised) 70%, transparent);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}

.persona-inspector__diagnostic-icon {
  margin-top: 1px;
}

.persona-inspector__diagnostic--error {
  border-color: color-mix(in srgb, var(--aipet-color-danger) 30%, var(--aipet-color-border-faint));
  background: color-mix(in srgb, var(--aipet-color-danger) 8%, var(--aipet-color-surface));
  color: var(--aipet-color-danger);
}

.persona-inspector__diagnostic--warning {
  border-color: color-mix(in srgb, var(--aipet-color-warning) 30%, var(--aipet-color-border-faint));
  background: color-mix(in srgb, var(--aipet-color-warning) 8%, var(--aipet-color-surface));
  color: var(--aipet-color-warning);
}

.persona-inspector__actions {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--aipet-space-2);
}

.persona-inspector__actions :deep(.el-button) {
  min-width: 0;
  margin-left: 0;
}

@container persona-workshop (max-width: 680px) {
  .persona-inspector {
    align-items: end;
    padding: var(--aipet-space-2);
  }

  .persona-inspector__shell {
    width: 100%;
    height: calc(100% - var(--aipet-space-2));
    min-height: 0;
    max-height: calc(100% - var(--aipet-space-2));
  }

  .persona-inspector__header {
    flex-direction: column;
    align-items: stretch;
    min-height: 74px;
    padding: var(--aipet-space-4) var(--aipet-space-4) var(--aipet-space-3);
  }

  .persona-inspector__close {
    align-self: flex-end;
  }

  .persona-inspector__editor {
    padding: var(--aipet-space-3) var(--aipet-space-4);
  }

  .persona-inspector__footer {
    flex-direction: column;
    align-items: stretch;
    padding: var(--aipet-space-3) var(--aipet-space-4) var(--aipet-space-4);
  }

  .persona-inspector__actions {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    justify-content: stretch;
  }
}

.persona-inspector-enter-active,
.persona-inspector-leave-active {
  transition: opacity var(--aipet-duration-base) var(--aipet-ease-standard);
}

.persona-inspector-enter-active .persona-inspector__shell,
.persona-inspector-leave-active .persona-inspector__shell {
  transition:
    opacity var(--aipet-duration-base) var(--aipet-ease-standard),
    transform var(--aipet-duration-base) var(--aipet-ease-standard);
}

.persona-inspector-enter-from,
.persona-inspector-leave-to {
  opacity: 0;
}

.persona-inspector-enter-from .persona-inspector__shell,
.persona-inspector-leave-to .persona-inspector__shell {
  opacity: 0;
  transform: translateY(10px) scale(0.98);
}
</style>
