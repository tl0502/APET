// TrialChatPanel 单测（A2-D）—— blocking 禁用 / 空态 / 发送转交 store / 流式显示停止 / 轮次上限。

import { mount, flushPromises } from '@vue/test-utils'
import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { nextTick } from 'vue'

import TrialChatPanel from '../TrialChatPanel.vue'
import { usePersonaTrialStore } from '@/stores/personaTrial'
import type { PersonaSourceDraft } from '@/features/persona-workshop/types'

// Channel mock（store.send 内 new Channel）
const { MockChannel } = vi.hoisted(() => {
  class MockChannel<T> {
    onmessage: ((m: T) => void) | null = null
  }
  return { MockChannel }
})
vi.mock('@/services/chat', () => ({ Channel: MockChannel, cancelChat: vi.fn() }))
vi.mock('@/services/persona', () => ({ trialSend: vi.fn() }))
vi.mock('@/features/persona-workshop/draft', () => ({ validatePersonaDraft: vi.fn(() => []) }))

import { trialSend } from '@/services/persona'
import { validatePersonaDraft } from '@/features/persona-workshop/draft'

const draft = { personaId: 'momo' } as unknown as PersonaSourceDraft

const stubs = {
  ElButton: {
    props: ['disabled', 'type'],
    template:
      '<button :disabled="disabled" v-bind="$attrs" @click="$emit(`click`)"><slot /></button>',
  },
  ElInput: {
    props: ['modelValue', 'disabled'],
    emits: ['update:modelValue'],
    template:
      '<textarea :disabled="disabled" :value="modelValue" @input="$emit(`update:modelValue`, $event.target.value)" />',
  },
}

function mountPanel() {
  const pinia = createPinia()
  setActivePinia(pinia)
  const wrapper = mount(TrialChatPanel, { props: { draft }, global: { plugins: [pinia], stubs } })
  return { wrapper, store: usePersonaTrialStore() }
}

describe('TrialChatPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(validatePersonaDraft as Mock).mockReturnValue([])
    ;(trialSend as Mock).mockResolvedValue({ messageId: 'asst-1' })
  })

  it('disables composer and shows hint when draft has blocking diagnostics', () => {
    ;(validatePersonaDraft as Mock).mockReturnValue([
      { code: 'identity.empty', severity: 'error', message: '身份不能为空' },
    ])
    const { wrapper } = mountPanel()
    expect(wrapper.text()).toContain('补全必填项后才能试聊')
    expect(wrapper.text()).toContain('身份不能为空')
    expect(wrapper.find('textarea').attributes('disabled')).toBeDefined()
  })

  it('shows empty state and keeps 发送 disabled until input typed', async () => {
    const { wrapper } = mountPanel()
    expect(wrapper.text()).toContain('发一句话')
    const sendBtn = wrapper.findAll('button').find((b) => b.text() === '发送')!
    expect(sendBtn.attributes('disabled')).toBeDefined()
  })

  it('sends through the store when input typed and 发送 clicked', async () => {
    const { wrapper } = mountPanel()
    await wrapper.find('textarea').setValue('你好')
    const sendBtn = wrapper.findAll('button').find((b) => b.text() === '发送')!
    await sendBtn.trigger('click')
    await flushPromises()
    expect(trialSend as Mock).toHaveBeenCalledTimes(1)
    expect((trialSend as Mock).mock.calls[0][2]).toBe('你好')
  })

  it('shows 停止 button while streaming', async () => {
    const { wrapper, store } = mountPanel()
    store.streaming = true
    await nextTick()
    const labels = wrapper.findAll('button').map((b) => b.text())
    expect(labels).toContain('停止')
    expect(labels).not.toContain('发送')
  })

  it('shows round-limit hint and disables composer at limit', async () => {
    const { wrapper, store } = mountPanel()
    for (let i = 0; i < 12; i++) {
      store.messages.push({ id: `a${i}`, role: 'assistant', content: 'x' })
    }
    await nextTick()
    expect(store.atLimit).toBe(true)
    expect(wrapper.text()).toContain('试聊到此为止')
    expect(wrapper.find('textarea').attributes('disabled')).toBeDefined()
  })
})
