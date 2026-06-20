<script setup lang="ts">
// TrialChatPanel（A2-D）—— 人格工坊「试聊」tab 的临时对话面。
//
// 职责单一：试聊会话表面。thread 状态在 usePersonaTrialStore（跨 tab 卸载存活，按 personaId 重置），
// 本组件只做：挂载时 ensureSession、本地 composer、blocking 时禁用、把发送/取消转交 store。
// 不落库、不写记忆——后端 persona_trial_send 保证零持久副作用。

import { computed, onMounted, ref, watch } from 'vue'
import { ElButton, ElInput } from 'element-plus'

import { validatePersonaDraft } from '@/features/persona-workshop/draft'
import { MAX_TRIAL_ROUNDS, usePersonaTrialStore } from '@/stores/personaTrial'
import type { PersonaSourceDraft } from '@/features/persona-workshop/types'

const props = defineProps<{ draft: PersonaSourceDraft }>()

const store = usePersonaTrialStore()
const input = ref('')

const blockingDiagnostics = computed(() =>
  validatePersonaDraft(props.draft).filter((d) => d.severity === 'error'),
)
const blocked = computed(() => blockingDiagnostics.value.length > 0)
const canSend = computed(
  () => !blocked.value && !store.streaming && !store.atLimit && input.value.trim().length > 0,
)

onMounted(() => store.ensureSession(props.draft.personaId))
watch(
  () => props.draft.personaId,
  (id) => store.ensureSession(id),
)

function onSend() {
  if (!canSend.value) return
  const text = input.value
  input.value = ''
  void store.send(props.draft, text)
}
</script>

<template>
  <section class="trial" aria-label="试聊沙盒">
    <p class="trial__banner">
      试聊不会保存，仅用于在保存前感受人格。切换人格或关闭工坊即清空。
    </p>

    <div class="trial__thread">
      <p v-if="store.messages.length === 0" class="trial__empty">
        发一句话，先感受一下 TA 的语气。
      </p>
      <div
        v-for="m in store.messages"
        :key="m.id"
        :class="['trial__bubble', `trial__bubble--${m.role}`]"
      >
        <span v-if="m.content" class="trial__bubble-text">{{ m.content }}</span>
        <span v-else class="trial__typing" aria-label="生成中">…</span>
      </div>
    </div>

    <p v-if="blocked" class="trial__hint trial__hint--warn">
      补全必填项后才能试聊：{{ blockingDiagnostics.map((d) => d.message).join('；') }}
    </p>
    <p v-else-if="store.atLimit" class="trial__hint">
      试聊到此为止（最多 {{ MAX_TRIAL_ROUNDS }} 轮），保存后可正式聊。
    </p>
    <p v-if="store.errorMsg" class="trial__hint trial__hint--error">{{ store.errorMsg }}</p>

    <div class="trial__composer">
      <ElInput
        v-model="input"
        type="textarea"
        :rows="2"
        :disabled="blocked || store.atLimit"
        placeholder="说点什么试试…"
        @keydown.enter.exact.prevent="onSend"
      />
      <div class="trial__actions">
        <ElButton
          size="small"
          :disabled="store.messages.length === 0 || store.streaming"
          @click="store.reset(props.draft.personaId)"
        >
          清空
        </ElButton>
        <ElButton v-if="store.streaming" size="small" @click="store.cancel()">停止</ElButton>
        <ElButton v-else type="primary" size="small" :disabled="!canSend" @click="onSend">
          发送
        </ElButton>
      </div>
    </div>
  </section>
</template>

<style scoped>
.trial {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  min-height: 0;
}

.trial__banner {
  margin: 0;
  padding: var(--aipet-space-2) var(--aipet-space-3);
  border-radius: var(--aipet-radius-base);
  background: color-mix(in srgb, var(--aipet-color-warning) 10%, var(--aipet-color-surface));
  color: var(--aipet-color-text-2);
  font-size: var(--aipet-font-size-xs);
}

.trial__thread {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  gap: var(--aipet-space-2);
  min-height: 160px;
  max-height: 320px;
  padding: var(--aipet-space-2);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-base);
  overflow-y: auto;
}

.trial__empty {
  margin: auto;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}

.trial__bubble {
  max-width: 84%;
  padding: var(--aipet-space-2) var(--aipet-space-3);
  border-radius: var(--aipet-radius-base);
  font-size: var(--aipet-font-size-sm);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

.trial__bubble--user {
  align-self: flex-end;
  background: var(--aipet-color-bubble-user, color-mix(in srgb, var(--aipet-color-accent) 16%, var(--aipet-color-surface)));
  color: var(--aipet-color-text-1);
}

.trial__bubble--assistant {
  align-self: flex-start;
  background: var(--aipet-color-bubble-assistant, var(--aipet-color-surface-raised));
  color: var(--aipet-color-text-1);
}

.trial__typing {
  color: var(--aipet-color-text-3);
}

.trial__hint {
  margin: 0;
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.trial__hint--warn {
  color: var(--aipet-color-warning);
}

.trial__hint--error {
  color: var(--aipet-color-danger);
}

.trial__composer {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.trial__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--aipet-space-2);
}
</style>
