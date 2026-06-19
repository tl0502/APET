<script setup lang="ts">
import { computed } from 'vue'
import { Delete, Plus } from '@element-plus/icons-vue'
import { ElButton, ElInput } from 'element-plus'
import type { PersonaExamplePair } from '@/features/persona-workshop/types'

const props = defineProps<{
  pairs: PersonaExamplePair[]
  personaName: string
  maxExamples: number
}>()

const emit = defineEmits<{
  'update:pairs': [pairs: PersonaExamplePair[]]
}>()

const canAdd = computed(() => props.pairs.length < props.maxExamples)
const assistantLabel = computed(() => props.personaName.trim() || '助手')

function updatePair(index: number, patch: Partial<PersonaExamplePair>) {
  emit(
    'update:pairs',
    props.pairs.map((pair, pairIndex) =>
      pairIndex === index ? { ...pair, ...patch } : pair,
    ),
  )
}

function addPair() {
  if (!canAdd.value) return
  emit('update:pairs', [...props.pairs, { user: '', assistant: '' }])
}

function removePair(index: number) {
  emit(
    'update:pairs',
    props.pairs.filter((_, pairIndex) => pairIndex !== index),
  )
}

function pairCharCount(pair: PersonaExamplePair): number {
  return pair.user.trim().length + pair.assistant.trim().length
}
</script>

<template>
  <section class="persona-example-editor" aria-label="示例对话编辑器">
    <div v-if="props.pairs.length === 0" class="persona-example-editor__empty">
      <p>还没有示例对话</p>
      <ElButton :icon="Plus" aria-label="添加示例" @click="addPair">添加示例</ElButton>
    </div>

    <div v-else class="persona-example-editor__list">
      <article
        v-for="(pair, index) in props.pairs"
        :key="index"
        class="persona-example-card"
      >
        <header class="persona-example-card__header">
          <div>
            <h4 class="persona-example-card__title">示例 {{ index + 1 }}</h4>
            <p class="persona-example-card__meta">{{ pairCharCount(pair) }} chars</p>
          </div>
          <ElButton
            :icon="Delete"
            circle
            text
            :aria-label="`删除示例 ${index + 1}`"
            @click="removePair(index)"
          />
        </header>

        <label class="persona-example-field">
          <span class="persona-example-field__label">用户</span>
          <ElInput
            type="textarea"
            :rows="2"
            :model-value="pair.user"
            @update:model-value="updatePair(index, { user: String($event) })"
          />
        </label>

        <label class="persona-example-field">
          <span class="persona-example-field__label">{{ assistantLabel }}</span>
          <ElInput
            type="textarea"
            :rows="2"
            :model-value="pair.assistant"
            @update:model-value="updatePair(index, { assistant: String($event) })"
          />
        </label>
      </article>

      <ElButton
        class="persona-example-editor__add"
        :icon="Plus"
        :disabled="!canAdd"
        aria-label="添加示例"
        @click="addPair"
      >
        添加示例
      </ElButton>
    </div>
  </section>
</template>

<style scoped>
.persona-example-editor {
  min-width: 0;
}

.persona-example-editor__empty {
  display: grid;
  gap: var(--aipet-space-3);
  min-height: 180px;
  place-items: center;
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-card);
  background: color-mix(in srgb, var(--aipet-color-surface-raised) 74%, transparent);
  color: var(--aipet-color-text-3);
}

.persona-example-editor__empty p {
  margin: 0;
}

.persona-example-editor__list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
}

.persona-example-card {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  min-width: 0;
  padding: var(--aipet-space-4);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-card);
  background: color-mix(in srgb, var(--aipet-color-surface-raised) 78%, transparent);
}

.persona-example-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--aipet-space-3);
}

.persona-example-card__title {
  margin: 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-1);
}

.persona-example-card__meta {
  margin: var(--aipet-space-1) 0 0;
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-example-field {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.persona-example-field__label {
  font-size: var(--aipet-font-size-xs);
  font-weight: 600;
  color: var(--aipet-color-text-2);
}

.persona-example-editor__add {
  align-self: flex-start;
}
</style>
