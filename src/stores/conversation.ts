// ConversationStore (#33 phase A)：chat 业务状态 + IPC 全栈。
//
// 设计要点（plan piped-kindling-backus.md 决策 #1/#2/#3）：
// - 用 Pinia setup-style store（与 nickname.ts / avatars.ts 同风格）
// - 决策 #2：switch 不调 setActiveConversation IPC。后端 KV 唯一消费 = ensure fallback；
//   前端永远显式传 conversationId 给 sendChat 即可，零 Rust diff，多实例不互相覆盖
// - 决策 #3：PENDING_CONV_KEY 保留字符串单例（chat.hub MVP singleton:true，无并发实例）
// - V3 状态模型（保留不变）：convStates: Map<convId, ConvState>，per-conv 流式独立
// - 流式中切会话不 abort 不锁，仅 sidebar lockedIds 显 spinner + 禁 archive/delete
// - 删除确认对话框（ElMessageBox.confirm）留在 ChatBody（store action 接 force flag）
//
// 抽离自 src/views/chat/ChatApp.vue 102-282 + 371-822 行（共约 510 行业务逻辑）。

import { computed, reactive, ref } from 'vue'
import { defineStore } from 'pinia'

import { useToast } from '@/composables/useToast'
import {
  Channel,
  archiveConversation,
  cancelChat,
  createConversation,
  deleteConversation,
  getChatDraft,
  listConversations,
  loadChatHistory,
  renameConversation,
  sendChat,
  setChatDraft,
} from '@/services/chat'
import type { ConversationSummary, Message, StreamEvent } from '@/types/chat'

// === 内部类型（store 私有，不导出业务消费）===

interface InflightBuffer {
  tokens: string[]
  done: { finishReason: string } | null
  error: { errorKind: string; message: string } | null
}

interface StreamSlot {
  /** null = prepare 期；非 null = stream 期（assistant placeholder 已 push 到 messages）。 */
  assistantId: string | null
  /** 早期事件缓冲（assistantId === null 期间到达的 delta/done/error）。
   *  assistantId 设值后此字段被 drain + 设 null。 */
  inflight: InflightBuffer | null
  /** 用户已点取消，等后端 done(cancelled) / error 抵达期间的 UI 中间态。 */
  cancelling: boolean
}

interface ConvState {
  messages: Message[]
  draft: string
  /** null = 该对话当前没有 in-flight 流；非 null = prepare 或 stream 中。 */
  stream: StreamSlot | null
}

// === 模块顶层常量 ===

const HISTORY_LIMIT = 50
const SIDEBAR_LIMIT = 50
const DRAFT_DEBOUNCE_MS = 200
/** 首启 0 conv 路径下，sourceConvId === null 时早期 channel 事件的占位 key。
 *  sendChat resolve 后 stream slot 整体迁移到 realConvId state；slot 引用保持，buffer 全保留。 */
const PENDING_CONV_KEY = '__pending_first_send__'

// === store 定义 ===

export const useConversationStore = defineStore('conversation', () => {
  const toast = useToast()

  // --- state ---
  const conversations = ref<ConversationSummary[]>([])
  const activeId = ref<string | null>(null)
  const convStates = reactive(new Map<string, ConvState>())
  const pendingDraft = ref('')
  /** #13：首启 0 conversations 路径的 in-flight 防御。sourceConvId === null 时
   *  下面 stream slot 不预占（无 conv id 可挂），用本独立 ref 拦住"await sendChat
   *  期间用户又输入新内容并按 Enter"导致并发 chat_send 建出两个 conversation 的边角。 */
  const firstSendInFlight = ref(false)
  const loaded = ref(false)

  // --- store-private（非 reactive，跨 action 共享）---
  const draftTimers = new Map<string, ReturnType<typeof setTimeout>>()
  /** switchConversation race 守护。用户快速点 A→B 时，A 和 B 的 IPC Promise.all 顺序不可控；
   *  给每次调用发号，过期请求直接吞掉。 */
  let switchSeq = 0

  // --- helpers ---
  function ensureConvState(id: string): ConvState {
    let s = convStates.get(id)
    if (!s) {
      s = { messages: [], draft: '', stream: null }
      convStates.set(id, s)
    }
    return s
  }

  function msgOf(e: unknown): string {
    return e instanceof Error ? e.message : String(e)
  }

  /** 把后端错误 raw 字符串里的标准 OpenAI envelope `{"error":{"message":"..."}}` 抽出来。 */
  function extractFriendlyMessage(raw: string): string {
    const idx = raw.indexOf('{')
    if (idx !== -1) {
      try {
        const obj = JSON.parse(raw.slice(idx))
        const msg = obj?.error?.message
        if (typeof msg === 'string' && msg.length > 0) return msg
      } catch {
        /* fall through to truncate */
      }
    }
    return raw.length > 200 ? raw.slice(0, 200) + '…' : raw
  }

  function errorHint(kind: string, raw: string): string {
    console.error('[ConversationStore] stream error:', kind, raw)
    const friendly = extractFriendlyMessage(raw)
    switch (kind) {
      case 'AuthFailed':
        return 'API Key 错误或已失效，请到设置面板更新'
      case 'RateLimit':
        return '请求过于频繁，稍后再试'
      case 'BadRequest':
        return `请求被拒绝：${friendly}`
      case 'ParseError':
        return '响应解析失败（可能 base_url / model 配置不兼容；详见控制台）'
      default:
        return `出错了：${friendly}`
    }
  }

  // --- getters ---
  const streamingConvIds = computed<Set<string>>(() => {
    const set = new Set<string>()
    convStates.forEach((s, id) => {
      if (s.stream !== null) set.add(id)
    })
    return set
  })

  const currentMessages = computed<Message[]>(() => {
    if (!activeId.value) return []
    return convStates.get(activeId.value)?.messages ?? []
  })

  const currentStreamingMessageId = computed<string | null>(() => {
    if (!activeId.value) return null
    return convStates.get(activeId.value)?.stream?.assistantId ?? null
  })

  const isCurrentStreaming = computed(() => {
    if (!activeId.value) return false
    return convStates.get(activeId.value)?.stream != null
  })

  /** 取消按钮可见：当前 view 是流式对话 + 已进入 stream phase（assistantId 已设）。 */
  const canCancelHere = computed(() => {
    if (!activeId.value) return false
    const s = convStates.get(activeId.value)?.stream
    return s != null && s.assistantId !== null
  })

  const isCancellingHere = computed(() => {
    if (!activeId.value) return false
    return convStates.get(activeId.value)?.stream?.cancelling ?? false
  })

  function isStreaming(id: string): boolean {
    return streamingConvIds.value.has(id)
  }

  // --- draft 持久化（debounced）---

  function scheduleDraftPersist(convId: string, draft: string) {
    const existing = draftTimers.get(convId)
    if (existing) clearTimeout(existing)
    const timer = setTimeout(() => {
      draftTimers.delete(convId)
      void setChatDraft(convId, draft).catch((e) => {
        console.warn('[ConversationStore] setChatDraft failed:', e)
      })
    }, DRAFT_DEBOUNCE_MS)
    draftTimers.set(convId, timer)
  }

  function flushDraftIfPending(convId: string) {
    const t = draftTimers.get(convId)
    if (!t) return
    clearTimeout(t)
    draftTimers.delete(convId)
    const state = convStates.get(convId)
    if (state) {
      void setChatDraft(convId, state.draft).catch((e) => {
        console.warn('[ConversationStore] flushDraft failed:', e)
      })
    }
  }

  function flushAllDrafts() {
    draftTimers.forEach((_, id) => flushDraftIfPending(id))
  }

  function getDraft(convId: string | null): string {
    if (!convId) return pendingDraft.value
    return convStates.get(convId)?.draft ?? ''
  }

  function setDraft(convId: string | null, v: string) {
    if (!convId) {
      pendingDraft.value = v
      return
    }
    const state = ensureConvState(convId)
    state.draft = v
    scheduleDraftPersist(convId, v)
  }

  // --- conv CRUD ---

  async function refresh() {
    try {
      conversations.value = await listConversations(SIDEBAR_LIMIT)
    } catch (e) {
      console.warn('[ConversationStore] listConversations failed:', e)
    }
  }

  async function switchTo(id: string) {
    if (id === activeId.value) return
    const mySeq = ++switchSeq

    // flush 当前 view 的 draft 再切
    if (activeId.value) {
      flushDraftIfPending(activeId.value)
    }

    try {
      // 之前没 cache 过的对话：从 DB 拉历史 + 草稿。
      // 已 cache（流式中切走又切回）：直接用 in-memory state，避免 reload 把流式中的 assistant
      // content 清回 DB 的 placeholder 空状态。
      const existing = convStates.get(id)
      if (!existing) {
        const [records, draftFromDb] = await Promise.all([
          loadChatHistory(id, HISTORY_LIMIT),
          getChatDraft(id),
        ])
        if (mySeq !== switchSeq) return // 我已过期，让最新的 switch win
        const state = ensureConvState(id)
        state.messages = records
        state.draft = draftFromDb ?? ''
      }
      if (mySeq !== switchSeq) return
      activeId.value = id
      // 决策 #2：不调 setActiveConversation IPC（后端 KV 仅 ensure fallback 用，前端始终显式传 id）
    } catch (e) {
      if (mySeq === switchSeq) toast.error(`切换会话失败：${msgOf(e)}`)
    }
  }

  async function create() {
    try {
      const newId = await createConversation()
      if (activeId.value) flushDraftIfPending(activeId.value)
      ensureConvState(newId)
      activeId.value = newId
      await refresh()
    } catch (e) {
      toast.error(`新建会话失败：${msgOf(e)}`)
    }
  }

  async function fallbackAfterActiveGone() {
    if (conversations.value.length > 0) {
      await switchTo(conversations.value[0].id)
    } else {
      activeId.value = null
    }
  }

  async function rename(payload: { id: string; title: string }) {
    // 流式中也允许 rename：title 写到 conversations 表，与 messages / prepare tx 互不读；
    // archive / delete 仍保留 lock，因为它们会让 prepare 内的 INSERT 撞 FK。
    try {
      await renameConversation(payload.id, payload.title)
      await refresh()
    } catch (e) {
      toast.error(`重命名失败：${msgOf(e)}`)
    }
  }

  async function archive(id: string) {
    if (streamingConvIds.value.has(id)) {
      toast.warn('该对话流式中，请先取消再归档')
      return
    }
    const wasActive = id === activeId.value
    try {
      await archiveConversation(id)
      convStates.delete(id)
      await refresh()
      toast.success('已归档')
      if (wasActive) await fallbackAfterActiveGone()
    } catch (e) {
      toast.error(`归档失败：${msgOf(e)}`)
    }
  }

  /** 删除（ElMessageBox 确认对话框由 ChatBody 处理后调本 action）。 */
  async function remove(id: string) {
    if (streamingConvIds.value.has(id)) {
      toast.warn('该对话流式中，请先取消再删除')
      return
    }
    const wasActive = id === activeId.value
    try {
      await deleteConversation(id)
      convStates.delete(id)
      await refresh()
      if (wasActive) await fallbackAfterActiveGone()
    } catch (e) {
      toast.error(`删除失败：${msgOf(e)}`)
    }
  }

  // --- stream 事件处理（按 convId 路由）---

  function appendToMessage(convId: string, targetMsgId: string, token: string) {
    const state = convStates.get(convId)
    if (!state) return
    const idx = state.messages.findIndex((m) => m.id === targetMsgId)
    if (idx === -1) return
    state.messages[idx] = {
      ...state.messages[idx],
      content: state.messages[idx].content + token,
    }
  }

  function finalizeStream(convId: string, messageId: string, finishReason: string) {
    const state = convStates.get(convId)
    if (!state) return
    state.stream = null
    if (finishReason === 'offline_rule') {
      const idx = state.messages.findIndex((m) => m.id === messageId)
      if (idx !== -1) {
        state.messages[idx] = { ...state.messages[idx], mode: 'offline_rule' }
      }
    } else if (finishReason === 'cancelled') {
      // 后端语义：partial 为空 → DELETE DB 行；非空 → UPDATE mode='cancelled'。
      // 前端跟上：trim 后空 splice 删气泡，非空 mode='cancelled' 显「（已取消）」标签。
      const idx = state.messages.findIndex((m) => m.id === messageId)
      if (idx !== -1) {
        const target = state.messages[idx]
        if (target.content.trim() === '') {
          state.messages.splice(idx, 1)
        } else {
          state.messages[idx] = { ...target, mode: 'cancelled' }
        }
      }
    }
    void refresh()
  }

  function handleStreamError(
    convId: string,
    messageId: string,
    errorKind: string,
    errorMsg: string,
  ) {
    const state = convStates.get(convId)
    if (state) {
      const idx = state.messages.findIndex((m) => m.id === messageId)
      if (idx !== -1) {
        state.messages.splice(idx, 1)
      }
      state.stream = null
    }
    toast.error(errorHint(errorKind, errorMsg), { duration: 5000 })
    void refresh()
  }

  // --- send / cancel ---

  async function send(draftRaw: string) {
    // 当前 view 流式中 → 二次防御
    if (isCurrentStreaming.value) return
    if (draftRaw.length === 0) return

    const sourceConvId = activeId.value

    if (sourceConvId === null && firstSendInFlight.value) return

    let stateAtSend: ConvState
    if (sourceConvId) {
      stateAtSend = ensureConvState(sourceConvId)
      if (stateAtSend.stream !== null) return // 双击防御
      stateAtSend.stream = {
        assistantId: null,
        inflight: { tokens: [], done: null, error: null },
        cancelling: false,
      }
    } else {
      firstSendInFlight.value = true
      stateAtSend = ensureConvState(PENDING_CONV_KEY)
      stateAtSend.stream = {
        assistantId: null,
        inflight: { tokens: [], done: null, error: null },
        cancelling: false,
      }
    }

    // 清输入草稿 + 立刻 flush 空字符串避免 race
    setDraft(sourceConvId, '')
    if (sourceConvId) flushDraftIfPending(sourceConvId)

    const nowIso = new Date().toISOString()
    const userTempId = `pending-user-${Date.now()}`
    if (sourceConvId !== null) {
      stateAtSend.messages.push({
        id: userTempId,
        conversation_id: sourceConvId,
        role: 'user',
        content: draftRaw,
        mode: 'online',
        created_at: nowIso,
      })
    }

    // === Channel 闭包路由 ===
    let routedConvId: string = sourceConvId ?? PENDING_CONV_KEY
    let routedAssistantId: string | null = null

    const channel = new Channel<StreamEvent>()
    channel.onmessage = (msg) => {
      const state = convStates.get(routedConvId)
      if (!state || !state.stream) return

      switch (msg.type) {
        case 'delta':
          if (routedAssistantId !== null) {
            appendToMessage(routedConvId, routedAssistantId, msg.token)
          } else if (state.stream.inflight) {
            state.stream.inflight.tokens.push(msg.token)
          }
          break
        case 'done':
          if (routedAssistantId !== null) {
            finalizeStream(routedConvId, routedAssistantId, msg.finishReason)
          } else if (state.stream.inflight) {
            state.stream.inflight.done = { finishReason: msg.finishReason }
          }
          break
        case 'error':
          if (routedAssistantId !== null) {
            handleStreamError(routedConvId, routedAssistantId, msg.errorKind, msg.message)
          } else if (state.stream.inflight) {
            state.stream.inflight.error = { errorKind: msg.errorKind, message: msg.message }
          }
          break
      }
    }

    try {
      const result = await sendChat(draftRaw, sourceConvId ?? undefined, channel)
      const realConvId = result.conversationId
      const isNewConv = realConvId !== sourceConvId

      // 首启路径：把 PENDING_CONV_KEY 上的 stream slot 整体搬到 realConvId state
      if (sourceConvId === null) {
        const state = ensureConvState(realConvId)
        const pendingState = convStates.get(PENDING_CONV_KEY)
        if (pendingState?.stream) {
          state.stream = pendingState.stream
        } else {
          state.stream = {
            assistantId: null,
            inflight: { tokens: [], done: null, error: null },
            cancelling: false,
          }
        }
      }

      routedConvId = realConvId
      const realState = convStates.get(realConvId)!

      // 回填 user message 真实 ID
      const userIdx = realState.messages.findIndex((m) => m.id === userTempId)
      if (userIdx !== -1) {
        realState.messages[userIdx] = {
          ...realState.messages[userIdx],
          id: result.userMessageId,
          conversation_id: realConvId,
        }
      } else if (sourceConvId === null) {
        // 首启路径之前没 push user 乐观 placeholder，现在补
        realState.messages.push({
          id: result.userMessageId,
          conversation_id: realConvId,
          role: 'user',
          content: draftRaw,
          mode: 'online',
          created_at: nowIso,
        })
      }

      // push assistant placeholder + drain 早期 token/done/error
      const earlyTokens = realState.stream?.inflight?.tokens ?? []
      realState.messages.push({
        id: result.messageId,
        conversation_id: realConvId,
        role: 'assistant',
        content: earlyTokens.join(''),
        mode: 'online',
        created_at: new Date().toISOString(),
      })

      if (realState.stream) {
        realState.stream.assistantId = result.messageId
        const earlyDone = realState.stream.inflight?.done
        const earlyError = realState.stream.inflight?.error
        realState.stream.inflight = null
        routedAssistantId = result.messageId

        if (earlyError) {
          handleStreamError(realConvId, result.messageId, earlyError.errorKind, earlyError.message)
        } else if (earlyDone) {
          finalizeStream(realConvId, result.messageId, earlyDone.finishReason)
        }
      }

      // 首启路径同步 activeId + 迁移 pendingDraft
      if (sourceConvId === null) {
        if (pendingDraft.value !== '') {
          realState.draft = pendingDraft.value
          void setChatDraft(realConvId, pendingDraft.value).catch((e) => {
            console.warn('[ConversationStore] migrate pendingDraft to realConvId failed:', e)
          })
        }
        convStates.delete(PENDING_CONV_KEY)
        activeId.value = realConvId
        pendingDraft.value = ''
      }

      if (isNewConv) {
        await refresh()
      }
    } catch (e) {
      // sendChat 抛 = 后端 prepare 阶段失败
      const userIdx = stateAtSend.messages.findIndex((m) => m.id === userTempId)
      if (userIdx !== -1) stateAtSend.messages.splice(userIdx, 1)
      stateAtSend.stream = null
      if (sourceConvId === null) {
        convStates.delete(PENDING_CONV_KEY)
      }
      // 复原输入草稿（让用户能改了重发）
      setDraft(sourceConvId, draftRaw)
      toast.error(`发送失败：${msgOf(e)}`, { duration: 5000 })
      // #9：ensure_active_conversation 可能在 tx 之前已创建一个空 conv 行；
      // build_messages / build_provider 失败留下空 conv，sidebar 不会自动看到。这里补一次 refresh。
      void refresh()
    } finally {
      if (sourceConvId === null) {
        firstSendInFlight.value = false
      }
    }
  }

  async function cancel() {
    if (!activeId.value) return
    const convId = activeId.value
    const slot = convStates.get(convId)?.stream
    if (!slot?.assistantId) return
    if (slot.cancelling) return // 双击防御
    slot.cancelling = true
    try {
      await cancelChat(slot.assistantId)
    } catch (e) {
      console.warn('[ConversationStore] cancelChat failed:', e)
      // IPC 失败后，复原 cancelling 前先确认 slot 仍是当前 stream（防 orphan 写）
      const current = convStates.get(convId)?.stream
      if (current === slot) {
        slot.cancelling = false
      }
    }
    // 成功路径不复位 cancelling：等后端 done(cancelled) / error 抵达，
    // finalizeStream / handleStreamError 把 stream slot 整体置 null，cancelling 隐式消失。
  }

  // --- 初始化 ---

  /** ChatBody onMounted 调用；幂等。第一次 mount = refresh + switch first；后续 mount = noop。 */
  async function loadInitial() {
    if (loaded.value) return
    loaded.value = true
    await refresh()
    if (conversations.value.length > 0 && activeId.value === null) {
      await switchTo(conversations.value[0].id)
    }
  }

  return {
    // state
    conversations,
    activeId,
    convStates,
    pendingDraft,
    firstSendInFlight,
    loaded,
    // getters
    streamingConvIds,
    currentMessages,
    currentStreamingMessageId,
    isCurrentStreaming,
    canCancelHere,
    isCancellingHere,
    // actions
    isStreaming,
    refresh,
    switchTo,
    create,
    rename,
    archive,
    remove,
    send,
    cancel,
    setDraft,
    getDraft,
    flushDraftIfPending,
    flushAllDrafts,
    ensureConvState,
    loadInitial,
    // 测试暴露（仅 __tests__/ 使用）
    _PENDING_CONV_KEY: PENDING_CONV_KEY,
  }
})
