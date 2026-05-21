// ConversationStore 单测（#33 phase A）—— 13 case 覆盖 plan 验收路径
//
// 覆盖：refresh / create+switch / switch race / setDraft+flush / archive 流式拒 / delete 流式拒 /
//      send 非首启 / send 首启 PENDING / send 双击防 / PENDING drain → real migration /
//      cancel prepare 期 / cancel 二次防 / 流式中切不 abort / send error 草稿复原 / streamingConvIds 含 prepare

import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { nextTick } from 'vue'

import { useConversationStore } from '../conversation'
import type { ConversationSummary, Message, SendResult, StreamEvent } from '@/types/chat'

// === Mock Channel（vi.hoisted 让 class 在 vi.mock 工厂前可见）===

const { MockChannel } = vi.hoisted(() => {
  class MockChannel<T> {
    static instances: MockChannel<unknown>[] = []
    onmessage: ((msg: T) => void) | null = null

    constructor() {
      MockChannel.instances.push(this as unknown as MockChannel<unknown>)
    }

    emit(msg: T) {
      this.onmessage?.(msg)
    }

    static reset() {
      MockChannel.instances = []
    }

    static last<U = unknown>(): MockChannel<U> {
      const inst = MockChannel.instances[MockChannel.instances.length - 1]
      if (!inst) throw new Error('No Channel instance')
      return inst as unknown as MockChannel<U>
    }
  }
  return { MockChannel }
})

// === Mock @/services/chat ===

vi.mock('@/services/chat', () => ({
  Channel: MockChannel,
  listConversations: vi.fn(),
  loadChatHistory: vi.fn(),
  sendChat: vi.fn(),
  cancelChat: vi.fn(),
  createConversation: vi.fn(),
  renameConversation: vi.fn(),
  archiveConversation: vi.fn(),
  deleteConversation: vi.fn(),
  getChatDraft: vi.fn(),
  setChatDraft: vi.fn(),
}))

// === Mock @/composables/useToast ===

const toastSpies = {
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
}

vi.mock('@/composables/useToast', () => ({
  useToast: () => toastSpies,
}))

// === imports after mock ===

import {
  listConversations,
  loadChatHistory,
  sendChat,
  cancelChat,
  createConversation,
  archiveConversation,
  deleteConversation,
  getChatDraft,
  setChatDraft,
} from '@/services/chat'

const mockListConversations = listConversations as unknown as Mock
const mockLoadChatHistory = loadChatHistory as unknown as Mock
const mockSendChat = sendChat as unknown as Mock
const mockCancelChat = cancelChat as unknown as Mock
const mockCreateConversation = createConversation as unknown as Mock
const mockArchiveConversation = archiveConversation as unknown as Mock
const mockDeleteConversation = deleteConversation as unknown as Mock
const mockGetChatDraft = getChatDraft as unknown as Mock
const mockSetChatDraft = setChatDraft as unknown as Mock

// === helpers ===

function summary(id: string, title = id): ConversationSummary {
  return {
    id,
    persona_id: 'p1',
    title,
    started_at: '2026-05-21T00:00:00Z',
    last_activity_at: '2026-05-21T00:00:00Z',
  }
}

function userMsg(id: string, conv: string, content: string): Message {
  return {
    id,
    conversation_id: conv,
    role: 'user',
    content,
    mode: 'online',
    created_at: '2026-05-21T00:00:00Z',
  }
}

function deferred<T>(): { promise: Promise<T>; resolve: (v: T) => void; reject: (e: unknown) => void } {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

// === setup ===

beforeEach(() => {
  setActivePinia(createPinia())
  MockChannel.reset()
  vi.clearAllMocks()
  // 默认 IPC stub —— 单测自行覆盖
  mockListConversations.mockResolvedValue([])
  mockLoadChatHistory.mockResolvedValue([])
  mockGetChatDraft.mockResolvedValue(null)
  mockSetChatDraft.mockResolvedValue(undefined)
  mockCreateConversation.mockResolvedValue('new-conv-id')
  mockArchiveConversation.mockResolvedValue(undefined)
  mockDeleteConversation.mockResolvedValue(undefined)
  mockCancelChat.mockResolvedValue(undefined)
})

// === tests ===

describe('ConversationStore', () => {
  it('case 1: refresh 写入 conversations', async () => {
    mockListConversations.mockResolvedValueOnce([summary('c1'), summary('c2')])
    const store = useConversationStore()
    await store.refresh()
    expect(store.conversations).toHaveLength(2)
    expect(store.conversations[0].id).toBe('c1')
  })

  it('case 2: create 新建 + refresh + 切 active', async () => {
    mockCreateConversation.mockResolvedValueOnce('new-id')
    mockListConversations.mockResolvedValueOnce([summary('new-id', '新对话')])
    const store = useConversationStore()
    await store.create()
    expect(store.activeId).toBe('new-id')
    expect(store.convStates.has('new-id')).toBe(true)
    expect(store.conversations[0].id).toBe('new-id')
  })

  it('case 3: switchTo race 守护（A→B 反序，B 最新）', async () => {
    const aDeferred = deferred<Message[]>()
    const bDeferred = deferred<Message[]>()

    mockLoadChatHistory.mockImplementationOnce(() => aDeferred.promise) // A 先调
    mockLoadChatHistory.mockImplementationOnce(() => bDeferred.promise) // B 后调

    const store = useConversationStore()
    const p1 = store.switchTo('A')
    const p2 = store.switchTo('B')
    // B 先 resolve
    bDeferred.resolve([userMsg('m_b', 'B', 'hi B')])
    await p2
    expect(store.activeId).toBe('B')
    // A 后 resolve，但 mySeq 已过期，不写
    aDeferred.resolve([userMsg('m_a', 'A', 'hi A')])
    await p1
    expect(store.activeId).toBe('B') // 仍是 B
  })

  it('case 4: setDraft + flush 走 debounce', async () => {
    vi.useFakeTimers()
    const store = useConversationStore()
    store.activeId = 'c1'
    store.ensureConvState('c1')

    store.setDraft('c1', 'hello')
    expect(mockSetChatDraft).not.toHaveBeenCalled()

    vi.advanceTimersByTime(200)
    await vi.runAllTimersAsync()
    expect(mockSetChatDraft).toHaveBeenCalledWith('c1', 'hello')

    vi.useRealTimers()
  })

  it('case 5: archive 流式中拒（toast.warn + archiveConversation 不调）', async () => {
    const store = useConversationStore()
    const state = store.ensureConvState('c1')
    state.stream = { assistantId: 'msg1', inflight: null, cancelling: false }

    await store.archive('c1')

    expect(toastSpies.warn).toHaveBeenCalledWith('该对话流式中，请先取消再归档')
    expect(mockArchiveConversation).not.toHaveBeenCalled()
  })

  it('case 6: remove 流式中拒', async () => {
    const store = useConversationStore()
    const state = store.ensureConvState('c1')
    state.stream = { assistantId: 'msg1', inflight: null, cancelling: false }

    await store.remove('c1')

    expect(toastSpies.warn).toHaveBeenCalledWith('该对话流式中，请先取消再删除')
    expect(mockDeleteConversation).not.toHaveBeenCalled()
  })

  it('case 7: send 非首启路径 + drain 早期 delta', async () => {
    const sendDeferred = deferred<SendResult>()
    mockSendChat.mockImplementationOnce(() => sendDeferred.promise)

    const store = useConversationStore()
    store.activeId = 'c1'
    store.ensureConvState('c1')

    const sendP = store.send('hello')
    await Promise.resolve() // 让 send 内部跑到 new Channel + sendChat 调用
    const ch = MockChannel.last<StreamEvent>()
    ch.emit({ type: 'delta', token: 'world' })

    sendDeferred.resolve({ messageId: 'asst1', userMessageId: 'usr1', conversationId: 'c1' })
    await sendP

    const state = store.convStates.get('c1')!
    // 应有 2 条消息：user + assistant，assistant content drain 拿到 'world'
    expect(state.messages).toHaveLength(2)
    expect(state.messages[1].role).toBe('assistant')
    expect(state.messages[1].content).toBe('world')
  })

  it('case 8: send 首启路径（sourceConvId=null）PENDING + firstSendInFlight + 迁移到 realConvId', async () => {
    const sendDeferred = deferred<SendResult>()
    mockSendChat.mockImplementationOnce(() => sendDeferred.promise)
    mockListConversations.mockResolvedValueOnce([summary('real-c1', '新对话')])

    const store = useConversationStore()
    expect(store.activeId).toBeNull()

    const sendP = store.send('hi')
    await Promise.resolve()
    expect(store.firstSendInFlight).toBe(true)
    expect(store.convStates.has(store._PENDING_CONV_KEY)).toBe(true)

    const ch = MockChannel.last<StreamEvent>()
    ch.emit({ type: 'delta', token: 'response' })

    sendDeferred.resolve({ messageId: 'asst1', userMessageId: 'usr1', conversationId: 'real-c1' })
    await sendP

    expect(store.activeId).toBe('real-c1')
    expect(store.firstSendInFlight).toBe(false)
    expect(store.convStates.has(store._PENDING_CONV_KEY)).toBe(false) // PENDING 已清

    const realState = store.convStates.get('real-c1')!
    expect(realState.messages).toHaveLength(2) // user + assistant
    expect(realState.messages[1].content).toBe('response') // 早期 delta drained
  })

  it('case 9: send 双击防御（first 在 prepare 期，second 立即 return）', async () => {
    const sendDeferred = deferred<SendResult>()
    mockSendChat.mockImplementationOnce(() => sendDeferred.promise)

    const store = useConversationStore()
    store.activeId = 'c1'
    store.ensureConvState('c1')

    const sendP1 = store.send('first')
    await Promise.resolve()
    expect(mockSendChat).toHaveBeenCalledTimes(1)

    // 第二次调，prepare 已占 stream slot → 立即 return
    await store.send('second')
    expect(mockSendChat).toHaveBeenCalledTimes(1) // 仍只一次

    sendDeferred.resolve({ messageId: 'm', userMessageId: 'u', conversationId: 'c1' })
    await sendP1
  })

  it('case 10: cancel prepare 期不调 cancelChat（slot.assistantId === null）', async () => {
    const store = useConversationStore()
    store.activeId = 'c1'
    const state = store.ensureConvState('c1')
    // prepare 期：stream 非 null 但 assistantId === null
    state.stream = {
      assistantId: null,
      inflight: { tokens: [], done: null, error: null },
      cancelling: false,
    }

    await store.cancel()
    expect(mockCancelChat).not.toHaveBeenCalled()
  })

  it('case 11: cancel 二次防（slot.cancelling=true，第二次 cancel return）', async () => {
    const cancelDeferred = deferred<void>()
    mockCancelChat.mockImplementationOnce(() => cancelDeferred.promise)

    const store = useConversationStore()
    store.activeId = 'c1'
    const state = store.ensureConvState('c1')
    state.stream = {
      assistantId: 'asst1',
      inflight: null,
      cancelling: false,
    }

    const p1 = store.cancel()
    await Promise.resolve() // 让 cancel 内部走到 slot.cancelling=true
    expect(state.stream?.cancelling).toBe(true)
    expect(mockCancelChat).toHaveBeenCalledTimes(1)

    // 第二次 cancel：slot.cancelling=true → 立即 return
    await store.cancel()
    expect(mockCancelChat).toHaveBeenCalledTimes(1) // 仍只一次

    cancelDeferred.resolve()
    await p1
  })

  it('case 12: 流式中切会话不 abort 后台流（state.stream 保留）', async () => {
    const store = useConversationStore()
    const stateA = store.ensureConvState('A')
    stateA.stream = { assistantId: 'asst-A', inflight: null, cancelling: false }
    store.activeId = 'A'

    mockLoadChatHistory.mockResolvedValueOnce([])
    mockGetChatDraft.mockResolvedValueOnce(null)
    await store.switchTo('B')

    expect(store.activeId).toBe('B')
    // A 的 stream 仍非 null（不 abort）
    expect(store.convStates.get('A')?.stream).not.toBeNull()
  })

  it('case 13: send 后端 prepare 失败 → 复原草稿 + toast', async () => {
    mockSendChat.mockRejectedValueOnce(new Error('provider missing'))

    const store = useConversationStore()
    store.activeId = 'c1'
    store.ensureConvState('c1')

    await store.send('important draft')
    await nextTick()

    const state = store.convStates.get('c1')!
    expect(state.stream).toBeNull() // 复位
    expect(state.draft).toBe('important draft') // 草稿复原
    expect(toastSpies.error).toHaveBeenCalledWith(
      expect.stringContaining('发送失败'),
      expect.any(Object),
    )
  })

  it('case 14: streamingConvIds 含 prepare 期（assistantId === null）', () => {
    const store = useConversationStore()
    const state = store.ensureConvState('c1')
    state.stream = {
      assistantId: null,
      inflight: { tokens: [], done: null, error: null },
      cancelling: false,
    }
    expect(store.streamingConvIds.has('c1')).toBe(true)
    expect(store.isStreaming('c1')).toBe(true)
  })

  it('case 15: loadInitial 幂等（第二次调用 noop）', async () => {
    mockListConversations.mockResolvedValue([summary('c1'), summary('c2')])
    mockLoadChatHistory.mockResolvedValueOnce([])

    const store = useConversationStore()
    await store.loadInitial()
    expect(mockListConversations).toHaveBeenCalledTimes(1)
    expect(store.activeId).toBe('c1') // first conv auto-switch

    // 第二次 loadInitial = noop
    await store.loadInitial()
    expect(mockListConversations).toHaveBeenCalledTimes(1) // 不再调
  })
})
