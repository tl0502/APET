<script setup lang="ts">
import { computed } from 'vue'
import { ElButton, ElInput, ElSlider } from 'element-plus'
import PersonaExampleEditor from './PersonaExampleEditor.vue'
import {
  getDraftExamplePairs,
  MAX_PERSONA_EXAMPLES,
  projectDraftToSource,
  withDraftExamplePairs,
} from '@/features/persona-workshop/draft'
import type {
  PersonaExamplePair,
  PersonaSourceDraft,
  PersonaWorkshopMode,
} from '@/features/persona-workshop/types'

const props = defineProps<{
  draft: PersonaSourceDraft
  mode: PersonaWorkshopMode
}>()

const emit = defineEmits<{
  'update:mode': [mode: PersonaWorkshopMode]
  'update:draft': [draft: PersonaSourceDraft]
}>()

const modeItems: Array<{ id: PersonaWorkshopMode; label: string }> = [
  { id: 'simple', label: '塑形' },
  { id: 'structured', label: '结构' },
  { id: 'examples', label: '示例' },
  { id: 'source', label: '源码' },
]

const sourceText = computed(() => projectDraftToSource(props.draft))
const examplePairs = computed(() => getDraftExamplePairs(props.draft))

function setMode(mode: PersonaWorkshopMode) {
  emit('update:mode', mode)
}

function updateSimple<K extends keyof PersonaSourceDraft['simple']>(
  key: K,
  value: PersonaSourceDraft['simple'][K],
) {
  emit('update:draft', {
    ...props.draft,
    simple: {
      ...props.draft.simple,
      [key]: value,
    },
  })
}

function updateStructured<K extends keyof PersonaSourceDraft['structured']>(
  key: K,
  value: PersonaSourceDraft['structured'][K],
) {
  emit('update:draft', {
    ...props.draft,
    structured: {
      ...props.draft.structured,
      [key]: value,
    },
  })
}

function updateRules(key: 'rulesDo' | 'rulesDont', value: string) {
  updateStructured(
    key,
    value
      .split(/\r?\n/)
      .map((line) => line.replace(/^- /, '').trim())
      .filter(Boolean),
  )
}

function updateExamples(pairs: PersonaExamplePair[]) {
  emit('update:draft', withDraftExamplePairs(props.draft, pairs))
}
</script>

<template>
  <section class="persona-editor" aria-label="人格编辑器">
    <div class="persona-editor__tabs" role="tablist" aria-label="编辑模式">
      <ElButton
        v-for="item in modeItems"
        :key="item.id"
        :type="props.mode === item.id ? 'primary' : 'default'"
        size="small"
        @click="setMode(item.id)"
      >
        {{ item.label }}
      </ElButton>
    </div>

    <div v-if="props.mode === 'simple'" class="persona-editor__body">
      <label class="persona-field">
        <span class="persona-field__label">名字</span>
        <ElInput
          :model-value="props.draft.simple.name"
          @update:model-value="updateSimple('name', String($event))"
        />
      </label>

      <label class="persona-field">
        <span class="persona-field__label">一句话定位</span>
        <ElInput
          :model-value="props.draft.simple.tagline"
          @update:model-value="updateSimple('tagline', String($event))"
        />
      </label>

      <div class="persona-sliders">
        <label class="persona-slider">
          <span>温暖</span>
          <ElSlider
            :model-value="props.draft.simple.warmth"
            :min="0"
            :max="5"
            @update:model-value="updateSimple('warmth', Number($event))"
          />
        </label>
        <label class="persona-slider">
          <span>俏皮</span>
          <ElSlider
            :model-value="props.draft.simple.playfulness"
            :min="0"
            :max="5"
            @update:model-value="updateSimple('playfulness', Number($event))"
          />
        </label>
        <label class="persona-slider">
          <span>主动</span>
          <ElSlider
            :model-value="props.draft.simple.proactivity"
            :min="0"
            :max="5"
            @update:model-value="updateSimple('proactivity', Number($event))"
          />
        </label>
      </div>
    </div>

    <div v-else-if="props.mode === 'structured'" class="persona-editor__body">
      <label class="persona-field">
        <span class="persona-field__label">身份</span>
        <ElInput
          type="textarea"
          :rows="4"
          :model-value="props.draft.structured.identity"
          @update:model-value="updateStructured('identity', String($event))"
        />
      </label>
      <label class="persona-field">
        <span class="persona-field__label">性格</span>
        <ElInput
          type="textarea"
          :rows="4"
          :model-value="props.draft.structured.personality"
          @update:model-value="updateStructured('personality', String($event))"
        />
      </label>
      <label class="persona-field">
        <span class="persona-field__label">能力</span>
        <ElInput
          type="textarea"
          :rows="3"
          :model-value="props.draft.structured.capabilities"
          @update:model-value="updateStructured('capabilities', String($event))"
        />
      </label>
      <label class="persona-field">
        <span class="persona-field__label">Do</span>
        <ElInput
          type="textarea"
          :rows="3"
          :model-value="props.draft.structured.rulesDo.join('\n')"
          @update:model-value="updateRules('rulesDo', String($event))"
        />
      </label>
      <label class="persona-field">
        <span class="persona-field__label">Don't</span>
        <ElInput
          type="textarea"
          :rows="3"
          :model-value="props.draft.structured.rulesDont.join('\n')"
          @update:model-value="updateRules('rulesDont', String($event))"
        />
      </label>
    </div>

    <div v-else-if="props.mode === 'examples'" class="persona-editor__body">
      <PersonaExampleEditor
        :pairs="examplePairs"
        :persona-name="props.draft.simple.name"
        :max-examples="MAX_PERSONA_EXAMPLES"
        @update:pairs="updateExamples"
      />
    </div>

    <div v-else class="persona-editor__body">
      <label class="persona-field persona-field--source">
        <span class="persona-field__label">源文件预览</span>
        <ElInput type="textarea" :rows="18" :model-value="sourceText" readonly />
      </label>
    </div>
  </section>
</template>

<style scoped>
.persona-editor {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.persona-editor__tabs {
  display: flex;
  gap: var(--aipet-space-2);
  padding-bottom: var(--aipet-space-4);
  border-bottom: 1px solid var(--aipet-color-border-faint);
}

.persona-editor__body {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  min-height: 0;
  padding-top: var(--aipet-space-4);
  overflow-y: auto;
}

.persona-field {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.persona-field--source {
  min-height: 0;
}

.persona-field__label {
  font-size: var(--aipet-font-size-sm);
  font-weight: 600;
  color: var(--aipet-color-text-2);
}

.persona-sliders {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--aipet-space-4);
}

.persona-slider {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
}
</style>
