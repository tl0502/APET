<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from 'vue'
import { ElButton } from 'element-plus'
import PersonaCardStage from './PersonaCardStage.vue'
import PersonaInspectorDrawer from './PersonaInspectorDrawer.vue'
import {
  createBlankPersonaDraft,
  createPersonaDraft,
  duplicatePersonaDraft,
  estimateDraftTokens,
  validatePersonaDraft,
} from '@/features/persona-workshop/draft'
import type {
  PersonaDiagnostic,
  PersonaSourceDraft,
  PersonaWorkshopMode,
} from '@/features/persona-workshop/types'
import {
  getActivePersona,
  listPersonas,
  loadPersona,
  saveAndActivatePersonaDraft,
  savePersonaDraft,
  validatePersonaDraft as validatePersonaDraftRemote,
} from '@/services/persona'
import type { PersonaListItem, PersonaSaveResult } from '@/types/persona'

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
const validating = shallowRef(false)
const saving = shallowRef(false)
const serverDiagnostics = ref<PersonaDiagnostic[] | null>(null)
const saveResult = shallowRef<PersonaSaveResult | null>(null)
const draftOrigin = shallowRef<'saved' | 'new' | 'copy'>('saved')
const savedDraftFingerprint = shallowRef('')

const localDiagnostics = computed(() => (draft.value ? validatePersonaDraft(draft.value) : []))
const diagnostics = computed(() => serverDiagnostics.value ?? localDiagnostics.value)
const tokenEstimate = computed(() => (draft.value ? estimateDraftTokens(draft.value) : 0))
const personaName = computed(() => draft.value?.simple.name ?? '未选择人格')
const draftStateLabel = computed(() => {
  if (!draft.value) return '未选择'
  if (draftOrigin.value === 'new') return '新建未保存'
  if (draftOrigin.value === 'copy') return '复制未保存'
  if (draftFingerprint(draft.value) !== savedDraftFingerprint.value) return '有未保存修改'
  if (saveResult.value) return `已保存 v${saveResult.value.version}`
  return '已保存'
})

function draftFingerprint(value: PersonaSourceDraft): string {
  return JSON.stringify(value)
}

function rememberSavedDraft(nextDraft: PersonaSourceDraft) {
  draftOrigin.value = 'saved'
  savedDraftFingerprint.value = draftFingerprint(nextDraft)
}

function resetTransientState() {
  serverDiagnostics.value = null
  saveResult.value = null
  errorMsg.value = null
}

async function loadInitial() {
  loading.value = true
  try {
    const [list, active] = await Promise.all([listPersonas(), getActivePersona()])
    personas.value = list
    selectedId.value = active.id
    draft.value = createPersonaDraft(active)
    rememberSavedDraft(draft.value)
    resetTransientState()
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
    rememberSavedDraft(draft.value)
    mode.value = 'simple'
    resetTransientState()
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  }
}

function createNewPersona() {
  const existingIds = personas.value.map((persona) => persona.id)
  const nextDraft = createBlankPersonaDraft(existingIds)
  draft.value = nextDraft
  selectedId.value = nextDraft.personaId
  draftOrigin.value = 'new'
  savedDraftFingerprint.value = ''
  inspectorOpen.value = true
  mode.value = 'simple'
  resetTransientState()
}

function duplicateCurrentPersona() {
  if (!draft.value) return
  const existingIds = personas.value.map((persona) => persona.id)
  const nextDraft = duplicatePersonaDraft(draft.value, existingIds)
  draft.value = nextDraft
  selectedId.value = nextDraft.personaId
  draftOrigin.value = 'copy'
  savedDraftFingerprint.value = ''
  inspectorOpen.value = true
  mode.value = 'simple'
  resetTransientState()
}

function updateDraft(nextDraft: PersonaSourceDraft) {
  draft.value = nextDraft
  serverDiagnostics.value = null
  saveResult.value = null
}

async function validateCurrentDraft() {
  if (!draft.value) return
  validating.value = true
  try {
    const result = await validatePersonaDraftRemote(draft.value)
    serverDiagnostics.value = result.diagnostics
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    validating.value = false
  }
}

async function persistCurrentDraft(activate: boolean) {
  if (!draft.value) return
  saving.value = true
  try {
    const result = activate
      ? await saveAndActivatePersonaDraft(draft.value)
      : await savePersonaDraft(draft.value)
    saveResult.value = result
    serverDiagnostics.value = result.diagnostics
    const savedDraft = {
      ...draft.value,
      version: result.version,
      source: 'user',
    }
    draft.value = savedDraft
    rememberSavedDraft(savedDraft)
    personas.value = await listPersonas()
    selectedId.value = result.persona_id
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
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
      <div class="persona-workshop__actions">
        <ElButton size="small" type="primary" @click="createNewPersona">新建人格</ElButton>
        <ElButton size="small" @click="loadInitial">刷新</ElButton>
      </div>
    </header>

    <p v-if="errorMsg" class="persona-workshop__error">读取失败：{{ errorMsg }}</p>

    <div class="persona-workshop__layout">
      <main
        class="persona-workshop__stage"
        :class="{ 'persona-workshop__stage--obscured': inspectorOpen }"
      >
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
        :validating="validating"
        :saving="saving"
        :save-result="saveResult"
        :draft-state-label="draftStateLabel"
        @close="inspectorOpen = false"
        @validate="validateCurrentDraft"
        @duplicate="duplicateCurrentPersona"
        @save="persistCurrentDraft(false)"
        @save-and-activate="persistCurrentDraft(true)"
        @update:mode="mode = $event"
        @update:draft="updateDraft"
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

.persona-workshop__actions {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--aipet-space-2);
}

.persona-workshop__error {
  margin: 0;
  color: var(--aipet-color-danger);
}

.persona-workshop__layout {
  display: grid;
  position: relative;
  grid-template-columns: minmax(0, 1fr);
  min-height: 0;
  flex: 1 1 auto;
  container-name: persona-workshop;
  container-type: inline-size;
  overflow: hidden;
}

.persona-workshop__stage {
  min-width: 0;
  min-height: 0;
  transition:
    opacity var(--aipet-duration-base) var(--aipet-ease-standard),
    transform var(--aipet-duration-base) var(--aipet-ease-standard);
}

.persona-workshop__stage--obscured {
  opacity: 0.82;
  transform: scale(0.992);
  pointer-events: none;
}
</style>
