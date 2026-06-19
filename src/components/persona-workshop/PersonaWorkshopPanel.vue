<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from 'vue'
import { ElButton } from 'element-plus'
import PersonaCardStage from './PersonaCardStage.vue'
import PersonaInspectorDrawer from './PersonaInspectorDrawer.vue'
import { createPersonaDraft, estimateDraftTokens, validatePersonaDraft } from '@/features/persona-workshop/draft'
import type { PersonaSourceDraft, PersonaWorkshopMode } from '@/features/persona-workshop/types'
import { getActivePersona, listPersonas, loadPersona } from '@/services/persona'
import type { PersonaListItem } from '@/types/persona'

defineProps<{
  isActive?: boolean
}>()

const loading = shallowRef(true)
const errorMsg = shallowRef<string | null>(null)
const personas = ref<PersonaListItem[]>([])
const selectedId = shallowRef<string | null>(null)
const mode = shallowRef<PersonaWorkshopMode>('simple')
const draft = ref<PersonaSourceDraft | null>(null)
const inspectorOpen = shallowRef(false)

const diagnostics = computed(() => (draft.value ? validatePersonaDraft(draft.value) : []))
const tokenEstimate = computed(() => (draft.value ? estimateDraftTokens(draft.value) : 0))
const personaName = computed(() => draft.value?.simple.name ?? '未选择人格')

async function loadInitial() {
  loading.value = true
  try {
    const [list, active] = await Promise.all([listPersonas(), getActivePersona()])
    personas.value = list
    selectedId.value = active.id
    draft.value = createPersonaDraft(active)
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function selectPersona(id: string) {
  inspectorOpen.value = true
  if (selectedId.value === id) return
  selectedId.value = id
  try {
    const persona = await loadPersona(id)
    draft.value = createPersonaDraft(persona)
    mode.value = 'simple'
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(() => {
  void loadInitial()
})
</script>

<template>
  <section class="persona-workshop panel panel--form" aria-label="Persona Workshop">
    <header class="persona-workshop__header">
      <div>
        <p class="persona-workshop__eyebrow">Persona Workshop</p>
        <h2 class="persona-workshop__title">人格工坊</h2>
      </div>
      <ElButton size="small" @click="loadInitial">刷新</ElButton>
    </header>

    <p v-if="errorMsg" class="persona-workshop__error">读取失败：{{ errorMsg }}</p>

    <div
      class="persona-workshop__layout"
      :class="{ 'persona-workshop__layout--inspector-open': inspectorOpen }"
    >
      <main class="persona-workshop__stage">
        <PersonaCardStage
          :personas="personas"
          :selected-id="selectedId"
          :loading="loading"
          @select="selectPersona"
        />
      </main>

      <PersonaInspectorDrawer
        :open="inspectorOpen"
        :draft="draft"
        :mode="mode"
        :persona-name="personaName"
        :diagnostics="diagnostics"
        :token-estimate="tokenEstimate"
        @close="inspectorOpen = false"
        @update:mode="mode = $event"
        @update:draft="draft = $event"
      />
    </div>
  </section>
</template>

<style scoped>
.persona-workshop {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  min-width: 0;
  min-height: 0;
  height: 100%;
}

.persona-workshop__header {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-4);
}

.persona-workshop__eyebrow {
  margin: 0;
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-workshop__title {
  margin: 0;
  font-size: var(--aipet-font-size-xl);
  color: var(--aipet-color-text-1);
}

.persona-workshop__error {
  margin: 0;
  color: var(--aipet-color-danger);
}

.persona-workshop__layout {
  display: grid;
  position: relative;
  grid-template-columns: minmax(0, 1fr);
  gap: var(--aipet-space-4);
  min-height: 0;
  flex: 1 1 auto;
  overflow: hidden;
}

.persona-workshop__layout--inspector-open {
  grid-template-columns: minmax(0, 1fr) minmax(340px, 380px);
}

.persona-workshop__stage {
  min-width: 0;
  min-height: 0;
}

@media (max-width: 900px) {
  .persona-workshop__layout--inspector-open {
    grid-template-columns: minmax(0, 1fr);
  }

  .persona-workshop__layout--inspector-open :deep(.persona-inspector) {
    position: absolute;
    inset: 0 0 0 auto;
    z-index: 1;
    width: min(380px, 100%);
    padding: var(--aipet-space-4);
    border-left: 1px solid var(--aipet-color-border-faint);
    background: var(--aipet-color-surface);
    box-shadow: -18px 0 32px rgb(0 0 0 / 12%);
  }
}
</style>
