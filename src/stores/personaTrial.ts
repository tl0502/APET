// PersonaTrialStore（A2-D 试聊沙盒）—— 人格工坊保存前的临时对话状态。
//
// 为什么用 store 而非组件内 composable：试聊面板挂在 PersonaEditorTabs 的
// `v-if="mode==='trial'"` 分支下，切到别的 tab 会卸载组件。把 thread 放 store 才能在
// 「改滑杆 → 切回试聊继续」时保留历史（spec §6.3）。store 按 personaId 自动重置，
// 切人格即清空，无需父组件改一行。
//
// 零持久副作用：本 store 只持内存 thread；后端 persona_trial_send 不落库
// （不建 conversation/message/snapshot）。关闭工坊 / 切人格 / 刷新即弃。

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { Channel, cancelChat } from '@/services/chat'
import { trialSend, type TrialTurn } from '@/services/persona'
import type { PersonaSourceDraft } from '@/features/persona-workshop/types'
import type { StreamEvent } from '@/types/chat'

/** 试聊轮次软上限（一问一答记 1 round）。后端另有 MAX_TRIAL_HISTORY=24 message 兜底。 */
export const MAX_TRIAL_ROUNDS = 12

export interface TrialMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
}

let seq = 0
function nextId(prefix: string): string {
  seq += 1
  return `trial-${prefix}-${seq}`
}

export const usePersonaTrialStore = defineStore('personaTrial', () => {
  /** 当前 thread 归属的 personaId；切换即重置（spec：切人格清空）。 */
  const personaKey = ref<string | null>(null)
  const messages = ref<TrialMessage[]>([])
  const streaming = ref(false)
  /** 在飞 assistant message 的后端 id（cancel 用）。 */
  const streamingId = ref<string | null>(null)
  const errorMsg = ref<string | null>(null)

  /** 已发起的一问一答轮数（用 assistant 条数近似，含在飞占位）。 */
  const roundCount = computed(() => messages.value.filter((m) => m.role === 'assistant').length)
  const atLimit = computed(() => roundCount.value >= MAX_TRIAL_ROUNDS)

  /** 挂载试聊面板时调用：personaId 变了（含首次）就清空 thread。 */
  function ensureSession(personaId: string) {
    if (personaKey.value !== personaId) reset(personaId)
  }

  function reset(personaId: string | null = personaKey.value) {
    personaKey.value = personaId
    messages.value = []
    streaming.value = false
    streamingId.value = null
    errorMsg.value = null
  }

  async function send(draft: PersonaSourceDraft, rawInput: string) {
    const input = rawInput.trim()
    if (!input || streaming.value || atLimit.value) return
    errorMsg.value = null

    // 发送前的历史（不含本轮 user）→ 传后端；后端无状态。
    const history: TrialTurn[] = messages.value.map((m) => ({ role: m.role, content: m.content }))
    messages.value.push({ id: nextId('u'), role: 'user', content: input })
    const assistantId = nextId('a')
    messages.value.push({ id: assistantId, role: 'assistant', content: '' })
    const assistantIdx = messages.value.length - 1
    streaming.value = true

    const channel = new Channel<StreamEvent>()
    channel.onmessage = (msg) => {
      // 通过 index + id 双重定位，且全程经 messages.value 走 reactive proxy（不缓存裸对象）。
      const target = messages.value[assistantIdx]
      if (!target || target.id !== assistantId) return
      switch (msg.type) {
        case 'delta':
          target.content += msg.token
          break
        case 'replaceMessage':
          // SafetyGuard redact/block → 覆盖临时气泡内容。
          target.content = msg.newContent
          break
        case 'done':
          streaming.value = false
          streamingId.value = null
          break
        case 'error':
          errorMsg.value = msg.message
          streaming.value = false
          streamingId.value = null
          if (!target.content) messages.value.splice(assistantIdx, 1)
          break
      }
    }

    try {
      const { messageId } = await trialSend(draft, history, input, channel)
      streamingId.value = messageId
    } catch (e) {
      // prepare 阶段失败（blocking draft / 未配置 provider / 输入被拦）：run_trial_stream 未 spawn，
      // 本轮没真正发出 → 把刚压入的 user + 空 assistant 一并撤掉，thread 回到发送前 + 报错。
      errorMsg.value = e instanceof Error ? e.message : String(e)
      streaming.value = false
      streamingId.value = null
      const target = messages.value[assistantIdx]
      if (target && target.id === assistantId) {
        messages.value.splice(assistantIdx - 1, 2)
      }
    }
  }

  function cancel() {
    if (streamingId.value) void cancelChat(streamingId.value)
  }

  return {
    personaKey,
    messages,
    streaming,
    streamingId,
    errorMsg,
    roundCount,
    atLimit,
    ensureSession,
    reset,
    send,
    cancel,
  }
})
