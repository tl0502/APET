// PersonaTrialStore 单测（A2-D）—— 覆盖：切人格重置 / 流式累积 + done 收尾 /
// replaceMessage 覆盖 / 流式 error 移除空占位 / 轮次上限拦截 / prepare 失败撤回本轮 / cancel。

import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'

import { MAX_TRIAL_ROUNDS, usePersonaTrialStore } from '../personaTrial'
import type { PersonaSourceDraft } from '@/features/persona-workshop/types'
import type { StreamEvent } from '@/types/chat'

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

vi.mock('@/services/chat', () => ({
  Channel: MockChannel,
  cancelChat: vi.fn(),
}))

vi.mock('@/services/persona', () => ({
  trialSend: vi.fn(),
}))

import { cancelChat } from '@/services/chat'
import { trialSend } from '@/services/persona'

const draft = { personaId: 'momo' } as unknown as PersonaSourceDraft

function emit(msg: StreamEvent) {
  MockChannel.last<StreamEvent>().emit(msg)
}

describe('personaTrial store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    MockChannel.reset()
    vi.clearAllMocks()
    ;(trialSend as Mock).mockResolvedValue({ messageId: 'asst-1' })
  })

  it('ensureSession resets thread only when personaId changes', () => {
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    store.messages.push({ id: 'x', role: 'user', content: 'hi' })
    store.ensureSession('momo') // 同一人格 → 保留
    expect(store.messages).toHaveLength(1)
    store.ensureSession('joker') // 换人格 → 清空
    expect(store.messages).toHaveLength(0)
    expect(store.personaKey).toBe('joker')
  })

  it('send passes prior history, accumulates delta, finalizes on done', async () => {
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    await store.send(draft, '你好')

    expect((trialSend as Mock).mock.calls[0][1]).toEqual([]) // 首轮无历史
    expect(store.streaming).toBe(true)
    expect(store.streamingId).toBe('asst-1')

    emit({ type: 'delta', token: '你' })
    emit({ type: 'delta', token: '好' })
    expect(store.messages[store.messages.length - 1].content).toBe('你好')

    emit({ type: 'done', totalTokens: 2, finishReason: 'stop' })
    expect(store.streaming).toBe(false)
    expect(store.streamingId).toBeNull()

    // 第二轮应把第一轮 user+assistant 作为历史传后端
    await store.send(draft, '再来')
    const secondHistory = (trialSend as Mock).mock.calls[1][1]
    expect(secondHistory).toEqual([
      { role: 'user', content: '你好' },
      { role: 'assistant', content: '你好' },
    ])
  })

  it('replaceMessage overwrites the assistant bubble', async () => {
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    await store.send(draft, 'x')
    emit({ type: 'delta', token: 'unsafe' })
    emit({
      type: 'replaceMessage',
      messageId: 'asst-1',
      newContent: '[已替换]',
      reason: 'final_blocked',
    })
    expect(store.messages[store.messages.length - 1].content).toBe('[已替换]')
  })

  it('stream error removes empty assistant placeholder, keeps user, surfaces message', async () => {
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    await store.send(draft, 'q')
    const after = store.messages.length // user + assistant placeholder
    emit({ type: 'error', errorKind: 'Network', message: '网络炸了' })
    expect(store.errorMsg).toBe('网络炸了')
    expect(store.streaming).toBe(false)
    expect(store.messages).toHaveLength(after - 1)
    expect(store.messages[store.messages.length - 1].role).toBe('user')
  })

  it('blocks send at round limit', async () => {
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    for (let i = 0; i < MAX_TRIAL_ROUNDS; i++) {
      store.messages.push({ id: `u${i}`, role: 'user', content: 'q' })
      store.messages.push({ id: `a${i}`, role: 'assistant', content: 'a' })
    }
    expect(store.atLimit).toBe(true)
    await store.send(draft, 'one more')
    expect(trialSend as Mock).not.toHaveBeenCalled()
  })

  it('prepare-stage rejection rolls back the round and surfaces error', async () => {
    ;(trialSend as Mock).mockRejectedValue(new Error('人格定义未完成，无法试聊'))
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    await store.send(draft, 'x')
    expect(store.errorMsg).toContain('人格定义未完成')
    expect(store.streaming).toBe(false)
    expect(store.messages).toHaveLength(0) // user + 空 assistant 都撤掉
  })

  it('cancel calls cancelChat with the streaming id', async () => {
    const store = usePersonaTrialStore()
    store.ensureSession('momo')
    await store.send(draft, 'x')
    store.cancel()
    expect(cancelChat as Mock).toHaveBeenCalledWith('asst-1')
  })
})
