<script setup lang="ts">
// PersonaSnapshotHistory（#51 / A2-C3）—— 人格工坊「历史」tab 的快照列表面。
//
// 职责单一：只读列出选中 persona 的全部快照（倒序），把「恢复」意图 emit 给上层。
// 实际恢复（activate + 刷新卡片/draft + 错误提示）由 PersonaWorkshopPanel 统一处理，
// 与 save/delete 同一套状态机；本组件不直接改库，只在 personaId/activeSnapshotId 变化时重拉。

import { onMounted, ref, watch } from 'vue'
import { ElButton, ElTag } from 'element-plus'

import { listPersonaSnapshots } from '@/services/persona'
import type { PersonaSnapshotSummary } from '@/types/persona'

const props = defineProps<{
  personaId: string
  /** 当前激活快照 id；变化（恢复/保存后）即作为重拉信号。 */
  activeSnapshotId: string | null
}>()

const emit = defineEmits<{
  restore: [snapshotId: number]
}>()

const snapshots = ref<PersonaSnapshotSummary[]>([])
const loading = ref(false)
const errorMsg = ref<string | null>(null)

async function loadHistory() {
  loading.value = true
  errorMsg.value = null
  try {
    snapshots.value = await listPersonaSnapshots(props.personaId)
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
    snapshots.value = []
  } finally {
    loading.value = false
  }
}

function formatTimestamp(raw: string): string {
  if (!raw) return ''
  const date = new Date(raw)
  if (Number.isNaN(date.getTime())) return raw
  return date.toLocaleString()
}

onMounted(() => void loadHistory())
watch([() => props.personaId, () => props.activeSnapshotId], () => void loadHistory())
</script>

<template>
  <section class="snapshot-history" aria-label="快照历史">
    <p class="snapshot-history__hint">
      每次「保存快照」都会留档。「恢复」可回到任一版本，且不影响已有会话的绑定。
    </p>

    <p v-if="loading" class="snapshot-history__state">加载历史…</p>
    <p
      v-else-if="errorMsg"
      class="snapshot-history__state snapshot-history__state--error"
    >
      {{ errorMsg }}
    </p>
    <p v-else-if="snapshots.length === 0" class="snapshot-history__state">
      还没有已保存的快照。先编辑后点「保存快照」。
    </p>

    <ul v-else class="snapshot-history__list">
      <li
        v-for="snap in snapshots"
        :key="snap.id"
        :class="[
          'snapshot-history__item',
          { 'snapshot-history__item--active': snap.is_active },
        ]"
      >
        <div class="snapshot-history__meta">
          <div class="snapshot-history__version-row">
            <strong class="snapshot-history__version">v{{ snap.version }}</strong>
            <ElTag v-if="snap.is_active" size="small" type="success">当前</ElTag>
          </div>
          <span class="snapshot-history__time">{{ formatTimestamp(snap.created_at) }}</span>
        </div>
        <ElButton
          size="small"
          :disabled="snap.is_active"
          @click="emit('restore', snap.id)"
        >
          {{ snap.is_active ? '使用中' : '恢复' }}
        </ElButton>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.snapshot-history {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  min-height: 0;
}

.snapshot-history__hint {
  margin: 0;
  padding: var(--aipet-space-2) var(--aipet-space-3);
  border-radius: var(--aipet-radius-base);
  background: color-mix(in srgb, var(--aipet-color-info, var(--aipet-color-accent)) 8%, var(--aipet-color-surface));
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-xs);
}

.snapshot-history__state {
  display: grid;
  min-height: 120px;
  margin: 0;
  place-items: center;
  border: 1px dashed var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-base);
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}

.snapshot-history__state--error {
  border-style: solid;
  border-color: color-mix(in srgb, var(--aipet-color-danger) 30%, var(--aipet-color-border-faint));
  color: var(--aipet-color-danger);
}

.snapshot-history__list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.snapshot-history__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-3);
  padding: var(--aipet-space-3);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-base);
  background: color-mix(in srgb, var(--aipet-color-surface-raised) 60%, transparent);
}

.snapshot-history__item--active {
  border-color: color-mix(in srgb, var(--aipet-color-success) 36%, var(--aipet-color-border-faint));
  background: color-mix(in srgb, var(--aipet-color-success) 7%, var(--aipet-color-surface));
}

.snapshot-history__meta {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
  min-width: 0;
}

.snapshot-history__version-row {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.snapshot-history__version {
  font-size: var(--aipet-font-size-base);
  color: var(--aipet-color-text-1);
}

.snapshot-history__time {
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}
</style>
