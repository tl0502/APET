<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from 'vue'
import { open, save as saveFileDialog } from '@tauri-apps/plugin-dialog'
import { ElButton, ElMessageBox } from 'element-plus'
import { CopyDocument, Delete, Download, Upload } from '@element-plus/icons-vue'
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
  activatePersonaSnapshot,
  deletePersona,
  exportPersonaSnapshot,
  getActivePersona,
  importPersonaFromPath,
  listPersonas,
  loadPersona,
  saveAndActivatePersonaDraft,
  savePersonaDraft,
  validatePersonaDraft as validatePersonaDraftRemote,
} from '@/services/persona'
import type { PersonaListItem, PersonaSaveResult, PersonaSummary } from '@/types/persona'

defineProps<{
  isActive?: boolean
}>()

const loading = shallowRef(true)
const errorMsg = shallowRef<string | null>(null)
const statusMsg = shallowRef<string | null>(null)
const personas = ref<PersonaListItem[]>([])
const selectedId = shallowRef<string | null>(null)
const selectedSnapshotId = shallowRef<string | null>(null)
const mode = shallowRef<PersonaWorkshopMode>('simple')
const draft = ref<PersonaSourceDraft | null>(null)
const inspectorOpen = shallowRef(false)
const validating = shallowRef(false)
const saving = shallowRef(false)
const importing = shallowRef(false)
const exporting = shallowRef(false)
const deleting = shallowRef(false)
const restoring = shallowRef(false)
const serverDiagnostics = ref<PersonaDiagnostic[] | null>(null)
const saveResult = shallowRef<PersonaSaveResult | null>(null)
const draftOrigin = shallowRef<'saved' | 'new' | 'copy'>('saved')
const savedDraftFingerprint = shallowRef('')

const localDiagnostics = computed(() => (draft.value ? validatePersonaDraft(draft.value) : []))
const diagnostics = computed(() => serverDiagnostics.value ?? localDiagnostics.value)
const tokenEstimate = computed(() => (draft.value ? estimateDraftTokens(draft.value) : 0))
const personaName = computed(() => draft.value?.simple.name ?? '未选择人格')
const selectedPersona = computed(() =>
  selectedId.value ? personas.value.find((persona) => persona.id === selectedId.value) : null,
)
// 有未保存修改：新建/复制草稿，或 draft 与上次保存的指纹不一致。两个「保存」键以此为准启用。
const isDirty = computed(
  () =>
    draftOrigin.value !== 'saved' ||
    (draft.value ? draftFingerprint(draft.value) !== savedDraftFingerprint.value : false),
)
// 当前选中的人格是否已是全局 active（决定「激活」键是否还有意义）。
const isSelectedActive = computed(() => selectedPersona.value?.is_active === true)
const canExportSnapshot = computed(() => {
  if (!selectedSnapshotId.value || !draft.value || draftOrigin.value !== 'saved') return false
  const snapshotId = Number(selectedSnapshotId.value)
  if (!Number.isFinite(snapshotId) || snapshotId <= 0) return false
  return draftFingerprint(draft.value) === savedDraftFingerprint.value
})
const canDeleteSelected = computed(() => {
  if (!draft.value || draftOrigin.value !== 'saved') return false
  if (!selectedPersona.value) return false
  if (selectedPersona.value.source === 'builtin' || selectedPersona.value.is_active) return false
  return !saving.value && !exporting.value && !deleting.value
})
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
  statusMsg.value = null
}

async function loadInitial() {
  loading.value = true
  try {
    const [list, active] = await Promise.all([listPersonas(), getActivePersona()])
    personas.value = list
    selectedId.value = active.id
    selectedSnapshotId.value = active.snapshot_id
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
  if (selectedId.value === id) return
  selectedId.value = id
  try {
    const persona = await loadPersona(id)
    draft.value = createPersonaDraft(persona)
    selectedSnapshotId.value = persona.snapshot_id
    rememberSavedDraft(draft.value)
    mode.value = 'simple'
    resetTransientState()
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  }
}

// 双击进编辑：先确保选中（含拉取 draft），再开抽屉。单击只选中（见 selectPersona）。
async function editPersona(id: string) {
  if (selectedId.value !== id) await selectPersona(id)
  inspectorOpen.value = true
}

function createNewPersona() {
  const existingIds = personas.value.map((persona) => persona.id)
  const nextDraft = createBlankPersonaDraft(existingIds)
  draft.value = nextDraft
  selectedId.value = nextDraft.personaId
  selectedSnapshotId.value = null
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
  selectedSnapshotId.value = null
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
  statusMsg.value = null
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
    selectedSnapshotId.value = result.snapshot_id
    rememberSavedDraft(savedDraft)
    personas.value = await listPersonas()
    selectedId.value = result.persona_id
    statusMsg.value = activate ? '已保存并激活' : '已保存快照'
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

async function importSoulMarkdown() {
  importing.value = true
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Soul Markdown', extensions: ['md'] }],
    })
    if (typeof selected !== 'string') return

    const result = await importPersonaFromPath(selected, false)
    const [nextList, imported] = await Promise.all([
      listPersonas(),
      loadPersona(result.persona_id),
    ])
    personas.value = nextList
    selectedId.value = result.persona_id
    selectedSnapshotId.value = result.snapshot_id
    draft.value = createPersonaDraft(imported)
    rememberSavedDraft(draft.value)
    inspectorOpen.value = true
    mode.value = 'source'
    serverDiagnostics.value = result.diagnostics
    saveResult.value = {
      persona_id: result.persona_id,
      snapshot_id: result.snapshot_id,
      version: result.version,
      activated: result.activated,
      diagnostics: result.diagnostics,
    }
    statusMsg.value = `已导入 ${imported.name} v${result.version}`
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    importing.value = false
  }
}

async function exportSoulMarkdown() {
  if (!selectedSnapshotId.value) return
  const snapshotId = Number(selectedSnapshotId.value)
  if (!Number.isFinite(snapshotId) || snapshotId <= 0) {
    errorMsg.value = '当前人格没有可导出的有效快照'
    return
  }
  exporting.value = true
  try {
    const defaultPath = `${draft.value?.personaId ?? 'persona'}-${draft.value?.version ?? '1.0.0'}.soul.md`
    const target = await saveFileDialog({
      defaultPath,
      filters: [{ name: 'Soul Markdown', extensions: ['md'] }],
    })
    if (!target) return

    const result = await exportPersonaSnapshot(snapshotId, target)
    statusMsg.value = `已导出 ${result.filename}`
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    exporting.value = false
  }
}

async function deleteCurrentPersona() {
  if (!draft.value || !selectedId.value || !canDeleteSelected.value) return
  const personaId = selectedId.value
  const label = personaName.value
  try {
    await ElMessageBox.confirm(
      `删除「${label}」？关联快照会一并清除，此操作不可撤销。`,
      '确认删除人格',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return
  }

  deleting.value = true
  try {
    await deletePersona(personaId)
    const [nextList, active] = await Promise.all([listPersonas(), getActivePersona()])
    personas.value = nextList
    selectedId.value = active.id
    selectedSnapshotId.value = active.snapshot_id
    draft.value = createPersonaDraft(active)
    rememberSavedDraft(draft.value)
    inspectorOpen.value = false
    mode.value = 'simple'
    resetTransientState()
    statusMsg.value = `已删除 ${label}`
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    deleting.value = false
  }
}

// 复用既有 activate 原语：把某快照设为 active，再走与 delete 同款的刷新序列（不新建版本）。
// successMsg 在 reload 后用最新 persona 构造（恢复要带版本号）。
async function activateSnapshotAndRefresh(
  snapshotId: number,
  successMsg: (persona: PersonaSummary) => string,
) {
  if (!selectedId.value || restoring.value) return
  const personaId = selectedId.value
  restoring.value = true
  try {
    await activatePersonaSnapshot(snapshotId)
    const [nextList, persona] = await Promise.all([listPersonas(), loadPersona(personaId)])
    personas.value = nextList
    selectedId.value = persona.id
    selectedSnapshotId.value = persona.snapshot_id
    draft.value = createPersonaDraft(persona)
    rememberSavedDraft(draft.value)
    resetTransientState()
    statusMsg.value = successMsg(persona)
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    restoring.value = false
  }
}

// 历史 tab「恢复」：把旧快照设为 active。draft 会切回旧快照内容；有未保存修改先确认避免静默丢弃。
async function restoreSnapshot(snapshotId: number) {
  if (!selectedId.value || restoring.value) return
  if (isDirty.value) {
    try {
      await ElMessageBox.confirm(
        '当前有未保存的修改，恢复旧快照会丢弃它们。继续？',
        '确认恢复快照',
        {
          confirmButtonText: '恢复',
          cancelButtonText: '取消',
          type: 'warning',
        },
      )
    } catch {
      return
    }
  }
  await activateSnapshotAndRefresh(snapshotId, (persona) => `已恢复到 v${persona.version}`)
}

// footer「激活」：把未改动、当前未激活的人格设为当前（复用现有快照，不 bump 版本）。
async function activateSelectedPersona() {
  if (isDirty.value || isSelectedActive.value || restoring.value) return
  const snapshotId = Number(selectedSnapshotId.value)
  if (!Number.isFinite(snapshotId) || snapshotId <= 0) return
  await activateSnapshotAndRefresh(snapshotId, () => '已激活')
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
        <div
          v-if="draft"
          class="persona-workshop__context-actions"
          aria-label="当前人格操作"
        >
          <ElButton
            size="small"
            :icon="CopyDocument"
            :disabled="saving || deleting"
            @click="duplicateCurrentPersona"
          >
            复制为新人格
          </ElButton>
          <ElButton
            size="small"
            :icon="Download"
            :disabled="!canExportSnapshot || exporting || saving || deleting"
            :loading="exporting"
            @click="exportSoulMarkdown"
          >
            导出 .soul.md
          </ElButton>
          <ElButton
            size="small"
            type="danger"
            plain
            :icon="Delete"
            :disabled="!canDeleteSelected"
            :loading="deleting"
            @click="deleteCurrentPersona"
          >
            删除人格
          </ElButton>
        </div>
        <ElButton
          size="small"
          :icon="Upload"
          :loading="importing"
          @click="importSoulMarkdown"
        >
          导入 .soul.md
        </ElButton>
        <ElButton size="small" type="primary" @click="createNewPersona">新建人格</ElButton>
        <ElButton size="small" @click="loadInitial">刷新</ElButton>
      </div>
    </header>

    <p v-if="errorMsg" class="persona-workshop__error">操作失败：{{ errorMsg }}</p>
    <p v-else-if="statusMsg" class="persona-workshop__status">{{ statusMsg }}</p>

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
          @edit="editPersona"
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
        :active-snapshot-id="selectedSnapshotId"
        :dirty="isDirty"
        :active-persona="isSelectedActive"
        @close="inspectorOpen = false"
        @validate="validateCurrentDraft"
        @save="persistCurrentDraft(false)"
        @save-and-activate="persistCurrentDraft(true)"
        @update:mode="mode = $event"
        @update:draft="updateDraft"
        @restore="restoreSnapshot"
        @activate="activateSelectedPersona"
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
  min-width: 0;
}

.persona-workshop__context-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--aipet-space-2);
  min-width: 0;
  padding-right: var(--aipet-space-2);
  border-right: 1px solid var(--aipet-color-border-faint);
}

.persona-workshop__actions :deep(.el-button) {
  margin-left: 0;
}

.persona-workshop__error {
  margin: 0;
  color: var(--aipet-color-danger);
}

.persona-workshop__status {
  margin: 0;
  color: var(--aipet-color-success);
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
