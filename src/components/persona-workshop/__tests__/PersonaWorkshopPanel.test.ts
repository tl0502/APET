import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { describe, expect, test, vi } from 'vitest'
import PersonaWorkshopPanel from '../PersonaWorkshopPanel.vue'
import { getActivePersona } from '@/services/persona'

const personaWithoutExamples = {
  id: 'momo',
  snapshot_id: 'momo-1.0.0',
  name: '默默',
  version: '1.0.0',
  source: 'builtin',
  raw_markdown: [
    '# 身份',
    '你叫默默。',
    '',
    '# 性格',
    '- 慵懒',
    '',
    '# 能力',
    '- 陪伴',
    '',
    '# 行为规则',
    '## Do',
    '- 用第二人称',
    "## Don't",
    '- 不空洞鼓励',
  ].join('\n'),
}

vi.mock('@/services/persona', () => ({
  getActivePersona: vi.fn(async () => ({
    id: 'momo',
    snapshot_id: 'momo-1.0.0',
    name: '默默',
    version: '1.0.0',
    source: 'builtin',
    raw_markdown: [
      '# 身份',
      '你叫默默。',
      '',
      '# 性格',
      '- 慵懒',
      '',
      '# 能力',
      '- 陪伴',
      '',
      '# 行为规则',
      '## Do',
      '- 用第二人称',
      "## Don't",
      '- 不空洞鼓励',
      '',
      '# 例对话',
      '- 用户：你好',
      '  默默：我在。',
    ].join('\n'),
  })),
  listPersonas: vi.fn(async () => [
    { id: 'momo', name: '默默', version: '1.0.0', source: 'builtin', is_active: true },
    { id: 'joker', name: '阿吉', version: '1.0.0', source: 'builtin', is_active: false },
  ]),
  loadPersona: vi.fn(async () => ({
    id: 'momo',
    snapshot_id: 'momo-1.0.0',
    name: '默默',
    version: '1.0.0',
    source: 'builtin',
    raw_markdown: [
      '# 身份',
      '你叫默默。',
      '',
      '# 性格',
      '- 慵懒',
      '',
      '# 能力',
      '- 陪伴',
      '',
      '# 行为规则',
      '## Do',
      '- 用第二人称',
      "## Don't",
      '- 不空洞鼓励',
      '',
      '# 例对话',
      '- 用户：你好',
      '  默默：我在。',
    ].join('\n'),
  })),
  validatePersonaDraft: vi.fn(async () => ({
    diagnostics: [],
    blocking: false,
    token_estimate: 80,
  })),
  savePersonaDraft: vi.fn(),
  saveAndActivatePersonaDraft: vi.fn(),
}))

describe('PersonaWorkshopPanel', () => {
  test('loads persona cards without opening the inspector initially', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    expect(wrapper.find('[aria-label="角色卡舞台"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('角色卡')
    expect(wrapper.text()).toContain('默默')
    expect(wrapper.text()).toContain('阿吉')
    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(false)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
  })

  test('opens a glass editor card when a persona card is clicked', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    expect(activeCard).toBeTruthy()
    await activeCard?.trigger('click')

    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(true)
    expect(wrapper.find('.persona-inspector__backdrop').exists()).toBe(true)
    expect(wrapper.find('.persona-inspector__shell').exists()).toBe(true)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
    expect(wrapper.text()).toContain('塑形')
    expect(wrapper.text()).toContain('结构')
    expect(wrapper.text()).toContain('示例')
    expect(wrapper.text()).toContain('源码')
    expect(wrapper.text()).toContain('编译诊断')
  })

  test('shows example tab content and keeps source preview in sync', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template: '<button :aria-label="$attrs[`aria-label`]" @click="$emit(`click`)"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    expect(exampleTab).toBeTruthy()
    await exampleTab?.trigger('click')

    expect(wrapper.find('[aria-label="示例对话编辑器"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('示例 1')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    expect(addExampleButton).toBeTruthy()
    await addExampleButton?.trigger('click')
    expect(wrapper.text()).toContain('示例 2')

    const inputs = wrapper.findAll('textarea')
    await inputs[0].setValue('今天有点累')
    await inputs[1].setValue('先慢一点。')

    const sourceTab = wrapper.findAll('button').find((button) => button.text() === '源码')
    await sourceTab?.trigger('click')
    await flushPromises()

    const sourceText = (wrapper.findAll('textarea').at(-1)?.element as HTMLTextAreaElement).value
    expect(sourceText).toContain('# 例对话')
    expect(sourceText).toContain('用户：今天有点累')
    expect(sourceText).toContain('默默：先慢一点。')
  })

  test('adds the first example from an empty example state', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce(personaWithoutExamples)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template:
              '<button :aria-label="$attrs[`aria-label`]" @click="$emit(`click`)"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    await exampleTab?.trigger('click')

    expect(wrapper.text()).toContain('还没有示例对话')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    await addExampleButton?.trigger('click')

    expect(wrapper.text()).toContain('示例 1')
  })

  test('adds the first example from the inspector with real Element Plus controls', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce(personaWithoutExamples)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    await exampleTab?.trigger('click')

    expect(wrapper.text()).toContain('还没有示例对话')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    await addExampleButton?.trigger('click')

    expect(wrapper.text()).toContain('示例 1')
  })

  test('writes a newly added example into source preview with real Element Plus controls', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce(personaWithoutExamples)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    await exampleTab?.trigger('click')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    await addExampleButton?.trigger('click')
    await flushPromises()

    const textareas = wrapper.findAll('textarea')
    await textareas[0].setValue('今天好累')
    await textareas[1].setValue('先坐一下，我陪你。')
    await flushPromises()

    const sourceTab = wrapper.findAll('button').find((button) => button.text() === '源码')
    await sourceTab?.trigger('click')
    await flushPromises()

    const sourceText = (wrapper.findAll('textarea').at(-1)?.element as HTMLTextAreaElement).value
    expect(sourceText).toContain('用户：今天好累')
    expect(sourceText).toContain('默默：先坐一下，我陪你。')
  })

  test('lets the inspector close without clearing the card stage', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    expect(activeCard).toBeTruthy()
    await activeCard?.trigger('click')

    const closeButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '关闭')
    expect(closeButton).toBeTruthy()
    await closeButton?.trigger('click')

    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(false)
    expect(wrapper.find('[aria-label="角色卡舞台"]').exists()).toBe(true)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
  })
})
